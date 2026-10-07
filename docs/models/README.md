# Formal mechanism sketches

**Status, 2026-09-11:** the shared-flow architecture is defined by
`../20260911_shared_flow_demo_contracts.md`. Models here are scoped sketches,
not an executable specification of the whole protocol or proof of implementation.

## Retired structural model

`invariants.als` was deleted, with its runner, on 2026-09-11. It encoded superseded
assumptions about flow-owner authorship, raw/derived disclosure, producer-local
custody, acyclic references, and unconditional discard. Keeping it as a required
check would reward conformity to the wrong design. History is available in git;
do not recreate it by renaming the old concepts.

## Retained lease sketch

`Lease.tla` and `Lease.cfg` model claim/lapse/revoke and acceptance of term-stamped
writes under one abstract holder variable. They are useful for discussing fencing.
They do **not** implement distributed agreement, model partitions/clock bounds,
validate changing membership, or establish safe direct service responses. A successful
TLC run proves only the configured bounded properties of that abstraction.
The model was originally written without a local TLC; do not infer a successful run
from its presence in the repository.

`check.sh` runs TLC only, downloading its tool jar into `.tools/` if needed. Exit
codes: 0 checked successfully, 1 failed, 2 unavailable tooling or invalid invocation.
The Rust harness treats 2 as a visible skip unless `SORADYNE_REQUIRE_FORMAL=1`.

```sh
docs/models/check.sh             # all retained models (currently lease only)
docs/models/check.sh tla
cargo test --manifest-path packages/soradyne_core/Cargo.toml --test formal_models --no-default-features
```

## Before relying on a model

1. Name the demo and mechanism it constrains, and write the property and assumptions.
2. Include meaningful positive scenarios as well as forbidden ones; satisfiability
   matters as much as absence of a counterexample.
3. Model the failure being claimed, including partitions when making partition claims.
4. Run the tool; record the result and bounds. A skip is not a successful check.
5. Tie implementation state/actions to the abstraction with tests or trace validation.
6. Update prose and model together when a design decision changes.

A new structural model is optional and should follow the shared-flow demos, not
constrain those demos to the previous architecture. Crypto scheme selection remains
open; this directory does not certify any handshake or group-key construction.
