# Meeting minutes: prompt reconciliation and inspectable fitting state

**Date:** 2026-10-16. **Status:** adopted design clarification; no runtime or
demonstration implementation in this change. These minutes record only generic
protocol implications. Application mathematics, schemas and consumer behavior remain
outside the protocol. The owning contract is
[shared-flow demonstrations](20260911_shared_flow_demo_contracts.md).

## Retention and use

The local good-faith participation profile intentionally permits defined shared
outputs to retain useful influence from private contributions after those contributions
are removed. History removal is not withdrawal of every downstream influence. Explain
the retained view when participation is authorized; continued use in that scope does
not need a new per-operation authorization prompt or an application numerical Boolean.
Output access still does not grant private input/accounting access or unrelated-flow
disclosure. Output availability also does not establish sufficient representation for
reuse in a fitting; applications own that check.

Memorization and application lifecycle reconciliation enforce which material remains
available. Keeping prohibited material indefinitely behind a per-use application check
is not completion of removal. Types declare which private/derived material must be
removed or transformed and which output/accounting can remain. Soradyne enforces the
declared opaque scopes and does not infer those classifications from payload bytes.

## Prompt eventual cleanup

The profile accepts prompt eventual reconciliation rather than millisecond-precise
erasure. Roughly 5–10 seconds of existing state continuing to participate during a
healthy local cleanup is tolerable; this is not a universal protocol latency promise,
an extension of every stream's deadline, or a guarantee for disconnected custodians.
Declare and measure the selected policy's target and surface pending/overdue duties.
The influence retained in an explicitly permitted shared output has its own lifetime.

A periodic/event-driven memory pass may notice affected material already in protected
storage and remove it. Temporary staging can be simpler than interception before every
write, but must keep its original scope/protection, bounded expiry and cleanup duty.
Retries/repeated arrivals cannot reset an old record's deadline. It cannot enter normal
archival replay or be forwarded as new history, and must not create forwarding/removal
cycles. Immediate removal with no staging is still a valid implementation and should
remain where simpler.

Participation-change acknowledgment installs the grant change and acquisition cutoff.
Removal completion separately acknowledges selected history, accounted pending work
and installed rules for later eligible input. Outstanding custodians remain pending;
application restart does not cancel their obligations. Later transient work is allowed
under those rules and must also be cleaned promptly; completion is not a promise that
there can never again be temporary work. Pending work must not silently become a
permanent retained archive.

At settlement, old fitting attempts, callbacks and saves must not restore removed
material or overwrite reconciled state with an impermissible view. A policy/control
revision check at installation or an equivalent fence is appropriate; no specific
distributed transaction or global stop-the-world barrier is prescribed. Exclusive
publication authority and grant scope still apply. Cleanup tolerance is not permission
to keep sending to withdrawn readers or to create new participation instances.

## Inspectable output versus consumer acceptance

A type may expose current/provisional/partial fitting state and also provide a
consumer-accepted view. Publication authority, permitted disclosure and coherent
revision binding remain mandatory. Quality thresholds need not gate all published
state. The application defines status, validity and what a reader can conclude; the
protocol transports those opaque representations without interpreting them.

For example, a room-monitoring flow can expose a provisional aggregate while a fitting
is still updating, alongside its last accepted aggregate. Consumers can inspect the
current state or request a result meeting their needs. Private sensor histories and
contributor mappings do not follow from access to either output. The application
decides whether a retained value is only an initial guess or a sufficient informative
input; the protocol must not impose identical recovery restrictions on both.

## Contract and demonstration consequences

LC-H/LC-A and the concrete surface now describe prompt reconciliation and bounded
staging. LC-V/LC-R distinguish scoped use permission from application reuse capability.
F6/F11 cover inspection versus accepted views without private disclosure; F9 adds
delayed cleanup, late work, stale save, bounded staging/retry and restart cases. These
are proposed generic fixtures, not passing evidence. No new callable method, payload
field, error type or underlying mechanism is selected by this amendment.

No application-specific numerical logic belongs in these tests. Existing exclusive
authority, per-stream rights, acquisition cutoff, owner archive and independent-flow
rights remain in force. The completed information-lifecycle investigation is not
reopened, and its experiments do not become production proof.
