# Flow Granularity Plan — Assessment of Contradictions and Tensions (Giantt-Specific)

**Date**: 2026-09-06
**Author**: Claude (Fable 5.1, Claude Code), at Dana's request
**Status**: Critique of a provisional plan. Currency **high**; settled-ness **low**.
**Subject**: Decision 2 of `20260729_flow_sharing_granularity_takeaways.md` — giantt
relations are **extrinsic-only** (edges live on the chart, a collection flow, not on
item-flows) — together with Issues B, C, F, and G and Open Question 5.
**Companion**: `20260906_fable_flow_granularity_assessment_concepts.md` (general
protocol-level tensions; read first).

Sources read in full for this part: `giantt-design-notes/giantt-dependency-and-timing-semantics.md`
(new, 2026-09-02, untracked at time of writing), `giantt-features.txt`,
`giantt-technical-specification.txt`, `giantt-design-priorities.md`, and giantt_core
code (`models/relation.dart`, `models/chart.dart`, `operations/giantt_operations.dart`,
`storage/flow_repository.dart`, the `commands/` directory).

---

## 1. Extrinsic-only edges contradict giantt's own edge semantics

The new semantics note, distilled from the May 2026 review session with Jaguar, defines a
dependency edge as **necessity**:

> "the latest possible point that you can do something is where the dependency goes …
> The test is counterfactual: *can the successor be done at all without the
> predecessor?* If yes, no edge."

That is a fact about the world. It is precisely the transcript's own definition of an
**intrinsic** relation ("a true REQUIRES — a fact about the world"), which the plan parks
under Open Question 5 as a hypothetical to "revisit if a use case demands." Giantt's
authoritative authoring rule makes *every* REQUIRES edge one.

The justification offered for extrinsic-only — "giantt items aren't meaningful by
themselves, they exist to be related" — argues that edges are *essential to items*. That
cuts toward intrinsic (the edge is part of what the item is), not away from it.

The honest options are:

- **(a)** Keep extrinsic-only and explicitly accept that a necessity edge is a per-chart
  *claim* that may be absent or contradicted elsewhere — i.e. giantt charts are
  *opinions about* necessity, not records of it. The semantics note would need a
  corresponding caveat.
- **(b)** Adopt the transcript's first proposal: intrinsic necessity edges on (or
  canonically alongside) the item-flow, presentational structure on the chart. Accept the
  dangling-edge and ID-leak consequences the transcript named.
- **(c)** Both, with a type distinction: `REQUIRES` intrinsic; `SUPERCHARGES` and
  ordering/grouping extrinsic. The semantics note's §8 (users reach for SUPERCHARGES
  precisely to *avoid* asserting necessity) suggests the two edge kinds already differ
  in exactly this way.

Option (c) is the one the evidence points to. Decision 2 should be reopened with the
semantics note in hand.

## 2. The migration reintroduces the global graph the plan renounces

The plan's stated trade: "you are giving up the idea of a single global task graph
entirely." Issue G's migration: collapse all existing on-item relations "into each user's
default/personal chart."

For a single user, that personal chart *is* the global graph under another name. The
CLAUDE.md and current tooling talk about "the live Giantt graph"; `giantt blocked`,
`giantt deps`, `giantt doctor`, and `giantt sort` operate on the whole graph today.

The plan does not say what happens when a **second chart is created**:

- Are edges copied into it? Then the same necessity fact is duplicated and will drift.
- Are they referenced from the personal chart? Then the personal chart is canonical, and
  intrinsic relations are back by construction.
- Are they re-authored from scratch? Then the semantics note's necessity rule has to be
  re-applied per chart, by hand, for every edge.

The intrinsic-versus-extrinsic question re-enters through this door. It cannot be
answered by migration alone.

## 3. Overlay is giantt's primary view; the plan makes it the inconsistent edge case

`giantt-features.txt` describes charts as **non-containing tags**: items belong to many
charts, charts are "sort of fractal-y," and the core interaction is to "open up multiple
of them at the same time and see them overlaid on each other so I can see how crazy it is
to do them at the same time or if they mesh or parallelize neatly." It adds an
**algorithmic requirement**: "The critical path should always be simple to see."

Under the plan, overlay is a **synthesized union** of per-chart edge sets that "may be
inconsistent" and must be "treated as its own validatable artifact, never as ground
truth." The plan is internally consistent, but it demotes the product's primary view to
its designated failure mode, and it makes the critical-path requirement chart-relative
across sets that may contradict each other.

The semantics note makes this sharper. Its §5 (**promotion**) describes the normal
pattern: "The mag-adapter chart will extend beyond Hope. It'll be a thing that has
dependencies *to* Hope." Cross-chart edges are routine, not rare. Under extrinsic-only,
every such edge lives in exactly one chart, and the plan never states:

- whether an edge endpoint that is not a chart member becomes one (membership by
  reference), or whether charts routinely contain edges to items "not in the chart";
- which chart owns an edge whose two endpoints are each in a different chart.

Chart membership and edge-endpoint set are conflated in the plan; the semantics note
shows they diverge in practice.

## 4. Derived state versus current tooling

Issue C (blocked/ready/critical-path are chart-relative; never show a "blocked" badge in
a chart-agnostic list) is a correct consequence of extrinsic-only edges. But it is in
direct tension with the current CLI, where `giantt blocked` and `giantt list` are
chart-agnostic, and with `giantt-features.txt`'s request to "sort the items by … urgency"
across the whole set. Either those commands become chart-scoped (and a default chart is
required), or Issue C is softened to "derived state is computed over the union, labelled
as such." The plan should say which.

## 5. Migration is small but not trivial

Issue G calls the migration "small" and "trivial." Checking against the code:

- **Identity**: item IDs today are human-readable strings (`task_1`, `mag-adapter-cad`),
  keyed by the parser and referenced by users and the AI chat surface. Item-as-flow
  requires UUID flow identity. Either the human ID becomes a field on the item-flow with
  a lookup, or every reference in existing charts, logs, and notation changes. This is a
  user-visible identity change, not a data reshuffle.
- **Bidirectionality**: relations are stored on *both* endpoints
  (`giantt_operations.dart` emits one `AddToSet` per relation per item; the graph layer
  auto-creates inverses). Collapsing into an edge set requires deduplicating each pair
  into one `{from, to, type}` element and deciding what to do with asymmetric leftovers
  from partial syncs.
- **Membership inversion**: chart membership is a `charts` set on the item. The plan
  inverts it: the chart holds references to items. The old field either goes away (and
  the "which charts is this item in?" query becomes a scan over all collection flows) or
  stays as a reverse-index cache with the same discipline problems as hints.
- **Time constraints and logs** are on items and are unaffected — but `LogEntry`
  occlusion and the include/occlude file system assume whole-graph files, which no
  longer exist.

None of this is hard. All of it should be written down before "trivial" is asserted.

## 6. Issue F is stale

Issue F ("structured CRDT set elements … pushes against the current largely-scalar
`Value` enum … now on the critical path") is factually out of date: `Value` already has
`List` and `Map` variants. An edge `{from, to, type}` is a `Map` element today. The
remaining design choice is only whether edge identity is structural equality (change of
type = remove + add) or whether edges get their own stable IDs so their fields can be
edited in place. The former is simpler and matches informed-remove semantics.

## 7. Hints on charts: two writers, one field

Dana's proposal to cache `duration` (and possibly `title`) into charts "so that gui
layout can run out ahead of flow sync" is good, and it aligns with
`giantt-design-priorities.md`'s "layout as a factored-out optimization." But on a shared
chart, two writers converge on the same hint field: the owner refreshing the cache from
the item-flow, and a collaborator adjusting the estimate ("Duration estimation is a
conversation with the tool," semantics note §7). Latest-wins picks one silently. The chart
type needs either separate `hint` (owner-refreshed) and `override` (collaborator-set)
fields, or an explicit rule that hints are read-only to non-owners.

## 8. What survives and what to decide

**Survives**: Issue B (validation after materialization, never at write time) is right
and `GraphDoctor` is the right home. Keeping charts lightweight enough to fork-fix-reshare
is right regardless of where edges live. Per-item stable identity is required regardless.

**Decide, in order**:

1. Reopen Decision 2 with the semantics note: are `REQUIRES` edges necessity facts
   (intrinsic) or planning claims (extrinsic)? The evidence favours a split by relation
   type.
2. Define chart membership versus edge endpoints, and which chart owns a cross-chart
   edge.
3. State what the personal/default chart *is* after migration: canonical graph, or
   just one chart among many.
4. Decide whether chart-agnostic CLI commands become chart-scoped or union-scoped.
5. Write the migration down with the identity change made explicit.
