//! Resolving an edge target to what it names.
//!
//! S2-resolve fixes that a target resolves to a sibling node document at the change the id
//! names, files alone, never parentage; S2-root fixes the one exception — zzzzzzzz names the
//! root and names no file; S2-resolve-load fixes that resolution yields the loaded document,
//! not the path. D3 names the root id, D4 fixes sibling resolution. Which edge kinds may
//! carry a root stays with the contract level, where types can carry it.

use serde_json::Value;

use crate::jj::Jj;
use crate::node_load::{load, LoadError};
use crate::store_path::Id;

/// The id that names the root (D3).
pub const ROOT_ID: &str = "zzzzzzzz";

/// What an edge target names: the root, or a sibling node's checked document (D4).
#[derive(Debug)]
pub enum Resolved {
    /// `zzzzzzzz`: the root, which names no file (S2-root).
    Root,
    /// A sibling node's document, loaded and checked (S2-resolve).
    Node(Value),
}

/// Resolve `id` to what it names: the root, or the sibling document at `nodes/<id>.json` at
/// the change the id names (S2-resolve, S2-root).
pub fn resolve(jj: &Jj, id: &Id) -> Result<Resolved, LoadError> {
    if id.as_str() == ROOT_ID {
        Ok(Resolved::Root)
    } else {
        load(jj, id).map(Resolved::Node)
    }
}

/// What a set-backed lookup found for an edge target (S2-resolve-in-set).
#[derive(Debug)]
pub enum Lookup<'a> {
    /// The set holds the target's checked document.
    Loaded(&'a Value),
    /// The target's document was read but did not load; the evidence is carried.
    Failed(&'a str),
    /// The set does not hold the target.
    Absent,
}

/// Resolve `id` from the set of node documents already read: the root, a loaded sibling,
/// absent, or failed — the same outcome as `resolve`, with no per-target read
/// (S2-resolve-in-set). The rule stays here; only the lookup is the caller's.
pub fn resolve_in<'a, F>(id: &Id, lookup: F) -> Result<(), LoadError>
where
    F: FnOnce(&str) -> Lookup<'a>,
{
    if id.as_str() == ROOT_ID {
        return Ok(());
    }
    match lookup(id.as_str()) {
        Lookup::Loaded(_) => Ok(()),
        Lookup::Absent => Err(LoadError::Missing),
        Lookup::Failed(reason) => Err(LoadError::Unreadable(reason.to_string())),
    }
}
