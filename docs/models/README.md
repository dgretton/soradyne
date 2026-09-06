# Formal Models for the rim Protocol — What Exists, How to Extend It, How to Keep It Honest

**Date**: 2026-09-06
**Author**: Claude (Fable 5.1, Claude Code), at Dana's request
**Audience**: agents about to implement anything in `soradyne_core` that touches
grants, custody, leases, logs, or replay. Read this before writing the code, and read
§4 before claiming the code matches the model.
**Status**: binding for the pieces named in §1. The models in this directory were
written without a local Java install; the first `check.sh` run on the development
machine is the acceptance test for this README.

---

## 0. What formal models are for here, and what they are not for

Three things went wrong in soradyne's design over two years, and none of them was a
bug in code: concepts contradicted each other, a plan contradicted the spec, and a
privacy rule lived in correspondence instead of the protocol. Formal models are here to
make that class of error *cheap to catch*. A model is a page of definitions that a
tool can search for contradictions in seconds. That is the whole value.

They are not here to prove the implementation correct. The implementation is kept
honest by tests, and by the trace discipline in §4 that ties tests to the model.
Nothing in this directory replaces the conformance suite across the blanket
(`20260906_fable_markov_blanket_discipline.md` §6), which remains the cross-project
truth.

Scope discipline, stated once: **three small models, each owning one mechanism, each
paired with a test.** If a model grows past a few hundred lines, or starts to mirror
code structure, it has stopped being a model and become a second implementation that
will drift. Split it or delete it.

---

## 1. What exists

| File | Tool | Owns | Pairs with |
|---|---|---|---|
| `invariants.als` | Alloy 6 | The consolidated model's structural invariants C1, C2, C3, C5 and delegation eligibility (cascade §3.2): capsules, flows, streams, records, roles, grants, custody, revocation | The grant/custody conformance tests (substrate contract §12 items 7–9) once they exist; until then, this file *is* the check on §3 |
| `Lease.tla` + `Lease.cfg` | TLA+ / TLC | The exclusive-role lease: claim, lapse, revoke, term-stamped writes, single writer per term, progress | The lease conformance test (substrate contract §12 item 6) and the delegation implementation (cascade §5 step 4) |
| `check.sh` | bash + Java | Runs both; downloads the jars on first use into `.tools/` (gitignored) | `packages/soradyne_core/tests/formal_models.rs`, which runs it under `cargo test` |

The third planned model, **log replay with horizons** (per-party append-only streams,
reconnect, exactly-once replay, discard on leave), is not written yet. §3 says how to
write it; it should be written *before* the log stream implementation (cascade §5
step 1), by whichever agent takes that step.

Run everything:

```
docs/models/check.sh          # all
docs/models/check.sh alloy    # invariants only
docs/models/check.sh tla      # TLC only
cargo test -p soradyne_core --test formal_models
```

`check.sh` exits 0 on success, 1 on a counterexample or an over-constrained `run`,
2 when Java or the network is unavailable. The cargo test treats 2 as a loud skip
unless `SORADYNE_REQUIRE_FORMAL=1` is set, which CI on the development machine should
set.

---

## 2. Alloy: how to read and extend `invariants.als`

### 2.1 What it says

The signatures are the nouns of cascade §3.1. The facts are the parts of those
definitions that are definitional (a derived stream belongs to its flow's owner; a raw
record has no citation field; references are acyclic). `Wellformed[s]` is the set of
rules a *state* must satisfy: grants cross capsules and need an introduction, outsiders
get derived streams only, role holders can read every input, authors hold their own
records, replicas imply authorization. `revoke` is the one transition. The `check`
commands ask Alloy to search, up to a bound, for a well-formed state and a revocation
that violate C1, C2, C3, or C5, or that leave the system malformed. The `run` commands
ask for witnesses that the design's intended situations are admissible: a shared scope
with an outside estimator and a nested parent that sees only the summary; an opaque
reference; a revocation that actually discards something.

### 2.2 What it already taught us while being written

Two rules in cascade §3 were sharpened by the act of writing the model, and the prose
should be read with these in mind:

- **Role holders are not parties.** A capsule that fills the estimator role reaches raw
  streams only through *role grants*, never through capsule-level grants. If it were
  otherwise, revoking the role would leave capsule-level raw access dangling. This is
  now a fact in the model (`parties` excludes role holders) and should be stated in the
  spec's Grants section.
- **Losing a role discards all of the role's inputs**, not just the stream whose grant
  was revoked. Otherwise the ex-holder keeps replicas it can no longer read. This is
  in `revoke` and belongs in the Custody section as the precise form of C2.

Expect more of these. When a `check` fails, the counterexample is a small concrete
world with three or four capsules and a few streams. Read it as a story ("capsule B
holds a record of A's raw stream after A revoked, because B's piece still holds a role
that reads it via a role grant from C"). Nine times out of ten the story is a real
hole in the prose, and the fix is a sentence in the cascade or the spec *and* a fact or
a clause in the model, in one change.

### 2.3 Extending it

- Add a signature only when a new noun enters cascade §3.1. Add a fact only for a
  definitional rule. Everything conditional on state goes in `Wellformed` or a
  transition.
- Every new invariant gets an `assert` *and* a `check` with a scope of at least 5.
  Every new intended situation gets a `pred` and a `run`. A model whose checks all pass
  because it is over-constrained is worse than no model; the runs are the guard.
- Keep it in plain Alloy (no `var`, no temporal operators) unless you genuinely need
  multi-step traces; the two-state `revoke` idiom covers everything so far.
- Scopes: `for 5..6` runs in seconds. If a check needs 8 to find anything, the model is
  too detailed.
- When the spec's Grants and Custody sections are written (cascade §4.1 items 4 and
  7), cite this file from them by clause id, and cite the spec from the assertions'
  comments. Two-way links are what make the model an instrument instead of a museum
  piece.

---

## 3. TLA+: how to do it well here

TLA+ earns its place on mechanisms with interleaving and failure: the lease, log
replay across reconnects, discard-on-leave racing a late write, key rotation racing a
read. It does not earn its place on data structure semantics (that is Alloy's job) or on
anything whose bugs are about values rather than orderings (that is proptest's job).

### 3.1 Requirements for a TLA+ model in this directory

1. **One mechanism per module.** `Lease.tla` is the lease. The log-replay model is a
   separate module. If two mechanisms must be composed, write a third, tiny module that
   instantiates both and checks only the cross-property.
2. **Under two hundred lines**, invariants included. Abstract aggressively: a write is a
   pair `<<node, term>>`, not a record with payload; a network is a set of in-flight
   messages, not a queue per link (unless ordering *is* the property).
3. **State the properties first, in prose, at the top of the module**, then the actions.
   If you cannot say the property in a sentence, you do not know what you are
   checking.
4. **Every safety property is an `INVARIANTS` entry in the `.cfg`. Every liveness
   property is a `PROPERTIES` entry** with the fairness it needs stated in `Spec`, and a
   comment saying why that fairness is *justified by the implementation* (a retry loop,
   a timer). Unjustified fairness makes liveness vacuous.
5. **Bound everything explicitly** (`MaxTerm`, three nodes) and say in the `.cfg` why
   the bound suffices. Model checking is exhaustive within the bound and silent outside
   it.
6. **Model failure as ordinary actions**, not as a mode: `Lapse`, `Revoke`, message loss
   as "Deliver never happens." A model whose failures are gated behind a flag gets
   checked without them.
7. **Name actions with the verbs the code will use.** `Claim`, `Lapse`, `Write`,
   `Deliver`, `Revoke` should be the names of the functions (or the events they emit)
   in `soradyne_core`. This is what makes §4 possible.
8. **Deadlock is off** (`CHECK_DEADLOCK FALSE`): a quiescent system is not a bug here.
   If you want "nothing is stuck," write it as a liveness property with the right escape
   clauses, as `Progress` does.
9. **Check it before committing** with `check.sh tla`, and paste the TLC summary
   (states found, distinct states, depth) into the commit message. A model nobody has
   run is prose with more punctuation.

### 3.2 The next model to write: log replay

Before implementing the per-party log stream (cascade §5 step 1), write `LogReplay.tla`:

- Variables: per-writer append-only sequences; per-reader horizon (last seq seen per
  writer); in-flight deliveries; a `left` flag per reader.
- Actions: `Append(w)`, `Deliver(w, r)` (possibly out of order across writers, in order
  per writer), `Disconnect(r)`, `Reconnect(r)` (which replays from the horizon),
  `Leave(r)` (discard).
- Invariants: a reader's view of each writer's log is a prefix of the writer's log (no
  gap, no duplicate); after `Leave`, the reader holds nothing from that stream; a
  derived record's cited horizon is dominated by the reader's horizon at the time it
  was written.
- Liveness: with fair delivery, a connected reader's horizon eventually equals the
  writer's length.

That is the model whose counterexamples matter most for privacy: it is where
"discard on leave" and "late delivery after leave" meet.

### 3.3 Optional: Apalache

If a model needs unbounded terms or symbolic reasoning, Apalache (symbolic model
checker for TLA+) accepts the same modules with type annotations. Do not reach for it
first; TLC at small bounds finds almost everything.

---

## 4. Keeping the code and the models in sync

This is the part most teams skip, and it is why their models rot. Four methods, in
increasing cost. Use the first two always; the third for the lease and the log; the
fourth only if a model outgrows TLC.

### 4.1 Invariants as tests (always)

Every invariant in every model is also a Rust test. Not "inspired by": the *same
predicate*, over the implementation's real state, exercised by property-based testing
(`proptest`) over random action sequences.

Mechanics:

- The implementation exposes a small **abstraction function**: `fn abstract_state(&self)
  -> LeaseState` (or `GrantState`, `LogState`) returning exactly the variables of the
  model, in the model's types (sets, sequences, options). It lives next to the
  implementation, is `#[cfg(test)]` or `pub(crate)`, and is the only place the
  correspondence is written down.
- Each model invariant becomes `fn inv_one_writer_per_term(s: &LeaseState) -> bool`,
  written as a direct transliteration of the TLA+ (comment it with the TLA+ line).
- A proptest driver generates random sequences of the model's *actions* (same names,
  §3.1 item 7), applies them to the implementation, and checks every invariant after
  every step. Shrinking then gives minimal failing action sequences, which read exactly
  like TLC counterexamples.

This catches most divergence for almost nothing, and it forces the abstraction
function to exist, which is the prerequisite for everything below.

### 4.2 Names and vocabulary (always)

Actions in the model are events in the code. When `Claim` in the model becomes
`acquire_lease()` in the code, one of them is renamed. When the spec adds a clause,
the model gets an `assert` and the code gets a test *in the same change*. Reviewers
reject a change to any one of spec, model, or code that does not touch the other two
or say why not.

### 4.3 Trace validation (lease and log)

The implementation emits a **trace**: one JSON line per model-level action, with the
action name, its arguments, and the abstract state after it (from §4.1's abstraction
function). A trace can be taken from a unit test, from the Docker scenarios, or from a
real run on hardware.

A small companion module, `LeaseTrace.tla`, reads the trace (TLA+ Community Modules'
`IOUtils`/`Json` operators) and constrains `Next` so that step *i* must be the action
the trace recorded at step *i*, with the recorded arguments, and that the model's
state after the step must equal the recorded abstract state. TLC then either accepts
the whole trace (the implementation took only steps the model allows, and reached only
states the model predicts) or reports the first step that is not a valid behaviour of
the model.

Why this is worth doing for the lease and the log specifically: their bugs are in
sequences that tests rarely construct (lapse, reclaim under a new term, late delivery
of the old write), and a trace from a chaos run through the Docker scenarios is
exactly such a sequence. Trace validation turns every integration run into a model
check of what actually happened.

Mechanics, in order:

1. Emit traces behind a feature flag (`--features trace`), as JSON lines, from the
   places that implement the model's actions.
2. Write `LeaseTrace.tla` extending `Lease` with the trace-following `Next`. Keep it
   generic: it should work for any trace file.
3. Add `check.sh trace <file>` that runs TLC on the trace module with the file.
4. Run it in the Docker scenario job on the recorded trace, and fail the job on
   rejection.
5. Store one known-good trace per scenario in the repo as a regression corpus.

A rejected trace is triaged the same way as a counterexample: first ask which of
model, abstraction function, or code is wrong. The abstraction function is the usual
culprit early on; the model is the usual culprit once the code has been running for a
while (the model was incomplete); the code is the interesting case.

### 4.4 Model-based test generation (optional)

TLC can enumerate behaviours (`-simulate`, or a `-dump` of the state graph). Those
behaviours can be replayed against the implementation as tests. This is the inverse of
§4.3 and is only worth it if the implementation's own action generator (proptest) is
not reaching the states TLC finds interesting. Try it once for the lease after §4.3
exists; keep it if it finds something.

---

## 5. Where verification does *not* go, and why

- **The CRDT engine**: proptest over random operation interleavings (commutativity,
  idempotence, convergence) plus the Docker scenarios. A proof would be expensive and
  would not cover the horizon-catch-up code paths where the bugs have actually been.
- **Cryptographic handshakes** (pairing today; introduction and per-stream keys with
  rotation later): adopt Noise for pairing and sessions and an MLS-style group key
  scheme for per-stream keys, both with published Tamarin proofs, and model in Tamarin
  only what remains bespoke (the PIN step; revoke-then-rotate). Decide this when
  cascade §5 step 7 is designed; do not write a Tamarin model of a protocol that is
  about to be replaced.
- **Coq / Lean / F\***: no.
- **SDL**: no; state diagrams in the spec cover what it would have.

---

## 6. Checklist for an agent adding a mechanism

1. Is it an ordering-and-failure mechanism? → TLA+ module, per §3.1, *before* code.
   Is it a structural rule about who may hold or read what? → clause in
   `invariants.als`, with a check and a run. Neither? → no model; tests only.
2. Write the abstraction function first. If you cannot say what the model's variables
   are in terms of your struct, the design is not ready.
3. Transliterate every invariant into a Rust predicate and a proptest driver (§4.1).
4. For the lease and the log: emit traces and validate them (§4.3).
5. `check.sh` green, TLC summary in the commit message, spec clause cited from the
   model and the model cited from the spec.
6. Update the table in §1 of this file.
