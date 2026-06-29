# Lupus

Lupus is a Rust conversion library for moving data between structured data,
markup, text, and binary formats through a shared intermediate representation.

The workspace also contains a small HTTP debugging application for testing
conversions interactively.

## Supported formats

| Format | Decode | Encode | Notes |
| --- | --- | --- | --- |
| JSON | Yes | Yes | Supports all internal data values |
| XML | Yes | Yes | Preserves elements, attributes, and text |
| CSV | Yes | Yes | Flat rows with identical string fields |
| HTTP Form | Yes | Yes | Flat objects with string values |
| Protobuf | Yes | Yes | Uses `tucana::shared::Value` |
| Text | Yes | Yes | UTF-8 text and text extraction |

Conversions that cannot preserve the source information return an
`InformationLoss` error instead of silently discarding or coercing values.

Structured data can optionally be validated with a standard JSON Schema Draft
2020-12 document. Validation works for data decoded from JSON, XML, CSV, HTTP
forms, and Protobuf.

## Run the debugging frontend

```sh
cargo run -p lupus-web
```

Open <http://127.0.0.1:7878>. Set `PORT` to use another port:

```sh
PORT=8080 cargo run -p lupus-web
```

The frontend includes sample inputs, formatted output, and visible decoding,
encoding, unsupported-format, and information-loss errors.

Protobuf input and output use Tucana's generated JSON representation of
`tucana::shared::Value` in the frontend.

## Library usage

```rust
use lupus::engine::Engine;
use lupus::format::Format;
use lupus::formats::{JsonCodec, XmlCodec};
use lupus::schema::{DecodeContext, EncodeContext};

let mut engine = Engine::new();
engine.register(JsonCodec);
engine.register(XmlCodec);

let output = engine.convert(
    br#"{"user":{"@id":"7","name":"Ada"}}"#,
    Format::Json,
    Format::Xml,
    &DecodeContext::default(),
    &EncodeContext { pretty: true },
)?;

assert_eq!(
    String::from_utf8(output)?,
    "<user id=\"7\">\n  <name>Ada</name>\n</user>"
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Register only the codecs required by the application. An unregistered format
returns `ConvertError::UnsupportedFormat`.

## Format rules

### JSON Schema validation

Call `Engine::validate` with the schema, source format, and input bytes:

```rust
use lupus::schema::{DecodeContext, JsonSchema};

let schema = JsonSchema {
    raw: r#"{
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string", "minLength": 1 }
        },
        "additionalProperties": false
    }"#
    .to_string(),
};

engine.validate(
    br#"{"name":"Ada"}"#,
    Format::Json,
    &schema,
    &DecodeContext::default(),
)?;
```

Invalid schema documents produce a decoding error. Objects that do not satisfy
the schema produce a validation error containing all reported instance paths.
