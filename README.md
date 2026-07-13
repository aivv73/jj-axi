# jj-axi

An agent-native command-line interface for [Jujutsu](https://jj-vcs.github.io/jj/).

jj-axi turns common version-control reasoning tasks into deterministic, non-interactive commands with compact TOON responses. It uses `jj-lib` directly while preserving ordinary Jujutsu repository compatibility.

> **Status:** Experimental · **Language:** Rust · **License:** MIT

## Why

Jujutsu already provides a strong model for agent-driven version control: stable change identities, rewrite-first history, first-class conflicts, and an operation log. Its standard CLI is intentionally designed for humans, however, and complex agent workflows can still require repeated inspection, parsing, and interactive editing.

jj-axi optimizes that interface boundary:

- one command per reasoning question;
- stable machine-first schemas;
- no prompts or editors;
- exact, fail-loud selectors;
- atomic workflow-level mutations;
- structured empty, conflict, partial-result, and recovery states;
- raw repository compatibility—no private jj-axi metadata model.

## Example

```bash
# No arguments opens the live repository home view.
jj-axi

# Equivalent explicit inspection.
jj-axi inspect

# Discover exact post-image hunks from one immutable snapshot.
jj-axi diff <change> --hunks

# Route one hunk into a new change without an editor.
jj-axi split <change> \
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
    change_id: ...
    description: ...
  diff_stat:
    changed_files: 2
    added_lines: 14
    removed_lines: 3
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
cat partition.json | jj-axi partition <change> --spec-file - --dry-run
cat partition.json | jj-axi partition <change> --spec-file -
```

Remainder policies:

- `remaining_change` — create a separate remainder change;
- `working_copy` — route unfinished content into the invoking workspace change;
- `require_empty` — reject the plan unless every source change is assigned.

Partition applies all parts, descendant rewrites, bookmarks, and workspace updates as one operation and one undo boundary. Rewrite conflicts are successful structured state rather than ambiguous command failure.

## Command surface

### Repository inspection

- `inspect` — current change, diff statistics, conflicts, and divergence;
- `log` — bounded structured history with selectable fields;
- `show` — one change and its patch;
- `diff [change] [--hunks]` — bounded patch and optional canonical hunk inventory;
- `operations` — classified operation history.

### Change construction and history editing

- `new`, `describe`, `checkpoint`;
- `split`, `partition`, `move`, `absorb`;
- `reorder`, `squash`, `abandon`;
- `undo [--to <operation-id>]`.

### Bookmarks and publication

- `bookmark list`, `bookmark set`, `bookmark push`;
- `finish` — readiness validation with optional exact bookmark publication;
- `pr status` — GitHub pull-request readiness through non-interactive `gh api`.

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

## Agent skill

Install the canonical skill with the Vercel Skills CLI:

```bash
npx skills add aivv73/jj-axi --skill jj-axi
```

Alternatively, an installed jj-axi binary can write the same skill without Node.js:

```bash
jj-axi setup skill --output .agents/skills/jj-axi/SKILL.md
```

The generated bytes are embedded from [`skills/jj-axi/SKILL.md`](./skills/jj-axi/SKILL.md). Existing differing files are protected unless `--force` is supplied. Installing the skill does not install the jj-axi binary itself.

## Compatibility and safety

The v0.1.0 compatibility contract is intentionally narrow:

| Component | Verified version or platform |
| --- | --- |
| Embedded `jj-lib` and `jj-cli` crates | exactly 0.43.0 |
| Installed `jj` executable | exactly 0.43.0, available on `PATH` |
| Linux | Ubuntu 24.04, x86-64; release binary targets static musl |
| macOS | macOS 15 on x86-64 and Apple Silicon |

Other Jujutsu versions, operating systems, and architectures may work but are not part of the v0.1.0 compatibility contract. The installed `jj` version should match the embedded libraries to avoid repository-format or working-copy synchronization differences.

- jj-axi operates on standard Jujutsu repositories through `jj-lib`.
- History selectors use exact post-image hunk boundaries; stale or partial ranges fail with bounded canonical recovery candidates.
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
| jj-axi + canonical skill | **15/15** | **46.3s** | **9.9** |

In the calibrated split task, jj-axi completed 3/3 runs with one mutation and 9.7 task VC commands on average, versus Git's 28.0 commands. These are small-sample pilot results, not a general ranking or statistical proof. Correctness remains the gate, and benchmark work does not define product semantics.

The harness and task methodology are maintained in the [`aivv73/version-control-bench`](https://github.com/aivv73/version-control-bench) fork.

## Design lineage

jj-axi is informed by the [AXI](https://github.com/kunchenguid/axi) principles, with deliberate product-specific adaptations. It does **not** claim strict AXI conformance. See the [AXI applicability audit](./docs/axi-applicability.md).

## Project documentation

- [Product requirements](./PRD.md)
- [Domain glossary](./CONTEXT.md)
- [AXI applicability audit](./docs/axi-applicability.md)
- [Architecture decisions](./docs/adr/)
- [Canonical agent skill](./skills/jj-axi/SKILL.md)
