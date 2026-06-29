use std::collections::BTreeMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::{Data, Number};
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub struct JsonCodec;

impl Codec for JsonCodec {
    fn format(&self) -> Format {
        Format::Json
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Data
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let value: serde_json::Value =
            serde_json::from_slice(input).map_err(|err| ConvertError::Decoding(err.to_string()))?;
        Ok(Artifact::Data(json_value_to_data(value)?))
    }

    fn encode(&self, artifact: &Artifact, ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Data(data) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Data,
                found: artifact.kind(),
            });
        };

        let value = data_to_json_value(data)?;
        if ctx.pretty {
            serde_json::to_vec_pretty(&value).map_err(ConvertError::from)
        } else {
            serde_json::to_vec(&value).map_err(ConvertError::from)
        }
    }
}

fn json_value_to_data(value: serde_json::Value) -> Result<Data, ConvertError> {
    match value {
        serde_json::Value::Null => Ok(Data::Null),
        serde_json::Value::Bool(value) => Ok(Data::Bool(value)),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Data::Number(Number::I64(value)))
            } else if let Some(value) = value.as_u64() {
                Ok(Data::Number(Number::U64(value)))
            } else if let Some(value) = value.as_f64() {
                Ok(Data::Number(Number::F64(value)))
            } else {
                Err(ConvertError::InvalidConversion(
                    "unsupported JSON number".to_string(),
                ))
            }
        }
        serde_json::Value::String(value) => Ok(Data::String(value)),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(json_value_to_data)
            .collect::<Result<Vec<_>, _>>()
            .map(Data::Array),
        serde_json::Value::Object(fields) => fields
            .into_iter()
            .map(|(key, value)| Ok((key, json_value_to_data(value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Data::Object),
    }
}

pub(crate) fn data_to_json_value(data: &Data) -> Result<serde_json::Value, ConvertError> {
    match data {
        Data::Null => Ok(serde_json::Value::Null),
        Data::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        Data::Number(Number::I64(value)) => Ok(serde_json::Value::Number((*value).into())),
        Data::Number(Number::U64(value)) => Ok(serde_json::Value::Number((*value).into())),
        Data::Number(Number::F64(value)) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                ConvertError::InvalidConversion(
                    "non-finite float cannot encode as JSON".to_string(),
                )
            }),
        Data::String(value) => Ok(serde_json::Value::String(value.clone())),
        Data::Array(values) => values
            .iter()
            .map(data_to_json_value)
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        Data::Object(fields) => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), data_to_json_value(value)?)))
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
    }
}
