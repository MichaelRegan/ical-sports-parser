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
- can include recent past events when `OPENCLAW_ICAL_PAST_DAYS` is set
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

To include recent past events, also set:

```bash
export OPENCLAW_ICAL_PAST_DAYS='7'
```

If the parser binary is not on `PATH`, also set:

```bash
export OPENCLAW_ICAL_PARSER_BIN='/opt/openclaw/bin/ical-sports-parser'
```

## Deploy to an OpenClaw host

One simple deployment layout is to place both files in the same directory on the OpenClaw host:

```text
/opt/openclaw/bin/ical-sports-parser
/opt/openclaw/bin/openclaw-ical-sports-parser.sh
```

Because the wrapper looks for the parser binary next to itself first, this layout avoids any repo-specific path assumptions.

Step by step:

1. Build the release binary locally:

```bash
cargo build --release
```

2. Create a destination directory on the host:

```bash
ssh your-user@your-openclaw-host 'mkdir -p /opt/openclaw/bin'
```

3. Copy the binary and wrapper script to the host:

```bash
scp target/release/ical-sports-parser \
  scripts/openclaw-ical-sports-parser.sh \
  your-user@your-openclaw-host:/opt/openclaw/bin/
```

4. Mark both files executable on the host:

```bash
ssh your-user@your-openclaw-host 'chmod +x /opt/openclaw/bin/ical-sports-parser /opt/openclaw/bin/openclaw-ical-sports-parser.sh'
```

5. Smoke test the wrapper on the host:

```bash
ssh your-user@your-openclaw-host "OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' /opt/openclaw/bin/openclaw-ical-sports-parser.sh"
```

6. Point each OpenClaw tool at `/opt/openclaw/bin/openclaw-ical-sports-parser.sh`.

7. Configure one tool per feed, either with a different `OPENCLAW_ICAL_URL` environment variable per tool or by passing the feed URL as the single tool argument.

8. If a tool should answer questions like "what was the last game" or "what changed recently," set `OPENCLAW_ICAL_PAST_DAYS` for that tool.

If OpenClaw runs inside a container, make sure `/opt/openclaw/bin` is mounted into that container or copy the files into the container instead of only the host OS.

## Tool contract

The safest contract is one OpenClaw tool per calendar feed.

- Recommended tool names:
  - `julie_high_school_softball_schedule`
  - `julie_select_softball_schedule`
  - `michael_high_school_baseball_schedule`
  - `michael_select_baseball_schedule`
- Tool description pattern: `Returns upcoming schedule events as JSON for a specific team. Use this before answering questions about game times, opponents, cancellations, updates, or locations.`
- Optional environment: set `OPENCLAW_ICAL_PAST_DAYS` when that tool should also expose recent past events.
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
- Use the first event in the returned events array as the current or next event.
- If the first event has already started but its end time has not passed, describe it as currently in progress instead of as the next game.
- If the events array is empty, say that no upcoming events were found in the configured lookahead window.
- If the tool is configured with a past window, recent past events may appear before future events in chronological order.
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
OPENCLAW_ICAL_PAST_DAYS=7 \
./scripts/openclaw-ical-sports-parser.sh
```

Pretty output for debugging:

```bash
OPENCLAW_ICAL_URL='webcal://api.team-manager.gc.com/...' \
OPENCLAW_ICAL_PRETTY=true \
./scripts/openclaw-ical-sports-parser.sh
```
