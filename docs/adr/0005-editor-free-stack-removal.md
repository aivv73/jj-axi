# Editor-free squash and abandon semantics

jj-axi makes full-change stack removal deterministic rather than inheriting raw Jujutsu’s editor fallback. `squash` moves all source content, abandons the emptied source, and requires an explicit `--message` when both source and destination descriptions are non-empty; it never concatenates descriptions or opens an editor. The default destination is the sole parent, while an explicit destination may be an ancestor or unrelated rewritable change but not the source or its descendant.

Squash and abandon use standard Jujutsu rewrite semantics for descendants and local bookmarks. Conflicts produced by a valid operation are committed first-class state and reported in the success response rather than converted into rollback failures. Neither command mutates remotes. `abandon` records enough operation metadata to make proven retries return an unchanged result; an unknown revision remains an error when history cannot prove prior abandonment.
