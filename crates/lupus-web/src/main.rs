use std::collections::BTreeMap;
use std::env;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use lupus::engine::Engine;
use lupus::error::ConvertError;
use lupus::format::Format;
use lupus::formats::{JsonCodec, TextCodec, XmlCodec};
use lupus::schema::{DecodeContext, EncodeContext};
use maud::{DOCTYPE, Markup, html};

const SAMPLE_JSON: &str = include_str!("./sample.json");
const SAMPLE_XML: &str = include_str!("./sample.xml");
const SAMPLE_TEXT: &str = include_str!("./sample.txt");
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
                error: Some("not found".to_string()),
                ..PageState::default()
            }),
        ),
    }
}

#[derive(Debug, Clone)]
struct PageState {
    input: String,
    output: String,
    from: String,
    to: String,
    error: Option<String>,
}

impl Default for PageState {
    fn default() -> Self {
        Self {
            input: SAMPLE_JSON.to_string(),
            output: String::new(),
            from: Format::Json.to_string(),
            to: Format::Xml.to_string(),
            error: None,
        }
    }
}

fn handle_form(body: &[u8]) -> PageState {
    let form = parse_form(body);
    let mut state = PageState {
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
        error: None,
    };

    match form.get("action").map(String::as_str) {
        Some("sample-json") => {
            state.input = SAMPLE_JSON.to_string();
            state.output.clear();
            state.from = Format::Json.to_string();
            state.to = Format::Xml.to_string();
        }
        Some("sample-xml") => {
            state.input = SAMPLE_XML.to_string();
            state.output.clear();
            state.from = Format::Xml.to_string();
            state.to = Format::Json.to_string();
        }
        Some("sample-text") => {
            state.input = SAMPLE_TEXT.to_string();
            state.output.clear();
            state.from = Format::Text.to_string();
            state.to = Format::Json.to_string();
        }
        Some("preset-json-xml") => {
            state.from = Format::Json.to_string();
            state.to = Format::Xml.to_string();
            if let Err(err) = convert_state(&mut state) {
                state.output.clear();
                state.error = Some(err.to_string());
            }
        }
        Some("preset-xml-json") => {
            state.from = Format::Xml.to_string();
            state.to = Format::Json.to_string();
            if let Err(err) = convert_state(&mut state) {
                state.output.clear();
                state.error = Some(err.to_string());
            }
        }
        Some("preset-text") => {
            state.to = Format::Text.to_string();
            if let Err(err) = convert_state(&mut state) {
                state.output.clear();
                state.error = Some(err.to_string());
            }
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
                state.error = Some(err.to_string());
            }
        }
    }

    state
}

fn convert_state(state: &mut PageState) -> Result<(), ConvertError> {
    let mut engine = Engine::new();
    engine.register(JsonCodec);
    engine.register(TextCodec);
    engine.register(XmlCodec);

    let decode_ctx = DecodeContext {
        schema: None,
    };
    let encode_ctx = EncodeContext {
        schema: None,
    };
    let from = normalize_format(&state.from)?;
    let to = normalize_format(&state.to)?;
    let output = engine.convert(state.input.as_bytes(), from, to, &decode_ctx, &encode_ctx)?;
    state.output = String::from_utf8(output).map_err(|err| {
        ConvertError::Serialization(format!("converted output is not valid utf-8: {err}"))
    })?;
    Ok(())
}

fn render_page(state: PageState) -> Markup {
    let status = if let Some(error) = &state.error {
        StatusView::Error(error.as_str())
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
                    header {
                        div class="brand" {
                            span class="brand-mark" { "Lupus" }
                       }
                    }
                    main {
                        form method="post" action="/" {
                            section class="tool-card" {
                                div class="conversion-bar" {
                                    (format_select("from", "From", &state.from))
                                    div class="switch-column" {
                                        button class="icon-button" type="submit" name="action" value="swap" title="Swap input and output formats" aria-label="Swap input and output formats" { "Swap" }
                                    }
                                    (format_select("to", "To", &state.to))
                                   div class="actions" {
                                        button class="primary" type="submit" name="action" value="convert" { "Convert" }
                                    }
                                }
                                div class="quick-row" {
                                    span class="quick-label" { "Presets" }
                                    button class="chip" type="submit" name="action" value="preset-json-xml" { "JSON to XML" }
                                    button class="chip" type="submit" name="action" value="preset-xml-json" { "XML to JSON" }
                                    button class="chip" type="submit" name="action" value="preset-text" { "Extract text" }
                                }
                            }
                            section class="workspace" {
                                (editor_panel(
                                    "Input",
                                    &format!("{} source", format_label(&state.from)),
                                    "input",
                                    &state.input,
                                    false,
                                    html! {
                                        button class="ghost" type="submit" name="action" value="sample-json" { "JSON" }
                                        button class="ghost" type="submit" name="action" value="sample-xml" { "XML" }
                                        button class="ghost" type="submit" name="action" value="sample-text" { "Text" }
                                    },
                                    text_stats(&state.input),
                                ))
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
                        (status_panel(status))
                    }
               }
            }
        }
    }
}

enum StatusView<'a> {
    Idle,
    Ready,
    Error(&'a str),
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
                (format_option(Format::Json.as_str(), "JSON / Data", selected))
                (format_option(Format::Xml.as_str(), "XML / Markup", selected))
                (format_option(Format::Text.as_str(), "Text", selected))
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
                div {
                    h2 { (title) }
                    p { (subtitle) }
                }
                div class="panel-actions" {
                    (actions)
                }
            }
            div class="editor-frame" {
                div class="gutter" aria-hidden="true" {
                    @for line in 1..=stats.lines.max(1).min(99) {
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
        StatusView::Error(error) => html! {
            div class="message problem" {
                strong { "Conversion failed" }
                span { (error) }
            }
        },
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
    match format {
        "json" => "JSON",
        "xml" => "XML",
        "text" => "Text",
        _ => "Unknown",
    }
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
    match format {
        "json" => Ok(Format::Json),
        "xml" => Ok(Format::Xml),
        "text" => Ok(Format::Text),
        value => Err(ConvertError::UnsupportedFormat(value.into())),
    }
}
