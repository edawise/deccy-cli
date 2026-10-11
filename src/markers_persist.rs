//! Loading and saving the marker file: read, parse, schema-check, decode; encode, write.
//!
//! S5-marker-file (ysqqrqsp) fixes where the file is — markers.json in the state directory,
//! `.deccy/markers.json` — and S5-markers-persist (nslkynln) and S5-marker-schema
//! (zwurlynr) that the markers and the position live in that one file across
//! invocations; S5-marker-load (uskysnox) fixes reading every marker in file order,
//! S5-marker-initial (lspnqzzz) that a missing file reads as empty, and S5-marker-corrupt
//! (vqqmwsml) that a marker file which cannot be read as valid refuses, naming the
//! corruption; S5-marker-schema (zwurlynr) fixes the file's contents, so a load parses
//! with M-markers' (qrqumusy) closed types (I1-serde, orrvsxqq) and checks the parsed
//! document against that schema the binary carries, embedded with `include_str!`
//! (I1-schema, zmmyomyp). It reads and writes the file through M-markers-file (qvkrwptt),
//! opened on M-root (ppwnzwwy): the read yields the contents as they stand — a missing file
//! reading as empty — and the write replaces them atomically, never a truncate. M-layout
//! (szmuyqrq) places the module flat at src/markers_persist.rs.
//!
//! What this fixes: the two directions over the marker file, each carrying its reason and
//! rendering nothing. Loading reads the file, reads a missing file as empty, parses,
//! checks the parsed document against the carried schema before decoding it into the
//! state in file order; a file that does not parse or does not conform is refused. Saving
//! encodes the state as the file's compact one-line form and replaces the file
//! atomically. This module carries the reason and never renders it.

use serde_json::Value;

use crate::markers::Markers;
use crate::markers_file;
use crate::store::Store;

/// The marker file, in the state directory beside `nodes/` (S5-marker-file, S2-markers).
pub use crate::markers_file::MARKERS_PATH;

/// The marker schema the binary carries, embedded at build time (S5-marker-schema).
const MARKERS_SCHEMA: &str = include_str!("../schemas/markers.json");

/// Why the marker file did not load or save: the reason is carried, not collapsed
/// (S5-marker-corrupt).
#[derive(Debug)]
pub enum PersistError {
    /// The file exists but could not be read.
    Unreadable(String),
    /// The file is not JSON.
    NotJson(String),
    /// The document parsed but does not conform to the marker schema.
    Invalid(String),
    /// The carried schema could not be parsed or compiled.
    Schema(String),
    /// The file could not be written.
    Unwritable(String),
}

/// Load the marker state: read, parse, schema-check, then decode into the state
/// (S5-marker-load).
///
/// The read goes through M-markers-file (qvkrwptt): it yields the contents as they stand
/// or nothing when the file is not there, and a missing file reads as empty
/// (S5-marker-initial), so absence is not a failure. A file that is there yields its
/// contents even when they are empty, so empty contents are read — and refused — rather
/// than mistaken for absence. Any other read failure is unreadable. Validation comes
/// before the typed decode, so a document that parses but does not conform to the schema
/// is refused rather than silently accepted (S5-marker-corrupt).
pub fn load(store: &Store) -> Result<Markers, PersistError> {
    // A missing file reads as empty (S5-marker-initial): absence is the empty state, not a
    // parse failure. A file that is there is read as its contents, empty or not, so an
    // empty file is not mistaken for a missing one.
    let text = match markers_file::read(store).map_err(carried)? {
        Some(text) => text,
        None => return Ok(Markers::new()),
    };
    let document: Value =
        serde_json::from_str(&text).map_err(|error| PersistError::NotJson(error.to_string()))?;
    let schema = schema()?;
    if let Err(errors) = schema.validate(&document) {
        let reason = errors
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(PersistError::Invalid(reason));
    }
    serde_json::from_value::<Markers>(document)
        .map_err(|error| PersistError::Invalid(error.to_string()))
}

/// Save the marker state: encode compactly, then replace the file atomically
/// (S5-markers-persist, S5-marker-schema).
///
/// The encoding is the corpus's one-line form — compact, no trailing newline — and the
/// write goes through M-markers-file (qvkrwptt), which replaces the file atomically, never
/// `std::fs` directly and never a truncate.
pub fn save(store: &Store, state: &Markers) -> Result<(), PersistError> {
    let text =
        serde_json::to_string(state).map_err(|error| PersistError::Unwritable(error.to_string()))?;
    markers_file::write(store, &text).map_err(carried)
}

/// Carry the file's reason into the persist error (S5-marker-corrupt): a read failure is
/// unreadable, a write failure unwritable; the reason is carried, never collapsed.
fn carried(error: markers_file::FileError) -> PersistError {
    match error {
        markers_file::FileError::Read(reason) => PersistError::Unreadable(reason),
        markers_file::FileError::Write(reason) => PersistError::Unwritable(reason),
    }
}

/// The compiled marker schema the binary carries (S5-marker-schema).
fn schema() -> Result<jsonschema::JSONSchema, PersistError> {
    let document: Value = serde_json::from_str(MARKERS_SCHEMA)
        .map_err(|error| PersistError::Schema(error.to_string()))?;
    jsonschema::JSONSchema::compile(&document)
        .map_err(|error| PersistError::Schema(error.to_string()))
}
