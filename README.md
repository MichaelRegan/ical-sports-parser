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
- Filters out events that have already ended while keeping in-progress events visible
- Can optionally include recent past events with `--past-days`
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
    "past_days": 0,
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

Examples below assume the binary is available on your `PATH` as `ical-sports-parser`.

```bash
ical-sports-parser ./team-calendar.ics
ical-sports-parser 'webcal://api.team-manager.gc.com/...'
ical-sports-parser --days 45 --limit 6 'webcal://api.team-manager.gc.com/...'
ical-sports-parser --days 0 --past-days 7 './team-calendar.ics'
ical-sports-parser --display-timezone America/Los_Angeles --pretty './team-calendar.ics'
./scripts/openclaw-ical-sports-parser.sh 'webcal://api.team-manager.gc.com/...'
```

## Filters

- `--days N` limits results to the next `N` days. Default: `30`
- `--past-days N` also includes events that started within the last `N` days. Default: `0`
- `--limit N` returns at most `N` upcoming events. Default: `10`

Use `--days 0 --past-days N` to return only recent past events.

## Presentation

- `--display-timezone TZ` reformats output timestamps into an IANA timezone such as `America/Los_Angeles`
- `--pretty` prints indented JSON instead of compact JSON

## Common commands

- `cargo build`
- `ical-sports-parser <source>`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

## VS Code

Open `ical-sports-parser.code-workspace` to use the bundled workspace settings, tasks, and debug profile.

## OpenClaw

See [OPENCLAW_INTEGRATION.md](OPENCLAW_INTEGRATION.md) for a practical wrapper command, environment variables, and a tool/prompt contract for schedule questions.
