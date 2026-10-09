# Cycle 0014 pre-run protocol — local crash and acceptance boundaries

2026-10-09. F04 only; third and last closing witness. Parent `c51a554`.
No distributed partitions/fencing, key rotation, correction backlog or new estimator.

## Question and assumptions

What must become durable together so an export, checkpoint, epoch close or grant
update cannot recover a mixed answer, lose an acknowledged replacement, duplicate
checkpoint evidence, or resurrect a locally applied invalidation?

Assume one serialized local writer and an abstract **atomic durable record replace**:
after completion, a record survives restart; interrupted replacement yields either
its complete old or complete new value. Staging writes are volatile. Dependencies
can be separate immutable durable records; one committed head/receipt record selects
an immutable complete manifest. Crash drops all staged bytes. No disk API, fsync,
database, torn-sector behavior, adversarial corruption or distributed primitive is
implemented or certified. Crashes immediately before/after an atomic step cover its
two permitted outcomes. No garbage collection occurs in this finite witness.

Authority is a trusted local policy state, independent of reachability. A restrictive
control is effective at its durable gate commit and acknowledged only afterward.
All serving is serialized with that commit; no read is served between observing a
restrictive event and making it durable. A crash before that commit leaves the event
unacknowledged and requires the caller to retry; global revocation before local receipt
is outside this model. This is an explicit local linearization rule, not permission
to serve indefinitely from old offline authority. An implementation needing validity
from an earlier external time must add current-authority discovery or a fail-closed
restart barrier. The pending replacement is not the invalidation record.

## Fixtures and candidate

Independent scalar observations have variance 1 m². a=0, b=10, c=20 metres.
Export: replace a-only (0,1) by a+b (5,1/2). Checkpoint: replace a-only natural
parameters by a+b, then replay [b,c,b,c]; correct result is (10,1/3) with exactly
{a,b,c}. Epoch close: close physical-state E0 at time 10 and start E1 (same chart),
reject old current-use a+b, then publish c in E1 (20,1). Grant update: strict P3
withdrawal of b, reject old a+b, then publish a (0,1). Retention of a and its
sufficient statistic is explicitly permitted; deletion/encryption enforcement is
not modeled. Epoch historical use is distinct from the current-use query tested here.

Manifest binds operation and result identity, payload (including uncertainty), exact
input coverage, dependency/calibration revision, model/representation, coordinate and
physical-state epoch, validity interval, and build-time policy revision. Current use
checks the durable gate separately. The checkpoint explicitly declares sufficient
natural parameters and coverage. An export is not automatically a checkpoint.

Stage and persist numerical payload and dependency record; stage and persist complete
manifest; atomically publish head plus retry receipt and the policy/epoch gate checked
at acceptance; only then acknowledge publication. Build, acceptance and current-use
authority remain distinct even if their permitted sets happen to agree.
For epoch/grant changes, stage and persist their gate and acknowledge control *before*
building the replacement. Current-use may be unavailable in between. Record control
and publication acknowledgments separately. Repeating one operation with identical
content is idempotent; reusing its identity with different content is rejected.

## Independent reference, checks and negative controls

Use a manually specified allowed-state table driven by completed durable events,
not by reading the candidate's manifest or invoking its acceptance routine. Before
ordinary replacement publish, old is allowed; afterward new is required. After the
restrictive gate commit and before new publication, current-use is unavailable. Any
acknowledged publication must recover its result. Any acknowledged control must remain
effective. Compare exact means, variances, coverage and epochs, plus availability,
manifest completeness and control state. Replay oracle is the independent raw arithmetic
mean and variance 1/N over the named unique records.

Crash after every staging, persistence, control acknowledgment, head publication and
publication acknowledgment step, plus the initial state. After each restart, retry the
same operation to completion and repeat it once more; require stable identity, one
receipt, correct result and no double count. Also check lost-ack retry and a delayed
old publication retry after a later local publication; it must not move the head back.

Deliberate negative controls: split new coverage from old numerical state; acknowledge
before head durability; acknowledge a volatile-only control; publish before payload
durability; serve old build-time-authorized output after a durable restrictive control.
Each must produce a concrete wrong result or broken acknowledged obligation. The
checked candidate must reject missing dependencies and incompatible gate/epoch state.
Wrong numerical content that is consistently mislabeled is not made correct by a hash.

Exact fractions, fixed finite traces, no randomness or timing thresholds. Preserve all
crash cuts and negative-control traces in one JSON artifact, source/protocol/test hashes,
commands, runtime and revision pins. Acceptance is zero unexpected allowed-state or
retry violations; negative controls must fail for their declared reason. Preserve a
failure rather than expanding the cycle or weakening the oracle. Test and report
contract obligations, not a production crash-safety or physical-safety guarantee.

After this witness: synthesis and the two independent reviews, with no further research
cycles. A new contradiction is reported immediately; a missing binding/order rule is
normally a refinement of existing obligations, not a new architectural conflict.
