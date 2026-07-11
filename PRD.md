# PRD: jj-axi

**Status:** Draft
**Author:** Данила
**Language:** Rust
**License:** MIT / Apache-2.0

See [README.md](./README.md) for vision, thesis, and design principles. This document is the working spec: command contracts, open design questions, licensing position, and milestones.

## 1. Command mapping

Each command must have an explicit contract: which jj operation(s) it wraps, what it combines, and which AXI principle it primarily serves. No command ships without this row filled in.

### Read

| Command | Wraps | Notes |
|---|---|---|
| `inspect` | `jj status` + `jj log` + `jj diff` (working copy) | Single combined-operation answer to "what's the current state." Replaces the 3-call sequence agents currently need. Default schema: current change, diff stat, conflict/divergence counts. |
| `log [--limit] [--conflicted] [--fields]` | `jj log` | Minimal schema (`change_id`, `description`, `status`) unless `--fields` requests more. No ASCII graph by default. |
| `show <change>` | `jj show` | Full description + diff stat + truncated diff body, with `--full` escape hatch. |
| `diff [<change>]` | `jj diff` | Truncated by default, same truncation contract as `show`. |

### Change lifecycle

| Command | Wraps | Notes |
|---|---|---|
| `new [--message]` | `jj new` | |
| `describe <change> --message` | `jj describe` | Idempotent: same message twice = no-op, exit 0. |
| `checkpoint --message` | `jj commit`/`jj new` | Snapshot working copy into a described change and open a new one on top. Distinct from `finish`: does not touch bookmarks. |
| `finish <change> [--message <message>] [--bookmark <name>]` | optional `jj describe` + readiness validation + `jj bookmark set` + `jj git push --bookmark` (if `--bookmark`) | Without `--bookmark`, applies an optional message and performs readiness-only validation. With `--bookmark`, publishes only that exact name using the configured remote. |

### Hunk routing

| Command | Wraps | Notes |
|---|---|---|
| `split <change> --hunks "<file>:<lines>,..." --into "<desc>"` | `jj split -i` | Declarative hunk spec instead of interactive editor. **Open question #1 — see §2.** |
| `move --from <change> --to <change> --hunks "..."` | manual multi-step today | Combined operation for routing hunks to an *existing* change (as opposed to creating a new one via `split`). |
| `absorb [--dry-run]` | `jj absorb` | Structured report of what moved where; `--dry-run` previews without mutating. |

### Stack editing

| Command | Wraps | Notes |
|---|---|---|
| `reorder --sequence "<id1>,<id2>,..."` | `jj rebase` chain | Declarative target order instead of manual rebase steps. |
| `squash <change> [--into <change>]` | `jj squash` | |
| `abandon <change>` | `jj abandon` | Idempotent: already-abandoned = no-op, exit 0. |
| `operations [--limit]` | `jj op log` | Observationally read-only, bounded reverse-topological operation list with parent IDs, classification, and undo eligibility. Does not snapshot the working copy or reconcile divergent heads. |
| `undo [--to <op-id>]` | `jj op log` + `jj op restore` | Bare undo reverses the latest user-visible repository mutation while preserving newer synchronized working-copy content; explicit restore selects an exact reachable operation. Strictly local, repeated undo walks backward, divergent history requires an explicit target, and foundation operations cannot be removed. See ADR 0002. |

### Repository health

| Command | Wraps | Notes |
|---|---|---|
| `validate` | TBD — see §2, open question #3 | **Not yet scoped.** Placeholder for repo-invariant checks (e.g. divergent bookmarks, orphaned conflicts) not already surfaced by `inspect`. Must not duplicate `undo`/`op log`. |

`repair` from the earlier draft is cut until `validate` defines what there would be to repair. Do not implement either until scoped.

### Collaboration

| Command | Wraps | Notes |
|---|---|---|
| `bookmark list/set/push` | `jj bookmark` + `jj git push` | Bounded multi-remote collaboration view over local tracking state; explicit safe local movement and exact-name publication. Listing never fetches. |
| `pr status <number> [--repo <[host/]owner/name>]` | GitHub GraphQL API through authenticated `gh` | Explicit PR selection; normalized multi-host repository identity; aggregate check, review, and mergeability state; derived merge readiness and ordered blocking reasons. No prompts or raw GitHub schema output. |

### Setup

| Command | Wraps | Notes |
|---|---|---|
| `setup hooks [--agent claude-code\|codex\|opencode]` | — | Session integration per AXI principle 7. Idempotent install. |
| `setup skill --output <path> [--force]` | — | Atomically materializes the embedded canonical `skills/jj-axi/SKILL.md`; idempotent for identical bytes and protective of differing or non-regular destinations. |

## 2. Open design questions

1. **Hunk addressing for split/move — resolved.** Post-image, exact-boundary, fail-loud semantics are specified by [ADR 0001](./docs/adr/0001-hunk-addressing.md).
2. **jj-lib API stability.** jj-lib is not a stable, versioned public API in the way the `jj` CLI's UX is. Need to pin target jj version(s) and decide how much abstraction insulates jj-axi from upstream breakage.
3. **Scope of `validate`.** What invariants does this actually check that `inspect` and `undo` don't already surface? If the answer is "nothing new," cut the command rather than ship a placeholder.
4. **Resolved `finish` composite boundary.** `--message` is optional; when omitted, the stored description must be non-empty. Without `--bookmark`, finish applies the optional message and returns readiness-only success without private finished metadata. With `--bookmark`, finish creates or fast-forwards only that exact name and pushes only that name. The remote is `git.push`, otherwise the sole configured remote, otherwise `origin`; no name or remote is inferred from tracking. Description and bookmark updates are one local operation retained when push fails, which returns a structured partial result.
5. **Resolved skill distribution.** The canonical artifact lives at `skills/jj-axi/SKILL.md`, where GitHub-based skill tooling can discover it directly. The native binary embeds those exact bytes and `setup skill` copies them to an explicit destination; jj-axi does not invoke npm, npx, or agent-specific installers.
6. **Agent-native fetch.** `bookmark list` deliberately reads cached local tracking state and never contacts remotes. A future top-level `fetch [--remote]` command should refresh repository-wide collaboration state, but its network, authentication, multi-remote, and partial-result contracts must be designed before implementation; it is not part of the bookmark slice.

## 3. Licensing position

jj-axi is written from scratch under MIT/Apache-2.0. It draws on:

- **AXI principles spec** (MIT, kunchenguid/axi) — design methodology, not code; freely applicable.
- **GitButler's documented AI-agent UX concepts** (docs.gitbutler.com/ai-agents) — ideas and workflow concepts only. No GitButler source code is used or derived from. GitButler's code is under FSL-1.1-MIT, whose Competing Use clause prohibits building a "same or substantially similar functionality" product *from their code*; jj-axi avoids this by not touching their codebase and not reproducing their documentation text verbatim.
- **gh-axi** (MIT) — reference implementation pattern (session hook + skill dual distribution), reimplemented independently in Rust.
- **gitsheets-axi** (Apache-2.0) — idempotent-commit convention as a design reference, not a code dependency (different domain: git-as-data-store, not VCS history editing).
- **onevcat-jj skill** (no license found) — topic coverage was reviewed as product research only. jj-axi's skill text, examples, structure, and frontmatter are independently authored; no prose or other copyrightable content is copied or adapted.

No code or content dependency on any non-compete-licensed or unlicensed project.

## 4. Distribution

- Standalone Rust binary, installable via `cargo install` (later `cargo binstall` for prebuilt binaries).
- `skills/jj-axi/SKILL.md` is the single canonical skill document and is distributed directly through the standard `skills` CLI (`npx skills add <owner>/jj-axi --skill jj-axi`). The native `setup skill` command embeds and atomically materializes the same bytes without invoking JavaScript tooling.
- `setup hooks` installs SessionStart hooks for Claude Code, Codex, and OpenCode per AXI principle 7.

## 5. Success metrics

- Baseline: raw jj averages ~39.6 commands and 9/10 success on vcbench's split-commit task. jj-axi's `split`/`move` must beat both numbers materially, not marginally.
- Full conformance to all 10 AXI principles, self-audited before v0.1 (checklist against the spec, one row per principle).
- First Rust entry in the AXI catalog, submitted via kunchenguid/axi's contributor workflow once M3 is stable.

## 6. Milestones

1. **M1 — Read-only interface.** `inspect`, `log`, `show`, `diff`. Establishes TOON output, schema conventions, truncation contract.
2. **M2 — Mutations.** `new`, `describe`, `checkpoint`, `finish`. Establishes idempotency and structured-error conventions.
3. **M3 — History editing.** `split`, `move`, `absorb`, `reorder`. Blocked on open question #1 (ADR required first). This is the milestone that actually targets the vcbench gap.
4. **M4 — Integrations.** Independent vertical slices: `operations`/`undo`, `bookmark`, `pr status`, `setup skill`, then `setup hooks`.
5. **M5 — Benchmark.** Run vcbench-style evaluation against raw jj; publish results; submit to AXI catalog if numbers hold up.

`validate`/`repair` are explicitly deferred pending open question #3 and are not part of any milestone above.

## 7. References

- vcbench.dev — VCS agent benchmark data
- AXI spec — github.com/kunchenguid/axi
- gh-axi — github.com/kunchenguid/gh-axi
- gitsheets-axi — github.com/JarvusInnovations/gitsheets
- docs.gitbutler.com/ai-agents/overview — UX concepts only, no code/text reuse
- GitButler LICENSE.md (FSL-1.1-MIT) — reason for clean-room approach
- vercel.com/docs/agent-resources/skills — skill distribution mechanism confirmation
