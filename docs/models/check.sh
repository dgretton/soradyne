#!/usr/bin/env bash
# Run the formal models in docs/models.
#
#   docs/models/check.sh            run everything
#   docs/models/check.sh alloy      only the Alloy invariants model
#   docs/models/check.sh tla        only the TLA+ models
#
# Requirements: Java 17+ on PATH, and network access the first time (the Alloy and
# TLA+ jars are downloaded into docs/models/.tools/, which is gitignored). Set
# ALLOY_JAR / TLA2TOOLS_JAR to use local copies instead.
#
# Exit codes: 0 all checks passed; 1 a check failed; 2 tooling unavailable.
#
# Written 2026-09-06 without a local Java install. The first run on a machine with
# Java must confirm that the Alloy command-line entry point below behaves as
# described (it is the documented batch runner shipped in every Alloy release since
# 4.x); if a newer release changes it, fix this script, not the models.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLS="$HERE/.tools"
mkdir -p "$TOOLS"

ALLOY_VERSION="${ALLOY_VERSION:-6.2.0}"
ALLOY_JAR="${ALLOY_JAR:-$TOOLS/org.alloytools.alloy.dist-$ALLOY_VERSION.jar}"
ALLOY_URL="https://github.com/AlloyTools/org.alloytools.alloy/releases/download/v$ALLOY_VERSION/org.alloytools.alloy.dist.jar"

TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-$TOOLS/tla2tools.jar}"
TLA2TOOLS_URL="https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar"

what="${1:-all}"
fail=0

need_java() {
  if ! command -v java >/dev/null 2>&1; then
    echo "check.sh: java not found on PATH; formal models not checked" >&2
    exit 2
  fi
}

fetch() { # url dest
  if [ ! -f "$2" ]; then
    echo "check.sh: fetching $(basename "$2")"
    curl -sSL --fail -o "$2" "$1" || { echo "check.sh: download failed: $1" >&2; exit 2; }
  fi
}

run_alloy() {
  fetch "$ALLOY_URL" "$ALLOY_JAR"
  local out
  out="$(mktemp)"
  echo "== Alloy: invariants.als"
  # ExampleUsingTheCompiler executes every `check` and `run` command in the file and
  # prints, per command, either "No counterexample found" / "No instance found" or
  # the solution. We fail on any counterexample and on any run that finds nothing.
  if ! java -cp "$ALLOY_JAR" edu.mit.csail.sdg.alloy4whole.ExampleUsingTheCompiler \
        "$HERE/invariants.als" >"$out" 2>&1; then
    echo "check.sh: Alloy exited non-zero"; cat "$out"; fail=1; return
  fi
  if grep -qi "counterexample found" "$out"; then
    echo "check.sh: Alloy found a COUNTEREXAMPLE to an invariant:"; cat "$out"; fail=1; return
  fi
  if grep -qi "no instance found" "$out"; then
    echo "check.sh: an Alloy run command found NO INSTANCE (the model is over-constrained):"; cat "$out"; fail=1; return
  fi
  if grep -qi "error\|exception" "$out"; then
    echo "check.sh: Alloy reported an error:"; cat "$out"; fail=1; return
  fi
  echo "   ok"
  rm -f "$out"
}

run_tla() {
  fetch "$TLA2TOOLS_URL" "$TLA2TOOLS_JAR"
  for spec in "$HERE"/*.tla; do
    local name cfg
    name="$(basename "$spec" .tla)"
    cfg="$HERE/$name.cfg"
    [ -f "$cfg" ] || { echo "check.sh: no $name.cfg, skipping $name.tla"; continue; }
    echo "== TLC: $name.tla"
    if ! ( cd "$HERE" && java -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC \
             -config "$name.cfg" -workers auto -deadlock -cleanup "$name.tla" ); then
      echo "check.sh: TLC reported a violation or error in $name"; fail=1
    else
      echo "   ok"
    fi
  done
}

need_java
case "$what" in
  all)   run_alloy; run_tla ;;
  alloy) run_alloy ;;
  tla)   run_tla ;;
  *) echo "usage: check.sh [all|alloy|tla]" >&2; exit 2 ;;
esac

exit $fail
