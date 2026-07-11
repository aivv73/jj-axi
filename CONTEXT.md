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
