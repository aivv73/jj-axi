# jj-axi

**Status:** Draft
**Language:** Rust
**License:** MIT / Apache-2.0

## Vision

Modern version control systems were designed for humans typing commands.
Modern software development is increasingly performed by autonomous agents operating through shell execution.

The bottleneck is no longer the version control model — it is the command-line interface.

jj-axi is an agent-native CLI for Jujutsu. It exposes jj's change-based model through an interface optimized for autonomous reasoning rather than interactive terminal use.

## Problem

Version control agents currently interact with VCSes by composing long sequences of shell commands. A typical workflow may require:

```
status
log
show
diff
split
describe
status
```

before an agent has enough information to continue. Each shell invocation has cost: latency, tokens, reasoning, failure surface. [vcbench](https://vcbench.dev/) demonstrates these costs dominate overall agent performance — jj's split-commit task alone averages ~39.6 commands per run, with the only sub-100% success rate in the benchmark.

GitButler currently achieves the strongest benchmark results by exposing higher-level operations over Git. Jujutsu already has a more agent-friendly internal model than Git:

- immutable change identities
- operation log
- first-class conflicts
- `absorb`
- rewrite-first workflow

However, those capabilities remain exposed through a human-oriented CLI.

## Thesis

The VCS model is not the bottleneck. The interface is.

jj-axi applies [AXI](https://github.com/kunchenguid/axi) principles to expose jj as an agent-native interface.

## Goals

Build a CLI that minimizes shell round-trips, output tokens, parsing complexity, and interactive workflows — while preserving full compatibility with standard jj repositories. Success is measured by objective improvements on vcbench-style tasks against raw jj.

## Non-goals

Not another VCS. Not another Git. Not a GUI. Not a GitButler clone. Not an alternative merge algorithm. Not another agent framework.

## Design principles

1. **Intent over mechanism.** Commands represent goals (`finish`, `publish`) rather than implementation details.
2. **One command, one reasoning question.** Instead of `status` + `log` + `diff`, agents ask `inspect`.
3. **Machine-first output.** Default output is TOON. Human-readable output is optional.
4. **Stable schemas.** Responses are contracts — never prose, never shifting field names.
5. **Combined operations.** Common multi-step workflows become single commands (e.g. moving hunks between changes as one call, not a `split`/`edit`/`restore`/`describe` chain).
6. **Deterministic UX.** No prompts, no interactive editors, no ambiguous errors. Every command is reproducible.

## Architecture

```
Claude Code / Codex / OpenCode / Slopflow
                │
                ▼
             jj-axi
                │
              jj-lib
                │
           Repository
```

jj-axi owns schemas, output, and workflow composition. jj owns storage, merge, history, and conflicts.

## Why this project exists

Git solved distributed version control. Jujutsu improved the version control model. GitButler demonstrated that interface design dramatically affects agent performance.

jj-axi asks the next question: what should a version control CLI look like if autonomous agents — not humans — are its primary users?

See [PRD.md](./PRD.md) for the working specification, open design questions, and licensing notes.
