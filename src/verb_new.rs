//! The new verb: create a node in a new change after the named dependencies.
//!
//! kmrmwopx fixes the create operation — `deccy new -m <summary> --on <id>...` opens a change
//! after the named dependencies, writes the node file named by the new change id, and
//! describes the change with the summary. The form refuses without `--on` (lptlmrtv) or
//! without `-m` (N7, zzuoupwo), refuses an argument it does not name, naming it (N8,
//! punvtxvs), and recognizes its flags in any order (N9, ronsyqlz). D22 (ukmntzqs, mtqypkwo,
//! nmnlqoyx) refuses an `--on` set naming anything that does not resolve, before birth,
//! naming each unknown id. The birth brackets through M-markers-capture (okkvnnvm) — the
//! lifecycle table's creation case (S5-lifecycle, vwlrqoxk) — opening the change through
//! M-jj-new (xmlwqpyr, S3-new); the position is the newest dependency, the unique head of the
//! --on set read from jj (S3-new-position, N1/olmoqxzo), and a set with more than one head
//! refuses before birth, naming each tied id (S3-new-position refusal, N2/xvyyrqwv); placement
//! is ylvpykys, the change id the crossing reports is
//! the node's id (D12) and its file is oyxxypuz; a birth that cannot complete — the crossing
//! reports no usable id, or the write fails — abandons the change it opened (S5-birth-fail,
//! kvwonkzy) through M-jj-abandon (urtkpxwv) and refuses naming the cause. The skeleton N6
//! fixes (rkqztuuw) is kind
//! decision (pzquqmrq), type text with ref inline and placeholder content (tvouykvz) and a
//! placeholder rationale (swqoqoso), and validates against schemas/node.json (wrmumorm); its
//! depends_on holds exactly the given ids (msyrpuol) and the change carries the summary
//! (yuznnnlx). The created id is printed as one line (O6, pqxtmwqw). It resolves through
//! M-node-resolve (wynwwtur), names the file through M-store-path (vkztyrzm), reads
//! through M-store-read across the runner (M-jj), writes through M-store-write (mxwoktuw), builds its refusals through
//! M-refusal (popowxnw), takes the injected context (ryuswoxz, zttrwlts), and returns its
//! lines for the envelope to write (utrzvyyt) under M-route's handler shape (ynvwvzrm),
//! carrying its own --help text (xlvoxmyl/rwlywqrk, per oqnsuzqu). I1-serde (orrvsxqq) fixes
//! the JSON layer the document is written with. This module renders nothing.

use crate::context::Context;
use crate::jj::{Jj, JjError};
use crate::jj_abandon;
use crate::jj_new::{self, OpenError};
use crate::markers_capture::{self, CaptureError};
use crate::markers_persist::PersistError;
use crate::oplog::OplogError;
use crate::refusal::Refusal;
use crate::store::Store;
use crate::store_path::Id;
use crate::node_resolve::resolve;
use crate::store_write;

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str =
    "deccy new -m <summary> --on <id>...: create a node in a new change after the named dependencies.";

/// The placeholder body the skeleton carries; the author replaces it (N6, tvouykvz).
const PLACEHOLDER_CONTENT: &str = "<placeholder>";

/// The placeholder justification the skeleton carries; the author replaces it (N6, swqoqoso).
const PLACEHOLDER_RATIONALE: &str = "<placeholder>";

/// The new handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    let store = context.store()?;
    create(store, context.jj(), args)
}

/// The create operation (kmrmwopx): parse the form, resolve every `--on` id, then bracket a
/// placed birth and report the new id as one line (O6).
fn create(store: &Store, jj: &Jj, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(vec![HELP.to_string()]);
    }
    let invocation = parse(args)?;
    refuse_unknown(jj, &invocation.on)?;
    let outcome = markers_capture::capture(store, jj, || {
        birth(store, jj, &invocation.summary, &invocation.on)
    })
    .map_err(capture_failed)?;
    let id = outcome?;
    Ok(vec![id])
}

/// The parsed form: one summary and the repeated `--on` ids, in the order given (D12, N9).
struct Invocation {
    summary: String,
    on: Vec<String>,
}

/// Parse the form's flags in any order (N9): one required `-m <summary>`, repeated required
/// `--on <id>`. Anything the form does not name refuses, naming the argument (N8).
fn parse(args: &[String]) -> Result<Invocation, Refusal> {
    let mut summary: Option<String> = None;
    let mut on: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "-m" => {
                if summary.is_some() {
                    return Err(Refusal::new("a second -m").with_evidence("-m"));
                }
                summary = Some(value(args, index, "-m")?);
                index += 2;
            }
            "--on" => {
                on.push(value(args, index, "--on")?);
                index += 2;
            }
            other => {
                return Err(Refusal::new("an unnamed argument").with_evidence(other));
            }
        }
    }
    let summary = summary.ok_or_else(|| Refusal::new("no -m").with_evidence("-m"))?;
    if on.is_empty() {
        return Err(Refusal::new("no --on").with_evidence("--on"));
    }
    Ok(Invocation { summary, on })
}

/// The value that follows a flag, or a refusal naming the flag that lacks one (N8, N7).
fn value(args: &[String], index: usize, flag: &str) -> Result<String, Refusal> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| Refusal::new(format!("{flag} without its value")).with_evidence(flag))
}

/// Resolve every `--on` id before anything is created; any id that does not resolve refuses
/// the whole invocation, naming each unknown id (D22, ukmntzqs, mtqypkwo, nmnlqoyx).
fn refuse_unknown(jj: &Jj, on: &[String]) -> Result<(), Refusal> {
    let mut unknown: Vec<String> = Vec::new();
    for raw in on {
        let resolved = Id::parse(raw)
            .map(|id| resolve(jj, &id).is_ok())
            .unwrap_or(false);
        if !resolved {
            unknown.push(raw.clone());
        }
    }
    if unknown.is_empty() {
        return Ok(());
    }
    let mut refusal = Refusal::new("an unknown --on id");
    for id in unknown {
        refusal = refusal.with_evidence(id);
    }
    Err(refusal)
}

/// Open the change after the named dependencies (ylvpykys, S3-new), take the change id the
/// crossing reports as the node's id (D12), and write the skeleton to `nodes/<id>.json`
/// through the atomic write seam (oyxxypuz, N6). A birth that cannot complete abandons the
/// change it opened (S5-birth-fail), so no half-born change is left behind.
fn birth(store: &Store, jj: &Jj, summary: &str, on: &[String]) -> Result<String, Refusal> {
    let parents: Vec<&str> = on.iter().map(String::as_str).collect();
    let output = jj_new::open(jj, summary, &parents).map_err(open_failed)?;
    if output.status != 0 {
        return Err(Refusal::new(output.stderr.trim().to_string()));
    }
    // The open succeeded: every failure below abandons the change it opened, so no half-born
    // change is left behind (S5-birth-fail). The change it opened is the one the crossing
    // reports; when it reports no id, the working copy it left is what is abandoned.
    let raw = match reported_id(&output.stderr) {
        Some(raw) => raw,
        None => {
            return Err(abandoned(
                jj,
                "@",
                Refusal::new("the crossing reported no change id"),
            ))
        }
    };
    let id = match Id::parse(raw) {
        Some(id) => id,
        None => {
            return Err(abandoned(
                jj,
                raw,
                Refusal::new("the crossing reported a malformed id").with_evidence(raw),
            ))
        }
    };
    let document = skeleton(on);
    let text = serde_json::to_string(&document).expect("a skeleton serializes");
    store_write::write(store, &id, &text)
        .map_err(|error| abandoned(jj, id.as_str(), Refusal::new(error.to_string())))?;
    Ok(id.as_str().to_string())
}

/// Abandon the change a failed birth opened, then refuse with `refusal`; when the abandon
/// itself does not complete, its own failure is carried instead of being swallowed
/// (S5-birth-fail, M-jj-abandon urtkpxwv).
fn abandoned(jj: &Jj, target: &str, refusal: Refusal) -> Refusal {
    match jj_abandon::abandon(jj, target) {
        Ok(output) if output.status == 0 => refusal,
        Ok(output) => Refusal::new(output.stderr.trim().to_string()),
        Err(JjError::Spawn(reason)) => Refusal::new(reason),
    }
}

/// The change id the crossing reports for the change it just opened: the token following
/// `now at:` on jj's report (D12). Never re-derived from `jj log`.
fn reported_id(stderr: &str) -> Option<&str> {
    stderr
        .split("now at:")
        .nth(1)?
        .split_whitespace()
        .next()
}

/// The skeleton `deccy new` writes (N6, rkqztuuw): kind decision, type text, ref inline with a
/// placeholder body and a placeholder rationale, and depends_on naming exactly the given ids
/// in the order given (msyrpuol).
fn skeleton(on: &[String]) -> serde_json::Value {
    serde_json::json!({
        "kind": "decision",
        "depends_on": on,
        "content": {
            "type": "text",
            "ref": "inline",
            "content": PLACEHOLDER_CONTENT,
            "rationale": PLACEHOLDER_RATIONALE,
        }
    })
}

/// A birth that could not open a change: a set with more than one head in jj history names
/// each tied id (N2), and a spawn failure carries its reason (S3-err).
fn open_failed(error: OpenError) -> Refusal {
    match error {
        OpenError::Spawn(reason) => Refusal::new(reason),
        OpenError::NonLinear(ids) => {
            let mut refusal = Refusal::new("a forked --on set");
            for id in ids {
                refusal = refusal.with_evidence(id);
            }
            refusal
        }
    }
}

/// A bracket that did not close: its reason is carried, both variants (okkvnnvm).
fn capture_failed(error: CaptureError) -> Refusal {
    match error {
        CaptureError::Oplog(error) => Refusal::new(oplog_reason(&error)),
        CaptureError::Append(error) => Refusal::new(persist_reason(&error)),
    }
}

/// Why the op head could not be read (S3-oplog, mvwnksou).
fn oplog_reason(error: &OplogError) -> String {
    match error {
        OplogError::Spawn(reason) => reason.clone(),
        OplogError::Failed(output) => output.stderr.trim().to_string(),
    }
}

/// Why the pair could not be appended (S5-marker-corrupt, vqqmwsml).
fn persist_reason(error: &PersistError) -> String {
    match error {
        PersistError::Unreadable(reason)
        | PersistError::NotJson(reason)
        | PersistError::Invalid(reason)
        | PersistError::Schema(reason)
        | PersistError::Unwritable(reason) => reason.clone(),
    }
}
