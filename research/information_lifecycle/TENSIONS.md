# Design tensions and open decisions

These are research questions, not changes to the binding contracts. New entries need
a witness or a concrete missing decision, not just general concern.

## T01 — irreversible reduction versus selective withdrawal

Known investigation target. A joint reduction can preserve the current marginal
exactly while discarding information needed for later removal of one contributing
source. R03 tests whether retained pre-elimination factors, a richer summary, or
authorized replay is required. A prior cannot be assumed invertible back into its
components. Evidence: [cycle 0001](cycles/0001.md) gives two models with the same
frozen marginal and same removed observation but different required results.
Established for that exact linear model, not a universal ban on richer summaries.
Affected layer: application representation
and provider retention contract. Candidate decision: rebuild from still-authorized
data when available; otherwise identify unsupported withdrawal or invalidate.

[Cycle 0004](cycles/0004.md) exercises this choice with explicit permissions. A
specifically authorized derivative remains usable after raw deletion under P2. Under
P3 the mixed artifact becomes unusable; without the surviving component's raw value,
the ledger reports unavailable until authorized replay. A surviving entitlement alone
does not reconstruct missing numerical state. This is a small independent-scalar
fixture, not a general result about richer or nonlinear summaries. Translating a
withdrawal intent into all affected artifact rights remains an application/contract
obligation; the experiment receives that complete policy snapshot as an input.

[Cycle 0005](cycles/0005.md) adds a distinct exact overlap witness: two worlds have
identical child lineage and aggregated conditional joint factors on (x,b), yet require
different full unique-data answers. Keeping the shared calibration variable addresses
correlation but does not recreate the overlap's numerical contribution. Known cross-
covariance can produce an honest reduced-information answer (variance 3/8 in the
witness without calibration uncertainty), while full raw data has variance 1/3; it does
not recover the discarded statistic. Possible choices remain finer decomposition, an
adequate overlap statistic, authorized replay, or explicit reduced/unavailable answers. Atomic blocks work in this fixture but are
raw-equivalent information and need corresponding retention/use authorization. This
strengthens T01 without establishing a new architectural contradiction.

[Cycle 0006](cycles/0006.md) shows why preserving an independently reusable local
contribution can avoid an outage when a returned mixed posterior becomes forbidden.
A matched additive residual recovers that contribution in the exact scalar fixture;
using the wrong parent revision can instead cancel new lawful evidence. The clean
residual's retention/use rights are explicitly assumed, not granted by mathematical
cancellation or inferred from historical causal ancestry. This adds a limited sufficient
case alongside the earlier insufficiency witnesses, without resolving general derivative
policy or subtraction after elimination.

## T02 — continuous consumer motion versus immediate loss of usable information

Known investigation target, partial dynamic evidence in [cycle 0002](cycles/0002.md).
P3 may forbid a transition that
continues to depend on the withdrawn estimate, while abruptly switching a moving
consumer's reference may itself produce a dangerous command. The solution cannot be
to secretly retain the forbidden estimate or delay invalidity notification. Study
permitted derived-result retention, independent local state, explicit transitions and
controlled hold/replan under stated dynamics. Affected layers: consent policy,
application result validity and consumer/controller contract.

0002 shows immediate invalidation plus independent local braking can respect P3
and preserve clearance in one ideal scalar case. It also falsifies unconditional
coalescing/smoothing: delaying a needed correction can cross the boundary despite
less reference/command variation. Gated braking is not a complete resolution; its
largest command jerk (400 m/s^3 at dt=0.01) is unconstrained by this plant model.
[Cycle 0003](cycles/0003.md) adds that finite-jerk/response-delay envelope. Seven of
nine variants of the same necessary-correction event have insufficient clearance even
under a necessary lower bound on stopping excursion. Immediate invalidation and zero
reference reuse still hold. A separate candidate crossing remains inconclusive against
the lower bound; its failure is not labeled unavoidable. This makes the fallback's
required operating margin concrete. It must be established before information loss;
smoothness or prompt invalidation alone cannot supply missing space. The planner in
the experiment does not enforce such a pre-event margin. A production consumer needs
an explicit local-state/response/operating-envelope contract, still to be designed.
Do not infer a general safe-stop policy or decide permission semantics from this
witness. Future physical coverage stays in the broader S03/compound work; subsequent
cycles investigate information and permission semantics.

[Cycle 0006](cycles/0006.md) also isolates a precursor to poor correction response:
recycling a parent's posterior can leave the mean steady while inventing precision.
After twelve no-data rounds, a new observation produces only about 8.6% of the required
mean correction in one exact fixture. A separately tested damped iteration converges
to the wrong answer. These are inference failures before any controller is modeled;
small jumps or convergence alone do not establish a valid response. No additional
physical-safety conclusion or new architectural contradiction follows.

## New tensions

None established yet. Test/model defects will be recorded as such before treating
them as architectural contradictions.
