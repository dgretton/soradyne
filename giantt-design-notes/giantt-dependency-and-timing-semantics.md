# Giantt — dependency, timing and chart-scope semantics

Source: Talkbox delivery `tbx_51b24f3f578231b1` ("jaguar and Dana review giantt items PDF",
2026-05-02) — a ~2 h working session in which Dana and Jaguar walked the full item PDF
(`giantt_backup_05-01-2026.pdf`) chart by chart. Most of that session is RIM project substance;
this file distills only the **giantt design content**, verbatim-faithful.

Companions: `giantt-design-priorities.md` (G3 — philosophy, relation richness, layout as a
factored-out optimization), `giantt-pdf-and-desktop-app-spec.md` (G1 — PDF export, anchored vs
floating, backdated completion), `giantt-notation.md` (the concrete syntax these rules author into),
`giantt-features.txt` and `giantt-technical-specification.txt` (the existing model). Where a rule
below is already served by the model, that is said explicitly rather than restated.

## 1. Dependency placement: attach at the LATEST feasible point

The rule, stated by Dana in the session and then used to settle several disputes in it:

> "Gantt charts have a pattern of dependencies, and the literal, actual, like — the *latest possible
> point that you can do something* is where the dependency goes. So if it's possible to do the
> outline without doing the wagon, it will not be a dependency."

So a dependency edge asserts **necessity**, not sequence-in-practice, and not preference. The test
is counterfactual: *can the successor be done at all without the predecessor?* If yes, no edge —
even when in real life you would obviously do them in that order.

Worked case from the session: transporting the demo hardware ("the wagon") is a real precursor to a
fully integrated dry run, but it is **not** a prerequisite for writing the talk outline. Making it
one would have modelled procrastination as structure. The transport question was instead placed as
a dependency of the *village* build-out and of the integrated run, not of the talk.

Consequence for chart shape, also stated explicitly: a milestone ends up with **"a big pile up of
things — a bunch of little trees that all slam into it right at the end, and most of them are
independent,"** with only a short serial string leading in (here: integrated run → practice pack-up
and unpack). Long serial chains are usually an authoring error under this rule.

## 2. Chart scope: "anything that we can't not do"

The inclusion test for an item, in Dana's words: *"this is a Gantt chart, and we can't not do that
thing before the talk — that's what the Gantt chart is for. Anything that we can't not do."*

The negative case is as sharp as the positive one. A weekly Instagram posting habit was proposed and
rejected: *"it's a thing that I wanted to create a habit around… that's not a giantt item. That's
just a thing you want to do."* Habits and hygiene are out; only what gates a goal is in. (Recurring
constraints do exist in the notation via `@@@ every(...)`, so the exclusion here is a *judgement
about what belongs in a goal chart*, not a claim that the model cannot express recurrence.)

## 3. Times go only on things that genuinely have times

> "Hard things that have, like, exact time that they definitely happen — that's the only things we
> put times on."

Everything else derives its position from dependencies and durations. This sharpens, rather than
contradicts, `giantt-features.txt`'s "there shouldn't be any concrete moment in time that anything
is supposed to happen precisely": genuine external deadlines *are* the exception. In the session the
hard-dated set was small and explicit — Lunabird (~July 1), Hope (conference Aug 14–17), the fashion
show — against dozens of undated items.

### 3a. Lead-ups are DURATION BLOCKS, never dates — and this is a requirement, not a style note

> "If they're, like, *we need to give ourselves enough time to do blank*, that is the kind of thing
> you don't want to put a time on directly in the event. We want to set that as, like, a five-day
> lead-up that's just, like, *have a five-day lead-up*. Because otherwise, if you change something
> later and you're like, oh, we need to put something inside the five-day lead-up, **it won't flow
> correctly**."

The failure mode being avoided is concrete: a lead-up encoded as a fixed date is rigid, so inserting
work into it later either overruns silently or forces a manual re-date of everything around it. A
lead-up encoded as an item with a duration absorbs the insertion and pushes its own start earlier.
Authoring form: an item with a duration that BLOCKS the dated event, rather than a `due(...)` on the
preparation work itself.

Worked case: "Lunabird should be prepared by June 27th" was *not* recorded as a June 27 deadline on
the preparation items; it was recorded as a five-day lead-up block feeding the Lunabird event.

### 3b. Uncertainty blocks

For something known to be bounded but not scheduled — *"I know it's not going to be earlier than
June 27th"* for driving the art truck — Dana wanted **"a little block that represents some
uncertainty,"** and noted this is *"a level of detail we haven't hit yet, and I like that."* Treat
this as a wanted feature, not an existing one: an item whose position carries an explicit
uncertainty span rather than a false point estimate. (Related but not the same as the model's
existing `window(...)` constraint and `earliest_start`/`latest_end` window, which bound a *task*'s
execution rather than expressing *ignorance about when an external event lands*.)

## 4. Two parallel paths to one objective — one critical, one nice-to-have

The mag-adapter discussion produced a shape worth naming. The same objective had a fast path (a
phone case with a plastic clip-in retention feature, cannibalising existing leaf-spring contacts —
buildable in a weekend) and a better path (custom pogo-pin connectors made with a manufacturer,
~2 months). The resolution was **not** to pick one: the fast path was put on the critical path to
Hope, and the better path was kept as a parallel "nice to have" track that continues past the
milestone. The notation's `⋲` ANY (alternate paths) covers the "either satisfies" case; what this
adds is that the two branches can carry *different criticality and different horizons*.

The bronze-ring casting item behaved the same way, with **four** enumerated strategies (outsource to
two manufacturers for test casts; castable-resin printed tree, test cast, then final tree printed at
JLC by June 1; dial in the Artisan's Asylum resin printer and solder own wax trunks; CNC with an
undercutter, kept as an unloved fourth). Four alternates, none deleted, one being pursued.

## 5. Promotion: an item that grows becomes its own chart

> "Mag adapter is a whole Gantt chart." … "The mag-adapter chart will extend beyond Hope. It'll be a
> thing that has dependencies to Hope, but it'll also be something that extends beyond that."

So a chart is not merely a tag for filtering: a sub-chart may have edges *into* a parent chart's
milestone and also a life beyond it. This is consistent with `giantt-features.txt`'s explicitly
non-containment, fractal chart model, and is a concrete instance of it. The counterpart authoring
smell is the reverse: *"we need to break it down more than 'make the mag adapter'"* — a single item
standing in for a program is what triggers promotion.

## 6. Deletion vs demotion: the aspiration chart, and the roadmap horizon

Repeatedly during the walk, an item was neither kept nor deleted but **moved to a future/aspiration
chart** — *"I can move it to, like, rim future aspirations or something, so it's not in the same
critical path as all the other stuff."* The stated purpose: keep the working chart to the things
under active weekly review while not losing the idea.

The related concern, in Dana's words, is horizon. A long-range roadmap should be reflected in the
chart *"so we're not just always running on a two-month time window, but things are kind of grounded
in a bigger framework."* Three-year aspirations belong in the record, but not in the weekly view.

Actual deletions in the session were of a different kind — items that were **badly phrased rather
than unwanted**: an over-specific "set up Etsy shop, five pieces, ten of each, photo ready" and an
"Instagram reels" item were deleted in favour of better-scoped replacements, and an item whose text
had degenerated into semicolons ("Port Giant Rim") was deleted as unreadable. Deletion here means
*re-author*, not *abandon*.

## 7. Duration estimation is a conversation with the tool

> "It'll just guess how much time it takes to do things. But if we tell it, it'll be better."

This validates the AI-assisted-entry premise in `giantt-overview.txt` from live use. The session also
showed why the guesses matter: the honest estimate for a talk outline was *"actually like four
hours… it can be spread across a few sessions"* against a felt sense of a week, and the CAD rework
was re-estimated upward mid-conversation from two hours to "a whole evening." Estimates were argued
about because they change what the chart says is possible — *"it's worth discussing, because that's
how long it takes to do stuff."*

## 8. Relation types confirmed in live use

`≫` SUPERCHARGES was reached for unprompted, twice, and both times specifically to *avoid* a false
requirement: microcontrollers-in-jewelry "supercharges" Hope rather than blocking it (*"it is not a
strict requirement"*), and the aesthetic reference deck supercharges the designer application rather
than gating it. Existing model, but the usage is evidence that the distinction between *requires*
and *enhances* is the one users actually reach for under time pressure.

---
*Distilled by the Ghost agent, cycle c687 (2026-09-02), from the Talkbox delivery named above.
Working-tree write only; not committed. The RIM project substance from the same session is triaged
separately and is not duplicated here.*
