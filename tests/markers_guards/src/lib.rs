//! Gate for M-markers-guards (undo and redo over the marker state).
//!
//! A crate of its own: the guards names M-markers' state type and M-refusal's
//! value, and this check reads the marker corpus with serde_json. Self-contained per
//! M-tests — the shims pull in the modules under test and their siblings; no shared
//! test-support module. It reads the marker corpus by path and claims it not.

#[path = "../../../src/refusal.rs"]
mod refusal;
#[path = "../../../src/markers.rs"]
mod markers;
#[path = "../../../src/markers_guards.rs"]
mod markers_guards;

#[cfg(test)]
mod gate {
    use std::fs;
    use std::path::Path;

    use crate::markers::{Marker, Markers};
    use crate::markers_guards::{redo, undo};

    /// The repo corpus document `<name>` under tests/fixtures/markers, as a state.
    fn corpus(name: &str) -> Markers {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/markers")
            .join(format!("{name}.json"));
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    /// A hand-built state: one pair per (pre, post), the undone count at `u`, the bound at
    /// `b`, the position at the last pair's post-boundary.
    fn state(pairs: &[(&str, &str)], u: u64, b: u64) -> Markers {
        let markers: Vec<Marker> = pairs
            .iter()
            .map(|(pre, post)| Marker {
                pre: pre.to_string(),
                post: post.to_string(),
            })
            .collect();
        let at = markers
            .last()
            .map(|m| m.post.clone())
            .unwrap_or_else(|| "0".to_string());
        Markers { markers, u, b, at }
    }

    #[test]
    fn undo_steps_back_one_operation_per_call() {
        let shape = corpus("undone2");
        let pairs: Vec<(&str, &str)> = shape
            .markers
            .iter()
            .map(|m| (m.pre.as_str(), m.post.as_str()))
            .collect();
        let start = state(&pairs, 0, 0);
        let once = undo(start.clone()).unwrap();
        assert_eq!(once.u, 1);
        assert_eq!(once.b, 2);
        assert_eq!(once.at, start.markers[1].pre);
        assert_eq!(once.markers, start.markers);
        let twice = undo(once.clone()).unwrap();
        assert_eq!(twice.u, 2);
        assert_eq!(twice.b, 2);
        assert_eq!(twice.at, start.markers[0].pre);
        assert_eq!(twice.markers, start.markers);
        let error = undo(twice).unwrap_err();
        assert_eq!(error.cause(), "no operation exists");
    }

    #[test]
    fn undo_on_an_empty_stack_refuses() {
        let error = undo(corpus("init")).unwrap_err();
        assert!(!error.cause().is_empty());
    }

    #[test]
    fn redo_steps_forward_one_operation_per_call() {
        let undone2 = corpus("undone2");
        let once = redo(undone2.clone()).unwrap();
        assert_eq!(once.u, 1);
        assert_eq!(once.b, 2);
        assert_eq!(once.at, undone2.markers[0].post);
        assert_eq!(once.markers, undone2.markers);
        let twice = redo(once.clone()).unwrap();
        assert_eq!(twice.u, 0);
        assert_eq!(twice.b, 2);
        assert_eq!(twice.at, undone2.markers[1].post);
        assert_eq!(twice.markers, undone2.markers);
    }

    #[test]
    fn redo_in_the_never_undone_case_refuses() {
        let completed = redo(corpus("undone")).unwrap();
        assert_eq!(completed.u, 0);
        for each in [corpus("appended"), completed] {
            let error = redo(each).unwrap_err();
            assert_eq!(error.cause(), "nothing is undone");
        }
    }

    #[test]
    fn redo_when_an_undo_run_is_superseded_by_a_bracket_refuses() {
        let newer = corpus("newer");
        assert_eq!(newer.u, 0, "the refusal fires even though nothing is undone");
        let error = redo(newer).unwrap_err();
        assert_eq!(error.cause(), "a newer operation exists");
    }

    #[test]
    fn a_count_or_bound_past_the_pairs_refuses_both_transitions() {
        let past_the_bound = state(&[("01", "02"), ("03", "04")], 3, 2);
        assert!(undo(past_the_bound.clone()).is_err());
        assert!(redo(past_the_bound).is_err());
        let past_the_pairs = state(&[("01", "02")], 1, 2);
        assert!(undo(past_the_pairs.clone()).is_err());
        assert!(redo(past_the_pairs).is_err());
    }

    #[test]
    fn the_corpus_round_trips() {
        let undone = undo(corpus("appended")).unwrap();
        assert_eq!(undone, corpus("undone"));
        assert_eq!(
            undo(undo(corpus("newer")).unwrap()).unwrap(),
            corpus("undone2")
        );
        assert_eq!(undo(redo(undone).unwrap()).unwrap(), corpus("undone"));
        assert_eq!(
            undo(redo(corpus("undone2")).unwrap()).unwrap(),
            corpus("undone2")
        );
    }
}
