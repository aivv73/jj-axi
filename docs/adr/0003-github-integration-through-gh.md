# GitHub integration through gh

jj-axi uses the authenticated `gh` CLI as its GitHub API transport rather than owning an HTTP client and credential store. jj-axi invokes `gh api graphql` non-interactively, validates its JSON response, and owns the stable agent-facing schema and derived merge readiness; `gh` owns credential storage, SSO, authentication refresh, GitHub Enterprise host routing, and low-level API transport.

This creates a runtime dependency for GitHub-specific commands, while core Jujutsu commands remain independent of `gh`. Missing authentication, transport failures, rate limits, and invalid responses are normalized into jj-axi error schemas without forwarding raw stderr. Replacing this boundary later requires an explicit reassessment of the security and enterprise-support responsibilities jj-axi would inherit.
