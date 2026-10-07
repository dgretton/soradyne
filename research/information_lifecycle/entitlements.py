"""Cycle 0004: trusted, synchronous research model, not an authorization service."""

from dataclasses import dataclass
from fractions import Fraction as Q

Scope = tuple[str, str, str]  # flow, audience, purpose; one fixed local materializer
MODES = (
    "scoped",
    "purge_on_revoke",
    "ignore_scope",
    "sticky_summary",
    "trust_historical_grant",
)


@dataclass(frozen=True)
class Record:
    identity: str
    value: Q
    measurement_time: int


@dataclass(frozen=True)
class Summary:
    identity: str
    dependencies: frozenset[str]
    precision: Q
    information: Q


@dataclass(frozen=True)
class Grant:
    identity: str
    generation: int
    kind: str
    resources: frozenset[str]
    operations: frozenset[str] = frozenset({"retain", "infer"})

    def __post_init__(self):
        # A frozen dataclass alone does not freeze caller-owned mutable containers.
        object.__setattr__(self, "resources", frozenset(self.resources))
        object.__setattr__(self, "operations", frozenset(self.operations))
        if (
            not self.identity
            or self.generation < 1
            or self.kind not in ("raw", "summary")
            or not self.resources
            or not self.operations
            or not self.operations <= {"retain", "infer"}
        ):
            raise ValueError("Invalid grant")


@dataclass(frozen=True)
class Result:
    scope: Scope
    policy_revision: int
    store_revision: int
    represented: frozenset[str]
    missing: frozenset[str]
    mean: Q | None
    variance: Q | None
    status: str


class Ledger:
    """Policy issuer, immutable origin identities and summary contents are trusted.

    Only retained payloads contain numerical evidence. Historical grant metadata
    survives deletion, but cannot admit payloads in the scoped candidate. Output
    disclosure, distributed ordering, source authentication and adversaries are absent.
    """

    def __init__(self, mode="scoped"):
        if mode not in MODES:
            raise ValueError("Unknown candidate")
        self.mode = mode
        self.policies: dict[Scope, tuple[int, dict[str, Grant]]] = {}
        self.history: dict[tuple[Scope, str, int], Grant] = {}
        self.raw: dict[str, Record] = {}
        self.summaries: dict[str, Summary] = {}
        self.store_revision = 0

    def permits(self, scope, kind, resource, operation):
        policies = (
            self.policies.values()
            if self.mode == "ignore_scope"
            else [self.policies.get(scope, (0, {}))]
        )
        return any(
            grant.kind == kind
            and resource in grant.resources
            and operation in grant.operations
            for _, grants in policies
            for grant in grants.values()
        )

    def retained_somewhere(self, kind, resource):
        return any(
            self.permits(scope, kind, resource, "retain") for scope in self.policies
        )

    def install(self, scope: Scope, revision: int, grants: tuple[Grant, ...]):
        if revision < 1 or len(grants) > 16:
            raise ValueError("Invalid revision or grant budget exceeded")
        proposed = {grant.identity: grant for grant in grants}
        if len(proposed) != len(grants):
            raise ValueError("Duplicate grant identities")
        old_revision, old = self.policies.get(scope, (0, {}))
        if revision <= old_revision:
            if revision == old_revision and proposed != old:
                raise ValueError("Conflicting policy at one revision")
            return "stale"
        for grant in grants:
            previous_generations = [
                generation
                for s, identity, generation in self.history
                if (s, identity) == (scope, grant.identity)
            ]
            previous = old.get(grant.identity)
            if previous == grant:
                continue
            if previous_generations and grant.generation <= max(previous_generations):
                raise ValueError("Grant change/re-grant needs a new generation")
        # Mutate only after all policy validation. This is one local atomic event.
        self.policies[scope] = revision, proposed
        for grant in grants:
            self.history[scope, grant.identity, grant.generation] = grant
        before = set(self.raw), set(self.summaries)
        if self.mode == "purge_on_revoke":
            for identity, grant in old.items():
                if grant.kind == "raw" and proposed.get(identity) != grant:
                    for resource in grant.resources:
                        self.raw.pop(resource, None)
        self.raw = {
            key: value
            for key, value in self.raw.items()
            if self.retained_somewhere("raw", key)
        }
        if self.mode != "sticky_summary":
            self.summaries = {
                key: value
                for key, value in self.summaries.items()
                if self.retained_somewhere("summary", key)
            }
        if before != (set(self.raw), set(self.summaries)):
            self.store_revision += 1
        return "applied"

    def _delivery_permitted(self, scope, token, kind, resource):
        identity, generation = token
        grant = self.policies.get(scope, (0, {}))[1].get(identity)
        if self.mode == "trust_historical_grant":
            grant = self.history.get((scope, identity, generation))
        return (
            grant is not None
            and grant.generation == generation
            and grant.kind == kind
            and resource in grant.resources
            and "retain" in grant.operations
        )

    def _store(self, store, value):
        previous = store.get(value.identity)
        if previous is not None:
            if previous != value:
                raise ValueError(
                    "Conflicting payload for a retained immutable identity"
                )
            return "duplicate"
        if len(self.raw) + len(self.summaries) >= 32:
            raise ValueError("Retained-object budget exceeded")
        store[value.identity] = value
        self.store_revision += 1
        return "stored"

    def receive_raw(self, scope, token, record: Record):
        if not self._delivery_permitted(scope, token, "raw", record.identity):
            return "denied"
        return self._store(self.raw, record)

    def receive_summary(self, scope, token, summary: Summary):
        if not self._delivery_permitted(scope, token, "summary", summary.identity):
            return "denied"
        return self._store(self.summaries, summary)

    def freeze(self, scope, identity, dependencies):
        dependencies = frozenset(dependencies)
        if (
            not dependencies
            or not self.permits(scope, "summary", identity, "retain")
            or any(
                key not in self.raw or not self.permits(scope, "raw", key, "infer")
                for key in dependencies
            )
        ):
            return "denied"
        summary = Summary(
            identity,
            frozenset(dependencies),
            Q(len(dependencies)),
            sum((self.raw[key].value for key in dependencies), Q(0)),
        )
        return self._store(self.summaries, summary)

    def query(self, scope, required=frozenset(), require_complete=False):
        represented = set()
        precision = information = Q(0)
        unsupported = False
        for identity, summary in sorted(self.summaries.items()):
            if not (
                self.mode == "sticky_summary"
                or self.permits(scope, "summary", identity, "infer")
            ):
                continue
            if summary.dependencies <= represented:
                continue
            if summary.dependencies & represented:
                unsupported = True
                break
            represented.update(summary.dependencies)
            precision += summary.precision
            information += summary.information
        if not unsupported:
            for identity, record in sorted(self.raw.items()):
                if identity not in represented and self.permits(
                    scope, "raw", identity, "infer"
                ):
                    represented.add(identity)
                    precision += 1
                    information += record.value
        missing = frozenset(required) - represented
        status = (
            "overlap_unsupported"
            if unsupported
            else "required_coverage_missing"
            if missing and require_complete
            else "no_evidence"
            if not precision
            else "available"
        )
        return Result(
            scope,
            self.policies.get(scope, (0, {}))[0],
            self.store_revision,
            frozenset(represented),
            missing,
            information / precision if status == "available" else None,
            1 / precision if status == "available" else None,
            status,
        )

    def accept_cached(self, scope, result: Result):
        # Returning an older version explicitly would be a different API/policy.
        return (
            result.scope == scope
            and result.policy_revision == self.policies.get(scope, (0, {}))[0]
            and result.store_revision == self.store_revision
        )
