# PRD: jj-axi

**Status:** Draft
**Author:** Данила
**Language:** Rust
**License:** MIT

See [README.md](./README.md) for vision, thesis, and design principles. This document is the working spec: command contracts, open design questions, licensing position, and milestones.

jj-axi is a companion for non-trivial history editing, not a replacement for routine raw Jujutsu workflows. Simple inspection, change creation, and description should remain on `jj` when one non-interactive command is sufficient.

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
| `partition <change> --spec-file <path\|-> [--dry-run] [--details]` | repeated `jj split -i` and remainder routing | Atomically decomposes one guarded source snapshot into ordered named parts with an explicit remainder disposition. See ADR 0006. |
| `move --from <change> --to <change> --hunks "..."` | manual multi-step today | Combined operation for routing hunks to an *existing* change (as opposed to creating a new one via `split`). |
| `absorb [--dry-run]` | `jj absorb` | Structured report of what moved where; `--dry-run` previews without mutating. |

### Stack editing

| Command | Wraps | Notes |
|---|---|---|
| `reorder --sequence "<id1>,<id2>,..."` | `jj rebase` chain | Declarative target order instead of manual rebase steps. |
| `squash <change> [--into <change>] [--message <message>]` | `jj squash` | Full-content, editor-free squash. Defaults to the sole parent; two non-empty descriptions require an explicit message. Conflicts are successful structured state. See ADR 0005. |
| `abandon <change>` | `jj abandon` | Idempotent when operation history proves prior abandonment; reports rewritten descendants, local bookmarks, conflicts, and resulting current change. See ADR 0005. |
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

### Agent discovery and setup

| Command | Wraps | Notes |
|---|---|---|
| `jj-axi` | — | Prints the short bootstrap guide that routes routine work to raw `jj` and non-trivial history editing to jj-axi. Does not open a repository. |
| `skill [--full \| --output <path> [--force]]` | — | By default, prints the compact command router and global safety rules. `--full` prints the detailed agent reference. `--output` atomically materializes the routing skill; it is idempotent for identical bytes and protective of differing or non-regular destinations. |
| `setup skill --output <path> [--force]` | — | Compatibility alias for protected skill installation. |

## 2. Open design questions

1. **Hunk addressing for split/move — resolved.** Post-image, exact-boundary, fail-loud semantics are specified by [ADR 0001](./docs/adr/0001-hunk-addressing.md).
2. **jj-lib API stability.** jj-lib is not a stable, versioned public API in the way the `jj` CLI's UX is. Need to pin target jj version(s) and decide how much abstraction insulates jj-axi from upstream breakage.
3. **Scope of `validate`.** What invariants does this actually check that `inspect` and `undo` don't already surface? If the answer is "nothing new," cut the command rather than ship a placeholder.
4. **Resolved `finish` composite boundary.** `--message` is optional; when omitted, the stored description must be non-empty. Without `--bookmark`, finish applies the optional message and returns readiness-only success without private finished metadata. With `--bookmark`, finish creates or fast-forwards only that exact name and pushes only that name. The remote is `git.push`, otherwise the sole configured remote, otherwise `origin`; no name or remote is inferred from tracking. Description and bookmark updates are one local operation retained when push fails, which returns a structured partial result.
5. **Resolved progressive disclosure.** The compact routing artifact lives at `skills/jj-axi/SKILL.md`, where GitHub-based skill tooling can discover it directly. It selects a command and preserves only cross-command safety rules. Exact workflows, examples, and command-specific safety semantics live in version-matched `<command> --help`. The binary embeds the routing skill: `skill` prints it and `skill --output` installs it, while `setup skill` remains an alias. `skill --full` prints the opt-in detailed reference from `docs/agent-reference.md`. Bare invocation prints the separate short bootstrap guide. jj-axi does not invoke npm, npx, or agent-specific installers.
6. **Agent-native fetch.** `bookmark list` deliberately reads cached local tracking state and never contacts remotes. A future top-level `fetch [--remote]` command should refresh repository-wide collaboration state, but its network, authentication, multi-remote, and partial-result contracts must be designed before implementation; it is not part of the bookmark slice.

## 3. Licensing position

jj-axi is written from scratch under the MIT License. It draws on:

- **AXI principles spec** (MIT, kunchenguid/axi) — design methodology, not code; freely applicable.
- **GitButler's documented AI-agent UX concepts** (docs.gitbutler.com/ai-agents) — ideas and workflow concepts only. No GitButler source code is used or derived from. GitButler's code is under FSL-1.1-MIT, whose Competing Use clause prohibits building a "same or substantially similar functionality" product *from their code*; jj-axi avoids this by not touching their codebase and not reproducing their documentation text verbatim.
- **gh-axi** (MIT) — reference implementation pattern for agent discovery and skill distribution; jj-axi adopts skill-led discovery but deliberately does not install session hooks (ADR 0004).
- **gitsheets-axi** (Apache-2.0) — idempotent-commit convention as a design reference, not a code dependency (different domain: git-as-data-store, not VCS history editing).
- **onevcat-jj skill** (no license found) — topic coverage was reviewed as product research only. jj-axi's skill text, examples, structure, and frontmatter are independently authored; no prose or other copyrightable content is copied or adapted.

No code or content dependency on any non-compete-licensed or unlicensed project.

## 4. Distribution

- Standalone Rust binary, installable via `cargo install` (later `cargo binstall` for prebuilt binaries).
- `skills/jj-axi/SKILL.md` is the compact routing skill and is distributed directly through the standard `skills` CLI (`npx skills add <owner>/jj-axi --skill jj-axi`). The native `skill` command prints or atomically materializes the same bytes without invoking JavaScript tooling.
- Command-specific workflows live in `<command> --help`, keeping instructions aligned with the installed binary.
- `docs/agent-reference.md` is the detailed opt-in reference printed by `skill --full`.
- `skills/jj-axi/BOOTSTRAP.md` is the substantially shorter routing guide printed by bare invocation.
- Agent discovery is skill- and bootstrap-led. jj-axi does not mutate agent configuration or install session hooks; see ADR 0004 and ADR 0007.

## 5. Success metrics

- Baseline: raw jj averages ~39.6 commands and 9/10 success on vcbench's split-commit task. A hybrid workflow using raw `jj` for routine work and jj-axi for non-trivial history editing must beat both numbers materially, not marginally.
- Maintain the [AXI applicability audit](./docs/axi-applicability.md), classifying each principle as applicable, adapted, or not applicable with product-specific rationale. jj-axi does not claim strict AXI conformance.
- Consider AXI catalog submission only if maintainers accept the documented adaptations; catalog inclusion is not a product-success requirement.

## 6. Milestones

1. **M1 — Read-only interface.** `inspect`, `log`, `show`, `diff`. Establishes TOON output, schema conventions, truncation contract.
2. **M2 — Mutations.** `new`, `describe`, `checkpoint`, `finish`. Establishes idempotency and structured-error conventions.
3. **M3 — History editing.** `split`, atomic multi-way `partition`, `move`, `absorb`, `reorder`. Hunk addressing is resolved by ADR 0001; partition identity, remainder, and transaction semantics are resolved by ADR 0006. This milestone addresses general editor-free history construction; benchmark results are supporting evidence, not its product definition.
4. **M4 — Integrations.** Independent vertical slices: `operations`/`undo`, `bookmark`, `pr status`, and `setup skill`. Session hooks are excluded by ADR 0004.
5. **M5 — Stack completion.** `squash` and `abandon`. Completes the general stack-editing command map before evaluation.
6. **M6 — Benchmark and discovery.** Evaluate the hybrid interface: raw `jj` for routine work and jj-axi for non-trivial history editing. Bare invocation provides a short routing guide, while `inspect` remains explicit. Run vcbench-style evaluation against raw jj and publish results. Catalog submission is optional and requires acceptance of documented AXI adaptations. Benchmarking is evaluation work, not product definition.

`validate`/`repair` are explicitly deferred pending open question #3 and are not part of any milestone above.

## 7. References

- vcbench.dev — VCS agent benchmark data
- AXI spec — github.com/kunchenguid/axi
- gh-axi — github.com/kunchenguid/gh-axi
- gitsheets-axi — github.com/JarvusInnovations/gitsheets
- docs.gitbutler.com/ai-agents/overview — UX concepts only, no code/text reuse
- GitButler LICENSE.md (FSL-1.1-MIT) — reason for clean-room approach
- vercel.com/docs/agent-resources/skills — skill distribution mechanism confirmation
