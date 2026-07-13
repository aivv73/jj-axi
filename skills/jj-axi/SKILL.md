---
name: jj-axi
description: "Use for deterministic, editor-free Jujutsu history editing: exact hunk routing, multi-part partitioning, stack reorder, absorb, undo, and validated publication. Use raw jj for routine single-step work."
---

# jj-axi

Use raw `jj` for routine `status`, `log`, `show`, `diff`, `new`, and `describe` operations. When `.jj/` exists, including in a colocated repository, use Jujutsu interfaces rather than mechanically translating Git staging, stash, or branch workflows.

Use jj-axi when history editing requires an editor, manual patch interpretation, or several dependent mutations:

- exact hunk routing: `split`, `move`, `partition`;
- inferred routing or stack edits: `absorb`, `reorder`, `squash`, `abandon`;
- operation inspection or recovery: `operations`, `undo`;
- deterministic validation or publication: `finish`, `bookmark push`.

Before `split`, `move`, or `partition`, run `jj-axi diff CHANGE --hunks` and copy the full snapshot ID and canonical selectors unchanged. Never infer ranges from patch text. Treat structured partial results as changed state.

Before using a command, run `jj-axi <command> --help` and follow that installed contract exactly. For nested commands, use the complete path, such as `jj-axi bookmark push --help`. Do not guess flags, manifest fields, history shapes, or selectors.
