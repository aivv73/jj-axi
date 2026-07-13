---
name: jj-axi
description: "Use for deterministic, editor-free Jujutsu history editing: exact hunk routing, multi-part partitioning, stack reorder, absorb, undo, and validated publication. Use raw jj for routine single-step work."
---

# jj-axi

Use raw `jj` for ordinary `status`, `log`, `show`, `diff`, `new`, and `describe` operations.

Use jj-axi when Jujutsu history editing would require an editor, manual patch interpretation, or several dependent mutations. When `.jj/` exists, including in a colocated repository, use Jujutsu interfaces rather than mechanically translating Git staging, stash, or branch workflows.

## Choose an operation

- Split selected hunks into a new change: `split`.
- Move known hunks between known changes: `move`.
- Split one change into multiple ordered changes: `partition`.
- Infer destinations from mutable ancestors: `absorb`.
- Reorder a contiguous linear stack: `reorder`.
- Move all content or remove a change: `squash`, `abandon`.
- Inspect or recover repository operations: `operations`, `undo`.
- Validate or publish one exact bookmark: `finish`, `bookmark push`.

Before using a command, run:

```bash
jj-axi <command> --help
```

For nested commands, use the complete path, such as `jj-axi bookmark push --help`. Follow the installed help exactly. Do not guess flags, manifest fields, history shapes, or hunk selectors.

Before `split`, `move`, or `partition`, obtain canonical selectors with:

```bash
jj-axi diff <change> --hunks
```

Copy the returned full snapshot commit ID and `path`/`lines` selectors unchanged. Never infer line ranges from patch text. Treat a structured partial result as changed repository state and inspect it before retrying.
