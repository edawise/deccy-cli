//! The custody read: every change the substrate reports, each with the paths it touched.
//!
//! S-read-custody (kywmkqlm) fixes one crossing — a single `jj log` whose template carries each
//! change's diff summary — and the pairing that rides it; M-jj (zruxtxrv) fixes the crossing and
//! its evidence-carrying runner; M-layout (szmuyqrq) fixes that this is one file under `src/`.
//! This maps the command to a machine-readable form and parses it into `(change id, paths)`
//! pairs, so the pass reads a listing rather than jj's output. Nothing narrows the set here: no
//! ownership test and no path filter, because either would decide the question the pass exists
//! to ask. No path's bytes cross this boundary either — the read reports which paths a change
//! touched, never what they hold.

use crate::jj::{Jj, JjError, Output};

/// The one crossing: `<full change id><TAB>` and then, per changed entry,
/// `<source path><TAB><target path><TAB>`; one change per line (S-read-custody).
const ENTRIES: &str = "change_id ++ \"\\t\" ++ diff.files().map(|f| f.source().path() ++ \"\\t\" ++ f.target().path() ++ \"\\t\").join(\"\") ++ \"\\n\"";

/// The degraded listing: the ids alone, so the changes can be read one at a time.
const IDS: &str = "change_id ++ \"\\n\"";

/// Why a custody read could not be produced (S3-err: the evidence is carried, never collapsed).
#[derive(Debug)]
pub enum CustodyError {
    /// The crossing itself could not run.
    Spawn(String),
    /// jj ran but failed; its captured output is the evidence.
    Failed(Output),
}

impl From<JjError> for CustodyError {
    fn from(error: JjError) -> Self {
        match error {
            JjError::Spawn(reason) => CustodyError::Spawn(reason),
        }
    }
}

/// Read every change with the paths it touched (S-read-custody).
///
/// One crossing for the whole set: the listing, whose template carries the diff summary of each
/// change it names. Each change arrives with its own paths, a rename's two sides arrive as two
/// paths, and no change or path is filtered out. A crossing that cannot run is a carried value,
/// never an empty listing, so no change is silently taken as clean.
pub fn read(jj: &Jj) -> Result<Vec<(String, Vec<String>)>, CustodyError> {
    let listing = jj.run(&["log", "--no-graph", "-r", "::", "-T", ENTRIES])?;
    if listing.status == 0 {
        return Ok(parse(&listing.stdout));
    }
    // A set read that cannot run is replaced by reading the changes one at a time. This is the
    // degraded form, not the healthy path: when it fails too, the evidence carried is the set
    // read's, since that is the crossing that was supposed to work.
    match one_at_a_time(jj) {
        Ok(pairs) => Ok(pairs),
        Err(_) => Err(CustodyError::Failed(listing)),
    }
}

/// The whole-set fallback: the ids in one crossing, then one crossing per change.
fn one_at_a_time(jj: &Jj) -> Result<Vec<(String, Vec<String>)>, CustodyError> {
    let out = jj.run(&["log", "--no-graph", "-r", "::", "-T", IDS])?;
    if out.status != 0 {
        return Err(CustodyError::Failed(out));
    }
    let mut pairs = Vec::new();
    for id in out.stdout.lines().filter(|line| !line.is_empty()) {
        let out = jj.run(&["log", "--no-graph", "-r", id, "-T", ENTRIES])?;
        if out.status != 0 {
            return Err(CustodyError::Failed(out));
        }
        pairs.extend(parse(&out.stdout));
    }
    Ok(pairs)
}

/// Split the fixed listing into `(change id, paths)` pairs.
///
/// Each line is one change: its id, then its changed entries as source and target path in pairs.
/// An ordinary change names the same path on both sides — one path, not two — while a rename or a
/// copy names two, so both sides arrive as paths the change touched. A path is paired with a
/// change once, however often the summary names it.
fn parse(stdout: &str) -> Vec<(String, Vec<String>)> {
    let mut changes = Vec::new();
    for line in stdout.lines().filter(|line| !line.is_empty()) {
        let mut fields = line.split('\t');
        let id = match fields.next() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => continue,
        };
        let mut paths: Vec<String> = Vec::new();
        for pair in fields.collect::<Vec<&str>>().chunks(2) {
            let source = pair.first().copied().unwrap_or_default();
            let target = pair.get(1).copied().unwrap_or_default();
            for path in [source, target] {
                if path.is_empty() || paths.iter().any(|seen| seen == path) {
                    continue;
                }
                paths.push(path.to_string());
            }
        }
        changes.push((id, paths));
    }
    changes
}
