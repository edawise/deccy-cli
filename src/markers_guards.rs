//! The marker stack and window guards: undo and redo as decisions over the state.
//!
//! S5-undo (qkqkuzxw) and G-select-undo (wwrrqtox) fix which boundary undo names — the
//! most recent applied pair's pre-boundary — and S5-redo (yvtmrnsr) with G-select-redo
//! (qtysmzxy) which post-boundary redo names; S5-undo-effect (zuqzrlrx) and S5-redo-effect
//! (xovuxpvq) fix the whole state each leaves. S5-ordered (kupzywsq), G-order (ukulkqqw)
//! and D15b (qwqwqtuq) fix redo's window — markers reveal the order operations ran in, so a
//! newer pair means redo is no longer re-application — S5-pointer-bounds (ltrmwytx) that a
//! count or bound past the pairs is not a valid state, and S5-empty-undo (twmzwwll) and
//! S5-empty-redo (tomomvzw) name the two empty cases. M-layout (szmuyqrq) places the module
//! flat at src/markers_guards.rs.
//!
//! What this fixes: two transitions over the state alone. `undo` answers with the state
//! whose undone count is one further back and whose position is the pre-boundary of the
//! operation it undoes; `redo` answers with the state whose position is the post-boundary
//! of the next undone operation and whose undone count is one forward, the pairs standing.
//! The bound marks the window's end: redo is valid only while the pair count equals it
//! (D15b). The position is the restore target M-markers records.
//! Neither reads a file, restores anything, or writes anything: the answer is the state the
//! invocation persists, and a state that cannot answer is a refusal naming its cause
//! (M-refusal). Rendering is the envelope's.

use crate::markers::Markers;
use crate::refusal::Refusal;

/// The state undo leaves: the undone count one further back, the position at the
/// pre-boundary of the operation it undoes, the pairs standing (S5-undo-effect). The
/// bound is set to the pair count when the count starts from nothing-undone.
///
/// Nothing left to undo — an empty stack or every operation already undone — refuses,
/// naming that no operation exists (S5-empty-undo); so does a count or bound past the pairs,
/// which is no valid state (S5-pointer-bounds).
pub fn undo(state: Markers) -> Result<Markers, Refusal> {
    if state.u > state.b || state.b > state.markers.len() as u64 {
        return Err(beyond_the_pairs(&state));
    }
    let b = if state.u == 0 { state.markers.len() as u64 } else { state.b };
    if b == state.u {
        return Err(Refusal::new("no operation exists"));
    }
    Ok(Markers {
        u: state.u + 1,
        b,
        at: state.markers[(b - state.u - 1) as usize].pre.clone(),
        markers: state.markers,
    })
}

/// The state redo leaves: the position at the post-boundary of the next undone
/// operation, the undone count one forward, the pairs standing (S5-redo-effect).
///
/// Nothing undone refuses (S5-empty-redo); so does a count or bound past the pairs
/// (S5-pointer-bounds) and a pair appended since the bound while an undo run is
/// open, which makes redo a branch rather than re-application (G-order, D15b).
pub fn redo(state: Markers) -> Result<Markers, Refusal> {
    if state.u > state.b || state.b > state.markers.len() as u64 {
        return Err(beyond_the_pairs(&state));
    }
    if state.b > 0 && state.markers.len() as u64 > state.b {
        return Err(Refusal::new("a newer operation exists")
            .with_evidence(state.b.to_string())
            .with_evidence(state.markers.len().to_string()));
    }
    if state.u == 0 {
        return Err(Refusal::new("nothing is undone"));
    }
    Ok(Markers {
        u: state.u - 1,
        b: state.b,
        at: state.markers[(state.b - state.u) as usize].post.clone(),
        markers: state.markers,
    })
}

/// The refusal a count or bound past the pairs gets (S5-pointer-bounds).
fn beyond_the_pairs(state: &Markers) -> Refusal {
    Refusal::new("the undone count or its bound exceeds the bracketed pairs")
        .with_evidence(state.u.to_string())
        .with_evidence(state.markers.len().to_string())
}
