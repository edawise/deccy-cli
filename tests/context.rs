//! Gate for M-context (the seam's caller), amended for the trunk state root.
//!
//! Self-contained per M-tests: the shim declares the module under test and its code parents
//! (jj, store, refusal); no shared test-support module. Plain `rustc --test`, std only — it
//! runs a real jj in scratch repos and pins the two-roots contract: resolution finds the trunk
//! state root from the invocation workspace root, the store is opened on that trunk root,
//! `state()` hands the store in any workspace while `store()` refuses outside the trunk
//! working copy naming the workspace it ran in, and a workspace whose repository entry exists
//! but cannot be resolved fails resolution instead of falling back to the invocation root.
//!
//! A directory with no workspace at all stays root-less (S3-root-optional, vxvsvzns): only a
//! workspace whose entry cannot be resolved is a failure (M-TWO-ROOTS).
//!
//! The shim declares `state_root` once at the crate root, as the crate root that owns the
//! module map does: the context's state-root failure must carry that module's type, not a
//! second copy the context declares for itself.

#[path = "../src/jj.rs"]
mod jj;
#[path = "../src/store.rs"]
mod store;
#[path = "../src/refusal.rs"]
mod refusal;
#[path = "../src/state_root.rs"]
mod state_root;
#[path = "../src/context.rs"]
mod context;

use std::path::{Path, PathBuf};
use std::process::Command;

use context::resolve;

/// A scratch directory of this gate, cleared before each use.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deccy-context-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A jj command with an explicit committer, so adding a linked workspace does not depend on
/// the machine's jj configuration.
fn jj_configured(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("jj")
        .arg("--config")
        .arg("user.name=\"context gate\"")
        .arg("--config")
        .arg("user.email=\"context-gate@example.invalid\"")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("jj is on PATH for the gate")
}

fn jj_repo(dir: &Path) {
    let status = Command::new("jj")
        .args(["git", "init"])
        .arg(dir)
        .status()
        .expect("jj is on PATH for the gate");
    assert!(status.success(), "jj git init must succeed");
}

fn workspace_root(dir: &Path) -> String {
    let out = Command::new("jj")
        .args(["workspace", "root"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A linked workspace at `dest` of the repo whose main workspace is `main` (M-TWO-ROOTS).
///
/// The pointer is whatever this jj writes: the fixture is the real thing, not a rewritten
/// entry.
fn linked_workspace(main: &Path, dest: &Path) {
    let dest = dest.to_str().expect("a scratch path is UTF-8");
    let out = jj_configured(main, &["workspace", "add", dest]);
    assert!(
        out.status.success(),
        "jj workspace add must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        Path::new(dest).join(".jj").join("repo").is_file(),
        "a linked workspace's .jj/repo is a pointer file"
    );
}

/// One path, canonicalized so two spellings of the same directory compare equal.
fn canon(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("{} resolves: {error}", path.display()))
}

#[test]
fn the_resolved_binary_is_a_real_executable_path() {
    let dir = scratch("binary");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    assert!(
        ctx.jj().binary().is_file(),
        "binary {:?} must be a real file",
        ctx.jj().binary()
    );
}

#[test]
fn the_resolved_root_is_the_workspace_root() {
    let dir = scratch("root");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    let root = ctx.root().expect("a workspace resolves a root");
    assert_eq!(root.to_string_lossy(), workspace_root(&dir));
}

#[test]
fn the_store_is_rooted_at_the_trunk_and_state_returns_it() {
    let dir = scratch("trunk-store");
    jj_repo(&dir);
    let ctx = resolve(&dir).expect("a jj repo resolves");
    let root = ctx.root().expect("a workspace resolves a root");
    assert_eq!(
        root.to_string_lossy(),
        workspace_root(&dir),
        "the context root is the invocation workspace root"
    );
    let store = ctx
        .store()
        .expect("the main workspace is the trunk working copy");
    assert_eq!(
        canon(store.root()),
        canon(root),
        "the store is opened on the trunk root, here the main workspace"
    );
    let state = ctx
        .state()
        .expect("the state directory is readable in any workspace");
    assert!(
        std::ptr::eq(state, store),
        "state() and store() hand the same store"
    );
}

#[test]
fn from_a_linked_workspace_the_store_is_rooted_at_the_main_workspace() {
    let base = scratch("linked-root");
    let main = base.join("main");
    let linked = base.join("linked");
    jj_repo(&main);
    linked_workspace(&main, &linked);

    let ctx = resolve(&linked).expect("a linked workspace resolves");
    let root = ctx.root().expect("a linked workspace resolves a root");
    assert_eq!(
        root.to_string_lossy(),
        workspace_root(&linked),
        "the context root is the invocation workspace root"
    );
    assert_eq!(
        ctx.jj().root(),
        root,
        "the runner stays anchored on the invocation workspace root"
    );

    let state = ctx
        .state()
        .expect("the state directory is readable from a linked workspace");
    assert_eq!(
        canon(state.root()),
        canon(Path::new(&workspace_root(&main))),
        "the store is rooted at the trunk, the main workspace"
    );
    assert_ne!(
        canon(state.root()),
        canon(root),
        "the store root is the trunk, not the invocation workspace root"
    );
}

#[test]
fn outside_the_trunk_store_refuses_naming_the_workspace_it_ran_in() {
    let base = scratch("outside-trunk");
    let main = base.join("main");
    let linked = base.join("linked");
    jj_repo(&main);
    linked_workspace(&main, &linked);

    let ctx = resolve(&linked).expect("a linked workspace resolves");
    let invocation = ctx
        .root()
        .expect("a linked workspace resolves a root")
        .to_string_lossy()
        .into_owned();
    let state = ctx
        .state()
        .expect("the state directory is readable from a linked workspace");
    assert!(
        state.root().is_dir(),
        "the state directory is reachable from the linked workspace: {}",
        state.root().display()
    );
    let refusal = match ctx.store() {
        Ok(_) => panic!("store() must refuse outside the trunk working copy"),
        Err(refusal) => refusal,
    };
    let named = format!("{} {:?}", refusal.cause(), refusal.evidence());
    assert!(
        named.contains(&invocation),
        "the refusal names the workspace it ran in ({invocation}): {named}"
    );
}

#[test]
fn a_workspace_whose_entry_cannot_be_resolved_fails_resolution() {
    let dir = scratch("broken-entry");
    std::fs::create_dir_all(dir.join(".jj")).expect("the .jj directory is creatable");
    let ghost = dir.join("ghost").join(".jj").join("repo");
    std::fs::write(dir.join(".jj").join("repo"), ghost.to_str().unwrap())
        .expect("the pointer file is writable");
    match resolve(&dir) {
        Ok(_) => panic!(
            "a broken repository entry must fail resolution, not fall back to {}",
            dir.display()
        ),
        Err(error) => assert!(
            !format!("{error:?}").trim().is_empty(),
            "the failure carries its cause"
        ),
    }
}

/// The trunk-state-root failure the context carries is the crate-root module's type.
///
/// One module map belongs to the crate root: the context reaches the trunk resolver as
/// `crate::state_root`, so a state-root failure it hands back is that module's
/// `StateRootError`. A context that keeps a private `#[path]` copy still compiles but
/// produces a different type, so a caller matching on the crate root's type would not see
/// the failure.
#[test]
fn the_state_root_failure_is_the_crate_root_type() {
    let dir = scratch("crate-root-state-root");
    std::fs::create_dir_all(dir.join(".jj")).expect("the .jj directory is creatable");
    let ghost = dir.join("ghost").join(".jj").join("repo");
    std::fs::write(dir.join(".jj").join("repo"), ghost.to_str().unwrap())
        .expect("the pointer file is writable");

    let error = match resolve(&dir) {
        Ok(_) => panic!(
            "a broken repository entry must fail resolution, not fall back to {}",
            dir.display()
        ),
        Err(error) => error,
    };
    let carried = match &error {
        context::ContextError::StateRoot(cause) => cause,
        other => panic!("a broken repository entry is a state-root failure, got {other:?}"),
    };
    assert_eq!(
        std::any::Any::type_id(carried),
        std::any::TypeId::of::<state_root::StateRootError>(),
        "ContextError::StateRoot must carry the crate-root state_root::StateRootError, the one \
         module map's type, not a copy declared inside the context"
    );
}

#[test]
fn a_directory_without_a_workspace_resolves_root_less_and_the_accessors_refuse() {
    let dir = scratch("no-repo");
    let ctx = resolve(&dir).expect("a directory without a workspace still resolves");
    assert!(ctx.root().is_err(), "a root-less context has no root");
    assert!(ctx.store().is_err(), "a root-less context has no store");
    assert!(
        ctx.state().is_err(),
        "a root-less context has no store to lend"
    );
    let refusal = ctx.root().unwrap_err();
    assert!(
        refusal.cause().contains("workspace"),
        "the refusal names the absence: {refusal:?}"
    );
}
