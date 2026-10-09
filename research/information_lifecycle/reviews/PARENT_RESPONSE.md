# Parent response to the independent reviews

2026-10-09. Both first reports were completed independently against the already-pushed
synthesis `d51cb5024102098d140effaabfacb82423d1c7d3`, with nestbox-ng reference
`ed141df17477c40a4b9365854aa385f82e1d9280`. Neither reviewer read the other's report
or received its findings before completing their own. This response was written only
after both reports were saved. The reviewers were specialist subagents, not external
human certification.

- [Network architecture report](network_architecture.md): conditionally implementable,
  adequate to close; three P2 findings; no new contradiction. The reviewer ran 90
  selected existing tests directly from pinned Git blobs, all passing.
- [Uncertainty propagation report](uncertainty_propagation.md): mathematically sound
  conditional preservation contract; two nonblocking refinements; no new contradiction
  or further research recommended. The reviewer verified all 31 research Python files
  against the pin, ran all 259 existing tests successfully and verified all 15 saved
  source manifests. This was not a new Monte Carlo study or artifact regeneration.

Both independent interpretations are preserved unchanged. Their SHA-256 values at
completion, before parent edits elsewhere:

```text
c190f66636a21d221597efaf6d06cc2d8dae5e2d74d67a0ee6d92f684154b341  network_architecture.md
a9be4fcae99ed8acaa20452dbc22215c1ec03b2136fa1e4c469172f633a62a49  uncertainty_propagation.md
```

## Dispositions

| Finding | Response and immediate change | Limit / remaining work |
|---|---|---|
| NA1: stale publication rejection must outlive retry receipts | Accepted. C7 now separates idempotency from scoped selection order and requires predecessor/supersession/acceptance-frontier state sufficient to reject late first completion or replay after receipt expiry. Shared-flow §5 proposal names this obligation. | No new ordering/GC experiment was run. Baseline replacement semantics and bounded F04 persistence have not been tested as one general concurrent implementation. Exact precondition, receipt lifetime and partial-order policy remain implementation decisions. |
| NA2 and U1: reconstructible clock conversion | Accepted. Both reviewers independently identified the same concrete preservation condition. C1/S05 and the owning time proposal now name original acquisition clock readings, clock domain/incarnation, mapping/model history or equivalent sufficient state, conversion revision, applicability and shared uncertainty. Add `SUBSTRATE.md` §4 T1–T5 as an owning target. | S05 remains deferred-but-not-precluded. This is a conditional retention requirement if reassociation is promised, not a new clock estimator or tested timing envelope. No always-live clock variable is required. |
| NA3: withdrawal expansion and durable acknowledgment ownership | Accepted as a handoff clarification. C6 assigns numerical support/descendant resolution to authorized application roles, opaque generic enforcement to the provider, and translation/ack semantics to the adapter. It distinguishes source-grant change from applied serving invalidation, permits a conservative pending scope barrier and requires lawful recovery of the support index. Owning proposals reflect this. | The complete affected set is still an assumption of the experimental policy gates. A concrete API, rights to the support index, effect point and exact descendant policy remain D1–D4/D6 and implementation work. No cross-network atomic transaction is implied. |
| U2: Gaussian/known-noise qualifiers | Accepted. The short I01/C2 claims now specify linear-Gaussian reduction; C4 specifies known-variance independent isotropic Gaussian noise for the planar study. | Wording was too compressed; detailed derivations/results were already scoped correctly. No result or source changed and no rerun was needed. |

The mathematics report also emphasizes two existing adoption conditions. C2 now names
body-fixed landmark/task-point queries explicitly: pose/structure uncertainty and cross
terms, or a lawful reconstruction path, may be required even if only body poses are
active. It also clarifies that “reduced information” can mean a lossy statistic of all
named observations, not only a subset. These are explanations of the existing declared
query/representation obligation, not new findings, algorithms or scenario coverage.

## Closing judgment

The parent agrees with both conditional verdicts. These small documentation changes
make hard-to-undo preservation and ownership requirements more concrete; they do not
establish a new architectural contradiction or resolve Dana's consent choices. T01/T02
remain explicit. No scenario disposition changed, no new cycle is proposed, no numerical
source/protocol/run was edited, and no owning contract or production code was changed.

Final implementation adoption still requires the appropriate owning contracts and
adapter conformance work. The research is complete once these reports/corrections are
committed and verified remotely and the automation is paused. Publication and pause
receipts belong to [cycle 0015](../cycles/0015.md), not to either independent report.
