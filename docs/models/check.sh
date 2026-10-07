#!/usr/bin/env bash
# Run retained TLA+ mechanism sketches. See README.md for their limited scope.
# Usage: check.sh [all|tla]
# Java and a local or downloadable TLC jar are required.
# Exit: 0 pass, 1 model failure, 2 tooling unavailable or invalid invocation.
set -euo pipefail

case "${1:-all}" in
  all|tla) ;;
  *) echo "usage: check.sh [all|tla] (Alloy model retired)" >&2; exit 2 ;;
esac

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLS="$HERE/.tools"
TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-$TOOLS/tla2tools.jar}"
TLA2TOOLS_URL="https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar"

if ! command -v java >/dev/null 2>&1; then
  echo "check.sh: java not found; TLA+ sketches not checked" >&2
  exit 2
fi
if [ ! -f "$TLA2TOOLS_JAR" ]; then
  mkdir -p "$(dirname "$TLA2TOOLS_JAR")"
  echo "check.sh: fetching TLC"
  curl -sSL --fail -o "$TLA2TOOLS_JAR" "$TLA2TOOLS_URL" || {
    echo "check.sh: TLC download failed" >&2; exit 2;
  }
fi

fail=0
for spec in "$HERE"/*.tla; do
  name="$(basename "$spec" .tla)"
  cfg="$HERE/$name.cfg"
  if [ ! -f "$cfg" ]; then
    echo "check.sh: missing $name.cfg" >&2
    fail=1
    continue
  fi
  echo "TLC: $name.tla (abstract mechanism only)"
  if ! ( cd "$HERE" && java -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC \
       -config "$name.cfg" -workers auto -deadlock -cleanup "$name.tla" ); then
    echo "check.sh: TLC reported a violation or error in $name" >&2
    fail=1
  fi
done
exit "$fail"
