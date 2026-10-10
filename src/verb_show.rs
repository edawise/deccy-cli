//! The show verb: read a node's document and render it, one output per valid node.
//!
//! D16a fixes the read and its one-output-per-valid-node rendering; D21 fixes refusal of an
//! id that names nothing and ouyrkmlr refusal of an invocation that names no id at all; O8
//! fixes the document rendering — the stored document as pretty
//! JSON, verbatim — and O8-root the marker the root prints in place of a document. The id is
//! parsed by store_path and resolved the way edge targets resolve, through node_resolve. The
//! verb changes nothing and returns its lines for the envelope to write (M-output-return),
//! carrying its own --help.

use crate::context::Context;
use crate::jj::Jj;
use crate::refusal::Refusal;
use crate::store_path::Id;
use crate::node_resolve::{resolve, Resolved};

/// The verb's own `--help` text (per-verb help is the verb's content: xlvoxmyl, rwlywqrk).
pub const HELP: &str = "deccy show <id>...: print each node's stored document as pretty JSON.";

/// The marker `deccy show zzzzzzzz` prints for the root, which names no file (O8-root).
pub const ROOT_MARKER: &str = "zzzzzzzz";

/// The show handler: M-route's shape, returning lines for the envelope to write.
pub fn run(context: &Context, args: &[String]) -> Result<Vec<String>, Refusal> {
    show(context.jj(), args)
}

/// Read each id's target and render it (D16a, O8, O8-root, D21); an id-less invocation
/// refuses (ouyrkmlr).
fn show(jj: &Jj, args: &[String]) -> Result<Vec<String>, Refusal> {
    if args.first().map(String::as_str) == Some("--help") {
        return Ok(lines_of(HELP));
    }
    if args.is_empty() {
        return Err(Refusal::new("no id given"));
    }
    let mut lines = Vec::new();
    for arg in args {
        let id = Id::parse(arg).ok_or_else(|| unknown(arg))?;
        match resolve(jj, &id).map_err(|_| unknown(arg))? {
            Resolved::Root => lines.push(ROOT_MARKER.to_string()),
            Resolved::Node(document) => lines.extend(render(&document)),
        }
    }
    Ok(lines)
}

/// O8's rendering: the loaded document as pretty JSON, split into lines.
fn render(document: &serde_json::Value) -> Vec<String> {
    lines_of(&serde_json::to_string_pretty(document).expect("a loaded document re-serializes"))
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

/// D21's refusal: the id that names nothing, named back.
fn unknown(raw: &str) -> Refusal {
    Refusal::new("unknown id").with_evidence(raw)
}
