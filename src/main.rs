use chrono::{DateTime, Duration as ChronoDuration, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use icalendar::{Calendar, Component, EventLike};
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Serialize;
use std::sync::LazyLock;
use std::{env, fs, process, time::Duration as StdDuration};
use url::Url;

static OPPONENT_VERSUS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:vs\.?|versus|against)\s+([^\-|,@;]+)").expect("valid versus regex")
});
static OPPONENT_AT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|\s)@\s*([^\-|,@;]+)").expect("valid at regex"));

#[derive(Debug)]
struct LoadedSource {
    requested: String,
    resolved: String,
    kind: String,
    contents: String,
}

#[derive(Debug)]
struct ParsedDateTimeValue {
    iso: String,
    sort_utc: DateTime<Utc>,
    timezone: Option<String>,
    is_all_day: bool,
}

#[derive(Debug)]
struct SortableEvent {
    sort_start: DateTime<Utc>,
    output: EventOutput,
}

#[derive(Debug)]
struct CliOptions {
    source: String,
    days: i64,
    limit: usize,
    display_timezone: Option<Tz>,
    pretty: bool,
}

#[derive(Debug, Clone)]
struct EventTemplate {
    uid: String,
    title: String,
    end_offset: Option<ChronoDuration>,
    timezone: Option<String>,
    status: Option<String>,
    is_all_day: bool,
    location: Option<String>,
    description: Option<String>,
    event_type: String,
    opponent: Option<String>,
}

#[derive(Debug, Serialize)]
struct CalendarOutput {
    source: SourceMetadata,
    generated_at: String,
    applied_filter: AppliedFilter,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    calendar_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    calendar_timezone: Option<String>,
    events: Vec<EventOutput>,
}

#[derive(Debug, Serialize)]
struct AppliedFilter {
    lookahead_days: i64,
    limit: usize,
}

#[derive(Debug, Serialize)]
struct SourceMetadata {
    requested: String,
    resolved: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct EventOutput {
    uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence_parent_uid: Option<String>,
    title: String,
    start_datetime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_datetime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    is_all_day: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    opponent: Option<String>,
}

fn main() {
    let options = parse_cli_options(env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let source = load_source(&options.source).unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let calendar = source.contents.parse::<Calendar>().unwrap_or_else(|err| {
        eprintln!("Error parsing ICS data from {}: {}", source.resolved, err);
        process::exit(1);
    });

    let output = build_calendar_output(&calendar, &source, Utc::now(), &options);

    println!(
        "{}",
        serialize_output(&output, options.pretty).expect("calendar output should serialize")
    );
}

fn serialize_output(output: &CalendarOutput, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(output)
    } else {
        serde_json::to_string(output)
    }
}

fn parse_cli_options(args: impl Iterator<Item = String>) -> Result<CliOptions, String> {
    let mut source = None;
    let mut days = 30_i64;
    let mut limit = 10_usize;
    let mut display_timezone = None;
    let mut pretty = false;
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--days" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --days".to_owned())?;
                days = value
                    .parse::<i64>()
                    .map_err(|_| format!("Invalid --days value: {value}"))?;
                if days <= 0 {
                    return Err("--days must be greater than 0".to_owned());
                }
            }
            "--limit" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --limit".to_owned())?;
                limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("Invalid --limit value: {value}"))?;
                if limit == 0 {
                    return Err("--limit must be greater than 0".to_owned());
                }
            }
            "--display-timezone" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --display-timezone".to_owned())?;
                display_timezone = Some(
                    parse_timezone(&value)
                        .ok_or_else(|| format!("Invalid --display-timezone value: {value}"))?,
                );
            }
            "--pretty" => {
                pretty = true;
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Unknown option: {arg}\nUsage: ical-sports-parser [--days N] [--limit N] [--display-timezone TZ] [--pretty] <path-to-file.ics|webcal-url|https-url>"
                ));
            }
            _ => {
                if source.is_some() {
                    return Err(
                        "Provide exactly one source argument after any options".to_owned(),
                    );
                }
                source = Some(arg);
            }
        }
    }

    let source = source.ok_or_else(|| {
        "Usage: ical-sports-parser [--days N] [--limit N] [--display-timezone TZ] [--pretty] <path-to-file.ics|webcal-url|https-url>"
            .to_owned()
    })?;

    Ok(CliOptions {
        source,
        days,
        limit,
        display_timezone,
        pretty,
    })
}

fn load_source(input: &str) -> Result<LoadedSource, String> {
    if input.contains("://") {
        let url = normalize_source_url(input)?;
        let client = Client::builder()
            .timeout(StdDuration::from_secs(20))
            .build()
            .map_err(|err| format!("Error creating HTTP client: {err}"))?;

        let contents = client
            .get(url.clone())
            .header(USER_AGENT, "ical-sports-parser/0.1")
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|err| format!("Error fetching {}: {}", url, err))?
            .text()
            .map_err(|err| format!("Error reading response body from {}: {}", url, err))?;

        Ok(LoadedSource {
            requested: input.to_owned(),
            resolved: url.to_string(),
            kind: "url".to_owned(),
            contents,
        })
    } else {
        let contents = fs::read_to_string(input)
            .map_err(|err| format!("Error reading file {}: {}", input, err))?;

        Ok(LoadedSource {
            requested: input.to_owned(),
            resolved: input.to_owned(),
            kind: "file".to_owned(),
            contents,
        })
    }
}

fn normalize_source_url(input: &str) -> Result<Url, String> {
    let normalized_input = if let Some(rest) = input.strip_prefix("webcal://") {
        format!("https://{rest}")
    } else {
        input.to_owned()
    };

    let url = Url::parse(&normalized_input)
        .map_err(|err| format!("Invalid URL {}: {}", input, err))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Unsupported URL scheme {}. Use webcal://, http://, or https://",
                scheme
            ));
        }
    }

    Ok(url)
}

fn build_calendar_output(
    calendar: &Calendar,
    source: &LoadedSource,
    now_utc: DateTime<Utc>,
    options: &CliOptions,
) -> CalendarOutput {
    let calendar_timezone = calendar.get_timezone().map(str::to_owned);
    let window_end = now_utc + ChronoDuration::days(options.days);

    let mut events = calendar
        .events()
        .flat_map(|event| expand_event(event, calendar_timezone.as_deref(), now_utc, window_end, options))
        .collect::<Vec<_>>();

    events.sort_by_key(|event| event.sort_start);
    events.truncate(options.limit);

    CalendarOutput {
        source: SourceMetadata {
            requested: source.requested.clone(),
            resolved: source.resolved.clone(),
            kind: source.kind.clone(),
        },
        generated_at: now_utc.to_rfc3339(),
        applied_filter: AppliedFilter {
            lookahead_days: options.days,
            limit: options.limit,
        },
        display_timezone: options
            .display_timezone
            .map(|timezone| timezone.name().to_owned()),
        calendar_name: calendar.get_name().map(str::to_owned),
        calendar_timezone,
        events: events.into_iter().map(|event| event.output).collect(),
    }
}

fn expand_event(
    event: &icalendar::Event,
    calendar_timezone: Option<&str>,
    now_utc: DateTime<Utc>,
    window_end: DateTime<Utc>,
    options: &CliOptions,
) -> Vec<SortableEvent> {
    let status = normalize_text(event.property_value("STATUS"));

    let Some(start_property) = event.properties().get("DTSTART") else {
        return Vec::new();
    };
    let start = match parse_datetime_property(start_property, calendar_timezone) {
        Some(value) => value,
        None => return Vec::new(),
    };
    let end = event
        .properties()
        .get("DTEND")
        .and_then(|property| parse_datetime_property(property, calendar_timezone));

    let title = normalize_text(event.property_value("SUMMARY"))
        .unwrap_or_else(|| "Untitled event".to_owned());
    let description = normalize_text(event.property_value("DESCRIPTION"));
    let location = normalize_text(event.property_value("LOCATION"));
    let timezone = start
        .timezone
        .clone()
        .or_else(|| calendar_timezone.map(str::to_owned));
    let base_uid = normalize_text(event.property_value("UID"))
        .unwrap_or_else(|| synthesize_uid(&title, &start.iso));
    let event_type = infer_event_type(&title, description.as_deref()).to_owned();
    let opponent = infer_opponent(&title, description.as_deref());
    let end_offset = end.as_ref().map(|value| value.sort_utc - start.sort_utc);

    let template = EventTemplate {
        uid: base_uid,
        title,
        end_offset,
        timezone,
        status,
        is_all_day: start.is_all_day,
        location,
        description,
        event_type,
        opponent,
    };

    if has_recurrence_rules(event) {
        let occurrences = expand_recurrences(event, &template, now_utc, window_end, options);
        if !occurrences.is_empty() {
            return occurrences;
        }
    }

    build_single_event(start, &template, now_utc, window_end, options)
        .into_iter()
        .collect()
}

fn has_recurrence_rules(event: &icalendar::Event) -> bool {
    event.properties().contains_key("RRULE")
        || event.multi_properties().contains_key("RDATE")
        || event.multi_properties().contains_key("EXDATE")
}

fn expand_recurrences(
    event: &icalendar::Event,
    template: &EventTemplate,
    now_utc: DateTime<Utc>,
    window_end: DateTime<Utc>,
    options: &CliOptions,
) -> Vec<SortableEvent> {
    let recurrence_budget = recurrence_budget(options.limit, now_utc, window_end);
    let Ok(rrules) = event.get_recurrence() else {
        return Vec::new();
    };

    rrules
        .all(recurrence_budget)
        .dates
        .into_iter()
        .filter_map(|occurrence| {
            let sort_start = occurrence.with_timezone(&Utc);
            if sort_start < now_utc || sort_start > window_end {
                return None;
            }

            let source_timezone = occurrence.timezone().to_string();
            let (start_iso, occurrence_timezone) = format_output_datetime(
                sort_start,
                None,
                Some(&source_timezone),
                options.display_timezone,
            );
            let end_datetime = template.end_offset.map(|offset| {
                format_output_datetime(
                    sort_start + offset,
                    None,
                    occurrence_timezone.as_deref(),
                    options.display_timezone,
                )
                .0
            });

            Some(SortableEvent {
                sort_start,
                output: EventOutput {
                    uid: format!("{}::{}", template.uid, start_iso),
                    recurrence_parent_uid: Some(template.uid.clone()),
                    title: template.title.clone(),
                    start_datetime: start_iso,
                    end_datetime,
                    timezone: occurrence_timezone,
                    status: template.status.clone(),
                    is_all_day: template.is_all_day,
                    location: template.location.clone(),
                    description: template.description.clone(),
                    event_type: template.event_type.clone(),
                    opponent: template.opponent.clone(),
                },
            })
        })
        .collect()
}

fn recurrence_budget(limit: usize, now_utc: DateTime<Utc>, window_end: DateTime<Utc>) -> u16 {
    let day_span = (window_end - now_utc).num_days().max(1) as usize;
    let budget = day_span.saturating_mul(4).max(limit.saturating_mul(8)).max(64);
    budget.min(u16::MAX as usize) as u16
}

fn build_single_event(
    start: ParsedDateTimeValue,
    template: &EventTemplate,
    now_utc: DateTime<Utc>,
    window_end: DateTime<Utc>,
    options: &CliOptions,
) -> Option<SortableEvent> {
    let compare_utc = template
        .end_offset
        .map_or(start.sort_utc, |offset| start.sort_utc + offset);
    if compare_utc < now_utc || start.sort_utc > window_end {
        return None;
    }

    let (start_datetime, timezone) = format_output_datetime(
        start.sort_utc,
        Some(&start.iso),
        template.timezone.as_deref(),
        options.display_timezone,
    );
    let end_datetime = template.end_offset.map(|offset| {
        format_output_datetime(
            start.sort_utc + offset,
            None,
            timezone.as_deref(),
            options.display_timezone,
        )
        .0
    });

    Some(SortableEvent {
        sort_start: start.sort_utc,
        output: EventOutput {
            uid: template.uid.clone(),
            recurrence_parent_uid: None,
            title: template.title.clone(),
            start_datetime,
            end_datetime,
            timezone,
            status: template.status.clone(),
            is_all_day: start.is_all_day,
            location: template.location.clone(),
            description: template.description.clone(),
            event_type: template.event_type.clone(),
            opponent: template.opponent.clone(),
        },
    })
}

fn parse_datetime_property(
    property: &icalendar::Property,
    calendar_timezone: Option<&str>,
) -> Option<ParsedDateTimeValue> {
    let value = property.value().trim();
    let timezone_hint = property
        .params()
        .get("TZID")
        .map(|param| param.value().trim())
        .filter(|value| !value.is_empty())
        .or(calendar_timezone);

    let is_all_day = property
        .params()
        .get("VALUE")
        .map(|param| param.value().eq_ignore_ascii_case("DATE"))
        .unwrap_or(false)
        || value.len() == 8;

    if is_all_day {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        let start_of_day = date.and_hms_opt(0, 0, 0)?;
        return resolve_localized_datetime(start_of_day, timezone_hint, true);
    }

    if let Some(utc_value) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc_value, "%Y%m%dT%H%M%S").ok()?;
        let utc_datetime = Utc.from_utc_datetime(&naive);
        return Some(ParsedDateTimeValue {
            iso: utc_datetime.to_rfc3339(),
            sort_utc: utc_datetime,
            timezone: Some("UTC".to_owned()),
            is_all_day: false,
        });
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    resolve_localized_datetime(naive, timezone_hint, false)
}

fn resolve_localized_datetime(
    naive: NaiveDateTime,
    timezone_hint: Option<&str>,
    is_all_day: bool,
) -> Option<ParsedDateTimeValue> {
    if let Some(timezone_name) = timezone_hint
        && let Some(timezone) = parse_timezone(timezone_name)
    {
        let localized = match timezone.from_local_datetime(&naive) {
            LocalResult::Single(value) => value,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => return None,
        };

        return Some(ParsedDateTimeValue {
            iso: localized.to_rfc3339(),
            sort_utc: localized.with_timezone(&Utc),
            timezone: Some(timezone.name().to_owned()),
            is_all_day,
        });
    }

    let localized = match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => value,
        LocalResult::Ambiguous(first, _) => first,
        LocalResult::None => return None,
    };

    Some(ParsedDateTimeValue {
        iso: localized.to_rfc3339(),
        sort_utc: localized.with_timezone(&Utc),
        timezone: timezone_hint.map(str::to_owned).or_else(|| Some("floating".to_owned())),
        is_all_day,
    })
}

fn format_output_datetime(
    sort_utc: DateTime<Utc>,
    fallback_iso: Option<&str>,
    fallback_timezone: Option<&str>,
    display_timezone: Option<Tz>,
) -> (String, Option<String>) {
    if let Some(timezone) = display_timezone {
        let localized = sort_utc.with_timezone(&timezone);
        return (localized.to_rfc3339(), Some(timezone.name().to_owned()));
    }

    if let Some(iso) = fallback_iso {
        return (iso.to_owned(), fallback_timezone.map(str::to_owned));
    }

    match fallback_timezone {
        Some("floating") => {
            let localized = sort_utc.with_timezone(&Local);
            (localized.to_rfc3339(), Some("floating".to_owned()))
        }
        Some(timezone_name) => {
            if let Some(timezone) = parse_timezone(timezone_name) {
                let localized = sort_utc.with_timezone(&timezone);
                (localized.to_rfc3339(), Some(timezone.name().to_owned()))
            } else {
                (sort_utc.to_rfc3339(), Some(timezone_name.to_owned()))
            }
        }
        None => (sort_utc.to_rfc3339(), Some("UTC".to_owned())),
    }
}

fn parse_timezone(value: &str) -> Option<Tz> {
    match value.trim_matches('"') {
        "Pacific Standard Time" => Some(chrono_tz::America::Los_Angeles),
        "Mountain Standard Time" => Some(chrono_tz::America::Denver),
        "Central Standard Time" => Some(chrono_tz::America::Chicago),
        "Eastern Standard Time" => Some(chrono_tz::America::New_York),
        timezone => timezone.parse::<Tz>().ok(),
    }
}

fn infer_event_type(title: &str, description: Option<&str>) -> &'static str {
    let haystack = match description {
        Some(description) => format!("{} {}", title, description),
        None => title.to_owned(),
    }
    .to_lowercase();

    if ["practice", "training", "workout", "tryout", "warmup"]
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        return "practice";
    }

    if ["tournament", "showcase", "pool play", "playoff", "championship"]
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        return "tournament";
    }

    if ["game", "match", "scrimmage", "versus", " against "]
        .iter()
        .any(|needle| haystack.contains(needle))
        || haystack.contains(" vs ")
        || haystack.contains(" vs.")
        || haystack.contains(" @ ")
    {
        return "game";
    }

    "event"
}

fn infer_opponent(title: &str, description: Option<&str>) -> Option<String> {
    [title, description.unwrap_or("")]
        .into_iter()
        .find_map(extract_opponent)
}

fn extract_opponent(text: &str) -> Option<String> {
    for regex in [&*OPPONENT_VERSUS_REGEX, &*OPPONENT_AT_REGEX] {
        if let Some(captures) = regex.captures(text) {
            let candidate = captures.get(1)?.as_str().trim();
            let cleaned = candidate
                .trim_matches(|character: char| character == '.' || character == ':' || character == '-')
                .trim();

            if !cleaned.is_empty() {
                return Some(cleaned.to_owned());
            }
        }
    }

    None
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn synthesize_uid(title: &str, start_iso: &str) -> String {
    format!("{}::{}", title, start_iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CALENDAR: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Julie Softball\r\nX-WR-TIMEZONE:America/Los_Angeles\r\nBEGIN:VEVENT\r\nUID:event-1\r\nSUMMARY:Julie Softball vs Wildcats\r\nDESCRIPTION:League game against Wildcats at Field 3\r\nLOCATION:Central Park Field 3\r\nDTSTART;TZID=America/Los_Angeles:20990412T183000\r\nDTEND;TZID=America/Los_Angeles:20990412T200000\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:event-2\r\nSUMMARY:Julie Softball Practice\r\nDTSTART;TZID=America/Los_Angeles:20990410T170000\r\nDTEND;TZID=America/Los_Angeles:20990410T183000\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    const SAMPLE_RECURRING_CALENDAR: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Julie Softball\r\nX-WR-TIMEZONE:America/Los_Angeles\r\nBEGIN:VEVENT\r\nUID:recurring-game\r\nSUMMARY:Julie Softball vs Wildcats\r\nDESCRIPTION:League game against Wildcats\r\nLOCATION:Central Park Field 3\r\nDTSTART;TZID=America/Los_Angeles:20990405T183000\r\nDTEND;TZID=America/Los_Angeles:20990405T200000\r\nRRULE:FREQ=WEEKLY;COUNT=4\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    const SAMPLE_UTC_CALENDAR: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:utc-game\r\nSUMMARY:Woodinville Falcons Varsity vs Redmond Varsity Mustangs\r\nDTSTART:20990414T020000Z\r\nDTEND:20990414T040000Z\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn test_options(limit: usize, days: i64) -> CliOptions {
        CliOptions {
            source: "sample.ics".to_owned(),
            days,
            limit,
            display_timezone: None,
            pretty: false,
        }
    }

    #[test]
    fn converts_webcal_to_https() {
        let url = normalize_source_url("webcal://example.com/team.ics?token=abc")
            .expect("webcal URL should normalize");

        assert_eq!(url.as_str(), "https://example.com/team.ics?token=abc");
    }

    #[test]
    fn extracts_future_sports_event_fields() {
        let calendar = SAMPLE_CALENDAR
            .parse::<Calendar>()
            .expect("sample calendar should parse");
        let source = LoadedSource {
            requested: "sample.ics".to_owned(),
            resolved: "sample.ics".to_owned(),
            kind: "file".to_owned(),
            contents: SAMPLE_CALENDAR.to_owned(),
        };
        let now = Utc
            .with_ymd_and_hms(2099, 4, 1, 0, 0, 0)
            .single()
            .expect("valid test datetime");

        let output = build_calendar_output(&calendar, &source, now, &test_options(10, 30));

        assert_eq!(output.calendar_name.as_deref(), Some("Julie Softball"));
        assert_eq!(output.events.len(), 2);

        let cancelled_event = &output.events[0];
        let event = &output.events[1];
        assert_eq!(cancelled_event.uid, "event-2");
        assert_eq!(cancelled_event.status.as_deref(), Some("CANCELLED"));
        assert_eq!(event.uid, "event-1");
        assert_eq!(event.title, "Julie Softball vs Wildcats");
        assert_eq!(event.opponent.as_deref(), Some("Wildcats"));
        assert_eq!(event.event_type, "game");
        assert_eq!(event.location.as_deref(), Some("Central Park Field 3"));
        assert_eq!(event.timezone.as_deref(), Some("America/Los_Angeles"));
    }

    #[test]
    fn excludes_cancelled_events_that_have_already_finished() {
        let calendar = SAMPLE_CALENDAR
            .parse::<Calendar>()
            .expect("sample calendar should parse");
        let source = LoadedSource {
            requested: "sample.ics".to_owned(),
            resolved: "sample.ics".to_owned(),
            kind: "file".to_owned(),
            contents: SAMPLE_CALENDAR.to_owned(),
        };
        let now = Utc
            .with_ymd_and_hms(2099, 4, 11, 2, 0, 0)
            .single()
            .expect("valid test datetime");

        let output = build_calendar_output(&calendar, &source, now, &test_options(10, 30));

        assert_eq!(output.events.len(), 1);
        assert!(output.events.iter().all(|event| event.uid != "event-2"));
    }

    #[test]
    fn expands_recurrence_instances_into_multiple_events() {
        let calendar = SAMPLE_RECURRING_CALENDAR
            .parse::<Calendar>()
            .expect("sample recurring calendar should parse");
        let source = LoadedSource {
            requested: "sample.ics".to_owned(),
            resolved: "sample.ics".to_owned(),
            kind: "file".to_owned(),
            contents: SAMPLE_RECURRING_CALENDAR.to_owned(),
        };
        let now = Utc
            .with_ymd_and_hms(2099, 4, 1, 0, 0, 0)
            .single()
            .expect("valid test datetime");

        let output = build_calendar_output(&calendar, &source, now, &test_options(10, 60));

        assert_eq!(output.events.len(), 4);
        assert!(output.events.iter().all(|event| event.recurrence_parent_uid.as_deref() == Some("recurring-game")));
        assert_eq!(output.events[0].uid, "recurring-game::2099-04-05T18:30:00-07:00");
        assert_eq!(output.events[1].uid, "recurring-game::2099-04-12T18:30:00-07:00");
    }

    #[test]
    fn applies_global_limit_filter() {
        let calendar = SAMPLE_RECURRING_CALENDAR
            .parse::<Calendar>()
            .expect("sample recurring calendar should parse");
        let source = LoadedSource {
            requested: "sample.ics".to_owned(),
            resolved: "sample.ics".to_owned(),
            kind: "file".to_owned(),
            contents: SAMPLE_RECURRING_CALENDAR.to_owned(),
        };
        let now = Utc
            .with_ymd_and_hms(2099, 4, 1, 0, 0, 0)
            .single()
            .expect("valid test datetime");

        let output = build_calendar_output(&calendar, &source, now, &test_options(2, 60));

        assert_eq!(output.events.len(), 2);
        assert_eq!(output.applied_filter.limit, 2);
        assert_eq!(output.applied_filter.lookahead_days, 60);
    }

    #[test]
    fn converts_output_to_requested_display_timezone() {
        let calendar = SAMPLE_UTC_CALENDAR
            .parse::<Calendar>()
            .expect("sample UTC calendar should parse");
        let source = LoadedSource {
            requested: "sample.ics".to_owned(),
            resolved: "sample.ics".to_owned(),
            kind: "file".to_owned(),
            contents: SAMPLE_UTC_CALENDAR.to_owned(),
        };
        let now = Utc
            .with_ymd_and_hms(2099, 4, 1, 0, 0, 0)
            .single()
            .expect("valid test datetime");
        let mut options = test_options(10, 30);
        options.display_timezone = Some(chrono_tz::America::Los_Angeles);

        let output = build_calendar_output(&calendar, &source, now, &options);

        assert_eq!(output.display_timezone.as_deref(), Some("America/Los_Angeles"));
        assert_eq!(output.events[0].start_datetime, "2099-04-13T19:00:00-07:00");
        assert_eq!(output.events[0].end_datetime.as_deref(), Some("2099-04-13T21:00:00-07:00"));
        assert_eq!(output.events[0].timezone.as_deref(), Some("America/Los_Angeles"));
    }

    #[test]
    fn infers_practice_event_type() {
        assert_eq!(infer_event_type("Julie Practice", None), "practice");
    }

    #[test]
    fn parses_cli_options_with_days_limit_timezone_and_pretty() {
        let options = parse_cli_options(
            [
                "--days",
                "45",
                "--limit",
                "6",
                "--display-timezone",
                "America/Los_Angeles",
                "--pretty",
                "sample.ics",
            ]
                .into_iter()
                .map(str::to_owned),
        )
        .expect("cli options should parse");

        assert_eq!(options.days, 45);
        assert_eq!(options.limit, 6);
        assert_eq!(options.display_timezone, Some(chrono_tz::America::Los_Angeles));
        assert!(options.pretty);
        assert_eq!(options.source, "sample.ics");
    }

    #[test]
    fn pretty_serialization_contains_newlines() {
        let output = CalendarOutput {
            source: SourceMetadata {
                requested: "sample.ics".to_owned(),
                resolved: "sample.ics".to_owned(),
                kind: "file".to_owned(),
            },
            generated_at: "2099-04-01T00:00:00Z".to_owned(),
            applied_filter: AppliedFilter {
                lookahead_days: 30,
                limit: 10,
            },
            display_timezone: Some("America/Los_Angeles".to_owned()),
            calendar_name: Some("Sample".to_owned()),
            calendar_timezone: Some("UTC".to_owned()),
            events: Vec::new(),
        };

        let serialized = serialize_output(&output, true).expect("pretty serialization should work");

        assert!(serialized.contains('\n'));
        assert!(serialized.contains("  \"source\""));
    }
}
