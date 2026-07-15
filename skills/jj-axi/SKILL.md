---
name: jj-axi
description: "Use for deterministic, editor-free Jujutsu history editing: exact hunk routing, multi-part partitioning, stack reorder, absorb, undo, and validated publication. Use raw jj for routine single-step work."
---

# jj-axi

Use Jujutsu interfaces when `.jj/` exists, including in a colocated repository. A Jujutsu working copy is a mutable change: there is no staging area, change IDs survive rewrites, and bookmarks are mainly publication references. Do not mechanically translate Git staging, stash, or branch workflows.

Use raw `jj` for routine single-step reads and writes such as `status`, `log`, `show`, `diff`, `new`, and `describe`. Use `jj-axi` when history editing would otherwise require an editor, manual patch interpretation, or several dependent mutations. Its output is bounded and structured, supported retries report explicit state, and unsupported history shapes fail instead of being guessed. Never blindly retry an unknown mutation outcome; inspect partial state first. Bare `undo` is intentionally not idempotent and continues backward when repeated.

## Inspect once, then act

Start with one compact state read when the target is uncertain:

```bash
jj-axi inspect
```

Use focused reads only when needed:

```bash
jj-axi log --limit 20
jj-axi show <change>
jj-axi diff <change>
```

Do not repeatedly inspect unchanged state. Run `jj-axi <command> --help` only when this skill does not cover the needed command or an installed-version error indicates a contract mismatch; do not probe guessed command names.

## Change lifecycle

```bash
jj-axi new --message "implement token refresh"
jj-axi describe <change> --message "implement token refresh"
jj-axi checkpoint --message "complete token refresh"
```

`checkpoint` describes the current work and opens a fresh child. For a local-only or Git-visible bookmark, always set it without publishing:

```bash
jj-axi bookmark set feature-name --to <change>
```

Validate readiness with `jj-axi finish <change>`. **`finish --bookmark` means remote publication**, not local placement. Use it only when the task explicitly asks to push or publish. For a local-only bookmark, use `jj-axi bookmark set`. `finish` never invents a bookmark.

## Route exact hunks

Before `split`, `move`, or `partition`, inventory the source once:

```bash
jj-axi diff <change> --hunks
```

Copy the full `snapshot.commit_id` and canonical `path`/`lines` selectors unchanged. Never infer ranges from rendered patch text. Use `N-0` exactly as returned for deletion-only boundaries.

Create a new destination:

```bash
jj-axi split <change> --source-commit-id <full-id> \
  --hunks 'src/lib.rs:12-18,tests/lib.rs:8-14' --into 'extract parser'
```

Move content when both changes already exist:

```bash
jj-axi move --from <source> --to <destination> \
  --source-commit-id <full-id> --hunks 'src/lib.rs:12-18'
```

For three or more destinations, prefer one guarded partition over repeated binary splits. Pipe the manifest through stdin so creating it cannot stale the source:

```json
{
  "schema_version": 1,
  "source_commit_id": "<full snapshot commit id>",
  "parts": [
    {"description": "refactor parser", "hunks": [{"path": "src/lib.rs", "lines": "12-18"}]},
    {"description": "test parser", "hunks": [{"path": "tests/lib.rs", "lines": "8-14"}]}
  ],
  "remainder": {"destination": "working_copy"}
}
```

```bash
cat partition.json | jj-axi partition <change> --spec-file - --dry-run --details
cat partition.json | jj-axi partition <change> --spec-file -
```

Parts are ordered oldest to newest. Use `working_copy` for unfinished residual work, `remaining_change` for a separate remainder change, or `require_empty` to reject unassigned content. After success, one targeted inspection is normally enough.

## Edit a stack

Preview automatic absorption, then apply the reviewed plan:

```bash
jj-axi absorb --dry-run
jj-axi absorb
```

Reorder a contiguous linear stack by listing every selected change once, oldest to newest:

```bash
jj-axi reorder --sequence '<oldest>,<middle>,<newest>'
```

Squash all content from a source into a destination:

```bash
jj-axi squash <source> --into <destination> --message 'combined change'
```

Use `move` instead for selected hunks. Use `jj-axi abandon <change>` to remove one change and reparent descendants.

## Recover and publish

```bash
jj-axi operations --limit 20
jj-axi undo
jj-axi undo --to <operation-id>
jj-axi bookmark list
jj-axi bookmark push feature-name
```

Bare `undo` reverses the latest user-visible repository mutation while retaining newer synchronized working-copy content. It does not reverse remote effects. Bookmark listing never fetches; bookmark push publishes only the exact existing name.

## Safety rules

- Use explicit change IDs, bookmark names, operation IDs, remotes, and PR numbers.
- Treat structured partial results and successful conflict results as changed state; inspect before retrying.
- Do not parse terminal prose when a structured jj-axi response exists.
- Do not invent selectors, manifest fields, flags, or history shapes.
- Avoid redundant state queries after a successful mutation receipt.
