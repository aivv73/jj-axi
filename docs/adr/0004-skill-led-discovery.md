# Skill-led discovery instead of session hooks

jj-axi uses the canonical `jj-axi` skill for agent discovery and `inspect` for fresh repository state on demand; it will not install session-start hooks. Agent hook systems have different lifecycle and configuration contracts, mutate user or project configuration, and inject state that may be stale by the time an agent acts. The skill already teaches agents in Jujutsu repositories to begin with `inspect` without coupling the core product to individual agent runtimes.

This deliberately rejects mechanical conformance with AXI’s session-hook principle. jj-axi will document each AXI principle as applicable, adapted, or not applicable and optimize for the product’s agent UX rather than checklist completion. Environments that do not discover skills automatically will not receive proactive jj-axi context and must invoke the skill or CLI explicitly.
