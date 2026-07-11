# ADR 0001: Post-image hunk addressing

## Status

Accepted

## Context

The M3 history-editing commands need an editor-free way to identify exact
changes in a file. PRD §2 question #1 left open whether hunk line numbers
refer to the pre-image or post-image and whether a stale range should fail or
be silently adjusted. Agents inspect the current file state before issuing a
command, so the address must describe that inspected state and a failed
request must be retryable without silently routing a different edit.

## Decision

`--hunks` is a comma-separated list of `PATH:LINES` entries. `LINES` is a
1-based, inclusive post-image range: `N` or `N-M`. A deletion-only hunk is
addressed by the zero-width boundary `N-0`, where `N` is the next post-image
line, or `line_count + 1` at end of file. Entries may escape only `\\`, `\,`,
and `\:`. Unknown escapes and trailing escapes are rejected. The last
unescaped colon separates the path from the range.

`PATH` is slash-separated and repository-root-relative. It has no empty `.`,
or `..` component and is not trimmed; spaces are valid filename bytes. A
range selects exactly one context-free `jj_lib::diff::ContentDiff::by_line`
`Different` hunk. Multiple hunks are selected by listing multiple entries.
Empty paths or ranges, absolute or parent-traversing paths, zero or descending
non-deletion ranges, duplicate entries, unchanged paths, and partial hunk
overlaps fail before an M3 transaction. Requested non-text, conflicted,
symlink, submodule, or mode-only changes also fail. Unsupported paths not
named by the caller remain in the unselected portion.

Addresses use the post-image because it is the file state an agent just
inspected. The selector never snaps. A stale or non-boundary input returns
`invalid_hunk_selection` and at most the three nearest canonical hunks for
that path. Candidates are ranked by absolute distance between the requested
and candidate start line, then candidate start, end, and path, so retries are
deterministic without returning an unbounded diff.

Selection and mutation use the one repository snapshot loaded by
`JjBridge::open`; there is no separate plan/apply token. `open` retains its
standard working-copy synchronization before validation. If the working copy
changed, the synchronized diff either matches the request exactly or fails.
An invalid M3 request creates no additional history-edit transaction.

All history mechanics remain in the existing `JjBridge` jj-lib boundary.
`jj-cli` and `jj-lib` stay pinned to `=0.43.0`; interactive `jj split -i` and
human CLI output are not used. The implementation reuses jj-lib transaction,
selection, rewrite, squash, absorb, merge-tree, and content-diff primitives.

## Consequences

Agents can address the file state they inspected without editor state, and
stale or partial selections fail loudly with deterministic retry hints.
`N-0` is additional syntax for deletion-only hunks, and callers must retry
after a failed selection. The supported content is deliberately narrower than
jj's general diff model: metadata-only, rename/copy, binary, symlink,
submodule, and conflicted routing is rejected rather than guessed. This
milestone accepts the jj-lib `=0.43.0` API surface and its upstream churn in
exchange for atomic history edits and structured reports. jj-lib may write
unreachable internal content objects while constructing a preview, but
visible repository state is not changed by an invalid request or an absorb
dry-run.
