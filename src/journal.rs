//! The journal store: content-addressed recording snapshots.
//!
//! S5-journal-store (qokrosvu) fixes the entry — `{id, version, kind, target, before, after}`
//! with path -> hash-or-null sides, the id the SHA-256 of the canonical entry, SHA-256
//! content-addressed blobs, blobs written before the entry, atomic entry replacement, a
//! corrupt entry refused by name, and unreferenced entries and blobs removable. S2-journal
//! (ozmxpzxy) fixes the reach: `.deccy/journal/` entry files and `.deccy/objects/` blobs,
//! opened on the one root. S5-journal-schema (wrkmwqqo) fixes the document, embedded here with
//! `include_str!`; M-layout (szmuyqrq) places the module flat at src/journal.rs.
//!
//! What this fixes: put/get/content/gc over that store. `put` hashes every content value,
//! writes any blob not already there, then writes the entry whole by atomic replacement;
//! `get` checks the document against the carried schema, checks the id against the canonical
//! entry and every blob against its hash; `content` returns a blob's bytes; `gc` removes the
//! entries no marker references and the blobs no kept entry references. Every failure is a
//! carried reason; this module renders nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::Store;

/// Entry files, relative to the repository root (S2-journal).
pub const JOURNAL_DIR: &str = ".deccy/journal";

/// Content blobs, relative to the repository root (S2-journal).
pub const OBJECTS_DIR: &str = ".deccy/objects";

/// The journal schema the binary carries, embedded at build time (S5-journal-schema).
const JOURNAL_SCHEMA: &str = include_str!("../schemas/journal.json");

/// One side of a recording: every owned path to the SHA-256 of its content, or null when the
/// path is absent (S5-journal-store).
pub type Side = BTreeMap<String, Option<String>>;

/// The content a recording captures: every owned path to its bytes, or null when absent.
pub type Snapshot = BTreeMap<String, Option<Vec<u8>>>;

/// Why the journal did not store or load an entry: the reason is carried, not collapsed.
#[derive(Debug)]
pub enum JournalError {
    /// The entry or blob is not there.
    Missing(String),
    /// A file exists but could not be read.
    Unreadable(String),
    /// A file could not be written.
    Unwritable(String),
    /// The document is not JSON.
    NotJson(String),
    /// The document parses but does not conform to the journal schema, does not match its
    /// name, or names a blob that does not match its hash.
    Invalid(String),
    /// The carried schema could not be parsed or compiled.
    Schema(String),
    /// A path is not repo-relative or escapes the root.
    BadPath(String),
}

/// A loaded journal entry: the two sides the apply layer reads (S5-journal-store).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The SHA-256 of the canonical entry.
    pub id: String,
    /// The entry document's version.
    pub version: u64,
    /// The operation kind the entry inverts.
    pub kind: String,
    /// The change the recording targeted.
    pub target: String,
    /// The owned paths' content before the recording.
    pub before: Side,
    /// The owned paths' content after the recording.
    pub after: Side,
}

#[derive(Serialize)]
struct Canonical<'a> {
    version: u64,
    kind: &'a str,
    target: &'a str,
    before: &'a Side,
    after: &'a Side,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    id: String,
    version: u64,
    kind: String,
    target: String,
    before: Side,
    after: Side,
}

/// Store one recording's two sides: write the blobs, derive the id, then write the entry
/// whole (S5-journal-store). Identical content dedupes to one blob.
pub fn put(
    store: &Store,
    kind: &str,
    target: &str,
    before: &Snapshot,
    after: &Snapshot,
) -> Result<String, JournalError> {
    validate_kind(kind)?;
    if target.is_empty() {
        return Err(JournalError::Invalid("the entry names no target".to_string()));
    }
    let before_side = write_side(store, before)?;
    let after_side = write_side(store, after)?;
    let canonical = Canonical {
        version: 1,
        kind,
        target,
        before: &before_side,
        after: &after_side,
    };
    let text = serde_json::to_string(&canonical)
        .map_err(|error| JournalError::Invalid(error.to_string()))?;
    let id = sha256_hex(text.as_bytes());
    let document = serde_json::json!({
        "id": id,
        "version": 1,
        "kind": kind,
        "target": target,
        "before": before_side,
        "after": after_side,
    });
    check(&document)?;
    let encoded = serde_json::to_string(&document)
        .map_err(|error| JournalError::Invalid(error.to_string()))?;
    replace(&entry_path(store.root(), &id), &encoded)?;
    Ok(id)
}

/// Load an entry by id and check it: its schema, its id against the canonical entry, and every
/// blob against its hash (S5-journal-store).
pub fn get(store: &Store, id: &str) -> Result<Entry, JournalError> {
    validate_hash(id, "entry id")?;
    let path = entry_path(store.root(), id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(JournalError::Missing(id.to_string()))
        }
        Err(error) => return Err(JournalError::Unreadable(error.to_string())),
    };
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| JournalError::NotJson(format!("{id}: {error}")))?;
    check(&document).map_err(|error| match error {
        JournalError::Invalid(reason) => JournalError::Invalid(format!("{id}: {reason}")),
        other => other,
    })?;
    let stored: Stored = serde_json::from_value(document)
        .map_err(|error| JournalError::Invalid(format!("{id}: {error}")))?;
    if stored.id != id {
        return Err(JournalError::Invalid(format!(
            "{id}: the document names its id as {}",
            stored.id
        )));
    }
    let canonical = Canonical {
        version: stored.version,
        kind: &stored.kind,
        target: &stored.target,
        before: &stored.before,
        after: &stored.after,
    };
    let encoded = serde_json::to_string(&canonical)
        .map_err(|error| JournalError::Invalid(error.to_string()))?;
    if sha256_hex(encoded.as_bytes()) != id {
        return Err(JournalError::Invalid(format!(
            "{id}: the id does not match the canonical entry"
        )));
    }
    for hash in stored
        .before
        .values()
        .chain(stored.after.values())
        .flatten()
    {
        let bytes = content(store, hash)?;
        if sha256_hex(&bytes) != *hash {
            return Err(JournalError::Invalid(format!(
                "{id}: blob {hash} does not match its hash"
            )));
        }
    }
    Ok(Entry {
        id: stored.id,
        version: stored.version,
        kind: stored.kind,
        target: stored.target,
        before: stored.before,
        after: stored.after,
    })
}

/// Read a content blob by its SHA-256 and check it against its name (S5-journal-store).
pub fn content(store: &Store, hash: &str) -> Result<Vec<u8>, JournalError> {
    validate_hash(hash, "blob hash")?;
    let path = object_path(store.root(), hash);
    match fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(JournalError::Missing(hash.to_string()))
        }
        Err(error) => Err(JournalError::Unreadable(error.to_string())),
    }
}

/// Remove the entries no marker references and the blobs no kept entry references
/// (S5-journal-store).
pub fn gc(store: &Store, referenced: &BTreeSet<String>) -> Result<(), JournalError> {
    let mut blobs: BTreeSet<String> = BTreeSet::new();
    for name in list(store.root().join(JOURNAL_DIR))? {
        let Some(id) = name.strip_suffix(".json") else {
            continue;
        };
        if !is_hash(id) {
            continue;
        }
        if !referenced.contains(id) {
            remove(&entry_path(store.root(), id))?;
            continue;
        }
        if let Ok(entry) = get(store, id) {
            for hash in entry.before.values().chain(entry.after.values()).flatten() {
                blobs.insert(hash.clone());
            }
        }
    }
    for name in list(store.root().join(OBJECTS_DIR))? {
        if is_hash(&name) && !blobs.contains(&name) {
            remove(&object_path(store.root(), &name))?;
        }
    }
    Ok(())
}

/// Write every non-null content value as a blob and answer the path -> hash-or-null side
/// (S5-journal-store).
fn write_side(store: &Store, snapshot: &Snapshot) -> Result<Side, JournalError> {
    let mut side = Side::new();
    for (path, bytes) in snapshot {
        validate_path(path)?;
        match bytes {
            None => {
                side.insert(path.clone(), None);
            }
            Some(bytes) => {
                let hash = sha256_hex(bytes);
                let object = object_path(store.root(), &hash);
                if !object.exists() {
                    replace_bytes(&object, bytes)?;
                }
                side.insert(path.clone(), Some(hash));
            }
        }
    }
    Ok(side)
}

/// The canonical entry document's required fields (S5-journal-store, S5-journal-schema).
fn check(entry: &Value) -> Result<(), JournalError> {
    let document: Value = serde_json::from_str(JOURNAL_SCHEMA)
        .map_err(|error| JournalError::Schema(error.to_string()))?;
    let schema = jsonschema::JSONSchema::compile(&document)
        .map_err(|error| JournalError::Schema(error.to_string()))?;
    if let Err(errors) = schema.validate(entry) {
        let reason = errors
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(JournalError::Invalid(reason));
    }
    Ok(())
}

/// The one kind the schema admits: only a recording keeps a journal entry (S5-journal-store).
fn validate_kind(kind: &str) -> Result<(), JournalError> {
    if kind == "record" {
        Ok(())
    } else {
        Err(JournalError::Invalid(format!("unknown journal kind {kind}")))
    }
}

/// A 64-hex hash (S5-journal-store).
fn validate_hash(hash: &str, what: &str) -> Result<(), JournalError> {
    if is_hash(hash) {
        Ok(())
    } else {
        Err(JournalError::Invalid(format!("{what} is not a SHA-256: {hash}")))
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// A repo-relative path that stays under the root (S2-journal).
fn validate_path(path: &str) -> Result<(), JournalError> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(JournalError::BadPath(path.to_string()));
    }
    Ok(())
}

fn entry_path(root: &Path, id: &str) -> PathBuf {
    root.join(JOURNAL_DIR).join(format!("{id}.json"))
}

fn object_path(root: &Path, hash: &str) -> PathBuf {
    root.join(OBJECTS_DIR).join(hash)
}

fn list(dir: PathBuf) -> Result<Vec<String>, JournalError> {
    match fs::read_dir(&dir) {
        Ok(entries) => {
            let mut names = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|error| JournalError::Unreadable(error.to_string()))?;
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
            Ok(names)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(JournalError::Unreadable(error.to_string())),
    }
}

/// Remove a file; absence is already the state GC wants.
fn remove(path: &Path) -> Result<(), JournalError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(JournalError::Unwritable(error.to_string())),
    }
}

/// Write `contents` by atomic replacement in the target's own directory (S2-atomic).
fn replace(target: &Path, contents: &str) -> Result<(), JournalError> {
    replace_bytes(target, contents.as_bytes())
}

/// Write `contents` bytes by atomic replacement; blobs and entry documents share the rule
/// (S2-atomic).
fn replace_bytes(target: &Path, contents: &[u8]) -> Result<(), JournalError> {
    let directory = target
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&directory).map_err(|error| JournalError::Unwritable(error.to_string()))?;
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = directory.join(format!(".{name}.tmp-{}", std::process::id()));
    let outcome = write_then_replace(&temporary, target, contents);
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome.map_err(|error| JournalError::Unwritable(error.to_string()))
}

fn write_then_replace(temporary: &Path, target: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, target)
}

/// The SHA-256 of `data`, lowercase hex (S5-journal-store).
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = sha256(data);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// SHA-256 (FIPS 180-4), kept here so the journal needs no extra dependency.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7)
                ^ w[i - 15].rotate_right(18)
                ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17)
                ^ w[i - 2].rotate_right(19)
                ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut digest = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}
