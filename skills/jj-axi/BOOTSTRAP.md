# jj-axi

Use raw `jj` for routine `status`, `log`, `show`, `diff`, `new`, and `describe` operations.

Use jj-axi when history editing requires an editor, manual patch interpretation, or several dependent mutations:

- exact hunk routing: `split`, `move`, `partition`;
- inferred routing or stack edits: `absorb`, `reorder`, `squash`;
- operation recovery or deterministic publication: `undo`, `finish`.

Before `split`, `move`, or `partition`, run `jj-axi diff CHANGE --hunks` and copy canonical selectors unchanged. Never infer ranges from patch text. Treat structured partial results as changed state.

Run `jj-axi skill` to choose a command, then `jj-axi <command> --help` for its installed contract.
