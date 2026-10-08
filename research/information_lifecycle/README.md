# Information lifecycle investigation

Status: active research, started 2026-10-07. Production behavior is unchanged.
Branch: `research/information-lifecycle`, based on `shared-flow-demo-contracts`
at `9cbba49`. Owner: this Codex chat, on Dana's instruction.

Investigate which simple representations and algorithms preserve the information,
authorization and stability needed by networked estimation when evidence is frozen,
replaced, shared, withdrawn or recovered. Establish where an answer must become
unavailable. Smoothness alone is not evidence of correctness or physical safety.

Start with [PLAN.md](PLAN.md), [METHODS.md](METHODS.md), [STATUS.md](STATUS.md) and
[TENSIONS.md](TENSIONS.md). Every cycle produces a report under `cycles/` and compact
reproducible evidence under `runs/`. Dead ends and experimental code are committed
and pushed; deletion comes later, with the conclusion and reproducer preserved.

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
system Python. Later experiments may use the existing NumPy/GTSAM environment;
record dependencies and versions for each. The initial exact-arithmetic witnesses
do not depend on GTSAM or import either application's implementation.

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
4. Choose ONE bounded hypothesis or scenario slice, normally 45-75 minutes and no
   more than 90 minutes of work. Simulations get explicit step/time/memory limits.
   Prefer the next unfinished highest-risk item, not a broad redesign. A cycle may
   end with a useful counterexample or an inconclusive result.
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
   email or messages to other chats. No physical actuation or external deployment.

Once the matrix has evidence of the specified scope and the synthesis identifies a
minimal approach with explicit limits, document completion, push, notify Dana and
pause the recurring job. Do not manufacture further work. New scenarios may be added
when evidence identifies a real gap; explain why, rather than growing scope by habit.
