# jj-axi

jj-axi presents Jujutsu's change and operation models as deterministic, agent-native workflows while preserving standard repository compatibility.

## Language

**User-visible repository mutation**:
An operation that records an explicit VCS intent, excluding automatic working-copy synchronization and other housekeeping operations.
_Avoid_: Meaningful operation, user operation

**Synchronization-only operation**:
An operation created solely to reconcile repository or working-copy state, without an explicit VCS intent from the agent.
_Avoid_: Snapshot noise, housekeeping commit

**Divergent operation history**:
A repository state with multiple concurrent operation heads and therefore no unique latest mutation.
_Avoid_: Conflicted oplog, ambiguous history

**Foundation operation**:
A repository or workspace initialization operation whose removal would invalidate the active workspace. It is visible in operation history but is not an undo candidate.
_Avoid_: Setup mutation, root action

**Working-copy change**:
The change currently edited by one Jujutsu workspace. A partition may route its remainder into the invoking workspace's existing working-copy change, or create a fresh one when the source is that change.
_Avoid_: Uncommitted files, dirty tree, unstaged changes

**Local bookmark**:
A named repository reference expressing the local target an agent may organize or publish.
_Avoid_: Branch, local branch

**Tracked remote bookmark**:
A locally recorded remote bookmark whose collaboration state Jujutsu follows across fetches and pushes.
_Avoid_: Remote branch, tracking branch

**Publication remote**:
The Git remote selected as the destination of one explicit push operation; it is not inferred from bookmark tracking state.
_Avoid_: Upstream, tracking remote

**GitHub repository identity**:
A normalized hostname, owner, and repository name used to address one GitHub API resource independently of Git remote spelling.
_Avoid_: Repo slug, origin repository

**Merge readiness**:
The derived state indicating that an open, non-draft pull request is mergeable, has acceptable reviews, and has no failed or pending checks.
_Avoid_: Green PR, merge status

**Blocking reason**:
A stable machine-readable condition that currently prevents merge readiness.
_Avoid_: PR problem, blocker message

**Bootstrap guide**:
The body of the canonical skill document, printed without YAML frontmatter by bare invocation to route an agent between routine Jujutsu work and non-trivial history editing.
_Avoid_: Home view, separate bootstrap file, mini skill

**Canonical skill document**:
The compact, versioned, repository-independent instruction artifact that routes an agent to the correct jj-axi command and preserves only cross-command safety rules. Exact execution contracts belong to installed command help.
_Avoid_: Skill template, generated prompt, command manual

**Agent reference document**:
The detailed, opt-in human-oriented instruction artifact for secondary commands and complete product semantics that do not belong in the canonical skill's routine context budget.
_Avoid_: Full skill, extended prompt

**Installed command contract**:
The version-matched workflow, example, syntax, and safety semantics printed by `jj-axi <command> --help` for one command.
_Avoid_: Flag list, usage text

**Routine Jujutsu workflow**:
A direct, non-interactive `jj` workflow for ordinary inspection, simple change creation, or another task that one raw command expresses clearly.
_Avoid_: Raw fallback, legacy workflow

**Non-trivial history editing**:
A workflow that requires exact content selection, an interactive editor, manual patch interpretation, or several dependent history mutations when expressed through raw Jujutsu.
_Avoid_: Advanced mode, complex Git operation

**Partition**:
Atomically decomposing one source change's content diff into an ordered sequence of parts and one explicit remainder disposition. The first part preserves the source change identity; binary split is its one-part specialization.
_Avoid_: Repeated split, batch split, multi-split

**Partition part**:
One named content selection in a partition, ordered from oldest to newest. The first part preserves the source change identity; later parts receive new identities.
_Avoid_: Split result, output commit

**Partition remainder**:
Every source hunk not assigned to a partition part, governed explicitly as a remaining change, working-copy content, or a requirement that the remainder be empty.
_Avoid_: Leftovers, unmatched changes

**Squash**:
Moving all content changes from one change into another and abandoning the emptied source while preserving rewritten descendants.
_Avoid_: Merge commits, combine branches

**Abandon**:
Removing one visible change from current history and reparenting its descendants through standard Jujutsu rewrite semantics without reversing external effects.
_Avoid_: Delete commit, discard repository
