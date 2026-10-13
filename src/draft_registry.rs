//! The draft record store: one JSON document per draft handle under the registry root.
//!
//! A draft's record is one JSON document at `<root>/<handle>.json`, its filename the record's
//! handle, and the stored records are found only by reading that directory
//! (M-draft-record, rqtowtqn; M-draft-store, twpvqstu). The root is the work-state registry
//! under `.deccy/work/`, untracked with the rest of the state directory (M-draft-state,
//! nolspvpw); this module is handed that directory and never derives it, so it holds no
//! repository and no current directory. The handle alone is the record's identity and the
//! node, the edit and the workspace are references (M-draft-identity, owyntkvw): every
//! document this module names, keys or returns is keyed by its handle and never by a node or
//! an edit. The document's form is `schemas/work.json` (M-draft-schema, wrvsxwmq), carried in
//! the binary with `include_str!` and compiled in-process by the `jsonschema` crate
//! (I1-schema, zmmyomyp) over serde_json values (I1-serde, orrvsxqq) — never by hand-written
//! field inspection. A read checks the document against that form and refuses it, naming the
//! record, when the check fails or when the document's `handle` is not the handle its filename
//! gives (M-draft-read, ntypuupx): never surfacing a non-conforming document as a record and
//! never passing over it silently. The module is the single flat file `src/draft_registry.rs`
//! (M-layout, szmuyqrq) and is self-contained — it uses nothing of the crate it is compiled
//! in, so a self-contained test crate (M-tests, knqkxzns) can include it alone.
//!
//! What this fixes: the path a handle names; the write that stores one document at that path,
//! creating the registry root when it is not there and replacing the document already there
//! rather than appending; the read that returns the document only when it conforms to the
//! carried schema and carries the handle its filename gives; the scan that returns every
//! `.json` document in the directory keyed by its filename handle, and nothing when the
//! directory is not there; and the removal that deletes exactly the document the handle names
//! — `<root>/<handle>.json` — and touches nothing else. Every refusal names the path it
//! refused.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;

/// The record schema the binary carries, embedded at build time (M-draft-schema, wrvsxwmq).
const WORK_SCHEMA: &str = include_str!("../schemas/work.json");

/// The one suffix a record's filename carries (M-draft-record, rqtowtqn): the handle is the
/// stem, and `<handle>.json` is the whole name.
const RECORD_SUFFIX: &str = ".json";

/// Why the store refused: every variant carries the path it refused, and `Display` names it
/// (M-draft-read, ntypuupx).
#[derive(Debug)]
pub enum Error {
    /// No document at the path the handle names (M-draft-record, rqtowtqn): a handle the
    /// directory does not name is a record that is not there.
    Missing(PathBuf),
    /// The document is there but is not JSON (M-draft-record, rqtowtqn).
    NotJson(PathBuf, serde_json::Error),
    /// The document parsed but does not conform to the record schema, or carries no handle
    /// string at all; the reason is carried (M-draft-read, ntypuupx).
    Invalid(PathBuf, String),
    /// The document's `handle` is not the handle its filename gives (M-draft-read, ntypuupx).
    HandleMismatch {
        /// The path that was read.
        path: PathBuf,
        /// The handle the filename gives — the name the document was read under.
        filename: String,
        /// The handle the document's own field carries.
        handle: String,
    },
    /// The directory or a document could not be read or written; the message names the path.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Missing(path) => write!(
                f,
                "{}: no draft record at this path — the handle names no document",
                path.display()
            ),
            Error::NotJson(path, error) => {
                write!(
                    f,
                    "{}: the draft record is not JSON: {error}",
                    path.display()
                )
            }
            Error::Invalid(path, reason) => write!(
                f,
                "{}: the document is not a draft record: {reason}",
                path.display()
            ),
            Error::HandleMismatch {
                path,
                filename,
                handle,
            } => write!(
                f,
                "{}: the document's handle is `{handle}`, but its filename gives `{filename}` — \
                 a record's filename is its handle",
                path.display()
            ),
            // The io error is built with the path in its message (see `named`).
            Error::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

/// The path a handle names: exactly `<root>/<handle>.json` (M-draft-record, rqtowtqn).
///
/// The handle is the record's identity and its filename (M-draft-identity, owyntkvw), so the
/// path derives from the handle alone — no per-draft subdirectory, no index, no second layout.
/// The root is the registry directory the caller supplies, so this module depends on no
/// current directory.
pub fn path_for(root: &Path, handle: &str) -> PathBuf {
    root.join(format!("{handle}{RECORD_SUFFIX}"))
}

/// Store one record: write `document` at the path its `handle` names (M-draft-record,
/// rqtowtqn; M-draft-store, twpvqstu).
///
/// The document is the whole record, so it lands as one file at `<root>/<handle>.json`, and a
/// second write under the same handle replaces that file instead of appending to it — one
/// document per handle is what lets the handle alone pick the record out (M-draft-identity,
/// owyntkvw). The document is checked against the carried schema before it is stored, so what
/// the directory holds is the same record form every reader checks (M-draft-schema,
/// wrvsxwmq). The first write creates the registry root when it is not there: the root is a
/// directory of untracked state (M-draft-state, nolspvpw) and the record being stored is what
/// makes it a registry, so a fresh repository's first `deccy draft` does not fail on a
/// directory nobody had reason to create yet. The check runs before the root is created and
/// before anything is stored, so a refused document leaves nothing behind.
pub fn write(root: &Path, document: &Value) -> Result<(), Error> {
    // The filename is the handle, so a document that carries no handle string names no file
    // and cannot be stored as a record. The registry root is the only path to name here.
    let handle = match document.get("handle").and_then(Value::as_str) {
        Some(handle) => handle,
        None => {
            return Err(Error::Invalid(
                root.to_path_buf(),
                "the document carries no handle string, so it names no file".to_string(),
            ))
        }
    };
    let path = path_for(root, handle);
    check(&path, document)?;
    // The registry root may not exist yet: a fresh repository has no `.deccy/work/`, and the
    // first record is what makes it a registry (M-draft-state, nolspvpw). Creating the root
    // here, after the document has passed the schema check, means a refused document stores
    // nothing and leaves no registry behind. `create_dir_all` is a no-op when the root is
    // already there, so every later write takes the same path.
    fs::create_dir_all(root).map_err(|error| named(root, error))?;
    let text = serde_json::to_vec_pretty(document).map_err(|error| {
        named(
            &path,
            std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        )
    })?;
    // One write, one file: `fs::write` truncates what is there, so a re-write replaces the
    // record rather than appending a second copy of it.
    fs::write(&path, text).map_err(|error| named(&path, error))
}

/// Remove the one record `handle` names: delete exactly `<root>/<handle>.json` (M-draft-record,
/// rqtowtqn; M-draft-store, twpvqstu).
///
/// The path is the handle, so removal is a path operation and nothing more: it deletes the
/// document at the path the handle names and touches nothing else. A document placed in the
/// directory by hand names the same path as one this module wrote, so it is removable too
/// (M-draft-record, rqtowtqn). The directory is never scanned and no document is read or
/// schema-checked: records the removal was not asked about are left byte-identical even when a
/// `read` would refuse them, and the target document is removed without being interpreted,
/// because the caller names the record to discard and the store does not second-guess a
/// discarded record's form. A handle that names no document is `Missing` carrying that path and
/// deletes nothing; any other filesystem failure is `Io`, naming the path.
pub fn remove(root: &Path, handle: &str) -> Result<(), Error> {
    let path = path_for(root, handle);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Error::Missing(path)),
        Err(error) => Err(named(&path, error)),
    }
}

/// Read the record `handle` names: parse it, check it against the carried schema, and require
/// its `handle` field to be the handle its filename gives (M-draft-read, ntypuupx).
///
/// A handle the directory does not name is `Missing` — the record is not there
/// (M-draft-record, rqtowtqn) — and any other read failure is carried. A document that is not
/// JSON is `NotJson`; one that does not conform to `schemas/work.json` is `Invalid`; one that
/// conforms but whose `handle` is some other handle is `HandleMismatch`, because the filename
/// is the record's handle (M-draft-record) and the schema types the field without fixing its
/// value space (M-draft-schema). Each refusal is an `Err` value naming the path: a
/// non-conforming document is never surfaced as a record and never passed over silently.
pub fn read(root: &Path, handle: &str) -> Result<Value, Error> {
    let path = path_for(root, handle);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::Missing(path))
        }
        Err(error) => return Err(named(&path, error)),
    };
    // Bytes, not text: a document that is not UTF-8 is not JSON either, and reads as the
    // parse refusal rather than as a separate unreadable file.
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|error| Error::NotJson(path.clone(), error))?;
    check(&path, &document)?;
    let found = document
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if found == handle {
        Ok(document)
    } else {
        Err(Error::HandleMismatch {
            path,
            filename: handle.to_string(),
            handle: found.to_string(),
        })
    }
}

/// Scan the registry: every `.json` document under `root`, keyed by the handle its filename
/// gives (M-draft-record, rqtowtqn; M-draft-store, twpvqstu).
///
/// The directory is the only list of the records, because they are untracked state and no
/// node, edge or change names one (M-draft-state, nolspvpw; M-draft-identity, owyntkvw) — so
/// discovery reads the directory and never an index. Every document goes through `read`, so a
/// scan surfaces nothing that a read would refuse and skips nothing it would refuse
/// (M-draft-read, ntypuupx). A directory that is not there holds no records: `Ok` of an empty
/// list. The returned pairs are ordered by handle, so a listing is the same list every time.
pub fn scan(root: &Path) -> Result<Vec<(String, Value)>, Error> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(named(root, error)),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| named(root, error))?;
        let path = entry.path();
        // A record's name is its handle, and a handle is the string a JSON document carries,
        // so a filename that is not UTF-8 names no record this store could return.
        let name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{}: the filename is not a handle", path.display()),
                )))
            }
        };
        let handle = match name.strip_suffix(RECORD_SUFFIX) {
            Some(handle) => handle,
            // Anything that is not a `.json` document is not a record document.
            None => continue,
        };
        let document = read(root, handle)?;
        records.push((handle.to_string(), document));
    }
    records.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(records)
}

/// Check a document against the carried record schema (M-draft-schema, wrvsxwmq;
/// I1-schema, zmmyomyp). The failure names the path and carries the validator's reasons.
fn check(path: &Path, document: &Value) -> Result<(), Error> {
    let schema = schema().map_err(|reason| Error::Invalid(path.to_path_buf(), reason))?;
    if let Err(errors) = schema.validate(document) {
        let reason = errors
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::Invalid(path.to_path_buf(), reason));
    }
    Ok(())
}

/// The compiled record schema the binary carries, compiled once per process and reused
/// (M-draft-schema, wrvsxwmq; I1-schema, zmmyomyp).
fn schema() -> Result<&'static jsonschema::JSONSchema, String> {
    static SCHEMA: OnceLock<Result<jsonschema::JSONSchema, String>> = OnceLock::new();
    match SCHEMA.get_or_init(|| {
        let document: Value =
            serde_json::from_str(WORK_SCHEMA).map_err(|error| error.to_string())?;
        jsonschema::JSONSchema::compile(&document).map_err(|error| error.to_string())
    }) {
        Ok(schema) => Ok(schema),
        Err(reason) => Err(reason.clone()),
    }
}

/// Wrap an io failure so its message names the path it happened at: every refusal names the
/// path it refused (M-draft-read, ntypuupx).
fn named(path: &Path, error: std::io::Error) -> Error {
    Error::Io(std::io::Error::new(
        error.kind(),
        format!("{}: {error}", path.display()),
    ))
}
