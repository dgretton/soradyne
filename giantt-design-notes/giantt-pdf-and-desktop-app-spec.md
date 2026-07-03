# Giantt — PDF export, export-groups & the desktop (Tauri) app

Feature spec dictated 2026-05-01 ("giantt pdfs and app"). It records feature asks that
have **no home** in the existing notes; where a concept is already documented, this file
points there instead of restating it (so nothing is duplicated).

Framing decision Dana stated up front: **what she actually needs is the desktop app** — the
PDF is "super cool" but the real target is a live editor that exports the PDF. So read the
PDF-model section below as the *export target*, and the app section as *where it lives*.

## 1. Multi-chart PDF export (the central new ask)

Nothing in the current notes covers PDF export at all — this is new. Giantt must be able to
produce a PDF (and, more generally, timing views) that **incorporate multiple charts at
once**. The model:

- **Export group** = a named list of charts you want to look at together (a "collection of
  decisions about what goes on a page"). This reuses the existing *chart-group* concept
  (already in `giantt-features.txt` "Phone app gui": chart-group cards, each with a time
  horizon start/end) — but here the group drives **page composition for export**, not just
  the on-screen card carousel. NB: Dana flagged that "multi-chart" is an overloaded word
  ("that's a type of flow") — prefer **export group** for this page-layout sense.
- **Merged rendering**: when you render a PDF for a group, all charts in the group appear
  with all their items **fully expanded and interleaved/merged together** on the page
  ("literally just interleaved on top of each other").
- **Default behavior** (nothing specified): merge *all* items from *all* charts in the group
  at the same time. Dana expects this to be rare/only-sensible for small groups (two or
  three charts).
- **Vertically-compressed cross-chart dependency hint**: an alternate view where one chart is
  rendered **really compressed at the top of another** (items become "long, skinny lines"),
  so you can *see that dependencies exist between charts* — i.e. which chart feeds
  dependencies into which — **without** showing which specific items they are (that's too
  much information; the goal is just to judge inter-chart coupling at a glance). Charts that
  are always rendered together must carry a **label**; the individual items in the squished
  chart do not need labels.

## 2. Item-level model additions (needed for good exports)

- **Anchored vs floating items**: a distinction between items that are *anchored in place* on
  the timeline and items that are *floating*. "Doesn't have to be crazy" — just enough to
  express it. (No equivalent today: `giantt-notation.md` time constraints like `window`/`due`
  are about deadlines, not anchored-vs-floating placement.)
- **Backdated completion**: when an item is completed, it must be **possible to specify when
  it was completed** so it can sit in the past on the timeline. Rationale: having every
  completed item "clutter up right at the present moment is silly." (Today the model has a
  `COMPLETED` status — `giantt-notation.md ●`, tech-spec status enum — but no notion of a
  completion *date* placing it in the past.) Corollary annoyance to fix: items **too far in
  the past currently have no indication**. Related to the existing occlude mechanism
  (`giantt-features.txt`: past items can be occluded) but distinct — this is about *placing
  and indicating* a completed item in time, not archiving it out of context.

## 3. The desktop app — Tauri, MS-Project-like, PDF export, recode from scratch

"If we're being real, what we really need is the giantt PDF *app*" — a **desktop app**.

- **Stack: Tauri** ("Tari" — she loves it). This matches the repo's own direction: the
  soradyne `CLAUDE.md` already states "The upcoming Tauri app will use `soradyne_core` as a
  plain Rust crate with no FFI overhead." So this spec *confirms and prioritizes* that app.
- **Shape: a stripped-down Microsoft Project.** A live editor where items are
  **movable/draggable around** directly, but with a **feature set limited to what giantt
  actually supports** (don't inherit MS-Project's full complexity). This aligns with the
  already-documented desktop GUI (`giantt-features.txt` "Desktop gui": drag handles on item
  rectangles to modify durations) — the new part is *packaging that as a standalone Tauri app*
  whose headline capability is PDF export.
- **PDF export is a first-class feature of the app**: everything in §1–2 above should work,
  but **live** — move things around, then export. Existing PDF examples can be used as a
  visual target for what the output should look like.
- **Build approach: recode from scratch.** Dana's explicit call: "I think recoding it from
  scratch is going to be a really good idea" (rather than growing the existing artifact into
  the app). Consistent with the Giantt Python→Dart/Rust port already underway per the repo
  CLAUDE.md.

## Cross-references (already documented — no action, pointer only)

- Chart-group cards + per-group time-horizon metadata → `giantt-features.txt` "Phone app gui".
- Desktop drag-to-resize/direct-manipulation editing → `giantt-features.txt` "Desktop gui".
- Tauri app on `soradyne_core` as a plain Rust crate → soradyne repo `CLAUDE.md` (Guiding
  constraint / Phase notes).
- Editability-first phasing (build editing before layout polish) → `giantt-design-priorities.md`
  Decision 1 (this app work fits that priority: a live editor, not visualization polish).

---
Provenance: re-homed from a Talkbox voice memo (delivery 95a0f94f, "giantt pdfs and app",
2026-05-01) during a cross-project backlog triage (workspace task T48). Original
audio/transcript lives in the Talkbox vault under its delivery id. Pairs with
`giantt-design-priorities.md` (G3, same triage).
