# ical-sports-parser

Rust MCP server for sports schedule questions backed by iCalendar feeds, with HTTP transport for LAN-friendly deployment.

The primary interface is a single MCP tool named `get_schedule`.

## Supported input

- Local `.ics` file path
- `webcal://...` URL
- `https://...` or `http://...` URL

`webcal://` sources are normalized to `https://` before fetch.

## MCP Tool

Tool name: `get_schedule`

Parameters:

- `uri` required iCalendar source URI or file path
- `mode` one of `raw`, `current`, `next`, or `upcoming`
- `days` lookahead window in days
- `limit` maximum number of events to return
- `display_timezone` optional IANA timezone such as `America/Los_Angeles`

Mode behavior:

- `raw` returns the normalized schedule payload for the requested window
- `current` returns only in-progress events where `start <= now < end`
- `next` returns only events where `start > now`
- `upcoming` returns events where `end > now`, so in-progress events remain visible

## Current behavior

- Fetches or reads an iCalendar source
- Parses `VEVENT` entries
- Expands recurring events from `RRULE`, `RDATE`, and `EXDATE` data when present
- Normalizes start and end values to ISO 8601
- Carries forward timezone information when available
- Filters out events that have already ended while keeping in-progress events visible
- Keeps upcoming cancelled events so downstream tools can detect schedule changes
- Filters to an upcoming lookahead window and a maximum result count
- Sorts remaining events by upcoming start time
- Preserves raw event fields and adds normalized helpers such as `event_type`, `venue_type`, and `opponent`
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
    "mode": "upcoming",
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
      "venue_type": "home",
      "opponent": "Wildcats"
    }
  ]
}
```

`recurrence_parent_uid` is included only for expanded recurring instances.

## Running

Build the project:

```bash
cargo build --release
```

Run the MCP server over HTTP:

```bash
./target/release/ical-sports-mcp --http 0.0.0.0:8080
```

## MCP Deployment

This implementation supports MCP over stdio and a simple HTTP JSON-RPC transport.

- For local MCP clients, launch `ical-sports-mcp` in stdio mode
- For a dedicated LXC serving your LAN, run `ical-sports-mcp --http 0.0.0.0:8080`
- Expose `POST /mcp` for MCP JSON-RPC requests
- Use `GET /healthz` for health checks

Example MCP HTTP call:

```bash
curl -s http://claw.lan.mjsquared.net:8080/mcp \
  -H 'content-type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "get_schedule",
      "arguments": {
        "uri": "webcal://example.com/team.ics",
        "mode": "upcoming",
        "days": 30,
        "limit": 5,
        "display_timezone": "America/Los_Angeles"
      }
    }
  }'
```

## LXC Deployment

One clean layout for a dedicated LXC is:

```text
/opt/ical-sports-mcp/bin/ical-sports-mcp
/etc/systemd/system/ical-sports-mcp.service
/etc/traefik/dynamic/ical-sports-mcp.dynamic.yml
```

Build and copy the binary:

```bash
cargo build --release
scp target/release/ical-sports-mcp root@claw.lan.mjsquared.net:/opt/ical-sports-mcp/bin/
```

The repo includes ready-to-adapt deployment files:

- [deploy/systemd/ical-sports-mcp.service](deploy/systemd/ical-sports-mcp.service)
- [deploy/traefik/ical-sports-mcp.dynamic.yml](deploy/traefik/ical-sports-mcp.dynamic.yml)

Suggested host setup:

```bash
ssh root@claw.lan.mjsquared.net 'useradd --system --home /opt/ical-sports-mcp --shell /usr/sbin/nologin ical-sports-mcp || true'
ssh root@claw.lan.mjsquared.net 'mkdir -p /opt/ical-sports-mcp/bin /etc/traefik/dynamic'
scp target/release/ical-sports-mcp root@claw.lan.mjsquared.net:/opt/ical-sports-mcp/bin/
scp deploy/systemd/ical-sports-mcp.service root@claw.lan.mjsquared.net:/etc/systemd/system/
scp deploy/traefik/ical-sports-mcp.dynamic.yml root@claw.lan.mjsquared.net:/etc/traefik/dynamic/
ssh root@claw.lan.mjsquared.net 'chown -R ical-sports-mcp:ical-sports-mcp /opt/ical-sports-mcp && systemctl daemon-reload && systemctl enable --now ical-sports-mcp'
```

The bundled Traefik file assumes Traefik runs in the same LXC and can reach the service at `127.0.0.1:8080`. If your Traefik instance runs elsewhere, change the backend URL in [deploy/traefik/ical-sports-mcp.dynamic.yml](deploy/traefik/ical-sports-mcp.dynamic.yml) to the LXC IP or hostname instead.

Example service unit:

```ini
[Unit]
Description=iCal Sports MCP Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=ical-sports-mcp
Group=ical-sports-mcp
WorkingDirectory=/opt/ical-sports-mcp
ExecStart=/opt/ical-sports-mcp/bin/ical-sports-mcp --http 0.0.0.0:8080
Restart=always
RestartSec=2
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectControlGroups=true
ProtectKernelModules=true
ProtectKernelTunables=true
RestrictSUIDSGID=true
LockPersonality=true
MemoryDenyWriteExecute=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
ReadWritePaths=/tmp
SystemCallArchitectures=native

[Install]
WantedBy=multi-user.target
```

After copying the Traefik file, verify:

```bash
curl -s https://claw.lan.mjsquared.net/healthz
```

## Filters

- `--days N` limits results to the next `N` days. Default: `30`
- `--limit N` returns at most `N` upcoming events. Default: `10`

## Presentation

- `--display-timezone TZ` reformats output timestamps into an IANA timezone such as `America/Los_Angeles`

## Common commands

- `cargo build`
- `cargo run --bin ical-sports-mcp -- --http 127.0.0.1:8080`
- `cargo run --bin ical-sports-mcp -- --stdio`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

## VS Code

Open `ical-sports-parser.code-workspace` to use the bundled workspace settings, tasks, and debug profile.

## OpenClaw

Point OpenClaw or any other MCP-capable client at this server and call the single `get_schedule` tool with the appropriate team feed URI.

## Do You Need Full MCP HTTP?

Probably not yet if OpenClaw is the primary client.

- The current HTTP transport is enough if OpenClaw only needs to call `initialize`, `tools/list`, and `tools/call` over a simple JSON-RPC endpoint.
- This is usually fine for a controlled internal deployment where you own both the server and the client behavior.
- A fuller MCP HTTP implementation becomes worth doing when you need broad compatibility with third-party MCP clients, streaming semantics, or stricter spec conformance checks.

So for your stated goal, I would not do step 3 yet unless OpenClaw proves incompatible with the current `/mcp` endpoint.
