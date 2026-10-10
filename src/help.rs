//! The bare verb list: one self-describing surface per invocation.
//!
//! D11 fixes help as the self-describing surface and omsvqqor the bare list; rwlywqrk and
//! O5 fix how the lines read (plain UTF-8, newline-terminated). This renders only the bare
//! list from the table it is handed and returns the text — the envelope owns the channel.
//! The table is generic over its context (the dispatcher threads it to handlers); the list
//! reads only names.

use crate::route::Verb;

/// The bare verb list: one verb name per line, in table order.
pub fn bare_list<C>(verbs: &[Verb<C>]) -> String {
    let mut out = String::new();
    for v in verbs {
        out.push_str(v.name);
        out.push('\n');
    }
    out
}
