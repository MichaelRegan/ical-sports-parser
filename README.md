# ical-sports-parser

Rust CLI that converts an iCalendar feed into OpenClaw-friendly JSON for schedule questions such as "when is my daughter's next game".

## Supported input

- Local `.ics` file path
- `webcal://...` URL
- `https://...` or `http://...` URL

`webcal://` sources are normalized to `https://` before fetch.

## Current behavior

- Fetches or reads an iCalendar source
- Parses `VEVENT` entries
- Expands recurring events from `RRULE`, `RDATE`, and `EXDATE` data when present
- Normalizes start and end values to ISO 8601
- Carries forward timezone information when available
- Filters out already-finished events, including cancelled events that have already ended
- Keeps upcoming cancelled events so downstream tools can detect schedule changes
- Filters to an upcoming lookahead window and a maximum result count
- Sorts remaining events by upcoming start time
- Adds light sports enrichment such as `event_type` and `opponent`
- Optionally reformats output timestamps into a chosen display timezone
- Optionally pretty-prints JSON for manual review

## Output shape

The command prints compact JSON to stdout with this structure:

```json
{
  "source": {
    "requested": "webcal://...",
    "resolved": "https://...",
    "kind": "url"
  },
  "generated_at": "2026-04-12T20:15:00Z",
  "applied_filter": {
    "lookahead_days": 30,
    "limit": 10
  },
  "display_timezone": "America/Los_Angeles",
  "calendar_name": "Julie Softball",
  "calendar_timezone": "America/Los_Angeles",
  "events": [
    {
      "uid": "event-1",
      "title": "Julie Softball vs Wildcats",
      "start_datetime": "2026-04-14T18:30:00-07:00",
      "end_datetime": "2026-04-14T20:00:00-07:00",
      "timezone": "America/Los_Angeles",
      "status": "CONFIRMED",
      "is_all_day": false,
      "location": "Central Park Field 3",
      "description": "League game against Wildcats at Field 3",
      "event_type": "game",
      "opponent": "Wildcats"
    }
  ]
}
```

`recurrence_parent_uid` is included only for expanded recurring instances.

## Usage

```bash
cargo run -- ./team-calendar.ics
cargo run -- 'webcal://api.team-manager.gc.com/...'
cargo run -- --days 45 --limit 6 'webcal://api.team-manager.gc.com/...'
cargo run -- --display-timezone America/Los_Angeles --pretty './team-calendar.ics'
./scripts/openclaw-ical-sports-parser.sh 'webcal://api.team-manager.gc.com/...'
```

## Filters

- `--days N` limits results to the next `N` days. Default: `30`
- `--limit N` returns at most `N` upcoming events. Default: `10`

## Presentation

- `--display-timezone TZ` reformats output timestamps into an IANA timezone such as `America/Los_Angeles`
- `--pretty` prints indented JSON instead of compact JSON

## Common commands

- `cargo build`
- `cargo run -- <source>`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

## VS Code

Open `ical-sports-parser.code-workspace` to use the bundled workspace settings, tasks, and debug profile.

## OpenClaw

See [OPENCLAW_INTEGRATION.md](/home/michael/projects/ical-sports-parser/OPENCLAW_INTEGRATION.md) for a practical wrapper command, environment variables, and a tool/prompt contract for schedule questions.
