# Meeting minutes: participation and retained shared results

**Date:** 2026-10-15, America/New_York.  
**Participants:** Dana and Codex.  
**Status:** agreed direction for a good-faith local shared-flow profile; not an
implementation report. Generic provider implications only. Application mathematics
and recovery strategies belong to the application.

## Decisions

- Preserve user autonomy with understandable leave and optional history-removal
  operations. Private inputs do not become public because they were used in shared
  work. The sharing explanation can authorize a defined shared output to survive
  removal of the underlying inputs and personal participation history. This is a
  profile, not a universal permission for every derivative in every flow type.
- The retained output may serve current and future authorized participants in that
  flow; it does not authorize onward disclosure into another flow. No repeated
  consent prompt is needed for already authorized output use. Owner archives and
  unrelated independently authorized uses remain unaffected. Prior plaintext is not
  promised remotely erasable.
- Rejoining does not silently restore historical authorization. Explicit backfill
  resubmits authorized records with their original opaque application evidence IDs.
  Grant generations, delivery attempts, and participation sessions do not create new
  evidence. Applications declare replacement and support; the provider preserves
  those references without interpreting their numerical meaning.
- Minimal restricted support/accounting metadata may outlive personal history while
  retained artifacts depend on it or old submissions remain admissible. Output
  readers do not gain access to input provenance or participant mappings. Minimize
  debugging metadata. Token/key mechanisms remain undecided; hashing a known UUID
  alone does not prevent linking it to the original.
- Under the initial good-faith profile, a participant acquires no new input after
  leaving. With a usable leave timestamp, admit only input acquired at or before it;
  without one, accept received input subject to the application's normal checks.
  Presence loss alone is not leave. Late permitted input must not recreate removed
  history; temporary processing is governed by that removal policy.
- Distinguish participation-change acknowledgment from completed history removal.
  Completion waits for obligated custodians; pending removal is visible. Retained
  authorized output may remain available. An application restarting does not undo
  provider authorization or custody obligations.
- Applications may initialize an empty fitting promptly and defer restoration of
  numerical state. A later clarification allows controlled reuse of retained state
  when the application establishes its sufficiency and dependence on new inputs;
  it must not become apparently new independent evidence. Permission to retain,
  read, or publish output alone does not establish that suitability. If recovery
  excludes affected information, it also excludes retained components whose
  independence from that information cannot be established. Generic storage does
  not choose the application's restart or replacement strategy. Distinct evidence
  identifiers alone do not certify independence; the provider preserves opaque
  application accounting without interpreting it.
- Unknown support/dependence is an explicit application rejection, not evidence of
  independence and not a new provider error. Acquisition of useful new input and
  rebuilding are application control decisions. The provider must not manufacture
  support, widen rights, or interpret application data to make a rejected input fit.

## Follow-up

Amend the current shared-flow contracts in generic terms, including proposed
conformance examples and explicit implementation gaps. Leave the completed research
evidence and its historical scope intact. No new experiment or automation resumption
is requested. Local good-faith semantics do not settle malicious participants,
expulsion, offline freshness, partitions, or distributed completion mechanisms.
