# Atomic change partitioning

## Status

Accepted

## Decision

jj-axi models multi-way partitioning as one atomic decomposition of a source change's content diff into an ordered oldest-to-newest sequence of named parts and one explicit remainder disposition. The first part preserves the source change ID; later parts receive new IDs. Trees are cumulative, descendants are rewritten once, bookmarks targeting the source remain on the first part, and the whole apply is one user-visible operation and undo boundary. Binary `split` is the one-part specialization and must share the partition engine.

Every selector addresses one guarded source snapshot. A versioned JSON manifest supplies the full source commit ID, ordered parts, and one of `remaining_change`, `working_copy`, or `require_empty`. All selectors, overlap, descriptions, topology, rewritability, limits, and remainder rules are validated before a transaction. `working_copy` means the invoking Jujutsu workspace's change and is allowed only when the source is that change or its ancestor; another workspace directly editing the source makes this disposition invalid. `remaining_change` always creates a remainder change, even when empty, preserving current split behavior. `require_empty` creates no remainder and rejects any unassigned or unsupported content.

Conflicts created by rewriting are successful structured state rather than command failure. Dry-run performs equivalent planning without visible repository mutation and does not promise newly allocated change IDs. The compact default receipt identifies the exact manifest by SHA-256 and reports bounded topology summaries. Applied descendant results are correlated by their pre-rewrite commit IDs, not change IDs, because divergent commits may share one change ID; detailed hunk echo is opt-in. Retrying after an unknown successful apply fails the source commit guard rather than claiming unprovable idempotency.

## Consequences

Partition replaces repeated diff/split/remainder-routing workflows without introducing fuzzy selectors or an interactive editor. Operational output remains TOON, while manifests use strict JSON because jj-axi already has a mature JSON parser dependency but no TOON parser. Applying a plan remains race-safe without a server-side plan token, at the cost of requiring callers to copy the immutable source commit ID returned by hunk inventory.
