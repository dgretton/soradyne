# Information lifecycle investigation

Status: active research, started 2026-10-07. Production behavior is unchanged.
Branch: `research/information-lifecycle`, based on `shared-flow-demo-contracts`
at `9cbba49`. Owner: this Codex chat, on Dana's instruction.

Investigate which representation, retention, identity and contract choices preserve
future correct withdrawal, recovery and consistent results when evidence is frozen,
replaced or shared. Establish where an answer must become unavailable. Smoothness
alone is not evidence of correctness or physical safety.

**Closing scope, Dana's 2026-10-08 steering:** near-term use is one local network or
Tailscale, with unavailable nodes repaired before sessions. At most four more cycles
after 0011: calibration/withdrawal (I07 + R05), snapshots/reset semantics (I08 + S02),
crash/atomic boundaries (F04), then synthesis and two independent expert reviews.
Skip the old converged-freeze/new-batch 0012. Estimator/controller tuning, broader
nonlinear/SE(3) coverage, R07, F05/F06, S04–S08 stay deferred with their future
requirements recorded. PLAN's closing sequence and STATUS's remaining budget override
old next-action notes and the former full-matrix completion gate.

Start with [PLAN.md](PLAN.md), [METHODS.md](METHODS.md), [STATUS.md](STATUS.md) and
[TENSIONS.md](TENSIONS.md). Every cycle produces a report under `cycles/` and compact
reproducible evidence under `runs/` when an experiment is run. The synthesis/review
cycle preserves documents rather than inventing a new simulation. Dead ends and
experimental code are committed and pushed; deletion comes later, with the conclusion
and reproducer preserved.

## Scope and placement

Dana explicitly requested this cross-project investigation on a new branch of this
repository. This isolated research directory is the bounded exception to the usual
placement of application studies: it is not an extension of the protocol's schemas,
application surface or numerical responsibilities. No production package imports it.
Generic retention/authority findings and application mathematics remain distinct.
Promoting a finding requires the appropriate contract and test in its owning layer.
Do not copy these experimental mathematical routines into production by default.

Read `../../CLAUDE.md`, `../../docs/20260911_shared_flow_demo_contracts.md`,
`../../docs/models/README.md` and `/Users/rim/Dev/BLANKET.md`. Application references
are listed in PLAN.md. The removed Alloy model is not a design authority.

## Reproduce cycle 0001

From the repository root, using Python 3.11+ (stdlib only):

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -v
python3 -B research/information_lifecycle/baseline.py
```

On this machine use `/Users/rim/Dev/.venv-nestbox/bin/python`. Do not change the
system Python. Cycle 0010 and the full test suite now require NumPy and SciPy;
recorded versions are 1.26.4 and 1.17.1 in that existing environment. Earlier exact
witnesses remain stdlib-only. No experiment imports either application's implementation.

## Reproduce cycle 0002

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -v
python3 -B research/information_lifecycle/dynamics.py --output-dir /tmp/information-lifecycle-cycle-0002
```

This writes metrics and complete primary-step CSV traces for the three fixed cases
and all response policies; finer timesteps are summarized in JSON. See the
[pre-run protocol](cycles/0002-protocol.md) and [findings](cycles/0002.md) for assumptions.
Use a fresh output directory for new experiments; do not overwrite a committed run
after changing its method or configuration.

## Reproduce cycle 0003

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -v
python3 -B research/information_lifecycle/stopping.py --output-dir /tmp/information-lifecycle-cycle-0003
```

This writes finite-jerk stopping metrics, independent distance bounds, timestep and
parameter checks, and twelve representative traces. See the [protocol](cycles/0003-protocol.md)
and [findings](cycles/0003.md). All models are synthetic; no physical actuation occurs.
Use a fresh output directory when changing configurations or methods.

## Reproduce cycle 0004

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -v
python3 -B research/information_lifecycle/entitlement_scenarios.py --output-dir /tmp/information-lifecycle-cycle-0004
```

This writes 180 scoped-entitlement scenario metrics and six JSONL files containing
canonical event traces for the admissible candidate and four negative controls.
See the [protocol](cycles/0004-protocol.md) and [findings](cycles/0004.md). Authority,
identities and local atomic policy installation are assumptions, not implemented
distributed guarantees. Use a fresh directory for changed methods/configurations.

## Reproduce cycle 0005

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
python3 -B research/information_lifecycle/shared_information.py --output-dir /tmp/information-lifecycle-cycle-0005
```

This writes 720 exact shared-calibration/lineage outcomes in one JSONL file, aggregate
metrics, two insufficiency witnesses and source hashes. See the [protocol](cycles/0005-protocol.md)
and [findings](cycles/0005.md). Atomic blocks are raw-equivalent in this model; passing
composition does not establish privacy, general feedback handling or hardware safety.
Use a fresh directory for changed methods/configurations.

## Reproduce cycle 0006

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
python3 -B research/information_lifecycle/feedback.py --output-dir /tmp/information-lifecycle-cycle-0006
```

This writes metrics for 240 feedback runs (3,040 event-query checks), ten representative
complete traces and artifact graphs, and wrong-base/damped-feedback witnesses. See the
[protocol](cycles/0006-protocol.md) and [findings](cycles/0006.md). Residual subtraction
is justified only for the declared exact additive model and matching retained base;
its derivative rights are explicit assumptions. Use a fresh directory for changed runs.

## Reproduce cycle 0007

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
python3 -B research/information_lifecycle/observability.py --output-dir /tmp/information-lifecycle-cycle-0007
```

This writes metrics for 120 observability runs (4,800 event-query checks), ten complete
representative traces and source hashes. See the [protocol](cycles/0007-protocol.md)
and [findings](cycles/0007.md). Relative queries can survive absolute-anchor withdrawal;
coordinate pinning supplies no absolute evidence. Exact fractions do not validate
floating-point rank decisions or consumer safety. Use a fresh directory for changed runs.

## Reproduce cycle 0008

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
python3 -B research/information_lifecycle/recovery.py --output-dir /tmp/information-lifecycle-cycle-0008
```

This writes metrics for 960 recovery runs (11,520 event-query checks), 25 complete
representative traces and source hashes. See the [protocol](cycles/0008-protocol.md)
and [findings](cycles/0008.md). Matching durable horizons do not identify unequal live
input histories. A sufficient checkpoint needs explicit coverage and permissions;
missing lawful state causes unavailability. Use a fresh directory for changed runs.

## Reproduce cycle 0009

```sh
python3 -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
python3 -B research/information_lifecycle/expiry.py --output-dir /tmp/information-lifecycle-cycle-0009
```

This writes 1,152 retention/withdrawal run metrics (14,976 query checks), 24 complete
representative traces, an aggregate-insufficiency witness and source hashes. See the
[protocol](cycles/0009-protocol.md) and [findings](cycles/0009.md). Summary granularity,
raw/derivative deadlines and explicit reduced coverage determine what remains usable.
No-pin/no-renewal rules are fixture policies. Use a fresh directory for changed runs.

## Reproduce cycle 0010

```sh
/Users/rim/Dev/.venv-nestbox/bin/python -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
/Users/rim/Dev/.venv-nestbox/bin/python -B research/information_lifecycle/nonlinear_freeze.py --output-dir /tmp/information-lifecycle-cycle-0010
```

This writes 128 deterministic nonlinear freeze configurations (512 method outputs),
eight complete event traces, two quadratic-insufficiency witnesses and source hashes.
See the [protocol](cycles/0010-protocol.md) and [findings](cycles/0010.md). Known exact
landmarks and isotropic noise permit a compact nonlinear sufficient statistic. Local
Gaussian uncertainty still needs qualification; no Monte Carlo or general survey-freeze
claim follows. The output directory must not already exist. Use a fresh directory for
changed methods/configurations. Dependencies: NumPy and SciPy, not GTSAM.

## Reproduce cycle 0011

```sh
/Users/rim/Dev/.venv-nestbox/bin/python -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
/Users/rim/Dev/.venv-nestbox/bin/python -B research/information_lifecycle/angular_coverage.py --phase all --output-dir /tmp/information-lifecycle-cycle-0011
```

This regenerates 131,072 seeded datasets, 786,432 interval evaluations, per-cell exact
confidence intervals, 92 selected trials and a numerical-CDF comparison. See the
[protocol](cycles/0011-protocol.md) and [report](cycles/0011.md) for the original separate
primary/held-out execution, seeds, model-specific coverage derivation and limitations.
The full circular interval is calibrated in this restricted angular model; broad
uncertainty does not establish usable precision or consumer safety. NumPy/SciPy required;
the output directory must not exist. Previous cycle files remain unchanged.

## Reproduce cycle 0012

Cycle [0012](cycles/0012.md), calibration/withdrawal. Reproduce with:

```sh
/Users/rim/Dev/.venv-nestbox/bin/python -B research/information_lifecycle/calibration_contracts.py --output-dir /tmp/information-lifecycle-cycle-0012
```

This stdlib-only slice contains 32 exact drift comparisons, four selection cases,
calibration revision/derivative-policy checks and a correlated-field counterexample.
The directory must be new. STATUS identifies the remaining closing work.

## Reproduce cycle 0013

```sh
/Users/rim/Dev/.venv-nestbox/bin/python -B research/information_lifecycle/snapshot_epochs.py --output-dir /tmp/information-lifecycle-cycle-0013
```

This stdlib-only slice checks eight nested revision/uncertainty combinations, 18 arrival
prefixes, private/public result bindings and exact known/unknown reset semantics. See
the [protocol](cycles/0013-protocol.md) and [report](cycles/0013.md). The output directory
must be new. Two closing cycles remain: F04, then synthesis and the independent reviews.

## Four-hour cycle procedure

1. Read current user steering in the chat, then STATUS, the last cycle, PLAN and
   TENSIONS. Work only on this research branch. Inspect status before any mutation.
   If another branch is checked out, do not switch over someone's dirty work; use an
   isolated checkout of this branch or report the conflict. Never reset/stash others'
   changes, rewrite published history, force-push, merge to main, or retire artifacts
   solely to make tests green.
2. Avoid overlapping research cycles. Create a directory lock at the path returned
   by `git rev-parse --git-path information-lifecycle-cycle.lock` using atomic mkdir;
   record chat/cycle and start time inside. If it exists, establish whether its owner
   is still active before reclaiming it. Never launch a duplicate cycle merely because
   the timer fired. Remove your own lock on completion, including ordinary failures.
3. Fetch the remote research branch. Fast-forward only when clean and appropriate.
   If it diverged, preserve both sides and report or resolve without history rewriting.
   Notice changes to the design base, but do not automatically merge unrelated work.
4. Choose the next authorized item in PLAN's **finite closing sequence**, normally
   45-75 minutes and no more than 90 minutes of work. A small exact witness may finish
   sooner; do not pad it with extra research. Simulations get explicit limits. A cycle
   may end with a counterexample or inconclusive result; neither expands the four-cycle
   budget. After at most three witness cycles go directly to synthesis and reviews.
5. Before running, record assumptions, competing methods, independent reference,
   acceptance thresholds and failure conditions. Implement a minimal test/simulation,
   run appropriate checks, and preserve seed/configuration plus compact raw evidence.
   A candidate that fails is evidence, not a reason to weaken the test.
6. Write `cycles/NNNN.md`: question, methods, exact commands, results, interpretation,
   limitations, files, new/resolved tensions and next smallest action. Update STATUS
   and TENSIONS. Separate observations, mathematical conclusions, hypotheses and
   open product-policy decisions. Do not label toy results hardware-safe.
7. Review the diff for unrelated changes and sensitive inputs. Stage only this
   cycle's research files and necessary documentation links. Commit even negative
   or incomplete experimental work, clearly labeled. Push this branch after EVERY
   cycle and verify the remote tip equals the local commit. If push is unavailable,
   retain the commit, report the failure and retry first next cycle; never claim pushed.
   If Git has no identity configured, use per-command
   `-c user.name=Codex -c user.email=codex@localhost` for commits; do not change global
   identity or impersonate the user.
8. Report meaningful findings, test failures, new material tensions, completion or
   required user action in this chat. Remain quiet on unchanged/non-actionable state;
   durable records still receive the required cycle updates and push. Do not send
   email or messages to other chats. A genuine new architectural contradiction must
   be recorded and reported immediately, not held until cycle end; distinguish it
   from a refinement of T01/T02. No physical actuation or external deployment.

Completion is the narrowed PLAN criterion: push `SYNTHESIS.md` with minimal contracts,
every scenario's scoped disposition (including must-not-be-precluded deferrals), Dana's
open policy choices and proposals for owning contracts. Then launch the two independent
expert review agents, preserve their reports and any separate parent response, push
and verify them, pause `information-lifecycle-investigation`, and notify Dana. Keep
reviews independent through their first reports. No production contract edits or new
research cycles follow merely because a reviewer suggests further work.
