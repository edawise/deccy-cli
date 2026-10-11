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

    use crate::markers::{Kind, Marker, Markers};
    use crate::markers_guards::{redo, undo};

    /// The repo corpus document `<name>` under tests/fixtures/markers, as a state.
    fn corpus(name: &str) -> Markers {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/markers")
            .join(format!("{name}.json"));
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    /// One marker: a recording's kept entry ids and the reverts recorded against it.
    fn marker(entries: &[&str], reverts: &[&str]) -> Marker {
        Marker {
            kind: Kind::Record,
            target: "aa".to_string(),
            entries: entries.iter().map(|id| id.to_string()).collect(),
            reverts: reverts.iter().map(|id| id.to_string()).collect(),
        }
    }

    /// One creation marker: the created change's id and the entries its window kept.
    fn creating(target: &str, entries: &[&str]) -> Marker {
        Marker {
            kind: Kind::Create,
            target: target.to_string(),
            entries: entries.iter().map(|id| id.to_string()).collect(),
            reverts: Vec::new(),
        }
    }

    /// A hand-built state: the marker list and the position.
    fn state(markers: Vec<Marker>, pos: u64) -> Markers {
        Markers { markers, pos }
    }

    #[test]
    fn undo_steps_back_one_operation_per_call() {
        let start = state(
            vec![marker(&["01", "02"], &[]), marker(&["03", "04"], &[])],
            2,
        );
        let once = undo(start.clone()).unwrap();
        assert_eq!(once.next.pos, 1, "one operation is undone");
        assert_eq!(
            once.entries,
            ["03", "04"],
            "the operation's kept entry ids are named"
        );
        assert_eq!(once.next.markers, start.markers, "the marker list stands");
        let twice = undo(once.next).unwrap();
        assert_eq!(twice.next.pos, 0, "the position steps back again");
        assert_eq!(twice.entries, ["01", "02"]);
        assert_eq!(twice.next.markers, start.markers);
        let error = undo(twice.next).unwrap_err();
        assert_eq!(error.cause(), "no operation exists");
    }

    #[test]
    fn undo_names_the_operations_kind_and_target() {
        let start = state(vec![marker(&["01"], &[]), creating("beef", &[])], 2);
        let once = undo(start).unwrap();
        assert_eq!(
            once.kind,
            Kind::Create,
            "the marker before the position's kind is named"
        );
        assert_eq!(once.target, "beef", "the change it targeted is named");
        assert!(
            once.entries.is_empty(),
            "a creation's undo reverts no entries — it removes the change"
        );
        let twice = undo(once.next).unwrap();
        assert_eq!(twice.kind, Kind::Record);
        assert_eq!(twice.entries, ["01"], "a recording's entries are named");
    }

    #[test]
    fn undo_on_an_empty_stack_refuses() {
        let error = undo(corpus("init")).unwrap_err();
        assert!(!error.cause().is_empty());
    }

    #[test]
    fn undo_on_an_exhausted_stack_refuses() {
        let error = undo(corpus("undone2")).unwrap_err();
        assert_eq!(error.cause(), "no operation exists");
    }

    #[test]
    fn redo_steps_forward_one_operation_per_call() {
        let undone2 = corpus("undone2");
        let once = redo(undone2.clone()).unwrap();
        assert_eq!(once.next.pos, 1, "one operation is redone");
        assert_eq!(
            once.entries, undone2.markers[0].reverts,
            "the reverts recorded against the marker at the position are named"
        );
        assert_eq!(
            once.next.markers.len(),
            undone2.markers.len(),
            "the marker list stands"
        );
        assert!(
            once.next.markers[0].reverts.is_empty(),
            "the reverts it named are cleared"
        );
        assert_eq!(
            once.next.markers[1], undone2.markers[1],
            "the other marker stands"
        );
    }

    #[test]
    fn redo_at_nothing_undone_refuses() {
        let redone = redo(corpus("undone")).unwrap();
        assert_eq!(redone.next.pos, 1);
        for each in [corpus("appended"), redone.next] {
            let error = redo(each).unwrap_err();
            assert_eq!(error.cause(), "nothing is undone");
        }
    }

    #[test]
    fn an_append_after_an_undo_leaves_nothing_to_redo() {
        // S5-marker-append discards the undone tail: the truncated document holds only the
        // new operation, the position at the end, so redo has nothing to re-apply.
        let truncated = corpus("truncated");
        assert_eq!(truncated.pos, truncated.markers.len() as u64);
        let error = redo(truncated).unwrap_err();
        assert_eq!(error.cause(), "nothing is undone");
        assert_ne!(
            error.cause(),
            "a newer operation exists",
            "there is no newer case in the linear history"
        );
    }

    #[test]
    fn a_position_past_the_markers_refuses_both_transitions() {
        let past_the_markers = state(vec![marker(&["01"], &[]), marker(&["02"], &[])], 3);
        assert!(undo(past_the_markers.clone()).is_err());
        assert!(redo(past_the_markers).is_err());
        let one_past = state(vec![marker(&["01"], &[])], 2);
        assert!(undo(one_past.clone()).is_err());
        assert!(redo(one_past).is_err());
    }

    #[test]
    fn the_recording_lands_against_the_operation_undone() {
        let recorded = vec!["aa".to_string(), "bb".to_string()];
        let next = undo(corpus("appended")).unwrap().recorded(recorded.clone());
        assert_eq!(next.pos, 0);
        assert_eq!(next.markers[0].reverts, recorded, "the reverts are recorded");
        assert_eq!(
            next.markers[0].entries,
            corpus("appended").markers[0].entries,
            "the operation's kept entry ids stand"
        );
    }

    #[test]
    fn the_corpus_documents_land_on_one_another() {
        // appended undone is undone.
        let undone = undo(corpus("appended"))
            .unwrap()
            .recorded(corpus("undone").markers[0].reverts.clone());
        assert_eq!(undone, corpus("undone"));

        // undone redone and undone again is undone.
        let back = undo(redo(corpus("undone")).unwrap().next)
            .unwrap()
            .recorded(corpus("undone").markers[0].reverts.clone());
        assert_eq!(back, corpus("undone"));

        // undone2 redone twice reaches the end of the tail; two undos land it back.
        let redone = redo(redo(corpus("undone2")).unwrap().next).unwrap().next;
        assert_eq!(redone.pos, 2, "the position is at the end of the list");
        assert!(redone.markers.iter().all(|m| m.reverts.is_empty()));
        let once = undo(redone)
            .unwrap()
            .recorded(corpus("undone2").markers[1].reverts.clone());
        let twice = undo(once)
            .unwrap()
            .recorded(corpus("undone2").markers[0].reverts.clone());
        assert_eq!(twice, corpus("undone2"), "the round trip lands");
    }
}
