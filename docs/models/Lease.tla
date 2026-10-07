---------------------------------- MODULE Lease ----------------------------------
(*
 * The exclusive-role lease of the rim consolidated model
 * (docs/20260906_fable_soradyne_cascade.md §3.1 item 6, §5 step 4; estimation
 * requirements §4 "lease semantics").
 *
 * One flow, one exclusive role (the estimator / aligner), a set of nodes that may
 * be eligible, a lease with a monotonically increasing term, and a role-written
 * register that accepts a write only if it carries the current term and comes from
 * the current holder. Holders may lapse (crash, go out of range, let the TTL expire)
 * at any time. Eligibility may be revoked at any time.
 *
 * Properties checked:
 *   SingleHolder        at most one holder at a time (by construction; stated anyway)
 *   HolderEligible      a holder is always an eligible node
 *   OneWriterPerTerm    all accepted writes of one term come from one node
 *   NoStaleAccept       an accepted write carries the term that was current when it
 *                       was delivered; a write from a former holder is rejected even
 *                       if that node later reclaims the lease under a new term
 *   Progress (liveness) if some node stays eligible and keeps claiming, the lease is
 *                       eventually held (under weak fairness on Claim)
 *
 * This is the seed model described in docs/models/README.md. Keep it small.
 * When the implementation exists, keep it in sync by trace validation (README §4),
 * not by growing this file to mirror the code.
 *
 * Status: written 2026-09-06 without a local TLC. First run must confirm that all
 * invariants hold and that Progress holds with the fairness in Spec.
 *)
(* Scope clarification, 2026-09-11: one abstract holder is assumed here.
 * This does not model distributed agreement, partitions, membership changes,
 * physical clock bounds, or safety of direct query responses. See README.md.
 *)
EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS Nodes,      \* the parties that might fill the role
          MaxTerm,    \* bound on lease terms, for finite model checking
          None        \* model value: "no holder"

ASSUME None \notin Nodes

VARIABLES
  holder,     \* current lease holder, or None
  term,       \* current lease term (0 = never held)
  eligible,   \* nodes currently eligible for the role
  pending,    \* set of in-flight writes <<node, term>>
  accepted    \* set of writes the register has accepted

vars == <<holder, term, eligible, pending, accepted>>

TypeOK ==
  /\ holder \in Nodes \cup {None}
  /\ term \in 0..MaxTerm
  /\ eligible \subseteq Nodes
  /\ pending \subseteq (Nodes \X 0..MaxTerm)
  /\ accepted \subseteq (Nodes \X 0..MaxTerm)

Init ==
  /\ holder = None
  /\ term = 0
  /\ eligible \in SUBSET Nodes
  /\ pending = {}
  /\ accepted = {}

\* An eligible node claims the lease when nobody holds it. The term advances.
Claim(n) ==
  /\ n \in eligible
  /\ holder = None
  /\ term < MaxTerm
  /\ holder' = n
  /\ term' = term + 1
  /\ UNCHANGED <<eligible, pending, accepted>>

\* The holder lapses: crash, TTL expiry, out of range. In-flight writes stay in flight.
Lapse ==
  /\ holder # None
  /\ holder' = None
  /\ UNCHANGED <<term, eligible, pending, accepted>>

\* The holder issues a write stamped with its term.
Write ==
  /\ holder # None
  /\ pending' = pending \cup {<<holder, term>>}
  /\ UNCHANGED <<holder, term, eligible, accepted>>

\* A write is delivered to the register, which accepts it only if it carries the
\* current term and the sender is the current holder. Everything else is rejected.
Deliver(w) ==
  /\ w \in pending
  /\ pending' = pending \ {w}
  /\ IF w[2] = term /\ w[1] = holder
       THEN accepted' = accepted \cup {w}
       ELSE accepted' = accepted
  /\ UNCHANGED <<holder, term, eligible>>

\* Eligibility is revoked (a contributor withdrew a grant, or the node's standing
\* changed). If the node held the lease, the lease lapses with it.
Revoke(n) ==
  /\ n \in eligible
  /\ eligible' = eligible \ {n}
  /\ holder' = IF holder = n THEN None ELSE holder
  /\ UNCHANGED <<term, pending, accepted>>

Next ==
  \/ \E n \in Nodes : Claim(n)
  \/ Lapse
  \/ Write
  \/ \E w \in pending : Deliver(w)
  \/ \E n \in Nodes : Revoke(n)

\* Weak fairness on claiming: an eligible node that can claim eventually does.
Spec == Init /\ [][Next]_vars /\ \A n \in Nodes : WF_vars(Claim(n))

--------------------------------------------------------------------------------
\* Invariants

SingleHolder == holder \in Nodes \cup {None}   \* one variable, hence at most one

HolderEligible == holder # None => holder \in eligible

OneWriterPerTerm ==
  \A w1, w2 \in accepted : w1[2] = w2[2] => w1[1] = w2[1]

\* Every accepted write was accepted under its own term, which is <= the term now.
NoStaleAccept == \A w \in accepted : w[2] <= term

Safety ==
  /\ TypeOK
  /\ SingleHolder
  /\ HolderEligible
  /\ OneWriterPerTerm
  /\ NoStaleAccept

--------------------------------------------------------------------------------
\* Liveness: whenever the lease is unheld, it is eventually held again, unless every
\* node has been revoked or the finite term bound is exhausted. (Stating it without
\* the two escape clauses is wrong: revocation can legitimately empty the eligible
\* set after the antecedent held, and TLC would report that as a violation.)

Progress ==
  [](  holder = None => <>(holder # None \/ eligible = {} \/ term = MaxTerm)  )

================================================================================
