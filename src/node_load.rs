//! Loading a node: read, parse, and check it against the schema the binary carries.
//!
//! S2-validate fixes that every load checks its document against the node schema and that
//! nothing unchecked surfaces to a verb; S2-invalid fixes the failure — invalid with the
//! reason attached, never an unverified node, never silent. D1c fixes the document, D1d
//! the schema. The schema is embedded in the binary with `include_str!` — no schema is read
//! from the repository — and the document is read through M-store-read (tmwyqnyz) at the
//! change the id names (D1b) and located through store_path. This carries its own failure
//! reasons rather than the verb refusal value.

use serde_json::Value;

use crate::jj::Jj;
use crate::store_path::Id;
use crate::store_read::{self, Document, ReadError};

/// The node schema the binary carries, embedded at build time (D1d, S2-validate).
const NODE_SCHEMA: &str = include_str!("../schemas/node.json");

/// Why a document did not load: the reason is carried, not collapsed (S2-invalid).
#[derive(Debug)]
pub enum LoadError {
    /// No document at the change the id names (S2-read-absent).
    Missing,
    /// The document is there but could not be read; the evidence is carried.
    Unreadable(String),
    /// The document is not JSON.
    NotJson(String),
    /// The document does not conform to the schema.
    Invalid(String),
    /// The carried schema could not be parsed or compiled.
    Schema(String),
}

/// Load `id`'s document: read at the change the id names, parse, and check it against the
/// carried schema (S2-validate).
pub fn load(jj: &Jj, id: &Id) -> Result<Value, LoadError> {
    let text = match store_read::read_node(jj, id) {
        Ok(Document::Whole(text)) => text,
        Ok(Document::Absent) => return Err(LoadError::Missing),
        Err(ReadError::Spawn(reason)) => return Err(LoadError::Unreadable(reason)),
        Err(ReadError::Failed(out)) => {
            return Err(LoadError::Unreadable(out.stderr.trim().to_string()))
        }
    };
    let document: Value =
        serde_json::from_str(&text).map_err(|error| LoadError::NotJson(error.to_string()))?;
    let schema = schema()?;
    if let Err(errors) = schema.validate(&document) {
        let reason = errors
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(LoadError::Invalid(reason));
    }
    Ok(document)
}

/// The compiled node schema the binary carries (D1d).
fn schema() -> Result<jsonschema::JSONSchema, LoadError> {
    let document: Value =
        serde_json::from_str(NODE_SCHEMA).map_err(|error| LoadError::Schema(error.to_string()))?;
    jsonschema::JSONSchema::compile(&document).map_err(|error| LoadError::Schema(error.to_string()))
}
