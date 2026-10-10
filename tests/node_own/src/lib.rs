//! Gate for M-node-own (files attributed to a change).
//!
//! A crate of its own: ownership reads the loaded document as serde_json, which
//! plain `rustc --test` cannot supply. Self-contained per M-tests — the shims pull in the
//! module under test and its siblings; no shared test-support module.
//!
//! The check covers the whole of D2's normal form as refined: not only the spellings that name
//! no directory (`./`, repeated `/`, `.`), but the one that names where a name sits — each `..`
//! folds against the ordinary component before it, and a `..` left after folding names a place
//! outside the repository, so it contributes no owned path.

#[path = "../../../src/store.rs"]
mod store;
#[path = "../../../src/store_path.rs"]
mod store_path;
#[path = "../../../src/node_own.rs"]
mod node_own;

#[cfg(test)]
mod gate {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use serde_json::Value;

    use crate::node_own::owned;
    use crate::store_path::Id;

    const NODE_FILE: &str = "nodes/abcdefgh.json";

    const ARRAY_REF: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs","tests/b.rs"],"rationale":"x"}}"#;
    const INLINE_REF: &str = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"text","ref":"inline","content":"x","rationale":"y"}}"#;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn doc(body: &str) -> Value {
        serde_json::from_str(body).unwrap()
    }

    /// A loaded document whose `content.ref` is the given array (D2: ref as an array of paths).
    fn doc_with_ref(refs: &[&str]) -> Value {
        let listed: Vec<String> = refs.iter().map(|path| format!("{path:?}")).collect();
        doc(&format!(
            r#"{{"kind":"decision","depends_on":["zzzzzzzz"],"content":{{"type":"rust","ref":[{}],"rationale":"x"}}}}"#,
            listed.join(",")
        ))
    }

    /// The owned set as a change's guards read it: one string per owned path, in the spelling
    /// the derivation yields. D2 reads each listed path in normal form, so the spelling is part
    /// of what the owned set answers, not a formatting detail.
    fn spellings(files: &BTreeSet<PathBuf>) -> BTreeSet<String> {
        files
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn an_array_ref_owns_its_node_file_and_each_ref_path() {
        let files = owned(&id("abcdefgh"), &doc(ARRAY_REF));
        assert_eq!(
            files,
            BTreeSet::from([
                PathBuf::from("nodes/abcdefgh.json"),
                PathBuf::from("src/a.rs"),
                PathBuf::from("tests/b.rs"),
            ])
        );
    }

    #[test]
    fn an_inline_ref_owns_the_node_file_alone() {
        let files = owned(&id("abcdefgh"), &doc(INLINE_REF));
        assert_eq!(files, BTreeSet::from([PathBuf::from("nodes/abcdefgh.json")]));
    }

    #[test]
    fn nothing_beyond_the_node_file_and_refs_is_owned() {
        let extra = r#"{"kind":"decision","depends_on":["zzzzzzzz"],"content":{"type":"rust","ref":["src/a.rs"],"rationale":"x","content":"ignored"}}"#;
        let files = owned(&id("abcdefgh"), &doc(extra));
        assert_eq!(
            files,
            BTreeSet::from([
                PathBuf::from("nodes/abcdefgh.json"),
                PathBuf::from("src/a.rs"),
            ])
        );
    }

    #[test]
    fn a_leading_dot_slash_does_not_buy_a_second_owner_for_one_file() {
        // D2: `./x` and `x` name one file, so the pair owns the file once.
        let files = owned(
            &id("abcdefgh"),
            &doc_with_ref(&["nodes/x.json", "./nodes/x.json"]),
        );
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "nodes/x.json".to_string()])
        );
    }

    #[test]
    fn a_repeated_separator_does_not_buy_a_second_owner_for_one_file() {
        // D2: `x//y` and `x/y` name one file, so the pair owns the file once.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["a//b", "a/b"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "a/b".to_string()])
        );
    }

    #[test]
    fn a_listed_path_is_owned_in_normal_form() {
        // D2 reads every listed path in normal form, duplicated or not: the owned entry is the
        // normal spelling, never the one the writer typed.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["./src//a.rs"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "src/a.rs".to_string()])
        );
    }

    #[test]
    fn three_or_more_separators_collapse_to_one() {
        // D2: repeated `/` separators do not distinguish paths, however many there are —
        // `a///b` names `a/b`, so it is owned under the one normal spelling.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["a///b"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "a/b".to_string()])
        );
    }

    #[test]
    fn an_interior_dot_component_is_not_a_second_owner() {
        // D2 reads every listed path in normal form: the `.` component names no directory,
        // so `a/./b` is owned as `a/b`, under the one normal spelling.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["a/./b"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "a/b".to_string()])
        );
    }

    #[test]
    fn a_leading_dot_slash_with_repeats_normalises_to_one_entry() {
        // D2: the two non-distinguishing spellings together still name one file —
        // `./a//b` is owned as `a/b`, not as a second owner of it.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["./a//b"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "a/b".to_string()])
        );
    }

    #[test]
    fn an_interior_parent_dir_does_not_buy_a_second_owner_for_one_file() {
        // D2 reads each `..` as folded against the ordinary component before it, so
        // `src/../src/x.rs` and `src/x.rs` name one file: the pair owns it once, under the
        // folded spelling.
        let files = owned(
            &id("abcdefgh"),
            &doc_with_ref(&["src/../src/x.rs", "src/x.rs"]),
        );
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "src/x.rs".to_string()])
        );
    }

    #[test]
    fn a_parent_dir_folds_against_the_component_before_it() {
        // D2: `a/b/../c` and `a/c` name one file, so a lone listed `a/b/../c` is owned as
        // `a/c` — folding, not discarding the path, is what normal form does here.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["a/b/../c"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "a/c".to_string()])
        );
    }

    #[test]
    fn a_listed_path_whose_normal_form_escapes_the_repo_owns_nothing() {
        // D2: a normal form in which a `..` remains after folding names a place outside the
        // repository and is not ownable — the change owns its node file, and no path for `../x`.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["../x"]));
        assert_eq!(spellings(&files), BTreeSet::from([NODE_FILE.to_string()]));
    }

    #[test]
    fn an_escape_after_folding_owns_nothing_and_its_neighbour_stays_owned() {
        // D2: `a/../../x` still carries a `..` after folding `..` against `a`, so it contributes
        // no owned path — and dropping it does not disturb the path listed beside it.
        let files = owned(&id("abcdefgh"), &doc_with_ref(&["a/../../x", "src/a.rs"]));
        assert_eq!(
            spellings(&files),
            BTreeSet::from([NODE_FILE.to_string(), "src/a.rs".to_string()])
        );
    }
}
