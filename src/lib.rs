//! The crate root: the module map, the verb table, and the assembled entry.
//!
//! Every module is a single file directly under `src/` (M-layout, szmuyqrq), so this crate
//! root is the one place the module tree is declared — that declaration is the module set.
//! It also holds the verb table, the single enumeration the dispatcher (M-route, ynvwvzrm)
//! and the bare list (M-help, llorllwk) read, and the assembled entry `run`: resolve the
//! substrate's handles once (M-context-injection, ryuswoxz), hand the table and the context
//! to the envelope (M-envelope, xpxlwusu), and return its status. The entry resolves the
//! substrate's handles before the envelope, so resolution is total (S3-root-optional,
//! vxvsvzns): a root-less context is a state the init verb runs on, and a genuinely failing
//! resolution is handed back to the envelope as a refusal (M-resolution-failure, oqpxpkoo).
//! The globals are not exempt from resolution (M-globals-context, krxxmnzz); an unreadable
//! working directory is not fatal — `"."` stands in (M-cwd-fallback, nlornzvy).

pub mod context;
pub mod divergence;
pub mod envelope;
pub mod exclude;
pub mod help;
pub mod jj;
pub mod jj_abandon;
pub mod jj_init;
pub mod jj_new;
pub mod jj_place;
pub mod jjlog;
pub mod jjread;
pub mod journal;
pub mod journal_apply;
pub mod markers;
pub mod markers_append;
pub mod markers_capture;
pub mod markers_file;
pub mod markers_guards;
pub mod markers_persist;
pub mod oplog;
pub mod refusal;
pub mod revert;
pub mod route;
pub mod status;
pub mod store;
pub mod node_set;
pub mod node_load;
pub mod node_own;
pub mod store_path;
pub mod store_read;
pub mod node_resolve;
pub mod store_write;
pub mod verb_init;
pub mod verb_log;
pub mod verb_new;
pub mod verb_record;
pub mod verb_redo;
pub mod verb_remove;
pub mod verb_show;
pub mod verb_undo;
pub mod verb_validate;
pub mod version;

use std::env;
use std::path::PathBuf;

use context::{Context, ContextError};
use refusal::Refusal;
use route::Verb;

/// The verb table: one row per verb, each row pointing at the verb's own handler. `init`
/// comes first, so adding it touched nothing else (D23, oywywrvu).
pub const VERBS: &[Verb<Context>] = &[
    Verb {
        name: "init",
        run: verb_init::run,
    },
    Verb {
        name: "show",
        run: verb_show::run,
    },
    Verb {
        name: "log",
        run: verb_log::run,
    },
    Verb {
        name: "validate",
        run: verb_validate::run,
    },
    Verb {
        name: "new",
        run: verb_new::run,
    },
    Verb {
        name: "record",
        run: verb_record::run,
    },
    Verb {
        name: "remove",
        run: verb_remove::run,
    },
    Verb {
        name: "undo",
        run: verb_undo::run,
    },
    Verb {
        name: "redo",
        run: verb_redo::run,
    },
];

/// The assembled entry: resolve the substrate's handles once at the process's working
/// directory and hand the table and the context to the envelope, returning its status. A
/// context that cannot be resolved is handed back to the envelope as a refusal
/// (M-resolution-failure), so every line still leaves by the envelope's channels; the
/// globals are not exempt from resolution (M-globals-context). An unreadable working
/// directory is not fatal — `"."` stands in (M-cwd-fallback).
pub fn run(argv: &[String]) -> i32 {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match context::resolve(&cwd) {
        Ok(context) => envelope::run(VERBS, &context, argv),
        Err(error) => {
            let refusal = Refusal::new(context_reason(&error));
            envelope::emit(&envelope::Outcome::Refused(refusal))
        }
    }
}

/// Why the context could not be resolved: the reason is carried (S3-err).
fn context_reason(error: &ContextError) -> String {
    match error {
        ContextError::BinaryNotFound(reason) | ContextError::Spawn(reason) => reason.clone(),
    }
}
