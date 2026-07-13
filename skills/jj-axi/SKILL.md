---
name: jj-axi
description: "Use for deterministic, editor-free Jujutsu history editing: exact hunk routing, multi-part partitioning, stack reorder, absorb, undo, and validated publication. Use raw jj for routine single-step work."
---

# jj-axi

Use raw `jj` for ordinary `status`, `log`, `show`, `diff`, `new`, and `describe` operations.

Use jj-axi when raw Jujutsu would require an editor, manual patch interpretation, or several dependent history mutations. Use Jujutsu interfaces whenever `.jj/` exists, including colocated repositories; do not mechanically translate Git staging, stash, or branch workflows.

## Choose an operation

- Split selected hunks into a new change: `split`.
- Move known hunks between known changes: `move`.
- Split one change into multiple ordered changes: `partition`.
- Infer destinations from mutable ancestors: `absorb`.
- Reorder a contiguous linear stack: `reorder`.
- Move all content into another change: `squash`.
- Inspect or recover repository operations: `operations`, `undo`.
- Validate or publish one exact bookmark: `finish`, `bookmark push`.

Use `jj-axi --help` to confirm installed syntax. Use `jj-axi skill --full` only when detailed semantics for a secondary command are necessary.

## Discover exact hunks first

Before `split`, `move`, or `partition`, run:

```bash
jj-axi diff <change> --hunks
```

Copy the returned full `snapshot.commit_id`, `path`, and `lines` values exactly. Never derive selectors from patch text or approximate line numbers. Selectors do not snap to nearby content: stale, partial, duplicate, binary, or unsupported selections fail without history mutation and return bounded recovery information.

Use `N-0` exactly as returned for deletion-only boundaries.

## Route selected hunks

Create a new change from selected hunks:

```bash
jj-axi split <change> \
  --hunks 'src/lib.rs:12-18,tests/lib.rs:8-14' \
  --into 'extract parser'
```

Move selected hunks between existing changes:

```bash
jj-axi move \
  --from <source> \
  --to <destination> \
  --hunks 'src/lib.rs:12-18'
```

Use `split` when the destination must be created. Use `move` when both source and destination already exist.

## Partition one change into several changes

Use `partition` when one source change must become multiple ordered changes. Build the manifest only from one `diff --hunks` result:

```json
{
  "schema_version": 1,
  "source_commit_id": "<full snapshot commit id>",
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

Pipe the manifest through stdin so writing it cannot stale the source snapshot:

```bash
cat partition.json | jj-axi partition <change> --spec-file - --dry-run --details
cat partition.json | jj-axi partition <change> --spec-file -
```

Inspect a detailed dry-run for multi-part partition unless every assignment is mechanically obvious. Choose one explicit remainder destination:

- `working_copy` for unfinished content;
- `remaining_change` for a separate remainder change;
- `require_empty` to reject any unassigned or unsupported content.

## Edit a stack without an editor

Preview absorption before applying it:

```bash
jj-axi absorb --dry-run
jj-axi absorb
```

Reorder a contiguous linear selection from oldest to newest:

```bash
jj-axi reorder --sequence '<oldest>,<middle>,<newest>'
```

Move all content from one change into another; this is not selective hunk routing:

```bash
jj-axi squash <from> --into <to>
```

History-shape errors and rewrite conflicts are reported explicitly rather than resolved by guessing.

## Recover or publish

Inspect operation history or undo the latest user-visible repository mutation:

```bash
jj-axi operations --limit 20
jj-axi undo
jj-axi undo --to <operation-id>
```

`finish <change>` validates local readiness. Adding `--bookmark` also publishes that exact bookmark:

```bash
jj-axi finish <change>
jj-axi finish <change> --bookmark feature-name
```

`finish` never invents a bookmark name. Local bookmark placement alone does not publish.

## Safety rules

- Treat a structured partial result as changed state, not an ordinary failure; inspect it before retrying.
- Use explicit change IDs, hunk selectors, bookmark names, operation IDs, and remotes.
- Do not parse human terminal prose when a jj-axi schema answers the same question.
- Do not use raw interactive `jj split` when exact jj-axi routing can express the task.
- Undo is local and does not reverse pushes or other external effects.
