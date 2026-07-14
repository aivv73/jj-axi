# jj-axi

A machine-first companion to [Jujutsu](https://jj-vcs.github.io/jj/) for non-trivial history editing.

Use `jj` for everyday work. Use jj-axi when an agent needs to route hunks, partition changes, reorder history, recover operations, or publish deterministically. jj-axi uses `jj-lib` directly while preserving ordinary Jujutsu repository compatibility.

> **Status:** Experimental · **Language:** Rust · **License:** MIT
>
> Listed in the [AXI community catalog](https://axi.md/).

## Why

Jujutsu already provides a strong model and an effective CLI for routine agent work: status, log, new changes, descriptions, and many other operations need no replacement. The leverage gap appears when history editing requires an interactive editor, manual interpretation of patches, or several dependent mutations.

jj-axi targets that narrow interface:

- one command per reasoning question;
- stable machine-first schemas;
- no prompts or editors;
- exact, fail-loud selectors;
- atomic workflow-level mutations;
- structured empty, conflict, partial-result, and recovery states;
- raw repository compatibility—no private jj-axi metadata model.

A compact fallback instruction is usually enough to introduce the tool:

> Use `jj` for ordinary repository inspection and simple change creation. Before interactive or multi-step history editing—such as full-content squash, patch splitting, moving hunks between changes, or rebasing a stack—run `jj-axi` and follow its instructions.

## Example

```bash
# No arguments prints a short guide explaining when to switch from raw jj.
jj-axi

# Repository inspection remains explicit when structured state is useful.
jj-axi inspect

# Discover exact post-image hunks from one immutable snapshot.
jj-axi diff <change> --hunks

# Route one hunk into a new change without an editor.
jj-axi split <change> \
  --source-commit-id '<full snapshot commit id>' \
  --hunks 'src/lib.rs:12-18' \
  --into 'extract parser'

# Undo the latest user-visible mutation while preserving newer work.
jj-axi undo
```

Normal operational output is TOON:

```yaml
schema_version: 1
kind: inspect
data:
  current_change:
    change_id: qxkokwlpypzqokvnrrnlxtqnrrpzoorl
    description: ""
    status:
      conflicted: false
  diff_stat:
    changed_files: 2
    added_lines: 14
    removed_lines: 3
    skipped_files: 0
  conflict_count: 0
  divergence_count: 0
```

## Atomic multi-way partitioning

A mixed change can be decomposed into several ordered changes from one guarded snapshot. This avoids repeated `diff → split → diff → split` loops and routes the remainder explicitly.

First obtain canonical hunks and the full snapshot commit ID:

```bash
jj-axi diff <change> --hunks
```

Create a JSON manifest:

```json
{
  "schema_version": 1,
  "source_commit_id": "<full commit id>",
  "parts": [
    {
      "description": "refactor validation helpers",
      "hunks": [
        {"path": "src/lead.rs", "lines": "21-27"}
      ]
    },
    {
      "description": "tune lead scoring",
      "hunks": [
        {"path": "src/lead.rs", "lines": "38-43"}
      ]
    }
  ],
  "remainder": {"destination": "working_copy"}
}
```

Preview and apply it through stdin so creating the plan cannot stale the guarded source:

```bash
cat partition.json | jj-axi partition <change> --spec-file - --dry-run --details
cat partition.json | jj-axi partition <change> --spec-file -
```

Remainder policies:

- `remaining_change` — create a separate remainder change;
- `working_copy` — route unfinished content into the invoking workspace change;
- `require_empty` — reject the plan unless every source hunk is assigned.

Partition applies all parts, descendant rewrites, bookmarks, and workspace updates as one operation and one undo boundary. Rewrite conflicts are successful structured state rather than ambiguous command failure.

## Command surface

### Repository inspection

- `inspect` — current change, diff statistics, conflicts, and divergence;
- `log` — bounded structured history with selectable fields;
- `show` — one change and its patch;
- `diff [change] [--hunks]` — bounded patch and optional canonical hunk inventory;
- `operations` — classified operation history.

### Change construction and history editing

Routine creation and description can stay on raw `jj`; compatibility and composite commands remain available:

- `new`, `describe`, `checkpoint`;
- `split`, `partition`, `move`, `absorb`;
- `reorder`, `squash`, `abandon`;
- `undo [--to <operation-id>]`.

### Bookmarks and publication

- `bookmark list`, `bookmark set`, `bookmark push`;
- `finish` — readiness validation with optional exact bookmark publication;
- `pr status` — GitHub pull-request readiness through non-interactive `gh api`.

### Agent integration

- no arguments — print the routing skill body without YAML frontmatter;
- `skill` — print the complete installable routing skill;
- `skill --full` — print the detailed agent reference;
- `skill --output <path> [--force]` — install the routing skill atomically with conflict protection;
- `setup skill` — compatibility alias for protected installation.

Run `jj-axi --help` or `jj-axi <command> --help` for the installed command contract.

## Installation

Prerequisites:

- Rust 1.89 or newer when building from source;
- Jujutsu 0.43.0 available as `jj` on `PATH` for working-copy synchronization;
- `gh` only when using `pr status`.

Prebuilt archives for Linux x86-64 and macOS x86-64/Apple Silicon are published on [GitHub Releases](https://github.com/aivv73/jj-axi/releases). Each release includes a `SHA256SUMS` file. Verify the selected archive before extracting it (`sha256sum -c SHA256SUMS` on Linux or `shasum -a 256 -c SHA256SUMS` on macOS), then place `jj-axi` somewhere on `PATH`.

Build from source:

```bash
git clone https://github.com/aivv73/jj-axi.git
cd jj-axi
cargo build --release --locked
./target/release/jj-axi --version
```

## Agent instructions

jj-axi uses one routing text for automatic skill discovery and manual bootstrap, avoiding two instruction files that can drift:

| Invocation | Purpose |
| --- | --- |
| `jj-axi` | Print the routing body without YAML frontmatter |
| `jj-axi skill` | Print the same routing body with installable skill frontmatter |
| `jj-axi skill --full` | Detailed reference for secondary commands and edge cases |
| `jj-axi <command> --help` | Version-matched workflow, examples, and safety contract |

Install the compact routing skill with the Vercel Skills CLI:

```bash
npx skills add aivv73/jj-axi --skill jj-axi
```

Alternatively, install the embedded skill atomically. Existing differing content is protected unless `--force` is supplied:

```bash
mkdir -p .agents/skills/jj-axi
jj-axi skill --output .agents/skills/jj-axi/SKILL.md
```

For shell composition, `jj-axi skill` prints the same exact bytes to stdout. Use `jj-axi skill --full` only when the detailed [`agent reference`](./docs/agent-reference.md) is needed. The older `jj-axi setup skill --output ...` spelling remains a compatibility alias. Installing the skill does not install the jj-axi binary itself.

## Compatibility and safety

The v0.2.0 compatibility contract is intentionally narrow:

| Component | Verified version or platform |
| --- | --- |
| Embedded `jj-lib` and `jj-cli` crates | exactly 0.43.0 |
| Installed `jj` executable | exactly 0.43.0, available on `PATH` |
| Linux | Ubuntu 24.04, x86-64; release binary targets static musl |
| macOS | macOS 15 on x86-64 and Apple Silicon |

Other Jujutsu versions, operating systems, and architectures may work but are not part of the v0.2.0 compatibility contract. The installed `jj` version should match the embedded libraries to avoid repository-format or working-copy synchronization differences.

- jj-axi operates on standard Jujutsu repositories through `jj-lib`.
- History selectors use exact post-image hunk boundaries; stale or partial ranges fail with bounded canonical recovery candidates.
- Hunk inventory and selection read at most 1 MiB per file and 8 MiB in aggregate; oversized paths are reported or rejected with `materialization_limit`.
- Read commands do not fetch remotes.
- Publication uses explicit bookmarks and structured partial results.
- GitHub authentication, SSO, and enterprise routing are delegated to `gh`.
- Bare undo skips synchronization-only and foundation operations.

The architecture and trade-offs are documented in [`docs/adr/`](./docs/adr/). Domain terminology lives in [`CONTEXT.md`](./CONTEXT.md).

## Preliminary benchmark evidence

A Codex `gpt-5.6-sol` low-effort, `k=3` calibration across five version-control tasks produced:

| Arm | Correct | Mean wall time | Mean task VC commands |
| --- | ---: | ---: | ---: |
| plain Git | 15/15 | 50.1s | 20.1 |
| GitButler + skill | 14/15 | 71.5s | 11.0 |
| raw Jujutsu + external skill | 14/15 | 98.8s | 15.3 |
| jj-axi + then-current canonical skill | **15/15** | **46.3s** | **9.9** |

In the calibrated split task, jj-axi completed 3/3 runs with one mutation and 9.7 task VC commands on average, versus Git's 28.0 commands. These are small-sample pilot results, not a general ranking or statistical proof. They predate the hybrid companion positioning and should not be read as evidence for that routing strategy; a dedicated raw-jj-plus-jj-axi benchmark is still needed. Correctness remains the gate, and benchmark work does not define product semantics.

The harness and task methodology are maintained in the [`aivv73/version-control-bench`](https://github.com/aivv73/version-control-bench) fork.

## Design lineage

jj-axi is informed by the [AXI](https://github.com/kunchenguid/axi) principles, with deliberate product-specific adaptations. It does **not** claim strict AXI conformance. See the [AXI applicability audit](./docs/axi-applicability.md).

## Project documentation

- [Product requirements](./PRD.md)
- [Domain glossary](./CONTEXT.md)
- [AXI applicability audit](./docs/axi-applicability.md)
- [Architecture decisions](./docs/adr/)
- [Compact routing skill](./skills/jj-axi/SKILL.md)
- [Detailed agent reference](./docs/agent-reference.md)
