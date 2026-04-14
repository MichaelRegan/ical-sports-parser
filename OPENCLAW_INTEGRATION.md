# OpenClaw Integration

This project is ready to be used as a local command-style tool for OpenClaw.

## Recommended shape

Use the wrapper script:

```bash
./scripts/openclaw-ical-sports-parser.sh
```

That wrapper:

- accepts a local ICS path or a `webcal`/`https` URL as the first argument
- or uses `OPENCLAW_ICAL_URL` from the environment
- defaults to `90` days, `12` upcoming events, and `America/Los_Angeles`
- prefers an `ical-sports-parser` binary from `OPENCLAW_ICAL_PARSER_BIN`, next to the script, in the repo build output, or on `PATH`
- falls back to `cargo run` only when executed from a cloned repo with Cargo available
- emits compact JSON by default for tool use

## Environment

Populate values from [.env.openclaw.example](.env.openclaw.example) in whatever secret or environment system you use for OpenClaw.

At minimum:

```bash
export OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...'
export OPENCLAW_ICAL_TIMEZONE='America/Los_Angeles'
```

If the parser binary is not on `PATH`, also set:

```bash
export OPENCLAW_ICAL_PARSER_BIN='/opt/openclaw/bin/ical-sports-parser'
```

## Tool contract

The safest contract is one OpenClaw tool per calendar feed.

- Recommended tool names:
  - `julie_high_school_softball_schedule`
  - `julie_select_softball_schedule`
  - `michael_high_school_baseball_schedule`
  - `michael_select_baseball_schedule`
- Tool description pattern: `Returns upcoming schedule events as JSON for a specific team. Use this before answering questions about game times, opponents, cancellations, updates, or locations.`
- Command: `./scripts/openclaw-ical-sports-parser.sh`
- Arguments: none if the feed URL is configured in that tool's environment; otherwise pass the feed URL as the single argument

Each tool can use the same wrapper command with a different `OPENCLAW_ICAL_URL` value or a different URL argument.

If your OpenClaw host supports per-tool environment variables, configure each tool with its own `OPENCLAW_ICAL_URL`.

If it does not, keep the command the same and pass the calendar URL as the tool's single argument.

## Prompt guidance

Use instructions along these lines in OpenClaw:

```text
Use the schedule tool that matches the child and team the user is asking about.

Examples:
- Julie high school softball questions -> julie_high_school_softball_schedule
- Julie select softball questions -> julie_select_softball_schedule
- Michael high school baseball questions -> michael_high_school_baseball_schedule
- Michael select baseball questions -> michael_select_baseball_schedule

Rules:
- Treat the tool output as the source of truth for schedule questions.
- Use the first event in the returned events array as the next upcoming event.
- If the events array is empty, say that no upcoming events were found in the configured lookahead window.
- If the relevant event status is `CANCELLED`, say clearly that the game was cancelled.
- If multiple tools might apply and the user's request is ambiguous, ask which child or team they mean.
- Mention the local displayed time, opponent, and location when available.
- Do not invent missing schedule details.
```

## Example OpenClaw answer flow

User asks:

```text
When is Julie's next high school softball game?
```

Tool returns JSON with `events[0]` equal to:

```json
{
  "title": "Woodinville Falcons Varsity vs Redmond Varsity Mustangs",
  "start_datetime": "2026-04-13T19:00:00-07:00",
  "location": "Woodinville High School\nWoodinville, WA, United States",
  "opponent": "Redmond Varsity Mustangs"
}
```

OpenClaw should answer:

```text
Julie’s next high school softball game is Monday, April 13 at 7:00 PM against Redmond Varsity Mustangs at Woodinville High School.
```

## Manual smoke test

```bash
OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' \
./scripts/openclaw-ical-sports-parser.sh
```

Pretty output for debugging:

```bash
OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' \
OPENCLAW_ICAL_PRETTY=true \
./scripts/openclaw-ical-sports-parser.sh
```
