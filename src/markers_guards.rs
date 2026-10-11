//! The marker cursor guards: undo and redo as decisions over the state.
//!
//! S5-undo (qkqkuzxw) and G-select-undo (wwrrqtox) fix which operation undo names — the
//! marker before the position — and S5-redo (yvtmrnsr) with G-select-redo (qtysmzxy) the
//! marker at it; S5-undo-effect (zuqzrlrx) and S5-redo-effect (xovuxpvq) fix the whole state
//! each leaves. S5-ordered (kupzywsq) and G-order (ukulkqqw) fix that the position splits the
//! applied prefix from the undo tail, D15b (qwqwqtuq) and the linear history (kpptprql) that
//! redo is valid exactly while the position is before the list, S5-count-bounds (ltrmwytx)
//! that a position past the markers is not a valid state, and S5-empty-undo (twmzwwll) and
//! S5-empty-redo (tomomvzw) name the two empty cases. M-layout (szmuyqrq) places the module
//! flat at src/markers_guards.rs.
//!
//! What this fixes: two cursor transitions over the state alone. `undo` answers with the
//! position one further back and the marker at pos-1 — its kind, target, kept entry ids and
//! the place to record the reverts — named for the verb to revert; the reverts the verb makes
//! are recorded against that marker. `redo` answers with the marker at pos — the reverts
//! recorded against it named for the verb — cleared and the position one forward. The marker
//! list stands under both. There is no newer-operation check: a new operation discards the
//! undone tail before it appends (S5-marker-append), so redo is never a branch.
//! Neither reads a file, reverts anything, or writes anything: the answer is the state the
//! invocation persists, and a state that cannot answer is a refusal naming its cause
//! (M-refusal). Rendering is the envelope's.

use crate::markers::{Kind, Markers};
use crate::refusal::Refusal;

/// What a transition answers with: the state the invocation persists and the entries the
/// verb reverts (S5-undo-effect, S5-redo-effect).
///
/// The state is shaped here — the position and the marker list — and `entries` names what
/// the verb reverts: the operation's own kept entry ids, or the reverts recorded against it.
/// `index` is the marker the transition names, the operation undone or redone; undo's
/// completion records the reverts it made there.
#[derive(Debug)]
pub struct Transition {
    /// The state the invocation persists.
    pub next: Markers,
    /// The entry ids the verb reverts (S5-undo-effect, S5-redo-effect).
    pub entries: Vec<String>,
    /// The kind of the operation the transition names (G-operation): undo dispatches on it.
    pub kind: Kind,
    /// The change that operation targeted (G-marker), for undo's removal half.
    pub target: String,
    /// The marker the transition names.
    index: usize,
}

impl Transition {
    /// Undo's completion: record the reverts the verb made against the operation it undoes,
    /// and take the state (S5-undo-effect). Redo's transition is complete as it stands: the
    /// reverts it names are already cleared.
    pub fn recorded(mut self, reverts: Vec<String>) -> Markers {
        self.next.markers[self.index].reverts = reverts;
        self.next
    }
}

/// The state undo leaves: the position one further back, the marker list standing, and the
/// operation's kept entry ids named for the verb to revert (S5-undo-effect). `recorded` lands
/// the reverts the verb makes against that operation.
///
/// A position at zero — an empty stack or every operation already undone — refuses, naming
/// that no operation exists (S5-empty-undo); so does a position past the markers, which is no
/// valid state (S5-count-bounds).
pub fn undo(state: Markers) -> Result<Transition, Refusal> {
    if beyond(&state) {
        return Err(beyond_the_markers(&state));
    }
    if state.pos == 0 {
        return Err(Refusal::new("no operation exists"));
    }
    let index = (state.pos - 1) as usize;
    let entries = state.markers[index].entries.clone();
    let kind = state.markers[index].kind;
    let target = state.markers[index].target.clone();
    Ok(Transition {
        next: Markers {
            pos: state.pos - 1,
            markers: state.markers,
        },
        entries,
        kind,
        target,
        index,
    })
}

/// The state redo leaves: the reverts recorded against the marker at the position named for
/// the verb and cleared, the position one forward, the marker list standing (S5-redo-effect).
///
/// A position at the end of the list — nothing undone — refuses (S5-empty-redo); so does a
/// position past the markers (S5-count-bounds). There is no newer-operation case: a new
/// operation discards the undone tail before it appends (kpptprql).
pub fn redo(state: Markers) -> Result<Transition, Refusal> {
    if beyond(&state) {
        return Err(beyond_the_markers(&state));
    }
    if state.pos == state.markers.len() as u64 {
        return Err(Refusal::new("nothing is undone"));
    }
    let index = state.pos as usize;
    let entries = state.markers[index].reverts.clone();
    let kind = state.markers[index].kind;
    let target = state.markers[index].target.clone();
    let mut markers = state.markers;
    markers[index].reverts = Vec::new();
    Ok(Transition {
        next: Markers {
            pos: state.pos + 1,
            markers,
        },
        entries,
        kind,
        target,
        index,
    })
}

/// Whether the position runs past the markers (S5-count-bounds).
fn beyond(state: &Markers) -> bool {
    state.pos > state.markers.len() as u64
}

/// The refusal a position past the markers gets (S5-count-bounds).
fn beyond_the_markers(state: &Markers) -> Refusal {
    Refusal::new("the position exceeds the markers")
        .with_evidence(state.pos.to_string())
        .with_evidence(state.markers.len().to_string())
}
