# AXI applicability audit

jj-axi applies the AXI design principles according to product fit rather than checklist conformance. This audit reviewed `kunchenguid/axi` at revision `d51c90b2efaed48eed9a6a9e876afc940571ec0c` (2026-07-12).

Classifications:

- **Applicable** — the upstream principle and its product goal fit jj-axi.
- **Adapted** — the product goal fits, but jj-axi deliberately uses a different or narrower mechanism.
- **Not applicable** — neither the prescribed mechanism nor the underlying goal fits the product.

A gap becomes implementation work only when closing it improves jj-axi without relying on benchmark justification.

## Audit

| # | Principle | Classification | Product rationale and evidence | Gap |
|---|---|---|---|---|
| 1 | Token-efficient output | Applicable | Normal success and error responses use stable TOON envelopes. Artifact commands write large content to files rather than returning it inline. Human help text is deliberately outside the operational schema. | None. |
| 2 | Minimal default schemas | Adapted | `log` and similar reads minimize defaults, but operation topology, conflicted references, pagination, and recovery state retain every safety-critical field. The governing rule is the smallest schema that answers one reasoning question without hiding ambiguity. | Continue reviewing new schemas for unnecessary fields; do not enforce a literal field-count ceiling. |
| 3 | Content truncation | Applicable | `show` and `diff` truncate patch content by default at file boundaries and provide `--full`. Large collections use limits or cursor pagination instead of malformed partial records. | None. |
| 4 | Pre-computed aggregates | Applicable | `inspect` combines state and diff statistics; bookmark listing computes ahead/behind; PR status aggregates checks and derives merge readiness; history edits report rewrite/conflict impact; absorb supports structured preview. | None. |
| 5 | Definitive empty states | Applicable | Collections render as `[]`, optional unavailable values have explicit statuses/nulls, no checks produce numeric zeroes plus `none`, and no-op mutations report `unchanged`. | None. |
| 6 | Structured errors and exit codes | Adapted | Errors are structured, failures exit non-zero, unknown flags fail loudly, commands avoid prompts, and partial mutations report retained state. Idempotency is provided where it can be proven; destructive history operations do not guess that an unknown retry previously succeeded. | Preserve provable retry safety rather than promise blanket mutation idempotency. |
| 7 | Ambient context | Adapted | The canonical skill provides proactive discovery, while bare invocation returns a short routing guide on demand. ADR 0004 rejects agent-specific session hooks because they mutate configuration and inject potentially stale startup state. | Runtimes without skill discovery must invoke jj-axi before receiving its routing context. |
| 8 | Content first | Applicable | Running `jj-axi` with no arguments returns the short bootstrap guide directly rather than generic help. Fresh structured repository state remains explicit through `jj-axi inspect`. | Keep the bootstrap substantially smaller than the canonical skill. |
| 9 | Contextual disclosure | Adapted | jj-axi returns deterministic recovery evidence—nearest hunks, blocking reasons, cursors, partial remote state, ambiguity candidates, and rewrite/conflict impact—without speculative next-step advice after ordinary successes. | Continue adding continuation data only where it is objectively actionable. |
| 10 | Consistent help | Applicable | Root and nested commands use one `--help` convention, unknown options fail consistently, and the canonical skill points to installed-version help when capability is uncertain. | None. |

## Summary

- Applicable: 1, 3, 4, 5, 8, 10
- Adapted: 2, 6, 7, 9
- Not applicable: none
- Open product gaps: none

## Positioning

jj-axi is a machine-first Jujutsu companion for non-trivial history editing. It applies AXI principles with documented product-specific adaptations and does not claim strict AXI conformance. Catalog submission is appropriate only if AXI maintainers accept skill- and bootstrap-led discovery in place of hook-first ambient context.

## Re-audit policy

Re-run this audit before a release when upstream AXI principles materially change. Record the reviewed upstream revision and evaluate product behavior before considering benchmark impact.
