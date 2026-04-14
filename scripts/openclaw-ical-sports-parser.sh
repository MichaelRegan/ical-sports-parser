#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

command=(cargo run --quiet -- --days "$days_arg" --limit "$limit_arg")

if [[ -n "$timezone_arg" ]]; then
  command+=(--display-timezone "$timezone_arg")
fi

if [[ "$pretty_arg" == "true" ]]; then
  command+=(--pretty)
fi

command+=("$source_arg")

cd "$PROJECT_ROOT"
exec "${command[@]}"