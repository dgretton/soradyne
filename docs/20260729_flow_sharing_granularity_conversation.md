# Conversation: Flow Granularity, Selective Sharing, and Extrinsic Relations

**Date**: 2026-07-29
**Participants**: Dana Gretton, Claude (Opus 4.8, Claude Code)
**Purpose**: High-level design discussion working through how selective sharing
between people interacts with the flow abstraction. Verbatim transcript.
Companion analysis and open questions: `20260729_flow_sharing_granularity_takeaways.md`.

> Note on transcript scope: this file records the design dialogue verbatim (user
> turns and assistant turns). The assistant's tool calls between turns (reading
> `docs/INDEX.md`, `authorization-model.md`, `app_soradyne_boundary.txt`, and
> `rim-self-data-flows.tex`) are elided; their substance is reflected in the
> replies. The final turn — the request to save these documents — is summarized
> at the end rather than quoted, since it is procedural.

---

## User (opening)

could you please help me work through some high-level design stuff? I think the flows concept described in the docs is pretty good & relatively extensible, but the big hangup for me is sharing between people. example, in the inventory app, most of the stuff I want to track is mine, but some of the stuff is stored at the rim workshop, and I'd want my friend jaguar to know about the stuff stored there because he's the main collaborator and manager of that space. if I bring my personal stuff over there, i should be able to move it there in the inventory app. but i don't want him/other people I share with to always have complete access to my entire inventory. some subset should be shared. similarly I have a room/living space description in the inventory app (which really should be its own flow... we'll get there) which only applies to me/shouldn't be accessible to others, but still needs to be present/loaded for the agent when contemplating moving something from the rim workshop back into my room. the naive treatment would be to have the union of multiple flows, each with different sharing states/permissions, represent the items, and just delete it from one and add it to the other, but doesn't this lose a lot? the item was not deleted. its continuity is disrupted by this treatment. in reality, it only disappears once it goes in the trash or otherwise out of my control, and a holistic sense of mass flow seems appropriate for a global picture later if I want to see patterns in my use of "stuff." it seems like there's this sort of fundamental conflict where if I have a collection of things in a flow, then sharing one or more items means creating a new flow with the new subset, and committing to updating it with deletions and additions (even if they do carry metadata about where they came from or went to), and if you bring in more parties that have access to various subsets of the items, and to whose items you have partial access, the whole thing starts to feel a little insane, very much like it's the wrong data structure. maybe the move is to define the flow always at the level of granularity where no one would ever be shared on part of it--an indivisibility condition, sort of--and for each one, manage permissions separately. this is much more like how actual physical things work in the real world. this seems like a problem on its surface, though--now I have thousands of flows, minimum, for any serious user of this inventory app? maybe this is completely fine, and I just have to think bigger & prepare/design for a multiplicity of flows. they just already seem like such heavyweight entities, but maybe they aren't really, they're just conceptually heavy but might be computationally light, or batch-able/combine-able as an optimization when implementation is considered, rather than fundamentally separate, requiring their own connections etc. if I did that, I'd need to introduce a whole new theory of sharing flows on top of what exists now? it seems like that must be true but I can't immediately think of what's missing. I think it does hold up when considering other flow types--a photo album is something you share people on as a unit, you don't generally share people on part but not all of an album. you might share each individual photo preferentially though. I think I was already comfortable with the idea of a single image being a flow. maybe I just need to get used to the idea that a single json dict less than a kilobyte can be its own flow. giantt represents an odd case--again I want to be able to share some charts with some people but not others while keeping some private, but they are basically entirely defined by their interconnections with other items, so this is a good stress-test for whatever framing we come up with--the parallel change would be to make each giantt item its own flow with a unique flow id, of course, but what would that mean for charts? maybe a chart is essentially an adjacency matrix for any set of items, each of which is a flow, sort of like a photo album is a collection of references to photos in some order (or not) unique to that album, and I guess you'd just have to make sure that anyone trying to access the meta-flow-type like an album or chart has access to all the pointed-to flows as well? that seems manageable, if a little unwieldy, and easy to implement at least conceptually--mostly in terms of exceptions about access and deciding how to manage them e.g. auto-grant or prompt depending on context... so I guess, is there still a place for plural flows? maybe the plural streams provide enough plurality :p i think this holds up fine for the more real-time types of flows like robotic joint angles/torques etc, it should be fine to manage each of those separately as its own flow, and there's no need to share some but not all of those streams, so, great. or if there were, e.g. for some reason a device has a million tiny pins each with its own controlled degree of freedom and you wanted to share half of it with someone else and keep the other half private... I guess we just hit that complexity when we come to it? I want to be sure the concepts hold up when scaled to impractical lengths, it's more of an exercise. in that case you would want each to be its own flow, I guess, following the pattern from before, but you'd also want a high degree of synchronicity--they shouldn't all be updating async, you'd want a packaged data structure, it's just that its dimensionality/completeness respecting the device might be changeable. I think this holds up, but it does mean there has to be some mechanism for defining synchronicity between closely-coupled flows. maybe this is getting at the theory of shared flows at a higher level i mentioned. anyway could you please look over the docs foremost, since this entire repo is entirely about figuring out the concepts and protocols and initial general implementations/design patterns, and then looking at the example apps keeping in mind that they are to be shaped to match & demonstrate what's going on in the docs/make it concrete, not to guide conceptual development at all really. please be mindful of the way docs have accumulated over time--not all are equally recent or faithful to the current situation. then, I'd be glad if you could walk me through some design discussion.

---

## Assistant (after reading INDEX.md, authorization-model.md, app_soradyne_boundary.txt, and the flows/streams/roles/boundaries/topology sections of rim-self-data-flows.tex)

I've read the three docs that bear on this — the protocol spec (`rim-self-data-flows.tex`, the authoritative one), `authorization-model.md`, and `app_soradyne_boundary.txt` — plus cross-checked against the index's freshness notes. Here's where I land, including where I think you're right, where the docs already agree with you, and the two or three places where you've actually found real gaps that the docs paper over.

### The short version: your two instincts aren't in conflict, they're at different layers

You're circling two ideas that feel opposed — "atomize flows down to the indivisible unit" vs. "but that's thousands of heavyweight flows" — and the resolution is that **one is a statement about the logical model and the other is a statement about the physical realization, and they don't have to match.**

- **Logically**: push the flow boundary down to the finest granularity at which you'd ever want *either* independent sharing *or* independent continuity. That's your indivisibility condition, and the spec already endorses it hard. The Flow Boundary Principle (§Flow Boundaries) literally says: if interconnections *can* be managed by splitting into separate flows and shuttling references in app-natural ways, they *should* be. And the participant-scope guideline says outright: "If not all participants need all data… A mesh of many small flows is often better." You're not fighting the design; you're finishing a thought it started.

- **Physically**: a flow is not obligated to be a heavyweight OS entity. In the *current* code it is — each `FullReplicaFlow` is a directory of journals with its own sync loop and ensemble wiring — but that's an implementation artifact, not the concept. A flow's irreducible logical footprint is a UUID, a type-hash pointer (policies live on the type, not per-instance), and its op journal. Thousands of UUIDs is nothing. The real costs are (a) transport connections and (b) per-flow loops/files, and both collapse under multiplexing: one connection per *peer* already carries everything (`RoutedEnvelope` is basically built for this — you just add a flow-id demux field), one sync loop can service all flows, and journals can be one-file-per-peer with flow-id-tagged ops instead of one-directory-per-flow. This is the same move `transport_architecture.md` makes one level down (TCP multiplexing emulating BLE's OS-managed connection sharing) — you're describing the same optimization one layer up.

So: **logical atomization, physical aggregation.** That's the reconciliation, and it's worth writing down as an explicit principle because it also dissolves your continuity problem.

### The continuity problem dissolves at the same granularity

Your objection to "delete from one flow, add to another" is correct and important: it destroys identity, and it makes the item's mass-flow history a lie. But notice *why* the album example in the spec doesn't have this problem — "move a photo to a new album" is explicitly two flows, and the photo *is its own flow* (a composite). Moving it between albums re-points references; the photo-flow never dies. Its continuity and its history are intact.

Apply that to inventory: the **item is the flow** (the continuity unit). "My room," "rim workshop," and your living-space description are *collection flows* — the album pattern generalized: a convergent set of references to item-flows, plus collection-level metadata. Moving a stapler from your room to the workshop is *re-pointing a reference* (and/or setting a `location` field on the item-flow), **not** delete+create. The item-flow keeps its UUID from acquisition to trash. Global mass-flow patterns fall out for free, because the item-flow *is* the ledger of its own existence across locations.

And critically, because logical flows can be physically batched, "splitting to share" stops being destructive: the workshop-stuff item-flows keep their UUIDs and just migrate from a batched-private partition into an independently-syncable partition. The user-visible "I shared this subset" operation is a *re-partition*, not a data-structure surgery. That's the thing that made the whole scheme feel insane, and it's an implementation detail once the logical model is per-item.

### So is there still a place for plural (multi-item) flows? Yes — as a genuine optimization, not a compromise

Keep something as one multi-item flow when it's atomic on *both* axes: you'd never share a subset **and** the parts have no independent lifecycle. A living-space description you never share and never sub-reference is fine as one flow. The rule isn't "one item per flow always"; it's "split wherever *either* independent shareability *or* independent continuity demands it." Album-level order/captions stay in the album flow; the photos are separate because you *might* share one. Same logic.

### Now the real gaps — where the docs stop and you've walked past them

These three are genuinely under-specified, and they're the "theory of sharing flows" you sensed was missing:

**1. Access transitivity across the mesh.** The spec's Flow Mesh section says flows reference each other by UUID and "the mesh is not a protocol-level construct." That's a cop-out for your case. When you share a collection-flow (or a giantt chart) that references sub-flows, *granting access to the container has to interact with access to the referents*, and the doc is silent on it. You need a real policy primitive here: sharing a collection = share the collection-flow **+ a selected subset of referenced sub-flows**, with the rest handled as **opaque references** — the recipient sees "there's an entry/node here I'm not authorized to open." The protocol requirement (hard) is that *the mesh must tolerate dangling/unauthorized references gracefully*. The auto-grant-vs-prompt default (soft, a product call) is: pre-select everything in the collection you own, let the user deselect. Jaguar sharing the workshop with you is exactly this — some referenced items are his, some yours-but-private, some co-owned.

**2. Where do giantt *relations* live? This is the one place the album model does NOT cleanly transfer, and it's your best stress test — good instinct.** An album's ordering is presentational and self-contained. But `A REQUIRES B` is a semantic edge spanning two item-flows, and you have to *decide* who owns it:
   - **Edge-on-chart** (album model): the chart-flow owns the adjacency matrix; items are bare nodes. Clean for sharing (share the chart, share the edges), but it makes relations chart-*relative* — a "real" dependency has to be replicated into every chart or live in a canonical chart.
   - **Edge-on-item** (giantt's current bidirectional model): the item-flow stores its own edges. Relations are global and intrinsic, but sharing item A without B yields a dangling "requires B" that also *leaks B's existence/ID*.

   I think the honest answer is that giantt has **both kinds** and you should name them: *intrinsic* relations (a true REQUIRES — a fact about the world) belong with the item (or a canonical relation-flow), and dangling is acceptable and even truthful ("this depends on something you can't see"); *presentational* structure (layout, grouping, chart membership) belongs on the chart-flow. Conflating them is what makes giantt feel like it resists the album framing. This is a fork worth resolving explicitly before you build.

**3. Coherence groups (your joint-angles case).** You're right that per-DOF flows want selective sharing but atomic, frame-synchronized updates — and the spec has *no* primitive for this. Drips are convergent-but-slow; jets are fast-but-lossy/non-consensus; neither is "atomic frame across N independently-shareable flows." That's a genuinely new concept — call it a **coherence group** or a synchronization barrier across flows. But I'd argue it's cleanly *deferrable*: nothing in inventory, giantt, or albums needs it (their cross-references are order-independent structural edits), and it only bites for tightly-coupled real-time actuation. Flag it, name it, build it when hardware forces the issue.

### Where I'd push back on you slightly

- **Over-atomization has one real cost**: aggregate queries over a partially-shared collection ("total mass of stuff at the workshop") are inherently *partial* — you can only sum the item-flows you can read. That's honest and probably fine, but worth accepting consciously rather than discovering later.
- The unstated dependency for all of this is the thing `authorization-model.md` already flags as *the* primary open problem: **inter-capsule auth** — per-stream E2E keys, invite/accept, revocation-by-key-rotation. Every "share item X with Jaguar" sentence above is writing checks that layer has to cash, and it's not designed yet. So the sequencing is: (1) commit to logical-atomization/physical-aggregation and the collection-flow pattern (design-only, no new crypto), (2) resolve the intrinsic-vs-extrinsic relation fork for giantt, (3) *then* inter-capsule auth + mesh access-transitivity together, since #1's access story is meaningless without them.

---

Two questions to steer where we go next, since this is your call and it changes what I'd flesh out:

1. On giantt relations — does the intrinsic-vs-extrinsic split feel right to you, or do you want relations to be purely one or the other? That decision cascades into everything about how charts are shared.
2. Do you want to treat "collection flow" (the generalized album) as a **first-class, protocol-blessed flow type** with defined access-transitivity semantics, or keep it as an app-level convention (as the spec currently frames the mesh)? I'd argue for blessing it, because gap #1 above can't be solved purely at the app level — but that's exactly the kind of thing worth deciding deliberately.

---

## User (second turn)

2 is easy--definite yes, let's bless it. I think synchronization can be bookmarked to go here, which is extra reason to make it official. as for 1--it's a tough problem. It's hard to know what framework to apply to judge. giantt items aren't all that meaningful by themselves--they are literally for making relations to other items and don't do much without their referents. but they do have some content, different people may want to edit them & still get a coherent single result (drip), and they should be assumed to be private just like everything else. so, I think they're extrinsic-only. this does open up some funky possibilities like if 2 people think 2 tasks have opposite dependencies, that is, one thinks a has to happen before b but the other thinks b has to happen before a, there would be no way to tell anything was wrong until both were loaded simultaneously, and then it would break. there may even be Boromean situations where any 2 of 3 can be loaded but the 3rd always breaks it. all the more reason to keep the charts lightweight, i think, just storing the relations, so it's light to clone/fork, fix and re-share if it isn't already re-shared automatically. and possibly duration hints could be in charts too so that gui layout can run out ahead of flow sync for responsiveness and navigation, if that isn't too revealing--i think non-revealing metadata in collection flows should be normal & encouraged e.g. last known number of photos/layout hints. what do you think, likely to hit issues?

---

## Assistant (second reply)

Both good calls. Blessing the collection flow gives you a real home for the coherence/synchronization primitive later — the collection is already the thing that *knows the set of related flows*, so "these referents form a coherence group, frame-align their updates" is a policy field that naturally hangs off the collection type. Leave it dormant, but reserve the slot. Good instinct to bookmark it there.

On extrinsic-only relations: I think it's the right call and it's internally consistent, but yes, you'll hit issues — three of them, all manageable, none fatal. The useful thing is to name them now so you don't build features that silently assume they don't exist.

### Issue 1: convergence ≠ validity (the big one)

Your 2-cycle and Borromean examples are real, but I'd reframe *when* they bite. It's not "both charts loaded simultaneously" — each chart loads and validates fine on its own. The breakage only happens if something **unions edges across charts into a combined graph**. So the rule that falls out is:

> There is no canonical dependency graph. "All my tasks with all dependencies" is a *synthesized* chart — a fresh collection unioning edges from many charts — and it may be inconsistent. Treat it as its own validatable artifact, never as ground truth.

But the sharper version of this problem lives *inside a single shared chart*, and extrinsic-only puts it front and center: if you and Jaguar both edit one shared chart, you add A→B, he adds B→A, the CRDT converges to `{A→B, B→A}` — a cycle — and it's *supposed to*. **Acyclicity is a global constraint; it is not expressible as a CRDT invariant.** No set of local ops can enforce it. So convergence will cheerfully produce invalid graphs, and this is inherent, not a bug in your design.

The resolution is the one you're already set up for: acyclicity (and time-constraint feasibility, and orphan detection) are **post-materialization validation**, not write-time enforcement. `giantt_core` already has `GraphDoctor` for exactly this. Extrinsic edges don't create this problem — they just make it the *normal case* rather than an edge case, which is actually good, because it forces validation to be a first-class, always-on part of the fitting rather than an afterthought. And it's the strongest argument for your "keep charts lightweight so they're cheap to clone/fix/re-share" instinct: the fix loop *is* the consistency mechanism. Lean into that.

### Issue 2: derived state goes chart-relative

Giantt's whole value is dependency-driven computation — blocked/ready, critical path, propagated deadlines. If edges are chart-local, then **"is task A ready to start?" has no global answer.** A can be blocked in your chart and ready in Jaguar's. The item-flow can hold *raw* status (done / not-done — that converges, it's content), but *derived* readiness is a property of a chart + its edges, and it's plural.

I think that's acceptable and even correct (readiness genuinely *is* relative to which plan you're looking at), but the app must never present a derived state as global. Concretely: raw status lives on the item-flow; blocked/ready/critical-path are computed per-chart and displayed within a chart's context. Don't put a "blocked" badge on an item in a chart-agnostic list view — there's no truth to it there.

### Issue 3: your duration-hint idea is great, but it's a cache — treat it like one

Two things to keep straight, and then a nice payoff.

**It's denormalization, so it needs cache discipline.** Duration exists authoritatively on the item-flow and as a hint in the chart. They *will* diverge. Rule: chart hint is a *provisional render placeholder*, shown until the item-flow resolves, then replaced — exactly the low-res-image-placeholder pattern. Never authoritative, and **never written back** from hint to item-flow. If Jaguar (who can edit the chart) touches the hint, it's a chart-local scribble, not an edit to your item.

**"Non-revealing metadata" is harder to define than it sounds** — photo count and layout are safe, but a private task's *duration* is borderline (it leaks how long your secret thing takes). So I'd reframe the whole thing, and this is the payoff: **caching a field into a shared collection IS a deliberate, per-field disclosure to that collection's audience.** That's not a footnote — it's a capability that falls out of blessing the collection flow. It gives you sharing granularity *finer than per-flow*:

> Share the chart with `title` + `duration` hints cached in, but grant no item-flow access → Jaguar sees "Foo, ~3 days," can lay it out, can add dependency edges around it, but cannot open the item (notes, status history, the other charts it's in).

So the model isn't "collections hold non-revealing metadata" (undefinable) — it's "the item's owner opts specific fields into a collection's cache, accepting they're visible to that collection's audience." Disclosure = denormalization, decided per-field by the owner. That's a clean privacy story *and* it's your GUI-responsiveness mechanism, same machinery.

### Two smaller flags

- **CRDT set elements get structured.** Extrinsic edges mean the chart's CRDT stores a *set of edge tuples* `{from, to, type}`, not scalars. That nudges against the current `Value` enum (which per the design docs is essentially scalar/untagged). You'll either string-encode edges (`"A>REQUIRES>B"`) or do the "schematized nested Value" work `app_soradyne_boundary.txt` already floated wanting. Not a blocker, but this decision is now on the critical path, where before it was optional.
- **Migration.** Today's giantt has relations *on items*, bidirectional, global. Extrinsic-only means a one-time collapse of those into chart(s) — trivially, dump them all into each user's default/personal chart. Worth writing the migration down, but it's small.

Net: extrinsic-only is sound. The one thing I'd have you internalize before building is that **you are giving up the idea of a single global task graph entirely** — there are only charts, each its own graph, freely inconsistent with the others, reconciled by lightweight fork-fix-reshare rather than by any invariant. If that trade feels right (and given "items aren't meaningful by themselves, they exist to be related," I think you've already accepted it), the rest follows cleanly.

Want to work through what the blessed collection flow's type definition actually contains next — the reference-set, the per-field hint cache, the access-transitivity policy, and the reserved coherence slot — or keep pressure-testing the concepts against another app case first?

---

## User (third turn — procedural, summarized)

Requested that this entire conversation be saved to `docs/` as two separate
documents (this verbatim transcript, and a takeaways/open-questions companion),
that the index be updated to reflect the currency (medium-high) and
settled-ness (low) of these conclusions, and that the results be committed and
pushed (pulling the index first). This document and its companion are the result.
