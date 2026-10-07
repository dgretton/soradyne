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
Next: finite jerk/reaction-delay stopping envelope in 0003. Do not infer a general
safe-stop policy or decide permission semantics from this witness.

## New tensions

None established yet. Test/model defects will be recorded as such before treating
them as architectural contradictions.
