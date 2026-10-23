//! The set read: map the node-set ids to their owning commits and read their documents in one
//! batch, with per-object and whole-set fallbacks.
//!
//! S2-read-set (mypprxpk) fixes the source: the set is read from the Git objects of the commits
//! its ids name, one `jj` crossing maps change id to commit id, one `git cat-file --batch`
//! requests `nodes/<id>.json` at that commit, an object the batch cannot serve falls back to the
//! single-document read for that id, and a missing mapping or git falls back for the whole set.
//! T-bulk-read (ltttzmvz) fixes the result: every served result equals the single-document
//! read's outcome, a divergent id is never served from the batch, the mapping/git-unavailable
//! case is total, every id has an outcome, and the batch runs at most once. This module owns
//! only the Rust mechanism. It receives the ids from M-node-set (nykqvuqr) and does not
//! enumerate `files("nodes/")`; the path is M-store-path's `node_path`; the single-document
//! fallback and the checking point are M-store-read (tmwyqnyz) and M-node-load (sxonuzmr); the
//! stdin crossing is M-jj's `run_with_input` (zruxtxrv); the identity is D1b (xwyryumr); and the
//! document layer is I1-serde (orrvsxqq). It returns documents, not `NodeRead`, so M-node-set
//! maps the outcomes and there is no cycle back into `node_set`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::jj::Jj;
use crate::node_load::{self, LoadError};
use crate::store_path::{node_path, Id};

/// Read `ids` from the commits they name, returning each id's checked document or its load
/// failure (S2-read-set, T-bulk-read). The healthy path is one mapping crossing and one batch
/// crossing; an unservable object falls back for that id, and an unavailable mapping or git
/// falls back for the whole set.
pub fn read(
    jj: &Jj,
    git: Option<&Jj>,
    ids: &[Id],
) -> BTreeMap<String, Result<Value, LoadError>> {
    if ids.is_empty() {
        return BTreeMap::new();
    }
    let mapping = match map_changes(jj, ids) {
        Some(mapping) => mapping,
        None => return whole_set(jj, ids),
    };
    let Some(git) = git else {
        return whole_set(jj, ids);
    };

    let mut requests: Vec<String> = Vec::new();
    let mut batched: Vec<&Id> = Vec::new();
    let mut results: BTreeMap<String, Result<Value, LoadError>> = BTreeMap::new();
    for id in ids {
        match mapping.get(id.as_str()) {
            // A change with exactly one commit is served from the batch; a divergent change
            // (more than one commit) and an unmapped id fall back per id (T-bulk-read).
            Some(commits) if commits.len() == 1 => {
                requests.push(format!("{}:{}", commits[0], path(id)));
                batched.push(id);
            }
            _ => {
                results.insert(id.as_str().to_string(), node_load::load(jj, id));
            }
        }
    }
    if requests.is_empty() {
        return results;
    }

    let mut input = requests.join("\n");
    input.push('\n');
    let output = match git.run_with_input(&["cat-file", "--batch"], input.as_bytes()) {
        Ok(output) if output.status == 0 => output,
        // The batch could not run: the whole set falls back, and every id still loads.
        _ => return whole_set(jj, ids),
    };

    let served = parse_batch(&output.stdout, batched.len());
    for (id, bytes) in batched.iter().zip(served) {
        let outcome = match bytes {
            Some(bytes) => match std::str::from_utf8(&bytes) {
                Ok(text) => node_load::check(text),
                Err(error) => Err(LoadError::NotJson(error.to_string())),
            },
            // One object the batch could not serve falls back for that id only.
            None => node_load::load(jj, id),
        };
        results.insert(id.as_str().to_string(), outcome);
    }
    results
}

/// Map each change id to the commit id(s) it names with one `jj` crossing (S2-read-set, D1b).
/// `None` when the mapping could not run or failed, which sends the whole set to the fallback.
fn map_changes(jj: &Jj, ids: &[Id]) -> Option<BTreeMap<String, Vec<String>>> {
    let revset = ids.iter().map(Id::as_str).collect::<Vec<_>>().join("|");
    let output = jj
        .run(&[
            "log",
            "--no-graph",
            "-r",
            &revset,
            "-T",
            "change_id.shortest(8) ++ \" \" ++ commit_id ++ \"\\n\"",
        ])
        .ok()?;
    if output.status != 0 {
        return None;
    }
    let mut mapping: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in output.stdout.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(change), Some(commit)) = (fields.next(), fields.next()) {
            mapping
                .entry(change.to_string())
                .or_default()
                .push(commit.to_string());
        }
    }
    Some(mapping)
}

/// The whole set through the single-document read (S2-read-set): every id gets its outcome,
/// so no node is lost when the mapping or git is unavailable.
fn whole_set(jj: &Jj, ids: &[Id]) -> BTreeMap<String, Result<Value, LoadError>> {
    ids.iter()
        .map(|id| (id.as_str().to_string(), node_load::load(jj, id)))
        .collect()
}

/// The object name the batch requests: `<commit>:nodes/<id>.json`, the path from M-store-path.
fn path(id: &Id) -> String {
    node_path(id)
        .to_str()
        .expect("a node path is valid UTF-8")
        .to_string()
}

/// Parse `git cat-file --batch` responses in request order: `Some(bytes)` for a served blob,
/// `None` for a missing or non-blob object, and `None` for anything the parser cannot trust so
/// the caller falls back for that id. A response header is `<name> <type> <size>` or
/// `<name> missing`; a blob body is `size` bytes followed by a newline.
fn parse_batch(stdout: &[u8], count: usize) -> Vec<Option<Vec<u8>>> {
    let mut served: Vec<Option<Vec<u8>>> = Vec::with_capacity(count);
    let mut cursor = 0;
    for _ in 0..count {
        let Some(header_end) = newline(stdout, cursor) else {
            break;
        };
        let Ok(header) = std::str::from_utf8(&stdout[cursor..header_end]) else {
            break;
        };
        cursor = header_end + 1;
        let fields: Vec<&str> = header.split_whitespace().collect();
        if fields.last() == Some(&"missing") {
            served.push(None);
            continue;
        }
        if fields.len() != 3 {
            break;
        }
        let Ok(size) = fields[2].parse::<usize>() else {
            break;
        };
        let Some(end) = cursor.checked_add(size) else {
            break;
        };
        if end > stdout.len() {
            break;
        }
        let body = stdout[cursor..end].to_vec();
        cursor = end + 1;
        if fields[1] == "blob" {
            served.push(Some(body));
        } else {
            served.push(None);
        }
    }
    served.resize(count, None);
    served
}

fn newline(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| from + offset)
}
