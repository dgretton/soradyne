# Flow Granularity Plan — Assessment of Contradictions and Tensions (General Concepts)

**Date**: 2026-09-06
**Author**: Claude (Fable 5.1, Claude Code), at Dana's request
**Status**: Critique of a provisional plan. Currency **high** (checked against the full
protocol spec, `authorization-model.md`, `app_soradyne_boundary.txt`, and current code);
settled-ness **low** (nothing here is a decision; it is a list of things the plan must
resolve or argue before it is written into the spec).
**Subject**: `20260729_flow_sharing_granularity_takeaways.md` and its transcript
companion — the plan to divide flows more finely ("item is the flow") and to bless a
first-class **collection flow** type.
**Companion**: `20260906_fable_flow_granularity_assessment_giantt.md` (giantt-specific
tensions).

Sources read in full: `rim-protocol/rim-self-data-flows.tex`, `authorization-model.md`,
`app_soradyne_boundary.txt`, both 20260729 granularity documents,
`giantt-design-notes/giantt-dependency-and-timing-semantics.md`, and the relevant code
(`convergent/operation.rs`, `flow/types/drip_hosted.rs`, giantt_core operations and
models).

---

## 1. Contradictions with the protocol spec

### 1.1 The plan redefines the Flow Boundary Principle rather than extending it

The spec (§Flow Boundaries) draws a flow boundary around "the streams and drips that
*must* be fitted back to each other invisibly." Its first guideline, *Atomic unit of
data*, names "an inventory's items" as the canonical example of one flow, and the
inventory example says outright: "The flow UUID stands for one complete inventory."

The plan replaces *fitting necessity* with *shareability or independent lifecycle* as the
boundary determinant. The transcript framed this as "finishing a thought the spec
started," citing the *participant scope* guideline. But the guidelines conflict with each
other on exactly this case, and the plan silently picks one. This is a substitution of
principle, not an extension. The spec text must change, and the change should be argued:
why does shareability outrank atomicity of data?

### 1.2 The coherence-group bookmark reinvents the flow's own definition

Decision 3 reserves a "coherence group" primitive on the collection type for the
per-DOF robot-joint case: many independently shareable flows whose updates must be
frame-aligned. But atomic, invisible fitting across several streams *is* what the spec
says a flow is (the boundary principle above). And `authorization-model.md` already sets
the unit of inter-capsule sharing at the **stream**, not the flow ("Per-stream
granularity: not all participants need the same access").

So the joint-angle case is already solved by the existing design: one flow, one stream per
DOF, per-stream access. Atomizing DOFs into flows and then adding a coherence primitive on
a collection duplicates that. The plan should either drop the bookmark or state why
per-stream auth is insufficient for it.

Corollary worth naming: the plan and the auth model **disagree on the unit of
sharing**. The plan atomizes to flows for shareability; the auth model's unit is the
stream. Note that per-stream auth does *not* deliver per-item sharing in the inventory
case (streams are per-party edit streams, not per-item), so the disagreement does not
dissolve the inventory motivation — but it does dissolve the joint-angle one.

### 1.3 Ensembles are scoped to exactly one capsule

The topology section is explicit: "An ensemble is always a subset of exactly one
capsule … each ensemble draws its membership from a single capsule's authorization
list." Routing (§Routing as Transport Fabric) runs over the ensemble's directed
multigraph.

A flow shared between Dana's capsule and Jaguar's has no ensemble to run on. The takeaways
attribute the blocking dependency to undesigned *auth* only. The transport and topology
layers are equally unprepared — there is no concept of a two-capsule ensemble, a
cross-capsule route, or cross-capsule advertisement. This is a larger gap than the plan
admits, and it should be split out from the auth problem (see §4).

### 1.4 Per-flow configuration does not collapse under multiplexing

The "physical aggregation" argument covers transport connections, sync loops, and journal
files. It does not cover **configuration and discovery**, which the spec makes per
instance (§Flow Configuration: "which parties participate, what network addresses to try,
what storage quotas apply"; plus per-flow delegation, memorization, error, and discovery
policies).

Under intra-capsule use every item-flow's configuration is identical and the cost is
nominal. The moment sharing exists, each item-flow has its own party list — and the
plan's "re-partition" operation *is* editing thousands of party lists. The per-flow
surface is the entire point of splitting; it cannot also be the thing that vanishes.

The transcript's supporting claim that "policies live on the type, not per-instance" is
wrong per the spec: type defines *what streams exist and how they behave in general*;
configuration defines *instance-specific details*.

### 1.5 One-file-per-peer journals collide with per-stream keys

Physical aggregation proposes journals become one file per peer with flow-id-tagged ops.
The auth model scopes key material per stream and requires key rotation on revocation.
Consequences:

- Ops for differently shared flows in one file are encrypted under different keys.
- The file's at-rest policy (plaintext / encrypted / dissolution) must be the strictest of
  any flow inside it, since memorization policy is per flow.
- Revocation-by-rotation of one flow touches a file shared with unrelated flows.
- Horizon tracking becomes per (flow, author) rather than per author; catch-up
  (`operations_since(horizon)`) needs a horizon vector per flow.

All workable as a container of independently encrypted records, but that is not the
simplification described. The "FullReplicaFlow heaviness is an implementation artifact"
claim is only half true: the sync-loop and connection cost is an artifact; the per-flow
key, policy, and horizon surface is intrinsic.

### 1.6 "UUIDs sufficient to bootstrap access" becomes false

The Flow Mesh section promises the protocol will ensure "UUIDs are stable, discoverable,
and sufficient to bootstrap access." Opaque references (Issue A) make *seeing a UUID and
being unable to open it* the designed normal case. The takeaways acknowledge blessing the
mesh as protocol-level but do not flip this sentence. It should become something like:
"a UUID is sufficient to *identify* and *request* access; access itself is governed by
per-stream authorization."

### 1.7 Multi-level references are not addressed

The spec's album example is already two levels deep: album → composite → image, each a
separate flow. The collection-flow blessing must decide whether hint caching and access
transitivity are one level or recursive. "Pre-select everything in the collection you
own" requires a transitive closure across the reference chain, and a dangling reference
can occur at any depth. Not mentioned in the plan.

---

## 2. Internal tensions within the plan

### 2.1 Graduated disclosure is not free; it is an authorization rule

The model is "the item's owner opts specific fields into a collection's cache." But
anyone with *item read access* and *collection write access* can cache any field into
the collection, disclosing it to the collection's audience. Enforcing owner-only hint
writes requires the collection type to know who owns each referent — which is
inter-capsule auth again, now surfaced inside a flow type definition.

Separately, the two rules in Issue D fight each other: the owner's periodic cache refresh
and a collaborator's "chart-local scribble" land on the **same latest-wins field**. Either
the hint needs an owner-write-only sub-field and a separate scribble field, or the
"never written back / provisional placeholder" story needs a conflict rule.

### 2.2 Location as reference and location as field are two sources of truth

The continuity section says moving an item means "re-pointing a reference (and/or setting
a `location` field on the item-flow)." The "and/or" is Issue D (cache discipline) in
another guise: which is authoritative when they disagree?

It also hides an **ownership asymmetry**. Moving my item into Jaguar's workshop collection
means either (a) writing a reference into *his* collection flow, which needs write access
to his flow, or (b) setting a field on *my* item-flow, which he cannot see without item
access. The plan's "moving = re-pointing" sentence only works when one party owns both
the item and the collection.

### 2.3 "No global picture" versus the original motivation

The opening user turn asked for "a holistic sense of mass flow … for a global picture."
Item-is-the-flow gives per-item continuity, but the global picture needs an **index of
all my item-flows**. That index is a root collection flow — which is the old inventory
monolith reborn as a directory. The plan never names it.

Root collections are therefore needed for *discovery of one's own flows*, not just for
sharing. That is a bootstrapping concern the spec currently assigns to "configuration in
on-device storage," and it means the collection type is load-bearing even before any
sharing exists. This should be stated as a decision, not left implicit.

### 2.4 Aggregate queries (Issue E) are partial in a second sense

Issue E accepts that sums over partially shared collections are partial. There is a second
partiality the plan misses: an aggregate over a collection is only as complete as the
collection's *reference set*, which is itself convergent and may be behind. "Total mass
at the workshop" is bounded both by what you can read and by which references have
synced. Minor, but it reinforces that collection state is never ground truth.

### 2.5 Layer violation in the sharing UX

"Pre-select everything in the collection you own, let the user deselect" needs the app to
know **ownership**, which is capsule-level information the Link model
(`authorization-model.md` §3–4) says apps never see. Ownership and authorization state
must surface through the app API as a per-flow attribute (e.g. `owned`, `readable`,
`writable`, `opaque`). That surface is undesigned and is a prerequisite for any
collection UI.

### 2.6 Issue F is stale against the code

Issue F says structured edge elements push against a "largely-scalar `Value` enum." The
enum in `convergent/operation.rs` already has `List` and `Map` variants. Structured set
elements are representable today; `RemoveFromSet` matches by element equality with
`observed_add_ids`, which works for map-valued elements. F is not on the critical path.
(Consequence to note: element identity is full structural equality, so changing one field
of a structured element is remove + add.)

---

## 3. Things the plan gets right and should keep

For balance, the parts that survive scrutiny:

- **Convergence ≠ validity** (Issue B) is correct and important, and the resolution
  (post-materialization validation, never write-time enforcement) is the right one.
- **Opaque references as a hard protocol requirement** (Issue A) is the right shape,
  independent of how auth is eventually designed.
- **Continuity via stable identity** is a real requirement the current design violates,
  and per-item UUID identity is the right fix regardless of whether each item is a
  "flow" in the full spec sense.
- **Physical aggregation of transport and sync loops** is sound; the objection in §1.4–1.5
  is to overclaiming its scope, not to the idea.

---

## 4. What to resolve first

1. **Restate the boundary principle** in the spec so shareability is a legitimate
   determinant, and argue the precedence between the *atomic unit* and *participant
   scope* guidelines.
2. **Name the root collection.** Decide whether "all my item-flows" is a collection flow,
   configuration, or something else, and whether collection is the discovery mechanism
   for one's own data.
3. **Split the "blocking open problem" into three**: inter-capsule auth (keys,
   invite, revoke), inter-capsule transport/topology (ensembles spanning capsules), and
   ownership/authorization state surfaced to apps through the Link-model API.
4. **Decide the unit of sharing**: stream (auth model) or flow (this plan), and reconcile
   whichever document loses. Drop or re-justify the coherence-group bookmark accordingly.
5. **Separate owner-cache from collaborator-scribble** in the collection type's hint
   design, or drop the "disclosure = denormalization" framing.
