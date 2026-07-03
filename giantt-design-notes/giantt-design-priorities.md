# Giantt — design priorities & scope decisions

Two standing decisions from a 2026-05-02 design monologue ("Other info for giantt ui").
Most of that recording restated principles already captured elsewhere in these notes; this
file records only the two genuinely-new, load-bearing **decisions** that had no home, with
pointers to where the already-documented principles live so they aren't duplicated.

## Decision 1 — current build priority is EDITABILITY, not layout/visualization polish

The near-term focus is making items easy to create and edit (the AI-assisted
inventory-style interaction loop). Layout, visualization, and the spring engine are the
*end-state* vision but are explicitly a later phase — do not let layout work gate or
precede getting editing solid. This is a phasing call, analogous to "build the simple thing
first, optimize later."

Corollary already in the notes: layout is meant to be a **factored-out, iterable
optimization step** (soft/hard spring constraints), never hard-coded. That architecture is
already specified — see `giantt-technical-specification.txt` → "Layout Engine Principles"
(two-phase: topological schedule resolution, then force-directed Phase 2 spring layout;
"More sophisticated algorithms can be substituted / Interface remains consistent regardless
of algorithm"), and `giantt-features.txt` ("Spring layout engine actively optimizing item
positions"). So: the *design* of layout-as-pluggable-optimization is settled; the
*priority* is that editability comes first.

## Decision 2 — Giantt is a private, personal, whole-life tool; NOT a product for sale

Giantt spans Dana's entire life rather than being bounded to a single project, and it is a
private tool — not something built to sell. Treat scope/feature decisions accordingly
(personal-use ergonomics over market/general-audience concerns; "hosted privately" at most,
per `giantt-features.txt`, never a commercial offering).

The "whole life, fractal cross-cutting charts, not project-bounded" framing itself is
already described in `giantt-overview.txt` and `giantt-features.txt`; what this records is
the explicit **not-for-sale / private** scope decision, which those docs only implied.

## Rich relation types — already documented (no action, cross-reference only)

The monologue also emphasized rich relation types (supercharge, AND/OR blocks). These are
already specified and need nothing new:
- `≫` SUPERCHARGES — `giantt-notation.md` (Relations).
- AND blocks = `⊢` REQUIRES; OR blocks = `⋲` ANY / "ANYOF" — `giantt-notation.md` plus
  `giantt-technical-specification.txt` ("REQUIRES (AND relations)" / "ANYOF (OR relations)").

---
Provenance: re-homed from a Talkbox voice memo (delivery 03f80a93, 2026-05-02) during a
cross-project backlog triage. Original audio/transcript lives in the Talkbox vault under its
delivery id.
