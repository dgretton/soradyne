# Inventory app — UX/prompt feedback & physical storage inventory

Two working-session notes for the `apps/inventory` soradyne demo app, captured from Dana's
voice memos (via Talkbox) and re-homed here so the inventory-app work has a design-notes
file of its own — mirroring `giantt-design-notes/`. Nothing is invented; this distills two
deliveries verbatim into an actionable backlog plus a physical-storage inventory to model.

Provenance: Talkbox deliveries `tbx_f70952aae49191d7` ("Inv changes", 2026-06-20) and
`tbx_8111914b3c044cb0` ("Room & storage info updates", 2026-06-03).

---

## Part A — Agent UX / prompt fixes (from "Inv changes", 2026-06-20)

Feedback on the AI-assisted inventory interaction loop. Ordered roughly by leverage.

### A1 — Make the agent decide by DEFAULT
The agent is "super not getting the picture that I just want decisions made for me." It
*will* decide if explicitly encouraged, but it treats deciding as opt-in rather than the
default. **Change the prompt so making a reasonable decision (and stating it) is the default
action**, not asking the user to choose. Only defer to the user when genuinely ambiguous or
irreversible.

### A2 — Bare "items" means put-away intent
When Dana just says "items" (a bare noun with no verb), the intent is **to put those items
away**, not to find/query them. Bake this default reading into the prompt.

### A3 — Confirmation before the irreversible "new conversation" reset
Starting a new conversation "clears the entire context and fully resets everything, and
that's not reversible" — the button is too easy to hit and has bitten Dana mid-task, losing
a lot of work. **Add a confirmation step** (modal / blurred overlay — "Are you sure?") before
resetting. This is the highest-friction, highest-regret item.

### A4 — Container-ID vs item-ID disambiguation
The distinction between a **container ID** and an **item ID** is important and "annoyingly
specific," and the agent keeps conflating them. Add **explicit disambiguation** — e.g. the
agent should recognize which kind of ID is meant (or ask precisely when it can't), and the
prompt should call out the two ID namespaces distinctly.

### A5 — Review the whole conversation history for friction
Dana's meta-ask: review an entire back-and-forth transcript, identify **all** friction
points, and derive whatever prompt changes make the agent behave better. A3/A4 came out of
that; treat the transcript itself as the spec for further prompt tuning.

### A6 — Let the user copy text out of the main chat
Currently can't copy text out of the main chat — "very frustrating." Add copy support.

### A7 — History order + scrolling
Show history **most-recent-first**. Provide a **"scroll to bottom"** affordance where the
bottom is the **oldest** content (i.e. reverse-chronological list, jump-to-oldest).

> Open routing question Dana raised: whether "Inv changes" should become its own Talkbox
> outbox rather than riding the `rim` route. Not an app change — noted for Talkbox config.

---

## Part B — Physical storage inventory to model (from "Room & storage info updates", 2026-06-03)

Storage locations / containers Dana wants the inventory system to know about. Some are
"exist but not a storage location" (structural context); flagged inline. Redwood-cabinet
orientation is "counter orientation"; a couple of items use a "conical orientation" — both
are Dana's own room reference frames.

- **Redwood living-cabinets** — three dark-redwood cabinets on the left of the bed (counter
  orientation). Primary function is to *absorb clutter out of sight*, so arguably **not a
  storage location to allocate into**, but their existence matters. Each ~**30 in wide**, in a
  line (left / right), each with a **vertical divider** inside; some have small compartments.

- **Jewelry box** — "quite nice." Left & right **necklace-holder** sides; **drawers** (incl. a
  little drawer); storage at the **top** (a bunch of little cartons). Currently lives **inside
  the armoire** (left side of its main area; takes up a lot of room). Jewelry itself is
  planned to be stored **behind the jewelry box**.

- **Armoire** — main area divided by a **horizontal shelf** into two areas:
  - **Top area**: a hanging bar; used for **short garments** (cropped / sleeveless things
    that won't drag on the bottom).
  - **Main drawer / lower**: the jewelry box (left side); behind it → planned jewelry storage.
  - **Bottom drawer**: looks like two but is **one big double drawer** — holds **pajamas +
    related** (e.g. athletic shorts).
  - **Platform at the very bottom** the armoire stands on: **sweaters** (+ more).
  - Also stored (in cases): a **travel electric guitar** and (electric) guitar — both still
    in their cases.

- **Two 5-drawer tray sets** — two identical sets of **five squat, tray-like drawers**, each
  ~**10 in wide**, not tall. Currently **stacked** into one ~**2 ft** tower of 10 drawers
  (may not stay stacked). All **allocatable**.

- **Closet cube-shelving** — the area **above the closet shelf** is now filled with two
  assembled bookshelf units (one medium, one large) pushed together. Three rows high (three
  shelving segments); no strict vertical grid, but the unequal divisions happen to give
  **5 across × 3 rows = 15 "cubes"** (Dana dislikes "cube" — prefer "shelving segment").
  Count across rows: 5 top, 5 middle, 5 bottom.

- **Cabinets in the pipeline** (soon, not yet done): **behind-the-door** shelving storage and
  **above the secretary desk** shelving. Only one done now; a couple pending. Dana suggests
  **pretending pipeline cabinets are finished so items can be allocated to them** — with the
  caveat that such items are in an **ambiguous placement state** until the cabinet exists
  (track them as allocated-but-not-physically-placed; a pipeline location could even be
  modeled as a container).

- **Under the secretary desk** — the place for **flat, important boxes** (this is a load-
  bearing definition). Includes an **oversized, very sturdy red box** (protection-grade
  storage) at floor level, just inside the door on the right (conical orientation), butted up
  against the secretary desk. The **stack of tray-like shelves** is currently perched on top
  of that box, also against the desk.

- **Inside the secretary desk** — small shelving on the **right side of the top panel**;
  **sliding doors** let the whole thing push back. A working space "you basically can't
  clear" — a fixed physical arrangement.

---

*Re-homed by the Ghost (T48 Talkbox triage). Working-tree note only — not committed. If this
should live elsewhere (e.g. its own Talkbox outbox, per Dana's routing question above), move
freely.*
