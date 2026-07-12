---
name: jj-axi
description: Use jj-axi for deterministic version-control work in Jujutsu repositories. Activate when .jj/ exists, project policy names Jujutsu or jj-axi, or a version-control request concerns a Jujutsu-managed repository.
---

# Agent workflows with jj-axi

Use jj-axi as the primary command interface in a Jujutsu repository. It provides bounded TOON responses, structured errors, declarative history editing, and retry-safe mutations while preserving an ordinary Jujutsu repository.

## Detect the repository model

Check for `.jj/` before choosing version-control commands. In a colocated repository, `.git/` may also exist; use Jujutsu or jj-axi for local history instead of Git commands.

Jujutsu models work as **changes**:

- The working copy is a mutable change; there is no staging step.
- Change IDs remain stable when commits are rewritten.
- Local bookmarks are named references, mainly useful for collaboration and publication.
- Operations record repository-level mutations and make recovery possible.

Do not translate a Git workflow mechanically. There is usually no need for `git add`, stash management, detached-HEAD handling, or a local branch per task.

## Start by inspecting

Ask one state question first:

```bash
jj-axi inspect
```

It combines the current change, working-copy diff summary, and conflict/divergence counts. Use focused reads when needed:

```bash
jj-axi log --limit 20
jj-axi log --conflicted
jj-axi show <change>
jj-axi diff
jj-axi diff <change> --full
```

Diff bodies are bounded by default. Request `--full` only when the complete patch is necessary.

## Manage the change lifecycle

Start a child change:

```bash
jj-axi new --message "implement token refresh"
```

Describe an existing change explicitly:

```bash
jj-axi describe <change> --message "implement token refresh"
```

Snapshot the current work into a described change and open a fresh child:

```bash
jj-axi checkpoint --message "complete token refresh"
```

Create or update a local bookmark without contacting a remote:

```bash
jj-axi bookmark set feature-name --to <change>
```

Use `bookmark set` when a task asks for a local branch, bookmark, or Git-visible ref but does not ask to publish or push. Do not use `finish --bookmark` for local-only completion.

Validate readiness without publication:

```bash
jj-axi finish <change>
```

Publish one exact bookmark after readiness validation:

```bash
jj-axi finish <change> --bookmark feature-name
```

`finish` never invents a bookmark. A failed push may retain the local desired state and return a structured partial result; inspect that result before retrying.

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

Choose `remaining_change` to preserve a separate remainder change, `working_copy` to route unfinished content into the invoking descendant working-copy change, or `require_empty` to reject any unassigned or unsupported content. Use `--details` only when the receipt must echo canonical part and remainder hunks. Otherwise rely on the manifest SHA-256, counts, realized identities, conflict status, and bounded affected-state summaries. After success, one `inspect` plus targeted `log` or `show` is normally sufficient; do not repeat binary split inventory loops.

Preview or apply automatic absorption:

```bash
jj-axi absorb --dry-run
jj-axi absorb
```

Reorder a linear stack by listing every selected change from oldest to newest:

```bash
jj-axi reorder --sequence "<oldest>,<middle>,<newest>"
```

History editing rejects unsupported shapes and content rather than guessing.

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

## Raw Jujutsu fallbacks

Use raw `jj` only when the installed jj-axi interface does not expose the required capability. Confirm availability with:

```bash
jj-axi --help
```

Common fallbacks include:

```bash
jj git init --colocate       # initialize an existing Git checkout
jj git fetch                 # refresh remote-tracking state
jj workspace add <path>      # create another workspace
jj resolve                   # resolve conflicted files
jj status                    # diagnose details absent from a structured error
```

Raw commands may produce human-oriented output or open interactive tools. Supply non-interactive arguments where available. Do not substitute raw interactive `jj split` when declarative jj-axi hunk routing can express the task.

## Safe operating rules

- Inspect before mutating when the current change or target is uncertain.
- Use explicit change IDs, bookmark names, operation IDs, remotes, and PR numbers.
- Treat structured partial results as changed state, not ordinary failures.
- Do not parse human terminal prose when a jj-axi schema exists.
- Do not create local bookmarks merely to separate unfinished tasks; changes already provide identity.
- Do not assume undo reverses remote effects.
- Re-run `jj-axi --help` after upgrading before relying on newly documented commands.
