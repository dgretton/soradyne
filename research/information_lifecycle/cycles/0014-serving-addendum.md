# Cycle 0014 protocol addendum — pending control serving barrier

Review after preserving `runs/0014` found that `Store.read()` served the old result
after `stage_gate` and before `persist_gate`. The original protocol explicitly forbids
that live response. Its recovery oracle was correct under the stated local effect
rule, but using the same allowed-state table for live and recovered queries missed
this distinction. The 255-test pass is therefore not full conformance to that protocol.
Keep original source, tests, protocol and artifact unchanged as diagnostic evidence.

Before corrected execution: add a serving barrier while a different control is pending.
After durable gate installation, normal current-use validation decides availability.
After a crash before installation, volatile knowledge is lost: the unacknowledged event
must be retried by the caller under this fixture's rule. This is not an external-time
revocation guarantee. That stronger contract needs durable knowledge or fail-closed
authority refresh at restart, as already stated in the original protocol.

Use a separate `crash_serving_barrier.py` candidate importing the unchanged primitive
model. Repeat all 46 crash cuts and idempotent retries; live answers at the two cuts
immediately after `stage_gate` must now be unavailable, while restarted answers may
still be old. All other allowed states stay unchanged. Preserve a seventeenth negative
control showing the original live leak. Require zero unexpected state/retry violations;
retain the original 16 negative controls and checkpoint oracle. Write a fresh
`runs/0014-corrected` artifact and hash both original and corrected sources/tests and
both protocols. This is a model/test correction inside F04, not another scenario cycle
or an architectural contradiction. No new fault mechanisms or experiments are added.
