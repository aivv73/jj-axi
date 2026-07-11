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
