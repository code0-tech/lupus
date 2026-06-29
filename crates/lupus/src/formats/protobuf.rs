use std::collections::{BTreeMap, HashMap};

use tucana::shared::{ListValue, NumberValue, Struct, Value, number_value, value::Kind};

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::{Data, Number};
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub struct ProtobufCodec;

impl Codec for ProtobufCodec {
    fn format(&self) -> Format {
        Format::Protobuf
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Data
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let value: Value = serde_json::from_slice(input).map_err(|err| {
            ConvertError::Decoding(format!("invalid Tucana Protobuf Value object: {err}"))
        })?;
        value_to_data(value).map(Artifact::Data)
    }

    fn encode(&self, artifact: &Artifact, ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Data(data) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Data,
                found: artifact.kind(),
            });
        };

        let value = data_to_value(data)?;
        if ctx.pretty {
            serde_json::to_vec_pretty(&value).map_err(ConvertError::from)
        } else {
            serde_json::to_vec(&value).map_err(ConvertError::from)
        }
    }
}

fn data_object_to_struct(fields: &BTreeMap<String, Data>) -> Result<Struct, ConvertError> {
    fields
        .iter()
        .map(|(key, value)| Ok((key.clone(), data_to_value(value)?)))
        .collect::<Result<HashMap<_, _>, _>>()
        .map(|fields| Struct { fields })
}

pub fn data_to_value(data: &Data) -> Result<Value, ConvertError> {
    let kind = match data {
        Data::Null => Kind::NullValue(0),
        Data::Bool(value) => Kind::BoolValue(*value),
        Data::Number(Number::I64(value)) => Kind::NumberValue(NumberValue {
            number: Some(number_value::Number::Integer(*value)),
        }),
        Data::Number(Number::U64(value)) => {
            let value = i64::try_from(*value).map_err(|_| {
                ConvertError::InformationLoss(
                    "Tucana integer values cannot represent u64 values above i64::MAX".to_string(),
                )
            })?;
            Kind::NumberValue(NumberValue {
                number: Some(number_value::Number::Integer(value)),
            })
        }
        Data::Number(Number::F64(value)) => Kind::NumberValue(NumberValue {
            number: Some(number_value::Number::Float(*value)),
        }),
        Data::String(value) => Kind::StringValue(value.clone()),
        Data::Array(values) => Kind::ListValue(ListValue {
            values: values
                .iter()
                .map(data_to_value)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Data::Object(fields) => Kind::StructValue(data_object_to_struct(fields)?),
    };
    Ok(Value { kind: Some(kind) })
}

fn struct_to_data(value: Struct) -> Result<Data, ConvertError> {
    value
        .fields
        .into_iter()
        .map(|(key, value)| Ok((key, value_to_data(value)?)))
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map(Data::Object)
}

pub fn value_to_data(value: Value) -> Result<Data, ConvertError> {
    match value.kind {
        Some(Kind::NullValue(_)) => Ok(Data::Null),
        Some(Kind::BoolValue(value)) => Ok(Data::Bool(value)),
        Some(Kind::NumberValue(value)) => match value.number {
            Some(number_value::Number::Integer(value)) => Ok(Data::Number(Number::I64(value))),
            Some(number_value::Number::Float(value)) => Ok(Data::Number(Number::F64(value))),
            None => Err(ConvertError::Decoding(
                "Tucana NumberValue has no number".to_string(),
            )),
        },
        Some(Kind::StringValue(value)) => Ok(Data::String(value)),
        Some(Kind::ListValue(value)) => value
            .values
            .into_iter()
            .map(value_to_data)
            .collect::<Result<Vec<_>, _>>()
            .map(Data::Array),
        Some(Kind::StructValue(value)) => struct_to_data(value),
        None => Err(ConvertError::Decoding(
            "Tucana Value has no kind".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::artifact::Artifact;
    use crate::codec::Codec;
    use crate::data::Data;
    use crate::formats::ProtobufCodec;
    use crate::schema::{DecodeContext, EncodeContext};

    #[test]
    fn protobuf_value_object_round_trips_nested_data() {
        let data = Data::Object(BTreeMap::from([(
            "user".to_string(),
            Data::Object(BTreeMap::from([
                ("active".to_string(), Data::Bool(true)),
                ("name".to_string(), Data::String("Ada".to_string())),
                (
                    "roles".to_string(),
                    Data::Array(vec![Data::String("admin".to_string())]),
                ),
            ])),
        )]));
        let codec = ProtobufCodec;
        let encoded = codec
            .encode(&Artifact::Data(data.clone()), &EncodeContext::default())
            .unwrap();
        let decoded = codec.decode(&encoded, &DecodeContext::default()).unwrap();
        assert_eq!(decoded, Artifact::Data(data));
        let rendered = String::from_utf8(encoded).unwrap();
        assert!(rendered.contains("\"structValue\""));
        assert!(rendered.contains("\"stringValue\":\"Ada\""));
    }
}
