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

Known investigation target, no dynamic evidence yet. P3 may forbid a transition that
continues to depend on the withdrawn estimate, while abruptly switching a moving
consumer's reference may itself produce a dangerous command. The solution cannot be
to secretly retain the forbidden estimate or delay invalidity notification. Study
permitted derived-result retention, independent local state, explicit transitions and
controlled hold/replan under stated dynamics. Affected layers: consent policy,
application result validity and consumer/controller contract. Next: cycle 0002.

## New tensions

None established yet. Test/model defects will be recorded as such before treating
them as architectural contradictions.
