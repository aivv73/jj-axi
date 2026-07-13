# jj-axi agent reference

This is the detailed semantic reference. Run `jj-axi skill` for the compact command router and `jj-axi <command> --help` for the version-matched execution contract.

jj-axi is a machine-first companion to Jujutsu, not a replacement for its everyday CLI. Use raw `jj` when one ordinary, non-interactive command answers the question. Switch to jj-axi when history editing would require an editor, manual patch interpretation, or several dependent mutations.

## Choose the narrowest interface

Use raw `jj` for routine work:

```bash
jj status
jj log -n 20
jj show <change>
jj diff -r <change>
jj new -m "implement token refresh"
jj describe -r <change> -m "implement token refresh"
```

Before full-content squash, patch splitting, moving hunks, multi-way partitioning, stack reordering, operation recovery, or validated publication, use jj-axi. Running `jj-axi` with no arguments prints a short routing guide; `jj-axi --help` reports the installed command surface.

## Preserve the Jujutsu model

Check for `.jj/` before choosing version-control commands. In a colocated repository, `.git/` may also exist; use Jujutsu interfaces for local history instead of Git commands.

Jujutsu models work as **changes**:

- The working copy is a mutable change; there is no staging step.
- Change IDs remain stable when commits are rewritten.
- Local bookmarks are named references, mainly useful for collaboration and publication.
- Operations record repository-level mutations and make recovery possible.

Do not translate a Git workflow mechanically. There is usually no need for `git add`, stash management, detached-HEAD handling, or a local branch per task.

## Request structured inspection only when it pays off

Use `jj-axi inspect` when one structured snapshot should combine the current change, working-copy diff summary, and conflict/divergence counts. Focused jj-axi reads are useful when bounded machine output or canonical selectors matter:

```bash
jj-axi inspect
jj-axi log --limit 20 --conflicted
jj-axi show <change>
jj-axi diff <change> --full
```

Diff bodies are bounded by default. Request `--full` only when the complete patch is necessary. For ordinary status, log, show, or diff questions, prefer raw `jj`.

## Validate and publish deterministically

Use raw `jj bookmark set` for straightforward local bookmark placement. Use jj-axi when publication needs readiness validation, exact-name selection, and structured partial-failure state:

```bash
jj-axi finish <change>
jj-axi finish <change> --bookmark feature-name
```

`finish <change>` validates local readiness. Adding `--bookmark` also publishes that exact bookmark. `finish` never invents a bookmark. A failed push may retain the local desired state and return a structured partial result; inspect that result before retrying.

## Route hunks without an editor

Discover canonical post-image selectors before routing nontrivial hunks:

```bash
jj-axi diff <change> --hunks
```

Pass the returned `path` and `lines` values unchanged:

```bash
jj-axi split <change> --hunks "src/lib.rs:12-18" --into "extract parser"
jj-axi move --from <change> --to <change> --hunks "src/lib.rs:12-18"
```

Use `N-0` for a deletion-only boundary. Select multiple hunks with comma-separated entries. Selectors never snap to nearby content: stale or partial ranges fail and return bounded retry candidates. `skipped_paths` identifies content that declarative routing cannot safely address.

Partition one guarded snapshot into several ordered changes when repeated binary splits would force re-inventory of each remainder. Copy the full `snapshot.commit_id` and canonical hunks from `diff --hunks` into a strict JSON manifest:

```json
{
  "schema_version": 1,
  "source_commit_id": "<full commit id>",
  "parts": [
    {
      "description": "refactor parser",
      "hunks": [{"path": "src/lib.rs", "lines": "12-18"}]
    },
    {
      "description": "test parser",
      "hunks": [{"path": "tests/lib.rs", "lines": "8-14"}]
    }
  ],
  "remainder": {"destination": "working_copy"}
}
```

Keep the manifest outside tracked repository content or pipe it through stdin so writing the plan does not stale its source guard:

```bash
cat partition.json | jj-axi partition <change> --spec-file - --dry-run
cat partition.json | jj-axi partition <change> --spec-file -
```

Choose `remaining_change` to preserve a separate remainder change, `working_copy` to route unfinished content into the invoking descendant working-copy change, or `require_empty` to reject any unassigned or unsupported content. For multi-part partition, inspect a `--details` dry-run unless every assignment is mechanically obvious. The compact apply receipt reports the manifest SHA-256, counts, realized identities, conflict status, and bounded affected-state summaries. After success, one `inspect` plus targeted `log` or `show` is normally sufficient; do not repeat binary split inventory loops.

Preview or apply automatic absorption:

```bash
jj-axi absorb --dry-run
jj-axi absorb
```

Reorder a linear stack by listing every selected change from oldest to newest:

```bash
jj-axi reorder --sequence "<oldest>,<middle>,<newest>"
```

Move all content from one change into another without an editor:

```bash
jj-axi squash <from> --into <to>
```

Use `move` rather than `squash` for selective hunk routing.

Remove one visible change and reparent its descendants:

```bash
jj-axi abandon <change>
```

Abandon does not reverse pushes or other external effects. History editing rejects unsupported shapes and content rather than guessing.

## Inspect and undo operations

Read the bounded operation graph without mutating it:

```bash
jj-axi operations --limit 20
```

Undo the latest user-visible repository mutation while retaining newer synchronized working-copy content:

```bash
jj-axi undo
```

Restore an explicit reachable operation:

```bash
jj-axi undo --to <operation-id>
```

Bare undo skips synchronization-only and prior undo operations. Divergent operation history requires an explicit target. Undo is local and does not reverse pushes or other external effects.

## Manage local bookmarks and publication

Inspect cached local and tracked-remote collaboration state:

```bash
jj-axi bookmark list
jj-axi bookmark list --name feature-name
jj-axi bookmark list --limit 50 --after previous-name
```

Listing never fetches. Ahead/behind values reflect locally recorded tracking state.

Create or safely move a local bookmark:

```bash
jj-axi bookmark set feature-name --to <change>
```

Backward or sideways movement requires explicit intent:

```bash
jj-axi bookmark set feature-name --to <change> --allow-backwards
```

Publish only one existing local bookmark:

```bash
jj-axi bookmark push feature-name
jj-axi bookmark push feature-name --remote origin
```

Local placement does not imply publication. Push applies readiness checks and lease protection and is retry-safe when the remote already matches.

## Inspect GitHub pull-request readiness

Query one explicit pull request:

```bash
jj-axi pr status 42 --repo owner/repository
jj-axi pr status 42 --repo github.example.com/owner/repository
```

When configured remotes identify one GitHub repository, `--repo` may be omitted. The response normalizes checks, reviews, mergeability, merge readiness, and all blocking reasons. This command requires an authenticated `gh` executable.

## Continue using raw Jujutsu

Raw `jj` remains the default for routine single-step work and for capabilities that jj-axi does not expose:

```bash
jj git init --colocate       # initialize an existing Git checkout
jj git fetch                 # refresh remote-tracking state
jj workspace add <path>      # create another workspace
jj resolve                   # resolve conflicted files
jj status                    # inspect ordinary working-copy state
jj new -m "description"      # create a straightforward child change
jj describe -m "description" # describe the current change
```

Raw commands may produce human-oriented output or open interactive tools. Supply non-interactive arguments where available. Do not substitute raw interactive `jj split` when declarative jj-axi hunk routing can express the task.

## Safe operating rules

- Inspect with raw `jj` or structured jj-axi output before mutating when the current change or target is uncertain.
- Use explicit change IDs, bookmark names, operation IDs, remotes, and PR numbers.
- Treat structured partial results as changed state, not ordinary failures.
- Do not parse human terminal prose when a jj-axi schema exists.
- Do not create local bookmarks merely to separate unfinished tasks; changes already provide identity.
- Do not assume undo reverses remote effects.
