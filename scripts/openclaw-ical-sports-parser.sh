#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

resolve_parser_command() {
  local candidate

  if [[ -n "${OPENCLAW_ICAL_PARSER_BIN:-}" ]]; then
    printf '%s\n' "$OPENCLAW_ICAL_PARSER_BIN"
    return 0
  fi

  for candidate in \
    "$SCRIPT_DIR/ical-sports-parser" \
    "$PROJECT_ROOT/ical-sports-parser" \
    "$PROJECT_ROOT/target/release/ical-sports-parser" \
    "$PROJECT_ROOT/target/debug/ical-sports-parser"
  do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  if command -v ical-sports-parser >/dev/null 2>&1; then
    command -v ical-sports-parser
    return 0
  fi

  if command -v cargo >/dev/null 2>&1 && [[ -f "$PROJECT_ROOT/Cargo.toml" ]]; then
    printf '%s\n' '__CARGO_RUN__'
    return 0
  fi

  echo "Could not find an ical-sports-parser executable." >&2
  echo "Set OPENCLAW_ICAL_PARSER_BIN, place the binary next to this script, add it to PATH, or run from a cloned repo with cargo available." >&2
  return 1
}

source_arg="${1:-${OPENCLAW_ICAL_URL:-}}"
if [[ -z "$source_arg" ]]; then
  echo "Usage: openclaw-ical-sports-parser.sh <ics-path-or-url>" >&2
  echo "Alternatively set OPENCLAW_ICAL_URL in the environment." >&2
  exit 1
fi

days_arg="${OPENCLAW_ICAL_DAYS:-90}"
limit_arg="${OPENCLAW_ICAL_LIMIT:-12}"
timezone_arg="${OPENCLAW_ICAL_TIMEZONE:-America/Los_Angeles}"
pretty_arg="${OPENCLAW_ICAL_PRETTY:-false}"

parser_command="$(resolve_parser_command)"

if [[ "$parser_command" == "__CARGO_RUN__" ]]; then
  command=(cargo run --quiet -- --days "$days_arg" --limit "$limit_arg")
else
  command=("$parser_command" --days "$days_arg" --limit "$limit_arg")
fi

if [[ -n "$timezone_arg" ]]; then
  command+=(--display-timezone "$timezone_arg")
fi

if [[ "$pretty_arg" == "true" ]]; then
  command+=(--pretty)
fi

command+=("$source_arg")

if [[ "$parser_command" == "__CARGO_RUN__" ]]; then
  cd "$PROJECT_ROOT"
fi

exec "${command[@]}"