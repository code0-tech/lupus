use lupus::engine::Engine;
use lupus::error::ConvertError;
use lupus::format::Format;
use lupus::schema::{DecodeContext, EncodeContext, JsonSchema};
use maud::{DOCTYPE, Markup, html};
use std::collections::BTreeMap;
use std::env;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const SAMPLE_JSON: &str = include_str!("./sample.json");
const SAMPLE_XML: &str = include_str!("./sample.xml");
const SAMPLE_TEXT: &str = include_str!("./sample.txt");
const SAMPLE_JSON_CSV: &str = r#"[
  {
    "email": "ada@example.com",
    "name": "Ada Lovelace"
  },
  {
    "email": "grace@example.com",
    "name": "Grace Hopper"
  }
]"#;
const SAMPLE_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["name", "email"],
  "properties": {
    "name": {
      "type": "string",
      "minLength": 2
    },
    "email": {
      "type": "string",
      "format": "email"
    }
  },
  "additionalProperties": false
}"#;
const SAMPLE_VALIDATION_INPUT: &str = r#"{
  "name": "Ada Lovelace",
  "email": "ada@example.com"
}"#;
const SAMPLE_VALIDATION_XML: &str = r#"<user>
  <name>Ada Lovelace</name>
  <email>ada@example.com</email>
</user>"#;
const SAMPLE_VALIDATION_XML_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["user"],
  "properties": {
    "user": {
      "type": "object",
      "required": ["name", "email"],
      "properties": {
        "name": { "type": "string", "minLength": 2 },
        "email": { "type": "string", "format": "email" }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}"#;
const SAMPLE_VALIDATION_CSV: &str =
    "email,name\nada@example.com,Ada Lovelace\ngrace@example.com,Grace Hopper\n";
const SAMPLE_VALIDATION_CSV_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "array",
  "minItems": 1,
  "items": {
    "type": "object",
    "required": ["name", "email"],
    "properties": {
      "name": { "type": "string", "minLength": 2 },
      "email": { "type": "string", "format": "email" }
    },
    "additionalProperties": false
  }
}"#;
const SAMPLE_VALIDATION_FORM: &str = "email=ada%40example.com&name=Ada+Lovelace";
const SAMPLE_VALIDATION_TUCANA: &str = r#"{
  "structValue": {
    "fields": {
      "email": {
        "stringValue": "ada@example.com"
      },
      "name": {
        "stringValue": "Ada Lovelace"
      }
    }
  }
}"#;
const CSS: &str = include_str!("./styles.css");

fn main() -> io::Result<()> {
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(7878);
    let listener = bind_with_fallback(port)?;

    println!("conversion viewer: http://{}", listener.local_addr()?);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(err) = handle_connection(&mut stream) {
                    eprintln!("request failed: {err}");
                }
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }

    Ok(())
}

fn bind_with_fallback(start_port: u16) -> io::Result<TcpListener> {
    for port in start_port..start_port.saturating_add(20) {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(listener),
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        "no free local port found",
    ))
}

fn handle_connection(stream: &mut TcpStream) -> io::Result<()> {
    let request = read_request(stream)?;

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => write_html(stream, 200, &render_page(PageState::default())),
        ("POST", "/") => write_html(stream, 200, &render_page(handle_form(&request.body))),
        ("GET", "/favicon.ico") => write_html(stream, 204, &Markup::default()),
        _ => write_html(
            stream,
            404,
            &render_page(PageState {
                error: Some(ConvertError::InvalidConversion("not found".to_string())),
                ..PageState::default()
            }),
        ),
    }
}

#[derive(Debug, Clone)]
struct PageState {
    mode: String,
    input: String,
    output: String,
    from: String,
    to: String,
    schema: String,
    validated: bool,
    error: Option<ConvertError>,
}

impl Default for PageState {
    fn default() -> Self {
        Self {
            mode: "conversion".to_string(),
            input: SAMPLE_JSON.to_string(),
            output: String::new(),
            from: Format::Json.to_string(),
            to: Format::Xml.to_string(),
            schema: String::new(),
            validated: false,
            error: None,
        }
    }
}

fn handle_form(body: &[u8]) -> PageState {
    let form = parse_form(body);
    let mut state = PageState {
        mode: form
            .get("mode")
            .cloned()
            .unwrap_or_else(|| "conversion".to_string()),
        input: form.get("input").cloned().unwrap_or_default(),
        output: form.get("output").cloned().unwrap_or_default(),
        from: form
            .get("from")
            .cloned()
            .unwrap_or_else(|| Format::Json.to_string()),
        to: form
            .get("to")
            .cloned()
            .unwrap_or_else(|| Format::Xml.to_string()),
        schema: form.get("schema").cloned().unwrap_or_default(),
        validated: false,
        error: None,
    };

    match form.get("action").map(String::as_str) {
        Some("tab-conversion") => {
            state.mode = "conversion".to_string();
        }
        Some("tab-validation") => {
            state.mode = "validation".to_string();
        }
        Some("validate") => {
            state.mode = "validation".to_string();
            match validate_state(&state) {
                Ok(()) => state.validated = true,
                Err(err) => state.error = Some(err),
            }
        }
        Some("apply-conversion-preset") => {
            apply_conversion_preset(
                &mut state,
                form.get("preset").map(String::as_str).unwrap_or_default(),
            );
        }
        Some("apply-validation-preset") => {
            apply_validation_preset(
                &mut state,
                form.get("validation-preset")
                    .map(String::as_str)
                    .unwrap_or_default(),
            );
        }
        Some("swap") => {
            std::mem::swap(&mut state.from, &mut state.to);
            if !state.output.is_empty() {
                state.input = std::mem::take(&mut state.output);
            }
        }
        Some("clear-output") => {
            state.output.clear();
        }
        _ => {
            if let Err(err) = convert_state(&mut state) {
                state.output.clear();
                state.error = Some(err);
            }
        }
    }

    state
}

fn set_validation_preset(state: &mut PageState, format: Format, schema: &str, input: &str) {
    state.mode = "validation".to_string();
    state.from = format.to_string();
    state.schema = schema.to_string();
    state.input = input.to_string();
    state.output.clear();
}

fn apply_conversion_preset(state: &mut PageState, preset: &str) {
    match preset {
        "json" => {
            state.input = SAMPLE_JSON.to_string();
            state.from = Format::Json.to_string();
            state.to = Format::Xml.to_string();
        }
        "xml" => {
            state.input = SAMPLE_XML.to_string();
            state.from = Format::Xml.to_string();
            state.to = Format::Json.to_string();
        }
        "text" => {
            state.input = SAMPLE_TEXT.to_string();
            state.from = Format::Text.to_string();
            state.to = Format::Json.to_string();
        }
        "json-csv" => {
            state.input = SAMPLE_JSON_CSV.to_string();
            state.from = Format::Json.to_string();
            state.to = Format::Csv.to_string();
        }
        "json-xml" => {
            state.from = Format::Json.to_string();
            state.to = Format::Xml.to_string();
        }
        "xml-json" => {
            state.from = Format::Xml.to_string();
            state.to = Format::Json.to_string();
        }
        "extract-text" => state.to = Format::Text.to_string(),
        _ => {}
    }
    state.output.clear();
}

fn apply_validation_preset(state: &mut PageState, preset: &str) {
    match preset {
        "json" => {
            set_validation_preset(state, Format::Json, SAMPLE_SCHEMA, SAMPLE_VALIDATION_INPUT)
        }
        "xml" => set_validation_preset(
            state,
            Format::Xml,
            SAMPLE_VALIDATION_XML_SCHEMA,
            SAMPLE_VALIDATION_XML,
        ),
        "csv" => set_validation_preset(
            state,
            Format::Csv,
            SAMPLE_VALIDATION_CSV_SCHEMA,
            SAMPLE_VALIDATION_CSV,
        ),
        "form" => set_validation_preset(
            state,
            Format::HttpForm,
            SAMPLE_SCHEMA,
            SAMPLE_VALIDATION_FORM,
        ),
        "tucana" => set_validation_preset(
            state,
            Format::Protobuf,
            SAMPLE_SCHEMA,
            SAMPLE_VALIDATION_TUCANA,
        ),
        _ => {}
    }
}

fn convert_state(state: &mut PageState) -> Result<(), ConvertError> {
    let engine = configured_engine();
    let decode_ctx = DecodeContext;
    let encode_ctx = EncodeContext { pretty: true };
    let from = normalize_format(&state.from)?;
    let to = normalize_format(&state.to)?;
    let output = engine.convert(state.input.as_bytes(), from, to, &decode_ctx, &encode_ctx)?;
    state.output = String::from_utf8(output).map_err(|err| {
        ConvertError::Encoding(format!("converted output is not valid utf-8: {err}"))
    })?;
    Ok(())
}

fn validate_state(state: &PageState) -> Result<(), ConvertError> {
    if state.schema.trim().is_empty() {
        return Err(ConvertError::Decoding(
            "a JSON Schema document is required".to_string(),
        ));
    }
    let engine = configured_engine();
    let format = normalize_format(&state.from)?;
    let schema = JsonSchema {
        raw: state.schema.clone(),
    };
    engine.validate(state.input.as_bytes(), format, &schema, &DecodeContext)
}

fn configured_engine() -> Engine {
    Engine::with_default_codecs()
}

fn render_page(state: PageState) -> Markup {
    let status = if let Some(error) = &state.error {
        StatusView::Error(error)
    } else if state.validated {
        StatusView::Valid
    } else if state.output.is_empty() {
        StatusView::Idle
    } else {
        StatusView::Ready
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Lupus Converter" }
                style { (CSS) }
            }
            body {
                div class="shell" {
                    main {
                        form method="post" action="/" {
                            input type="hidden" name="mode" value=(state.mode);
                            nav class="tabs" aria-label="Converter tools" {
                                button
                                    class=(if state.mode == "conversion" { "tab active" } else { "tab" })
                                    type="submit"
                                    name="action"
                                    value="tab-conversion"
                                { "Schema conversion" }
                                button
                                    class=(if state.mode == "validation" { "tab active" } else { "tab" })
                                    type="submit"
                                    name="action"
                                    value="tab-validation"
                                { "Schema validation" }
                            }
                            section class="tab-content" {
                                @if state.mode == "validation" {
                                    (validation_tab(&state, status))
                                } @else {
                                    (conversion_tab(&state, status))
                                }
                            }
                        }
                    }
               }
            }
        }
    }
}

fn conversion_tab(state: &PageState, status: StatusView<'_>) -> Markup {
    html! {
        section class="tool-card command-bar" {
            div class="conversion-bar" {
                (format_select("from", "From", &state.from))
                div class="switch-column" {
                    button class="icon-button" type="submit" name="action" value="swap" { "Swap" }
                }
                (format_select("to", "To", &state.to))
                div class="actions" {
                    button class="primary" type="submit" name="action" value="convert" { "Convert" }
                }
            }
            div class="preset-control" {
                label class="field" {
                    span class="field-label" { "Preset" }
                    select name="preset" {
                        option value="json" { "JSON sample" }
                        option value="xml" { "XML sample" }
                        option value="text" { "Text sample" }
                        option value="json-csv" { "JSON rows for CSV" }
                        option value="json-xml" { "JSON → XML" }
                        option value="xml-json" { "XML → JSON" }
                        option value="extract-text" { "Extract text" }
                    }
                }
                button type="submit" name="action" value="apply-conversion-preset" { "Apply" }
            }
        }
        (status_panel(status))
        section class="workspace" {
            (input_editor(state))
            (editor_panel(
                "Output",
                &format!("{} result", format_label(&state.to)),
                "output",
                &state.output,
                true,
                html! {
                    button class="ghost" type="submit" name="action" value="clear-output" { "Clear" }
                },
                text_stats(&state.output),
            ))
        }
    }
}

fn validation_tab(state: &PageState, status: StatusView<'_>) -> Markup {
    html! {
        section class="tool-card validation-bar command-bar" {
            (format_select("from", "Input format", &state.from))
            div class="preset-control" {
                label class="field" {
                    span class="field-label" { "Example" }
                    select name="validation-preset" {
                        option value="json" { "JSON object" }
                        option value="xml" { "XML document" }
                        option value="csv" { "CSV rows" }
                        option value="form" { "HTTP Form" }
                        option value="tucana" { "Tucana Value" }
                    }
                }
                button type="submit" name="action" value="apply-validation-preset" { "Load" }
            }
            div class="actions" {
                button class="primary" type="submit" name="action" value="validate" { "Validate" }
            }
        }
        (status_panel(status))
        section class="workspace validation-workspace" {
            (editor_panel(
                "JSON Schema",
                "Draft 2020-12 validation schema",
                "schema",
                &state.schema,
                false,
                Markup::default(),
                text_stats(&state.schema),
            ))
            (input_editor(state))
        }
    }
}

fn input_editor(state: &PageState) -> Markup {
    editor_panel(
        "Input",
        &format!("{} source", format_label(&state.from)),
        "input",
        &state.input,
        false,
        Markup::default(),
        text_stats(&state.input),
    )
}

enum StatusView<'a> {
    Idle,
    Ready,
    Valid,
    Error(&'a ConvertError),
}

struct TextStats {
    lines: usize,
    chars: usize,
    bytes: usize,
}

fn format_select(name: &'static str, label: &'static str, selected: &str) -> Markup {
    html! {
        label class="field" {
            span class="field-label" { (label) }
            select name=(name) {
                @for format in Format::ALL {
                    (format_option(format.as_str(), format.label(), selected))
                }
            }
        }
    }
}

fn editor_panel(
    title: &'static str,
    subtitle: &str,
    name: &'static str,
    value: &str,
    readonly: bool,
    actions: Markup,
    stats: TextStats,
) -> Markup {
    html! {
        div class="editor-panel" {
            div class="editor-head" {
                div class="editor-title" {
                    h2 { (title) }
                    span { (subtitle) }
                }
                div class="panel-actions" {
                    (actions)
                }
            }
            div class="editor-frame" {
                div class="gutter" aria-hidden="true" {
                    @for line in 1..=stats.lines.clamp(1, 99) {
                        span { (line) }
                    }
                }
                textarea name=(name) spellcheck="false" readonly[readonly] { (value) }
            }
            div class="metrics" {
                (metric("lines", stats.lines))
                (metric("chars", stats.chars))
                (metric("bytes", stats.bytes))
            }
        }
    }
}

fn metric(label: &'static str, value: usize) -> Markup {
    html! {
        span class="metric" {
            strong { (value) }
            " "
            (label)
        }
    }
}

fn status_panel(status: StatusView<'_>) -> Markup {
    match status {
        StatusView::Idle => html! {
            div class="message neutral" {
                strong { "Idle" }
                span { "Choose formats, paste input, then submit the form." }
            }
        },
        StatusView::Ready => html! {
            div class="message ready" {
                strong { "Converted" }
                span { "The output pane contains the rendered result." }
            }
        },
        StatusView::Valid => html! {
            div class="message ready" {
                strong { "Valid" }
                span { "The input satisfies the JSON Schema." }
            }
        },
        StatusView::Error(error) => html! {
            div class="message problem" role="alert" {
                div class="error-heading" {
                    strong { (error_title(error)) }
                    span class="error-badge" { "Conversion failed" }
                }
                code { (error.to_string()) }
            }
        },
    }
}

fn error_title(error: &ConvertError) -> &'static str {
    match error {
        ConvertError::Decoding(_) => "Input could not be decoded",
        ConvertError::Encoding(_) => "Output could not be encoded",
        ConvertError::InformationLoss(_) => "Conversion would lose information",
        ConvertError::Validation(_) => "Object does not match the JSON Schema",
        ConvertError::UnsupportedFormat(_) => "Unsupported format",
        ConvertError::WrongArtifact { .. } => "Incompatible artifact",
        ConvertError::InvalidConversion(_) => "Invalid conversion",
    }
}

fn format_option(value: &'static str, label: &'static str, selected: &str) -> Markup {
    html! {
        option value=(value) selected[value == selected] { (label) }
    }
}

fn text_stats(value: &str) -> TextStats {
    TextStats {
        lines: value.lines().count().max(1),
        chars: value.chars().count(),
        bytes: value.len(),
    }
}

fn format_label(format: &str) -> &'static str {
    Format::try_from(format).map_or("Unknown", Format::label)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut buffer = Vec::new();
    let mut chunk = [0; 2048];

    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);

        let Some(header_end) = find_header_end(&buffer) else {
            continue;
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = content_length(&headers);
        let total = header_end + 4 + content_length;
        if buffer.len() >= total {
            break;
        }
    }

    let Some(header_end) = find_header_end(&buffer) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing HTTP headers",
        ));
    };

    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    let body_start = header_end + 4;
    let length = content_length(&headers);
    let body_end = body_start.saturating_add(length).min(buffer.len());

    Ok(HttpRequest {
        method,
        path,
        body: buffer[body_start..body_end].to_vec(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn write_html(stream: &mut TcpStream, status: u16, markup: &Markup) -> io::Result<()> {
    let body = markup.clone().into_string();
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())
}

fn parse_form(body: &[u8]) -> BTreeMap<String, String> {
    let body = String::from_utf8_lossy(body);
    body.split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            Some((decode_form_component(key)?, decode_form_component(value)?))
        })
        .collect()
}

fn decode_form_component(input: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut iter = input.as_bytes().iter().copied();

    while let Some(byte) = iter.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let high = hex_value(iter.next()?)?;
                let low = hex_value(iter.next()?)?;
                bytes.push(high << 4 | low);
            }
            byte => bytes.push(byte),
        }
    }

    String::from_utf8(bytes).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn normalize_format(format: &str) -> Result<Format, ConvertError> {
    Format::try_from(format).map_err(|()| ConvertError::UnsupportedFormat(format.into()))
}

#[cfg(test)]
mod tests {
    use super::{handle_form, validate_state};

    #[test]
    fn every_validation_preset_is_valid() {
        for preset in ["json", "xml", "csv", "form", "tucana"] {
            let body = format!("action=apply-validation-preset&validation-preset={preset}");
            let state = handle_form(body.as_bytes());
            validate_state(&state).unwrap_or_else(|error| {
                panic!("validation preset {preset} failed: {error}");
            });
        }
    }
}
