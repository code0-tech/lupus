use std::collections::{BTreeMap, BTreeSet};

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::Data;
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub struct CsvCodec;

impl Codec for CsvCodec {
    fn format(&self) -> Format {
        Format::Csv
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Data
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let input = std::str::from_utf8(input)
            .map_err(|err| ConvertError::Decoding(format!("CSV is not valid UTF-8: {err}")))?;
        Ok(Artifact::Data(parse_csv(input)?))
    }

    fn encode(&self, artifact: &Artifact, _ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Data(data) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Data,
                found: artifact.kind(),
            });
        };

        Ok(write_csv(data)?.into_bytes())
    }
}

fn write_csv(data: &Data) -> Result<String, ConvertError> {
    let Data::Array(rows) = data else {
        return Err(information_loss(
            "CSV requires a top-level array of flat objects",
        ));
    };

    if rows.is_empty() {
        return Ok(String::new());
    }

    let Data::Object(first) = &rows[0] else {
        return Err(information_loss("each CSV row must be an object"));
    };
    let headers = first.keys().cloned().collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(information_loss("CSV rows must contain at least one field"));
    }

    let mut output = headers
        .iter()
        .map(|header| escape_field(header))
        .collect::<Vec<_>>()
        .join(",");
    output.push('\n');

    for row in rows {
        let Data::Object(fields) = row else {
            return Err(information_loss("each CSV row must be an object"));
        };
        if fields.len() != headers.len()
            || !headers.iter().all(|header| fields.contains_key(header))
        {
            return Err(information_loss(
                "all CSV rows must contain the same fields",
            ));
        }

        let values = headers
            .iter()
            .map(|header| match &fields[header] {
                Data::String(value) => Ok(escape_field(value)),
                _ => Err(information_loss(
                    "CSV fields must be strings; nested and typed values cannot round-trip",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        output.push_str(&values.join(","));
        output.push('\n');
    }

    Ok(output)
}

fn parse_csv(input: &str) -> Result<Data, ConvertError> {
    if input.is_empty() {
        return Ok(Data::Array(Vec::new()));
    }

    let rows = parse_rows(input)?;
    let Some(headers) = rows.first() else {
        return Ok(Data::Array(Vec::new()));
    };
    if headers.is_empty() || headers.iter().any(String::is_empty) {
        return Err(ConvertError::Decoding(
            "CSV headers cannot be empty".to_string(),
        ));
    }
    let unique = headers.iter().collect::<BTreeSet<_>>();
    if unique.len() != headers.len() {
        return Err(ConvertError::Decoding(
            "CSV headers must be unique".to_string(),
        ));
    }

    let mut data_rows = Vec::new();
    for (index, row) in rows.iter().enumerate().skip(1) {
        if row.len() != headers.len() {
            return Err(ConvertError::Decoding(format!(
                "CSV row {} has {} fields; expected {}",
                index + 1,
                row.len(),
                headers.len()
            )));
        }
        let fields = headers
            .iter()
            .cloned()
            .zip(row.iter().cloned().map(Data::String))
            .collect::<BTreeMap<_, _>>();
        data_rows.push(Data::Object(fields));
    }

    Ok(Data::Array(data_rows))
}

fn parse_rows(input: &str) -> Result<Vec<Vec<String>>, ConvertError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            _ => field.push(ch),
        }
    }

    if quoted {
        return Err(ConvertError::Decoding(
            "CSV contains an unterminated quoted field".to_string(),
        ));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

fn escape_field(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn information_loss(message: &str) -> ConvertError {
    ConvertError::InformationLoss(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_csv, write_csv};

    #[test]
    fn csv_round_trips_quoted_flat_rows() -> Result<(), Box<dyn std::error::Error>> {
        let input = "email,name\nada@example.com,\"Ada, Countess\"\n";
        let data = parse_csv(input)?;
        assert_eq!(write_csv(&data)?, input);
        Ok(())
    }
}
