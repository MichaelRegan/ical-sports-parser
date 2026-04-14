use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use ical_sports_parser::{
    build_calendar_output_from_query, parse_display_timezone, serialize_output, ScheduleMode,
    ScheduleQuery, DEFAULT_DAYS, DEFAULT_LIMIT,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process;
use tokio::net::TcpListener;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct GetScheduleArgs {
    uri: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    days: Option<i64>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    display_timezone: Option<String>,
}

#[derive(Clone, Copy)]
enum TransportMode {
    Stdio,
    Http(SocketAddr),
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_server().await {
        eprintln!("{err}");
        process::exit(1);
    }
}

async fn run_server() -> Result<(), String> {
    match parse_transport_mode(env::args().skip(1))? {
        TransportMode::Stdio => run_stdio_server(),
        TransportMode::Http(bind_addr) => run_http_server(bind_addr).await,
    }
}

fn parse_transport_mode(args: impl Iterator<Item = String>) -> Result<TransportMode, String> {
    let mut args = args.peekable();
    let mut mode = TransportMode::Stdio;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stdio" => {
                mode = TransportMode::Stdio;
            }
            "--http" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --http. Use host:port".to_owned())?;
                let bind_addr = value
                    .parse::<SocketAddr>()
                    .map_err(|err| format!("Invalid --http address {value}: {err}"))?;
                mode = TransportMode::Http(bind_addr);
            }
            _ => {
                return Err(format!(
                    "Unknown option: {arg}\nUsage: ical-sports-mcp [--stdio] [--http host:port]"
                ));
            }
        }
    }

    Ok(mode)
}

fn run_stdio_server() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(payload) = read_message(&mut reader).map_err(|err| err.to_string())? {
        let request = match serde_json::from_str::<JsonRpcRequest>(&payload) {
            Ok(request) => request,
            Err(err) => {
                write_response(
                    &mut writer,
                    json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {err}"),
                        }
                    }),
                )
                .map_err(|io_err| io_err.to_string())?;
                continue;
            }
        };

        if let Some(response) = handle_request(request)? {
            write_response(&mut writer, response).map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

async fn run_http_server(bind_addr: SocketAddr) -> Result<(), String> {
    let app = Router::new()
        .route("/healthz", get(http_health))
        .route("/mcp", post(http_mcp))
        .with_state(());

    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|err| format!("Failed to bind HTTP listener on {bind_addr}: {err}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|err| format!("HTTP server error: {err}"))
}

async fn http_health() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "server": "ical-sports-mcp",
        "protocol": MCP_PROTOCOL_VERSION,
    }))
}

async fn http_mcp(State(()): State<()>, body: Bytes) -> Response {
    let request = match serde_json::from_slice::<JsonRpcRequest>(&body) {
        Ok(request) => request,
        Err(err) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {err}"),
                    }
                }),
            );
        }
    };

    match tokio::task::spawn_blocking(move || handle_request(request)).await {
        Ok(Ok(Some(response))) => json_response(StatusCode::OK, response),
        Ok(Ok(None)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(err)) => json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32603,
                    "message": err,
                }
            }),
        ),
        Err(err) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32603,
                    "message": format!("Internal task error: {err}"),
                }
            }),
        ),
    }
}

fn json_response(status: StatusCode, body: Value) -> Response {
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

fn handle_request(request: JsonRpcRequest) -> Result<Option<Value>, String> {
    let JsonRpcRequest {
        id,
        method,
        params,
        ..
    } = request;

    let response = match method.as_str() {
        "initialize" => id.map(|request_id| {
            ok_response(
                request_id,
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "ical-sports-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
        }),
        "notifications/initialized" => None,
        "ping" => id.map(|request_id| ok_response(request_id, json!({}))),
        "tools/list" => id.map(|request_id| ok_response(request_id, list_tools_result())),
        "tools/call" => {
            let request_id = id.ok_or_else(|| "tools/call requires an id".to_owned())?;
            Some(handle_tool_call(request_id, params)?)
        }
        _ => id.map(|request_id| {
            error_response(request_id, -32601, format!("Method not found: {method}"))
        }),
    };

    Ok(response)
}

fn list_tools_result() -> Value {
    json!({
        "tools": [
            {
                "name": "get_schedule",
                "description": "Returns normalized schedule data from an iCalendar feed. Accepts webcal://, http://, and https:// URIs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "uri": {
                            "type": "string",
                            "description": "Required iCalendar source URI. webcal:// is normalized to https:// before fetch."
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["raw", "current", "next", "upcoming"],
                            "default": "upcoming",
                            "description": "raw returns the normalized schedule payload for the query window; current returns in-progress events; next returns future-starting events; upcoming returns events that have not yet ended."
                        },
                        "days": {
                            "type": "integer",
                            "minimum": 0,
                            "default": 30,
                            "description": "Number of days to look ahead from now."
                        },
                        "limit": {
                            "type": "integer",
                            "minimum": 1,
                            "default": 10,
                            "description": "Maximum number of events to return."
                        },
                        "display_timezone": {
                            "type": "string",
                            "description": "Optional IANA timezone name like America/Los_Angeles."
                        }
                    },
                    "required": ["uri"]
                }
            }
        ]
    })
}

fn handle_tool_call(id: Value, params: Option<Value>) -> Result<Value, String> {
    let params = params.ok_or_else(|| "Missing params for tools/call".to_owned())?;
    let tool_call = serde_json::from_value::<ToolCallParams>(params)
        .map_err(|err| format!("Invalid tools/call params: {err}"))?;

    if tool_call.name != "get_schedule" {
        return Ok(error_response(
            id,
            -32602,
            format!("Unknown tool: {}", tool_call.name),
        ));
    }

    let args = match serde_json::from_value::<GetScheduleArgs>(tool_call.arguments) {
        Ok(args) => args,
        Err(err) => {
            return Ok(ok_response(id, tool_error_result(format!(
                "Invalid get_schedule arguments: {err}"
            ))));
        }
    };

    let mode = match args.mode.as_deref() {
        Some(value) => match value.parse::<ScheduleMode>() {
            Ok(mode) => mode,
            Err(err) => return Ok(ok_response(id, tool_error_result(err))),
        },
        None => ScheduleMode::Upcoming,
    };

    let days = args.days.unwrap_or(DEFAULT_DAYS);
    if days < 0 {
        return Ok(ok_response(
            id,
            tool_error_result("days must be greater than or equal to 0".to_owned()),
        ));
    }

    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 {
        return Ok(ok_response(
            id,
            tool_error_result("limit must be greater than 0".to_owned()),
        ));
    }

    let display_timezone = match args.display_timezone {
        Some(value) => match parse_display_timezone(&value) {
            Ok(timezone) => Some(timezone),
            Err(err) => return Ok(ok_response(id, tool_error_result(err))),
        },
        None => None,
    };

    let query = ScheduleQuery {
        source: args.uri,
        days,
        past_days: 0,
        limit,
        display_timezone,
        pretty: false,
        mode,
    };

    match build_calendar_output_from_query(&query, Utc::now()) {
        Ok(output) => {
            let text = serialize_output(&output, true)
                .map_err(|err| format!("Failed to serialize tool result: {err}"))?;
            let structured = serde_json::to_value(&output)
                .map_err(|err| format!("Failed to encode structured tool result: {err}"))?;
            Ok(ok_response(
                id,
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": text
                        }
                    ],
                    "structuredContent": structured,
                    "isError": false
                }),
            ))
        }
        Err(err) => Ok(ok_response(id, tool_error_result(err))),
    }
}

fn tool_error_result(message: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

fn ok_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Ok(None);
        }

        if line == "\r\n" || line == "\n" {
            break;
        }

        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let value = parts.next().unwrap_or_default().trim();
        if name == "content-length" {
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Content-Length"))?,
            );
        }
    }

    let content_length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length header"))?;
    let mut payload = vec![0_u8; content_length];
    reader.read_exact(&mut payload)?;

    String::from_utf8(payload)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn write_response<W: Write>(writer: &mut W, response: Value) -> io::Result<()> {
    let payload = serde_json::to_vec(&response)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)?;
    writer.flush()
}