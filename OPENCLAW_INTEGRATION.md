# OpenClaw Integration

This project is ready to be used as a local command-style tool for OpenClaw.

## Recommended shape

Use the wrapper script:

```bash
/home/michael/projects/ical-sports-parser/scripts/openclaw-ical-sports-parser.sh
```

That wrapper:

- accepts a local ICS path or a `webcal`/`https` URL as the first argument
- or uses `OPENCLAW_ICAL_URL` from the environment
- defaults to `90` days, `12` upcoming events, and `America/Los_Angeles`
- emits compact JSON by default for tool use

## Environment

Populate values from [.env.openclaw.example](/home/michael/projects/ical-sports-parser/.env.openclaw.example) in whatever secret or environment system you use for OpenClaw.

At minimum:

```bash
export OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...'
export OPENCLAW_ICAL_TIMEZONE='America/Los_Angeles'
```

## Tool contract

The safest contract is simple:

- Tool name: `daughter_schedule`
- Tool description: `Returns upcoming softball schedule events as JSON. Use this before answering questions about game times, opponents, or locations.`
- Command: `/home/michael/projects/ical-sports-parser/scripts/openclaw-ical-sports-parser.sh`
- Arguments: none if the URL is in `OPENCLAW_ICAL_URL`; otherwise pass the feed URL as the single argument

If you also want a separate tool for your son, create another OpenClaw tool with the same command but a different environment variable value or argument.

## Prompt guidance

Use instructions along these lines in OpenClaw:

```text
When the user asks about Julie's next game, softball schedule, opponent, or location, call the daughter_schedule tool first.

Rules:
- Treat the tool output as the source of truth for schedule questions.
- Use the first event in the returned events array as the next upcoming event.
- If the events array is empty, say that no upcoming events were found in the configured lookahead window.
- Mention the local displayed time, opponent, and location when available.
- Do not invent missing schedule details.
```

## Example OpenClaw answer flow

User asks:

```text
When is my daughter's next game?
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
Your daughter's next game is Monday, April 13 at 7:00 PM against Redmond Varsity Mustangs at Woodinville High School.
```

## Manual smoke test

```bash
OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' \
/home/michael/projects/ical-sports-parser/scripts/openclaw-ical-sports-parser.sh
```

Pretty output for debugging:

```bash
OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' \
OPENCLAW_ICAL_PRETTY=true \
/home/michael/projects/ical-sports-parser/scripts/openclaw-ical-sports-parser.sh
```