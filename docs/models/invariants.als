/*
 * rim consolidated model — structural invariants C1..C9
 *
 * Source of truth: docs/20260906_fable_soradyne_cascade.md §3 ("The consolidated
 * model"). This file is the executable form of §3.2. If this model and that prose
 * disagree, one of them is wrong; fix the prose or the model in the same change.
 *
 * What is modelled: capsules, pieces, flows, streams (raw vs derived, with summaries
 * as a special derived kind), records and what they cite, roles and who holds them,
 * custody (who holds a copy of which record), grants (to a capsule or to a role),
 * introductions, and one transition: revoking a grant.
 *
 * What is deliberately NOT modelled: time, transport, encryption, the app surface
 * (C8), application validity (C9), and the content of records (C6 is a rule about
 * what a derivation may contain, which is not structural). Those are covered by
 * prose and by conformance tests, not by this file.
 *
 * Dialect: plain Alloy (works in Alloy 5 and 6; no temporal operators). Run with
 * docs/models/check.sh, or open in the Alloy Analyzer and Execute All.
 *
 * Status: written 2026-09-06 without a local Alloy install. The first run on a
 * machine with Java must confirm that every `check` reports no counterexample and
 * every `run` finds an instance. If a check FAILS, read the counterexample before
 * touching the model: it is more likely to be a real hole in §3 than a modelling slip.
 */

module rim/invariants

-- ---------------------------------------------------------------------------
-- Static structure
-- ---------------------------------------------------------------------------

abstract sig Kind {}
one sig Log, Jet, Drip, Query extends Kind {}

sig Capsule {}

sig Piece { capsule: one Capsule }

sig Flow {
  owner: one Capsule,
  refs:  set Flow            -- the mesh: this flow references those flows
}

-- No flow references itself, directly or through others. Mutual nesting is exactly
-- the double-counting loop the estimation load forbids (parent ingesting a child
-- that ingests the parent), so it is ruled out structurally.
fact ReferencesAreAcyclic { no f: Flow | f in f.^refs }

abstract sig Stream {
  flow:   one Flow,
  kind:   one Kind,
  origin: one Capsule        -- the capsule whose party writes this stream
}

-- Raw streams carry observations and edits written by a party: logs, jets, and the
-- per-party keyed drip. Derived streams carry what a role's fitting produced: the
-- role-written drip, projections, public caches, query answers. A Summary is a
-- derived stream whose audience is another flow that references this one.
sig Raw extends Stream {}
sig Derived extends Stream {}
sig Summary extends Derived {}

fact StreamShape {
  -- a derived stream is written on behalf of the flow's owner
  all s: Derived | s.origin = s.flow.owner
  -- query-response streams are served by a role, hence derived
  all s: Stream | s.kind = Query implies s in Derived
  -- raw streams are logs, jets, or drips
  all s: Raw | s.kind != Query
}

-- ---------------------------------------------------------------------------
-- Records and citations (C5)
-- ---------------------------------------------------------------------------

sig Record {
  on:     one Stream,
  author: one Piece,
  cites:  set Record         -- the horizon a derived record was computed from
}

fact RecordShape {
  -- a record is authored by a piece of the stream's origin capsule
  all r: Record | r.author.capsule = r.on.origin
  -- C5, structural half: raw records have no field for a citation
  all r: Record | r.on in Raw implies no r.cites
  -- a derived record cites only raw records of its own flow, or summary records of
  -- flows its own flow references. Never another derived record of its own flow
  -- (that would be an estimator consuming its own output as evidence).
  all r: Record | r.on in Derived implies
    all c: r.cites |
      (c.on in Raw and c.on.flow = r.on.flow) or
      (c.on in Summary and c.on.flow in r.on.flow.refs)
}

-- ---------------------------------------------------------------------------
-- Roles
-- ---------------------------------------------------------------------------

sig Role {
  of:     one Flow,
  reads:  set Stream,        -- inputs the role's fitting consumes
  writes: set Derived        -- outputs the role's fitting produces
}

fact RoleShape {
  all ro: Role | ro.writes.flow = ro.of
  all ro: Role | all st: ro.reads |
    st.flow = ro.of or (st in Summary and st.flow in ro.of.refs)
}

-- ---------------------------------------------------------------------------
-- Grants
-- ---------------------------------------------------------------------------

-- A grant opens one stream to one grantee. The grantee is either a capsule (all of
-- its pieces, equal access within a capsule) or a role (whoever currently holds it).
sig Grant {
  on:     one Stream,
  toCap:  lone Capsule,
  toRole: lone Role
}

fact GrantShape {
  all g: Grant | one (g.toCap + g.toRole)
  -- a role grant is only meaningful for a role that reads that stream
  all g: Grant | some g.toRole implies g.on in g.toRole.reads
}

-- ---------------------------------------------------------------------------
-- State: what varies when grants come and go
-- ---------------------------------------------------------------------------

sig State {
  grants:     set Grant,
  holder:     Role -> lone Piece,    -- lease: at most one holder per role
  holds:      Piece -> Record,       -- custody: who holds a copy of what
  introduced: Capsule -> Capsule     -- which capsules may exchange grants at all
}

-- Which capsules are parties of a flow: its owner plus everyone writing a raw
-- stream into it. Role holders are NOT parties by virtue of the role; they reach
-- raw streams only through role grants, which is what lets a role be revoked
-- without leaving dangling capsule-level access behind.
fun parties[f: Flow]: set Capsule {
  f.owner + (flow.f & Raw).origin
}

-- Can capsule c read stream st in state s?
pred canRead[s: State, c: Capsule, st: Stream] {
  c = st.origin
  or (some g: s.grants | g.on = st and g.toCap = c)
  or (some g: s.grants | g.on = st and some g.toRole and (s.holder[g.toRole]).capsule = c)
}

pred Wellformed[s: State] {
  -- introductions are symmetric and never reflexive
  s.introduced = ~(s.introduced)
  no iden & s.introduced

  -- grants only ever cross capsules; inside a capsule access is implicit and total
  all g: s.grants | some g.toCap implies g.toCap != g.on.origin

  -- a cross-capsule grant requires a prior introduction
  all g: s.grants | some g.toCap implies (g.on.origin -> g.toCap) in s.introduced
  all g: s.grants | some g.toRole implies
    (all p: s.holder[g.toRole] | p.capsule = g.on.origin or (g.on.origin -> p.capsule) in s.introduced)

  -- C3, reference half: a capsule that is not a party of a flow may be granted only
  -- its derived streams. Raw streams are shared only among parties (contributors
  -- sharing with each other) or with roles.
  all g: s.grants | (some g.toCap and g.on in Raw) implies g.toCap in parties[g.on.flow]

  -- a summary is readable only by the owner of a flow that references its flow, or by
  -- a role of such a flow
  all g: s.grants | g.on in Summary implies
    ((some g.toCap and some f: refs.(g.on.flow) | f.owner = g.toCap) or
     (some g.toRole and g.on.flow in g.toRole.of.refs))

  -- delegation eligibility (authorization coverage): a role holder can read every
  -- input of the role
  all ro: Role | all p: s.holder[ro] | all st: ro.reads | canRead[s, p.capsule, st]

  -- C1: origin custody — the author always holds its own record
  all r: Record | r.author -> r in s.holds

  -- replica custody implies authorization: nobody holds a copy they may not read
  all p: Piece, r: Record | (p -> r in s.holds and p != r.author) implies canRead[s, p.capsule, r.on]
}

-- ---------------------------------------------------------------------------
-- Transition: revoking a grant
-- ---------------------------------------------------------------------------

-- Revoking a capsule grant: the grantee's pieces lose replica custody of that stream,
-- and any role held by one of its pieces that read the stream is dropped (it is no
-- longer eligible). Revoking a role grant: the current holder loses the role and its
-- replicas of that stream.
-- Roles dropped by revoking g in state s: for a capsule grant, every role read by a
-- piece of that capsule whose inputs include the stream; for a role grant, that role.
fun droppedRoles[s: State, g: Grant]: set Role {
  { ro: Role | (some g.toCap and g.on in ro.reads and (s.holder[ro]).capsule = g.toCap)
            or ro = g.toRole }
}

pred revoke[s, s': State, g: Grant] {
  g in s.grants
  s'.grants = s.grants - g
  s'.introduced = s.introduced
  -- C2, lease half: dropped roles lose their holders
  s'.holder = s.holder - (droppedRoles[s, g] -> Piece)
  -- C2, custody half: the grantee discards replicas of the revoked stream, and a
  -- dropped role's holder discards replicas of everything it held only as that
  -- role's holder. Origin custody (author == holder) is never touched: C1.
  s'.holds = s.holds -
    { p: Piece, r: Record | p != r.author and r.on = g.on and
        ((some g.toCap and p.capsule = g.toCap) or (some g.toRole and p in s.holder[g.toRole])) } -
    { p: Piece, r: Record | p != r.author and
        (some ro: droppedRoles[s, g] | p in s.holder[ro] and r.on in ro.reads) }
}

-- ---------------------------------------------------------------------------
-- Assertions: the invariants of §3.2 that are structural
-- ---------------------------------------------------------------------------

-- C1: no revocation reaches origin custody.
assert C1_RevokeKeepsOriginCustody {
  all s, s': State, g: Grant |
    (Wellformed[s] and revoke[s, s', g]) implies
      all r: Record | r.author -> r in s'.holds
}

-- C2: after revocation, the former grantee holds no replica of the revoked stream.
assert C2_RevokeDiscardsReplicas {
  all s, s': State, g: Grant |
    (Wellformed[s] and revoke[s, s', g]) implies
      (some g.toCap implies
         no p: capsule.(g.toCap), r: on.(g.on) | p != r.author and p -> r in s'.holds)
      and
      (some g.toRole implies
         no p: s.holder[g.toRole], r: on.(g.on) | p != r.author and p -> r in s'.holds)
}

-- Revocation leaves the system well-formed: no dangling eligibility, no orphaned
-- custody, no grant that now lacks its justification.
assert RevokePreservesWellformed {
  all s, s': State, g: Grant |
    (Wellformed[s] and revoke[s, s', g]) implies Wellformed[s']
}

-- C3, grant half: nobody outside a stream's origin capsule holds a copy of one of
-- its records without an explicit grant in force.
assert C3_NoReplicaWithoutGrant {
  all s: State | Wellformed[s] implies
    all p: Piece, r: Record |
      (p -> r in s.holds and p.capsule != r.on.origin) implies
        (some g: s.grants | g.on = r.on and
          (g.toCap = p.capsule or (some g.toRole and p in s.holder[g.toRole])))
}

-- C3, reference half: a capsule that is not a party of a flow can never read one of
-- its raw streams by a capsule grant. Only roles reach raw streams from outside.
assert C3_OutsidersReadDerivedOnly {
  all s: State | Wellformed[s] implies
    all c: Capsule, st: Raw |
      (c != st.origin and c not in parties[st.flow] and canRead[s, c, st]) implies
        (some g: s.grants | g.on = st and some g.toRole and (s.holder[g.toRole]).capsule = c)
}

-- C5: no record is in its own causal past. With acyclic references and the citation
-- rules, an estimator can never be fed its own output.
assert C5_NoCitationCycles {
  no r: Record | r in r.^cites
}

-- Delegation eligibility survives as a consequence of well-formedness: every holder
-- of every role can read every input of that role.
assert EligibilityHolds {
  all s: State | Wellformed[s] implies
    all ro: Role | all p: s.holder[ro] | all st: ro.reads | canRead[s, p.capsule, st]
}

check C1_RevokeKeepsOriginCustody for 5 but 2 State
check C2_RevokeDiscardsReplicas   for 5 but 2 State
check RevokePreservesWellformed    for 5 but 2 State
check C3_NoReplicaWithoutGrant     for 6 but 1 State
check C3_OutsidersReadDerivedOnly  for 6 but 1 State
check C5_NoCitationCycles          for 6 but 1 State
check EligibilityHolds             for 6 but 1 State

-- ---------------------------------------------------------------------------
-- Runs: the model must admit the situations the design is for
-- ---------------------------------------------------------------------------

-- A shared scope: two capsules each write a raw stream into one flow; a third
-- capsule's piece holds the estimator role via role grants on both raw streams; the
-- estimator writes a summary; a parent flow owned by a fourth capsule references the
-- scope and its owner is granted the summary and nothing else.
pred SharedScopeWithNesting[s: State] {
  Wellformed[s]
  some f, parent: Flow, ro: Role, a, b: Raw, sum: Summary |
    f in parent.refs and f.owner != parent.owner and
    a.flow = f and b.flow = f and a.origin != b.origin and
    sum.flow = f and
    ro.of = f and a + b in ro.reads and sum in ro.writes and
    (some p: s.holder[ro] | p.capsule not in parties[f]) and
    (some g: s.grants | g.on = sum and g.toCap = parent.owner) and
    (no g: s.grants | g.on in (a + b) and g.toCap = parent.owner) and
    (some r: Record | r.on = sum and some r.cites)
}
run SharedScopeWithNesting for 6 but 1 State

-- An opaque reference: a flow references another whose owner has not granted it
-- anything, so the referencing owner can read none of its streams.
pred OpaqueReference[s: State] {
  Wellformed[s]
  some f, g: Flow | g in f.refs and f.owner != g.owner and
    (no st: flow.g | canRead[s, f.owner, st])
}
run OpaqueReference for 4 but 1 State

-- A revocation actually happens and something is discarded by it.
pred RevocationWithEffect[s, s': State, g: Grant] {
  Wellformed[s]
  revoke[s, s', g]
  s.holds != s'.holds
}
run RevocationWithEffect for 5 but 2 State
