//! Ownership attribution: the files a change owns, derived, not scanned.
//!
//! S2-own fixes that a change's files are attributed through the ownership rule — its node
//! file plus its ref-listed paths, nothing else, never by scanning; S2-own-inline fixes the
//! degenerate case (inline ref yields the node file alone). D2 states the rule. The node
//! file's path comes from store_path, so there is one layout; the document arrives already
//! loaded. Each listed path is read in normal form (D2), so `./x` and `x` fold onto one entry
//! and the owned set holds one file per file, not one per spelling; each `..` folds against
//! the ordinary component before it, and a `..` left after folding names a place outside the
//! repository, so that listed path contributes no owned path at all.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::store_path::{node_path, Id};

/// The files `id`'s change owns, given its loaded document: its node file plus its
/// ref-listed paths, nothing else (S2-own, S2-own-inline, D2).
pub fn owned(id: &Id, node: &Value) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    files.insert(node_path(id));
    if let Some(paths) = node.pointer("/content/ref").and_then(Value::as_array) {
        for path in paths.iter().filter_map(Value::as_str) {
            if let Some(owned_path) = normal_form(path) {
                files.insert(owned_path);
            }
        }
    }
    files
}

/// A listed path as D2 reads it: repo-relative and in normal form, so a leading `./` and
/// repeated `/` separators do not distinguish paths — `./x` and `x` name one file, `x//y` and
/// `x/y` name one file. `Path::components` is what folds those away; a path that is `.` alone
/// keeps that spelling rather than collapsing to the empty path, which would name the same
/// file under a spelling no one wrote.
///
/// `..` is folded too, against the ordinary component before it: `src/../src/x.rs` names
/// `src/x.rs` and `a/b/../c` names `a/c`. A `..` with nothing ordinary left to fold against —
/// no component before it, or only `..` before it — survives as a `..` and names a place
/// outside the repository, which owns no file: such a listed path yields `None` and contributes
/// nothing to the owned set. A path whose components all fold away (`a/..`) names the
/// repository root, which is inside the repository, so it is owned under the one spelling `.`
/// rather than dropped.
fn normal_form(path: &str) -> Option<PathBuf> {
    let mut folded: Vec<Component> = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match folded.last() {
                Some(Component::Normal(_)) => {
                    folded.pop();
                }
                _ => return None,
            },
            ordinary => folded.push(ordinary),
        }
    }
    let mut normal = folded.iter().collect::<PathBuf>();
    if normal.as_os_str().is_empty() && !path.is_empty() {
        normal.push(Component::CurDir.as_os_str());
    }
    Some(normal)
}
