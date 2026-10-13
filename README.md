# deccy-cli

Manage every decision that goes into a codebase.

`deccy-cli` is the command-line interface to [deccy](https://edawise.com/), a framework for responsible and careful agentic coding. You can use it yourself, but it works best when you use it with [deccy-agent](https://github.com/edawise/deccy-agent) and [deccy-viewer](https://github.com/edawise/deccy-viewer).

## Requirements

- [`jj`](https://jj-vcs.github.io/jj/) (jujutsu) on `PATH`
- `git` on `PATH`

## Install

```sh
brew install edawise/tap/deccy-cli
```

Or take a prebuilt binary for macOS (arm64/x86_64) or Linux (x86_64/arm64) from
the [releases page](https://github.com/edawise/deccy-cli/releases).

Or, with a Rust toolchain, install from source:

```sh
git clone https://github.com/edawise/deccy-cli
cargo install --path deccy-cli
```

Or install the published crate:

```sh
cargo install deccy-cli
```

## Quick start

```sh
cd my-project
deccy init .                              # initialise a deccy project

deccy new -m "a todo app" --on zzzzzzzz
# define a decision node with its label and dependencies

deccy record nodes/a1b2c3d4.json --to a1b2c3d4   
# make changes to the node's document, then record them into the node
deccy show a1b2c3d4                       # the node's document
deccy log                                 # view every node: <id> <kind> <summary>
deccy validate                            # report validity, resolution and coverage failures

deccy undo                                # step back over the last operation
deccy redo
```

`zzzzzzzz` is the root: an independent node (such as an invariant) depends on it. `deccy --help` lists the
verbs; `deccy <verb> --help` describes one.

## Verbs

| Verb | What it does |
| --- | --- |
| `init [path]` | prepare a repository for dec-graph use |
| `new -m <summary> --on <id>...` | create a node in a new change after the named dependencies; prints its id |
| `record <path>... --to <id>` | place the named working-copy edits into `<id>`'s change |
| `show <id>...` | print each node's stored document |
| `log` | list every node as `<id> <kind> <summary>` |
| `validate` | report validity, resolution and coverage failures as `<check> <id> <reason>` lines |
| `remove <id>` | abandon the node's change when nothing depends on it |
| `undo` / `redo` | undo reverts the last operation's entries; redo reverts the reverts undo recorded |

A verb refuses with one line on stderr and exit status 2; success is exit 0.

## The graph

A node is a small document at `nodes/<id>.json`, where `<id>` is the jj change
id — stable across rebases, unlike a commit id. A node is a `decision` (what was
decided, why) or an `evaluation` (review criteria, a test, another check). Its
content is either inline or a list of repo-relative paths to the files carrying
it.

Edges are what make the graph more than a list:

- a **decision** declares what it `depends_on`;
- an **evaluation** declares what it `evaluates` — and every dependency edge
  between decisions is covered by an evaluation that checks the dependent's use
  of its prerequisite.

That coverage rule is the point: a change that quietly leans on something no one
checked is a change caught by the graph, not by a reviewer's memory.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
