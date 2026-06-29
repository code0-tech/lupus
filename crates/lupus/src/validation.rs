use crate::data::Data;
use crate::error::ConvertError;
use crate::formats::json::data_to_json_value;
use crate::schema::JsonSchema;

pub fn validate_json_schema(data: &Data, schema: &JsonSchema) -> Result<(), ConvertError> {
    let schema_value: serde_json::Value = serde_json::from_str(&schema.raw)
        .map_err(|err| ConvertError::Decoding(format!("invalid JSON Schema document: {err}")))?;
    jsonschema::draft202012::meta::validate(&schema_value).map_err(|err| {
        ConvertError::Decoding(format!("invalid JSON Schema Draft 2020-12 schema: {err}"))
    })?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema_value)
        .map_err(|err| ConvertError::Decoding(format!("could not compile JSON Schema: {err}")))?;
    let instance = data_to_json_value(data)?;
    let errors = validator
        .iter_errors(&instance)
        .map(|error| {
            let path = error.instance_path().to_string();
            if path.is_empty() {
                format!("at $: {error}")
            } else {
                format!("at ${path}: {error}")
            }
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ConvertError::Validation(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::data::Data;
    use crate::error::ConvertError;
    use crate::schema::JsonSchema;
    use crate::validation::validate_json_schema;

    #[test]
    fn reports_all_instance_validation_errors_with_paths() {
        let schema = JsonSchema {
            raw: r#"{
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "required": ["name", "email"],
                "properties": {
                    "name": { "type": "string", "minLength": 2 },
                    "email": { "type": "string" }
                },
                "additionalProperties": false
            }"#
            .to_string(),
        };
        let data = Data::Object(BTreeMap::from([
            ("name".to_string(), Data::String(String::new())),
            ("extra".to_string(), Data::Bool(true)),
        ]));

        let error = validate_json_schema(&data, &schema).unwrap_err();
        let ConvertError::Validation(message) = error else {
            panic!("expected validation error");
        };
        assert!(message.contains("email"));
        assert!(message.to_lowercase().contains("additional properties"));
        assert!(message.contains("$/name"));
    }
}
