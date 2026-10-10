//! Op log: read the current operation id, the run window's entries, and the invoking workspace.
//!
//! S3-oplog fixes the read — an operation's place in the jj op log, the entries the run
//! window between two head reads carries, each with the workspace it is attributed to, and
//! which workspace that work belongs to — feeding markers' read-run-read bracketing;
//! S5-observation (urqytyqn) fixes that every read here is observational — it carries
//! `--ignore-working-copy`, so reading the head, the window or the invoking workspace
//! snapshots and updates nothing, and settling belongs to the bracket, before the window.
//! M-jj fixes the crossing and its evidence-carrying runner. `jj op log` carries a template,
//! so this fixes the machine-readable form; bracketing around an operation is the markers
//! layer's concern.

use crate::jj::{Jj, JjError, Output};

/// What the op-log template emits per operation: id, workspace, parent ids.
const TEMPLATE: &str = "self.id() ++ \"\\t\" ++ self.workspace_name() ++ \"\\t\" ++ self.parents().map(|op| op.id()).join(\",\") ++ \"\\n\"";

/// What the workspace template emits per workspace: name, root.
const WORKSPACE_TEMPLATE: &str = "self.name() ++ \"\\t\" ++ self.root() ++ \"\\n\"";

/// Why the op log could not be read (S3-err: the evidence is carried).
#[derive(Debug)]
pub enum OplogError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for OplogError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => OplogError::Spawn(reason),
        }
    }
}

/// One op-log entry: an operation with the workspace it is attributed to (S3-oplog).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The operation id.
    pub id: String,
    /// The workspace the operation is attributed to.
    pub workspace: String,
}

/// The current (head) operation id (S3-oplog).
pub fn oplog(jj: &Jj) -> Result<String, OplogError> {
    let out = jj.run(&[
        "op",
        "log",
        "--ignore-working-copy",
        "--no-graph",
        "--limit",
        "1",
        "-T",
        "id",
    ])?;
    if out.status != 0 {
        return Err(OplogError::Failed(out));
    }
    Ok(out.stdout.trim().to_string())
}

/// The entries the run window between two head reads carries: every operation reachable
/// from `post` but not from `pre`, in run order (oldest first), each with the workspace it
/// is attributed to (S3-oplog).
pub fn window(jj: &Jj, pre: &str, post: &str) -> Result<Vec<Entry>, OplogError> {
    let out = jj.run(&["op", "log", "--ignore-working-copy", "--no-graph", "-T", TEMPLATE])?;
    if out.status != 0 {
        return Err(OplogError::Failed(out));
    }
    let log = parse(&out.stdout);
    let before = reachable(&log, pre);
    let after = reachable(&log, post);
    let mut entries: Vec<Entry> = log
        .iter()
        .filter(|(entry, _)| after.contains(&entry.id) && !before.contains(&entry.id))
        .map(|(entry, _)| entry.clone())
        .collect();
    entries.reverse();
    Ok(entries)
}

/// The invoking workspace's name (S3-oplog): the workspace whose root the runner stands in.
///
/// The op log attributes an operation to a workspace but does not say which workspace is
/// invoking, so that is read from `jj workspace list`, matched by the root the invoking
/// workspace reports for itself — the same name the entries are attributed by.
pub fn workspace(jj: &Jj) -> Result<String, OplogError> {
    let root = jj.run(&["workspace", "root", "--ignore-working-copy"])?;
    if root.status != 0 {
        return Err(OplogError::Failed(root));
    }
    let invoking = root.stdout.trim().to_string();
    let out = jj.run(&[
        "workspace",
        "list",
        "--ignore-working-copy",
        "-T",
        WORKSPACE_TEMPLATE,
    ])?;
    if out.status != 0 {
        return Err(OplogError::Failed(out));
    }
    Ok(listed(&out.stdout, &invoking))
}

/// The name of the workspace rooted at `root`, as `jj workspace list` reports it (S3-oplog).
fn listed(list: &str, root: &str) -> String {
    list.lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, path)| path.trim() == root)
        .map(|(name, _)| name.to_string())
        .unwrap_or_default()
}

/// Parse the template output into (entry, parent ids), in the order jj lists it — newest
/// first.
///
/// The op log reports a workspace's work as `<name>@`; the workspace an entry is attributed
/// to is the name, the way `jj workspace list` reports it — so the marking `@` is not part
/// of it here.
fn parse(stdout: &str) -> Vec<(Entry, Vec<String>)> {
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut fields = line.split('\t');
            let id = fields.next().unwrap_or_default().to_string();
            let workspace = fields
                .next()
                .unwrap_or_default()
                .trim_end_matches('@')
                .to_string();
            let parents = fields
                .next()
                .unwrap_or_default()
                .split(',')
                .filter(|parent| !parent.is_empty())
                .map(str::to_string)
                .collect();
            (Entry { id, workspace }, parents)
        })
        .collect()
}

/// Every operation reachable from `head` through the log's parent links, `head` included.
fn reachable(log: &[(Entry, Vec<String>)], head: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut pending = vec![head.to_string()];
    while let Some(id) = pending.pop() {
        if seen.iter().any(|seen| seen == &id) {
            continue;
        }
        if let Some((_, parents)) = log.iter().find(|(entry, _)| entry.id == id) {
            seen.push(id);
            pending.extend(parents.iter().cloned());
        }
    }
    seen
}
