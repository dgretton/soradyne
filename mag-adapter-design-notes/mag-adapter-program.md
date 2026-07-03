# MagAdapter — engineering program & build-order decision

A working-session decision (Jaguar + Dana) on the two SD-card **MagAdapter** designs and the
order to build them. Captured from a Talkbox voice memo and re-homed here so the mag-adapter
program has a design-notes file of its own — mirroring `giantt-design-notes/` and
`inventory-design-notes/`. Nothing is invented; this distills the recording verbatim-faithful
into the decision plus the context that motivates it. The note's own disposition was that this
"should become giantt items under a mag-adapter chart" — so treat this as roadmap material to
be turned into giantt entries.

Provenance: Talkbox delivery `tbx_6ae7dd2d957caeda` ("Fashion show artist showcase / mag
adapters / timelines / short-term soradyne development goals", 2026-06-21). NB a second,
malformed raw copy of this same recording sits un-delivered in the rim Talkbox `new/` queue
("fashion show artist showcase mag adapters sync while Jaguar had to pee really bad.md") — it
is the **same session** (a filler-word-heavier raw transcript, no frontmatter), not a separate
earlier delivery. Earlier triage listed it as a distinct item "R5"; it is a duplicate of "R6".

---

## The problem MagAdapters solve

The product is SD cards mounted inside jewelry ("rims"). To use an SD card's data you must
either pull it out of the jewelry (not easy, and the rims currently protect/frame the card but
don't let you access the data — so today they're mostly aesthetic) or use an adapter that
reads the card **while it stays in the jewelry**. So each jewelry piece ideally ships with a
cable/adapter that lets you use the card in place.

There's a second, deeper need: the intended data model is **dissolution & crystallization** —
K-of-N secret-sharing across multiple SD cards (e.g. "3 of 5 required to recover"). To dissolve
data for the first time across five cards you want all five connected at once; to reconstruct
you want at least K connected at once. Serial access (insert card, do a step, swap, repeat —
"like loading a game off floppies") is technically possible but inconvenient, unreliable, and
at best a retro gimmick. So the real requirement is **connecting multiple SD cards to one host
at the same time** — which is exactly what a MagAdapter is for.

## Two MagAdapter designs (Jaguar's single concept split into two)

1. **Dumb / copper-only / metal-only MagAdapter** — no chip, no smarts. Physically connects
   the metal contacts of the SD card to the metal of an SD-card-compatible host, e.g. via a
   ribbon-cable extender designed for this (modify an off-the-shelf one, or make our own —
   it's mechanically simple). The whole challenge is **mechanical engineering**: get it to
   seat reliably into the designed rim, reach the right contacts, and make dependable
   electrical contact. Hard, but within reach. Does NOT require electrical-engineering skill —
   only enough to know "conductive things conduct."

2. **Smart / chip / multiplexed MagAdapter** — a real controller chip that lets **many** SD
   cards attach to one host at once. Jaguar's vision: business-card-sized, sticks to the back
   of a phone (possibly onto the Apple MagSafe magnet — "mag" may be for both "magnetic click"
   and MagSafe), with many slots so total capacity is ~10 TB (a 1 TB card per slot, or more).
   Realistically this is a USB buffer circuit feeding a shared serial bus (which is what USB
   is), with a device per SD card tapping in. A full EE effort — not out of reach for an
   experienced electrical engineer ("does it in their sleep"), but out of reach for us without
   either a long time-sink or a contract.

## THE DECISION — build order: **dumb → smart → fuse** (reverse of the prior plan)

Jaguar's earlier plan: contract the **smart** adapter first; fall back to the dumb one only if
smart fails ("build smart, stop if it doesn't work out"). **New decision (this session): do it
the other way around.**

- **Dumb first.** Contract a **mechanical engineer** (the memo names/uses "Emmy" as the
  archetype) to deliver the dumb adapter as its own standalone task. Low EE knowledge needed.
- **Smart in parallel / next.** Have an **EE** design the chip: a board with a bunch of normal
  SD-card slots (minimal, "make it look like a credit card"), delivered as its own project.
- **Then fuse.** When (whensoever) *both* succeed, the two contractors work together to build
  the smart adapter **out of** the dumb one — reuse the dumb adapter's mechanical design.

Framing that makes this the right order: don't structure it as "do the big job, and if you
can't, fall back to a smaller job." Structure it as "**do the small job, then as a natural
add-on finish the bigger job.**" Ship value early and let the hard version extend the easy one.

## Why dumb-first matters commercially (the Hope / fashion-show timing)

- We want an adapter — probably the **dumb** one — ready for **Hope** (the event before the
  fashion show), so that when we sell the SD-card jewelry we can confidently include a nice
  adapter that makes the card easy to use in place.
- Ideally by the **fashion show** the **smart** adapter is also ready to sell. If it is, we can
  truthfully demo dissolution/crystallization — encrypted, redundant, lose-a-piece-keep-your-
  data storage — as **technology that works today**, and pitch "buy multiple jewelry pieces and
  do dissolution/crystallization now."

## Messaging constraint (a real worry, drives the above)

Do **not** oversell technology that doesn't exist yet. The nightmare: someone tunes out, tunes
back in on "lose one card, don't lose your data," buys it, then asks "how does it work?" and the
honest answer is "we can't actually do that yet." That sets up disappointment for buyers and for
us. So: only pitch the dissolution/crystallization story around the jewelry **if the adapters are
actually ready**. If they're not, sell the jewelry with (at least) the dumb adapters, and keep
the future tech clearly framed as a **demo** — e.g. a runway model wearing deliberately thicker /
light-up "clearly-a-prototype" devices that ARE wirelessly networked, explicitly "this is where
we're going," not a product you can buy.

## A fallback option Jaguar dismissed (recorded for completeness)

Skip the smart adapter by leaning on the fact that every networked device you already own
contributes to the K-of-N total: a MacBook's two USB ports (two dumb adapters), a phone with an
SD slot plus a USB port, etc. — all connected over the network, so you can dissolve/crystallize
across all of them at once. It could push N reasonably high, but it's a lot of setup and only
partially sidesteps the smart adapter (it doesn't actually *solve* the "many cards, one device"
problem the way a real adapter does). Jaguar didn't want to go there; noted, not chosen.

---

## Cross-references (the rest of this same recording, homed elsewhere)

This recording was a multi-topic working session. The two non-mag-adapter threads are tracked
separately and are NOT duplicated here:

- **Fashion show / artist-showcase** (Armory vs Crystal Ballroom venue; long twisty catwalk;
  pillars-as-feedback entry; "tacking"/tactile-comm demo; ≤12 models; bring-your-own-pillow):
  already currency-checked against Dana's `rim-obsidian` vault — the venue move (Crystal
  Ballroom → Arts at the Armory) is already homed and more current in her own vault; the
  "colors vs pillars / count" question is held for Dana. This mag-adapter note does not touch
  the rim vault.
- **Short-term soradyne software priorities** (prioritize the microcontrollers / ESP32 boards;
  get reliable read/write to large-capacity SD cards past the FAT32 / small-capacity ceiling;
  write the storage layer **modularly** — abstract read/write/file interfaces, dummy fast
  stubs now — favoring modularity over raw efficiency so bigger-card support drops in later;
  networking is for *storing/syncing* data on-body, not sync-for-its-own-sake): this is
  soradyne dev-roadmap material and overlaps the open "soradyne terminology / near-term dev
  goals" thread. Left for that thread rather than re-stated here.
