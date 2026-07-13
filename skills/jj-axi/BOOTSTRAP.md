# jj-axi: deterministic history editing for agents

Use raw `jj` for everyday repository work:

- `jj status`, `jj log`, `jj show`, and ordinary diffs;
- `jj new` and `jj describe`;
- other simple, non-interactive single-step operations.

Use `jj-axi` when history editing would otherwise require an editor, manual patch interpretation, or several dependent mutations:

- discover exact hunks with `jj-axi diff CHANGE --hunks`;
- route selected hunks with `split`, `move`, or `partition`;
- preview and apply `absorb`;
- reorder a stack or undo an earlier repository operation;
- validate and publish an exact bookmark deterministically.

Typical selective-editing flow:

1. Run `jj-axi diff CHANGE --hunks`.
2. Copy the returned canonical `path` and `lines` selectors unchanged.
3. Pass them to `jj-axi split`, `move`, or a guarded `partition` manifest.
4. Inspect the structured result before continuing.

Run `jj-axi skill` for compact operational instructions, `jj-axi skill --full` for the detailed agent reference, or `jj-axi --help` for installed command syntax.
