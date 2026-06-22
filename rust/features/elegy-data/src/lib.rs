use csv::{ReaderBuilder, WriterBuilder};
use jsonschema::JSONSchema;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataFormat {
    Json,
    Yaml,
    Toml,
    Csv,
}

impl DataFormat {
    fn parse(input: &str) -> Result<Self, DataError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "toml" => Ok(Self::Toml),
            "csv" => Ok(Self::Csv),
            format => Err(DataError::UnsupportedFormat {
                format: format.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueryKind {
    JsonPointer,
    DottedPath,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub from_format: DataFormat,
    pub to_format: DataFormat,
    pub value: Value,
    pub output: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub query_kind: QueryKind,
    pub normalized_query: String,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Error)]
pub enum DataError {
    #[error("unsupported format `{format}`")]
    UnsupportedFormat { format: String },
    #[error("failed to parse json: {source}")]
    JsonParse {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse yaml: {source}")]
    YamlParse {
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to parse toml: {source}")]
    TomlParse {
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to convert toml to json value: {source}")]
    TomlToJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize yaml: {source}")]
    YamlSerialize {
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to serialize toml: {source}")]
    TomlSerialize {
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to read csv: {source}")]
    CsvRead {
        #[source]
        source: csv::Error,
    },
    #[error("failed to write csv: {source}")]
    CsvWrite {
        #[source]
        source: csv::Error,
    },
    #[error("failed to finalize csv output: {source}")]
    CsvFinalize {
        #[source]
        source: std::io::Error,
    },
    #[error("csv output was not valid utf-8: {source}")]
    CsvUtf8 {
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("csv output requires a top-level array of objects")]
    CsvOutputRequiresArrayOfObjects,
    #[error("csv row {index} must be an object")]
    CsvRowMustBeObject { index: usize },
    #[error("query cannot be empty")]
    EmptyQuery,
    #[error("dotted path `{query}` contains an empty segment")]
    InvalidDottedPath { query: String },
    #[error("query `{query}` did not match any value")]
    QueryNotFound { query: String },
    #[error("failed to compile json schema: {message}")]
    SchemaCompile { message: String },
}

pub fn convert(input: &str, from: &str, to: &str) -> Result<ConvertResult, DataError> {
    let from_format = DataFormat::parse(from)?;
    let to_format = DataFormat::parse(to)?;
    let value = parse_value(input, from_format)?;
    let output = render_value(&value, to_format)?;

    Ok(ConvertResult {
        from_format,
        to_format,
        value,
        output,
    })
}

pub fn extract(input: &str, query: &str) -> Result<ExtractResult, DataError> {
    let document =
        serde_json::from_str::<Value>(input).map_err(|source| DataError::JsonParse { source })?;
    let normalized_query = query.trim();

    if normalized_query.is_empty() {
        return Err(DataError::EmptyQuery);
    }

    let (query_kind, value) = if normalized_query.starts_with('/') {
        let value = document.pointer(normalized_query).cloned().ok_or_else(|| {
            DataError::QueryNotFound {
                query: normalized_query.to_string(),
            }
        })?;
        (QueryKind::JsonPointer, value)
    } else {
        let normalized = normalize_dotted_path(normalized_query)?;
        let value = extract_dotted_path(&document, &normalized)?.clone();
        return Ok(ExtractResult {
            query_kind: QueryKind::DottedPath,
            normalized_query: normalized,
            value,
        });
    };

    Ok(ExtractResult {
        query_kind,
        normalized_query: normalized_query.to_string(),
        value,
    })
}

pub fn validate(schema_json: &str, input_json: &str) -> Result<ValidateResult, DataError> {
    let schema = serde_json::from_str::<Value>(schema_json)
        .map_err(|source| DataError::JsonParse { source })?;
    let input = serde_json::from_str::<Value>(input_json)
        .map_err(|source| DataError::JsonParse { source })?;
    let validator =
        JSONSchema::options()
            .compile(&schema)
            .map_err(|error| DataError::SchemaCompile {
                message: error.to_string(),
            })?;

    let mut errors = match validator.validate(&input) {
        Ok(()) => Vec::new(),
        Err(iter) => iter.map(|error| error.to_string()).collect::<Vec<_>>(),
    };
    errors.sort();

    Ok(ValidateResult {
        valid: errors.is_empty(),
        errors,
    })
}

fn parse_value(input: &str, format: DataFormat) -> Result<Value, DataError> {
    match format {
        DataFormat::Json => {
            serde_json::from_str(input).map_err(|source| DataError::JsonParse { source })
        }
        DataFormat::Yaml => {
            serde_yaml::from_str(input).map_err(|source| DataError::YamlParse { source })
        }
        DataFormat::Toml => {
            let value = toml::from_str::<toml::Value>(input)
                .map_err(|source| DataError::TomlParse { source })?;
            serde_json::to_value(value).map_err(|source| DataError::TomlToJson { source })
        }
        DataFormat::Csv => parse_csv(input),
    }
}

fn render_value(value: &Value, format: DataFormat) -> Result<String, DataError> {
    match format {
        DataFormat::Json => {
            serde_json::to_string_pretty(value).map_err(|source| DataError::JsonParse { source })
        }
        DataFormat::Yaml => {
            serde_yaml::to_string(value).map_err(|source| DataError::YamlSerialize { source })
        }
        DataFormat::Toml => {
            toml::to_string_pretty(value).map_err(|source| DataError::TomlSerialize { source })
        }
        DataFormat::Csv => render_csv(value),
    }
}

fn parse_csv(input: &str) -> Result<Value, DataError> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input.as_bytes());
    let headers = reader
        .headers()
        .map_err(|source| DataError::CsvRead { source })?
        .clone();
    let mut rows = Vec::new();

    for record in reader.records() {
        let record = record.map_err(|source| DataError::CsvRead { source })?;
        let mut row = Map::new();

        for (header, field) in headers.iter().zip(record.iter()) {
            row.insert(header.to_string(), Value::String(field.to_string()));
        }

        rows.push(Value::Object(row));
    }

    Ok(Value::Array(rows))
}

fn render_csv(value: &Value) -> Result<String, DataError> {
    let rows = value
        .as_array()
        .ok_or(DataError::CsvOutputRequiresArrayOfObjects)?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let mut headers = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let object = row
            .as_object()
            .ok_or(DataError::CsvRowMustBeObject { index })?;
        headers.extend(object.keys().cloned());
    }

    if headers.is_empty() {
        return Ok(String::new());
    }

    let ordered_headers = headers.into_iter().collect::<Vec<_>>();
    let mut writer = WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(ordered_headers.iter())
        .map_err(|source| DataError::CsvWrite { source })?;

    for (index, row) in rows.iter().enumerate() {
        let object = row
            .as_object()
            .ok_or(DataError::CsvRowMustBeObject { index })?;
        let record = ordered_headers
            .iter()
            .map(|header| stringify_csv_cell(object.get(header)))
            .collect::<Vec<_>>();
        writer
            .write_record(record.iter())
            .map_err(|source| DataError::CsvWrite { source })?;
    }

    writer
        .flush()
        .map_err(|source| DataError::CsvFinalize { source })?;
    let bytes = writer
        .into_inner()
        .map_err(|error| DataError::CsvFinalize {
            source: error.into_error(),
        })?;

    String::from_utf8(bytes).map_err(|source| DataError::CsvUtf8 { source })
}

fn stringify_csv_cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::String(value)) => value.clone(),
        Some(other) => other.to_string(),
    }
}

fn normalize_dotted_path(query: &str) -> Result<String, DataError> {
    let segments = query.split('.').map(str::trim).collect::<Vec<_>>();

    if segments.is_empty() || segments.iter().any(|segment| segment.is_empty()) {
        return Err(DataError::InvalidDottedPath {
            query: query.to_string(),
        });
    }

    Ok(segments.join("."))
}

fn extract_dotted_path<'a>(value: &'a Value, query: &str) -> Result<&'a Value, DataError> {
    let mut current = value;

    for segment in query.split('.') {
        current = match current {
            Value::Object(object) => {
                object
                    .get(segment)
                    .ok_or_else(|| DataError::QueryNotFound {
                        query: query.to_string(),
                    })?
            }
            Value::Array(items) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| DataError::QueryNotFound {
                        query: query.to_string(),
                    })?;
                items.get(index).ok_or_else(|| DataError::QueryNotFound {
                    query: query.to_string(),
                })?
            }
            _ => {
                return Err(DataError::QueryNotFound {
                    query: query.to_string(),
                });
            }
        };
    }

    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn convert_csv_input_to_json_array() {
        let result = convert(
            "name,level\nrust,advanced\njson,intermediate\n",
            "csv",
            "json",
        )
        .expect("csv should convert to json");

        assert_eq!(
            result.value,
            json!([
                {"name": "rust", "level": "advanced"},
                {"name": "json", "level": "intermediate"}
            ])
        );
        assert!(result.output.contains("\"name\": \"rust\""));
    }

    #[test]
    fn convert_json_output_to_csv_uses_sorted_union_headers() {
        let input = r#"[{"b":2,"a":1},{"c":3,"a":4}]"#;
        let result = convert(input, "json", "csv").expect("json should convert to csv");

        assert_eq!(result.output, "a,b,c\n1,2,\n4,,3\n");
    }

    #[test]
    fn convert_toml_to_yaml_preserves_structure() {
        let input = "name = \"data\"\n[tool]\nenabled = true\n";
        let result = convert(input, "toml", "yaml").expect("toml should convert to yaml");
        let rendered = serde_yaml::from_str::<Value>(&result.output).expect("valid yaml");

        assert_eq!(
            rendered,
            json!({
                "name": "data",
                "tool": {
                    "enabled": true
                }
            })
        );
    }

    #[test]
    fn extract_supports_json_pointer() {
        let result = extract(
            r#"{"skill":{"capabilities":["data-convert","data-extract"]}}"#,
            "/skill/capabilities/1",
        )
        .expect("json pointer should resolve");

        assert_eq!(result.query_kind, QueryKind::JsonPointer);
        assert_eq!(result.value, json!("data-extract"));
        assert_eq!(result.normalized_query, "/skill/capabilities/1");
    }

    #[test]
    fn extract_supports_dotted_path() {
        let result = extract(
            r#"{"skill":{"identity":{"name":"data"}}}"#,
            "skill.identity.name",
        )
        .expect("dotted path should resolve");

        assert_eq!(result.query_kind, QueryKind::DottedPath);
        assert_eq!(result.value, json!("data"));
        assert_eq!(result.normalized_query, "skill.identity.name");
    }

    #[test]
    fn validate_reports_success_and_failure() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "count": {"type": "integer"}
            },
            "required": ["name", "count"]
        }"#;

        let success = validate(schema, r#"{"name":"data","count":1}"#).expect("valid input");
        assert!(success.valid);
        assert!(success.errors.is_empty());

        let failure = validate(schema, r#"{"name":"data","count":"wrong"}"#)
            .expect("invalid input still returns result");
        assert!(!failure.valid);
        assert_eq!(failure.errors.len(), 1);
        assert!(!failure.errors[0].is_empty());
    }
}
