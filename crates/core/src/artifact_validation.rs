//! validate_artifact : StructuredArtifactObservation -> Result<ArtifactOutcome, ArtifactValidationError>   (pure, deterministic)

use thiserror::Error;
use tracing::instrument;

use crate::dispatch::{AbsoluteOutputPath, AbsoluteSchemaPath};
use crate::event_log::ArtifactOutcome;

/// An adapter's explicit observation of one required file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileObservation<'a> {
    /// The adapter observed that the file is absent.
    Missing,
    /// The adapter found the file but could not read it.
    Unreadable { detail: &'a str },
    /// The adapter read the complete available byte sequence.
    Readable { bytes: &'a [u8] },
}

/// A complete structured-dispatch observation with paired schema and output facts.
pub struct StructuredArtifactObservation<'a> {
    schema_path: &'a AbsoluteSchemaPath,
    schema: FileObservation<'a>,
    output_path: &'a AbsoluteOutputPath,
    artifact: FileObservation<'a>,
}

impl<'a> StructuredArtifactObservation<'a> {
    /// Construct a complete paired observation.
    ///
    /// ```compile_fail
    /// use pce_core::{AbsoluteOutputPath, AbsoluteSchemaPath, FileObservation, StructuredArtifactObservation};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let schema_path = AbsoluteSchemaPath::parse("/tmp/schema.json")?;
    /// let schema_observation = FileObservation::Readable { bytes: br#"{}"# };
    /// let output_path = AbsoluteOutputPath::parse("/tmp/output.json")?;
    /// let artifact_observation = FileObservation::Readable { bytes: br#"{}"# };
    /// let _input = StructuredArtifactObservation::new(&schema_path, schema_observation, &output_path);
    /// # Ok(()) }
    /// ```
    /// ```
    /// use pce_core::{AbsoluteOutputPath, AbsoluteSchemaPath, FileObservation, StructuredArtifactObservation};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let schema_path = AbsoluteSchemaPath::parse("/tmp/schema.json")?;
    /// let schema_observation = FileObservation::Readable { bytes: br#"{}"# };
    /// let output_path = AbsoluteOutputPath::parse("/tmp/output.json")?;
    /// let artifact_observation = FileObservation::Readable { bytes: br#"{}"# };
    /// let _input = StructuredArtifactObservation::new(&schema_path, schema_observation, &output_path, artifact_observation);
    /// # Ok(()) }
    /// ```
    pub fn new(
        schema_path: &'a AbsoluteSchemaPath,
        schema: FileObservation<'a>,
        output_path: &'a AbsoluteOutputPath,
        artifact: FileObservation<'a>,
    ) -> Self {
        Self {
            schema_path,
            schema,
            output_path,
            artifact,
        }
    }
}

/// A structural artifact observation was rejected.
#[derive(Debug, Error)]
pub enum ArtifactValidationError {
    /// Fires when the output artifact is absent.
    #[error("artifact output `{output_path}` is missing")]
    MissingArtifact { output_path: String },
    /// Fires when the output artifact exists but cannot be read.
    #[error("artifact output `{output_path}` is unreadable: {detail}")]
    UnreadableArtifact { output_path: String, detail: String },
    /// Fires when readable output bytes do not form complete valid JSON.
    #[error("artifact output `{output_path}` is not complete valid JSON: {detail}")]
    InvalidArtifactJson { output_path: String, detail: String },
    /// Fires when the schema is absent after the artifact has parsed.
    #[error("artifact schema `{schema_path}` is missing")]
    MissingSchema { schema_path: String },
    /// Fires when the schema exists but cannot be read after the artifact has parsed.
    #[error("artifact schema `{schema_path}` is unreadable: {detail}")]
    UnreadableSchema { schema_path: String, detail: String },
    /// Fires when readable schema bytes do not form valid JSON.
    #[error("artifact schema `{schema_path}` is not valid JSON: {detail}")]
    InvalidSchemaJson { schema_path: String, detail: String },
    /// Fires when parsed schema JSON cannot compile as a JSON Schema.
    #[error("artifact schema `{schema_path}` cannot be compiled: {detail}")]
    SchemaCompilation { schema_path: String, detail: String },
    /// Fires when parsed artifact JSON violates a valid compiled schema.
    #[error("{detail}")]
    SchemaViolation { output_path: String, detail: String },
}

impl ArtifactValidationError {
    /// Return the closed dispatch outcome represented by this rejection.
    pub const fn outcome(&self) -> ArtifactOutcome {
        match self {
            Self::MissingArtifact { .. } | Self::UnreadableArtifact { .. } => {
                ArtifactOutcome::Missing
            }
            Self::InvalidArtifactJson { .. } => ArtifactOutcome::Truncated,
            Self::MissingSchema { .. }
            | Self::UnreadableSchema { .. }
            | Self::InvalidSchemaJson { .. }
            | Self::SchemaCompilation { .. } => ArtifactOutcome::SchemaInvalid,
            Self::SchemaViolation { .. } => ArtifactOutcome::SchemaViolating,
        }
    }
}

struct ViolationMeasurement {
    instance_pointer: String,
    schema_pointer: String,
    rendered_detail: String,
}

/// Validate a complete paired structural artifact observation.
///
/// # Errors
///
/// Returns [`ArtifactValidationError`] when the artifact is missing, unreadable, or malformed;
/// when the schema is missing, unreadable, malformed, or uncompilable; or when a compiled schema
/// rejects the parsed artifact.
#[instrument(skip(input))]
pub fn validate_artifact(
    input: StructuredArtifactObservation<'_>,
) -> Result<ArtifactOutcome, ArtifactValidationError> {
    let output_path = input.output_path.as_path().display().to_string();
    let artifact = match input.artifact {
        FileObservation::Missing => {
            return Err(ArtifactValidationError::MissingArtifact { output_path });
        }
        FileObservation::Unreadable { detail } => {
            return Err(ArtifactValidationError::UnreadableArtifact {
                output_path,
                detail: detail.to_owned(),
            });
        }
        FileObservation::Readable { bytes } => serde_json::from_slice(bytes).map_err(|source| {
            ArtifactValidationError::InvalidArtifactJson {
                output_path: output_path.clone(),
                detail: source.to_string(),
            }
        })?,
    };

    let schema_path = input.schema_path.as_path().display().to_string();
    let schema = match input.schema {
        FileObservation::Missing => {
            return Err(ArtifactValidationError::MissingSchema { schema_path });
        }
        FileObservation::Unreadable { detail } => {
            return Err(ArtifactValidationError::UnreadableSchema {
                schema_path,
                detail: detail.to_owned(),
            });
        }
        FileObservation::Readable { bytes } => serde_json::from_slice(bytes).map_err(|source| {
            ArtifactValidationError::InvalidSchemaJson {
                schema_path: schema_path.clone(),
                detail: source.to_string(),
            }
        })?,
    };
    let validator = jsonschema::validator_for(&schema).map_err(|source| {
        ArtifactValidationError::SchemaCompilation {
            schema_path,
            detail: source.to_string(),
        }
    })?;

    let mut violations = validator
        .iter_errors(&artifact)
        .map(|violation| ViolationMeasurement {
            instance_pointer: violation.instance_path().to_string(),
            schema_pointer: violation.schema_path().to_string(),
            rendered_detail: violation.to_string(),
        })
        .collect::<Vec<_>>();
    violations.sort_by(|left, right| {
        (
            &left.instance_pointer,
            &left.schema_pointer,
            &left.rendered_detail,
        )
            .cmp(&(
                &right.instance_pointer,
                &right.schema_pointer,
                &right.rendered_detail,
            ))
    });
    if violations.is_empty() {
        return Ok(ArtifactOutcome::Validated);
    }
    let detail = violations
        .into_iter()
        .map(|violation| {
            let instance_location = if violation.instance_pointer.is_empty() {
                "<root>"
            } else {
                &violation.instance_pointer
            };
            let keyword_or_location = violation
                .schema_pointer
                .rsplit('/')
                .next()
                .filter(|segment| !segment.is_empty())
                .unwrap_or("<root-schema>");
            format!(
                "artifact output `{output_path}` violates schema keyword/location `{keyword_or_location}` at instance `{instance_location}`: {}",
                violation.rendered_detail
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(ArtifactValidationError::SchemaViolation {
        output_path,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::artifact_validation::{
        ArtifactValidationError, FileObservation, StructuredArtifactObservation, validate_artifact,
    };
    use crate::dispatch::{AbsoluteOutputPath, AbsoluteSchemaPath};
    use crate::event_log::ArtifactOutcome;

    const VALID_SCHEMA: &[u8] = br#"{
      "type":"object",
      "required":["verdict","summary"],
      "properties":{
        "verdict":{"type":"string"},
        "summary":{"type":"string"},
        "nested":{"type":"object","properties":{"count":{"type":"integer"}}}
      }
    }"#;

    fn paths() -> (AbsoluteSchemaPath, AbsoluteOutputPath) {
        (
            AbsoluteSchemaPath::parse("/fixtures/artifact.schema.json")
                .expect("absolute schema path"),
            AbsoluteOutputPath::parse("/fixtures/artifact.json").expect("absolute output path"),
        )
    }

    fn validate(
        schema: FileObservation<'_>,
        artifact: FileObservation<'_>,
    ) -> Result<ArtifactOutcome, ArtifactValidationError> {
        let (schema_path, output_path) = paths();
        validate_artifact(StructuredArtifactObservation::new(
            &schema_path,
            schema,
            &output_path,
            artifact,
        ))
    }

    #[test]
    fn validator_maps_every_reachable_outcome() {
        let cases = [
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Readable {
                    bytes: br#"{"verdict":"pass","summary":"ok"}"#,
                },
                Ok(ArtifactOutcome::Validated),
            ),
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Missing,
                Err(ArtifactOutcome::Missing),
            ),
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Unreadable {
                    detail: "permission denied",
                },
                Err(ArtifactOutcome::Missing),
            ),
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Readable { bytes: b"{" },
                Err(ArtifactOutcome::Truncated),
            ),
            (
                FileObservation::Missing,
                FileObservation::Readable { bytes: br#"{}"# },
                Err(ArtifactOutcome::SchemaInvalid),
            ),
            (
                FileObservation::Unreadable {
                    detail: "permission denied",
                },
                FileObservation::Readable { bytes: br#"{}"# },
                Err(ArtifactOutcome::SchemaInvalid),
            ),
            (
                FileObservation::Readable { bytes: b"{" },
                FileObservation::Readable { bytes: br#"{}"# },
                Err(ArtifactOutcome::SchemaInvalid),
            ),
            (
                FileObservation::Readable {
                    bytes: br#"{"type":5}"#,
                },
                FileObservation::Readable { bytes: br#"{}"# },
                Err(ArtifactOutcome::SchemaInvalid),
            ),
            (
                FileObservation::Readable {
                    bytes: br#"{"type":"string"}"#,
                },
                FileObservation::Readable { bytes: br#"{}"# },
                Err(ArtifactOutcome::SchemaViolating),
            ),
        ];
        for (schema, artifact, expected) in cases {
            let actual = validate(schema, artifact).map_err(|error| error.outcome());
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn missing_artifact_precedes_uncompilable_schema() {
        let error = validate(
            FileObservation::Readable {
                bytes: br#"{"type":5}"#,
            },
            FileObservation::Missing,
        )
        .expect_err("missing artifact must reject");
        assert_eq!(error.outcome(), ArtifactOutcome::Missing);
    }

    #[test]
    fn malformed_artifact_precedes_uncompilable_schema() {
        let error = validate(
            FileObservation::Readable {
                bytes: br#"{"type":5}"#,
            },
            FileObservation::Readable { bytes: b"{" },
        )
        .expect_err("malformed artifact must reject");
        assert_eq!(error.outcome(), ArtifactOutcome::Truncated);
    }

    #[test]
    fn rejection_diagnostics_name_paths_and_causes() {
        let cases = [
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Missing,
                "artifact output `/fixtures/artifact.json` is missing",
            ),
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Unreadable {
                    detail: "permission denied",
                },
                "artifact output `/fixtures/artifact.json` is unreadable: permission denied",
            ),
            (
                FileObservation::Readable {
                    bytes: VALID_SCHEMA,
                },
                FileObservation::Readable { bytes: b"{" },
                "artifact output `/fixtures/artifact.json` is not complete valid JSON: EOF while parsing an object at line 1 column 1",
            ),
            (
                FileObservation::Missing,
                FileObservation::Readable { bytes: br#"{}"# },
                "artifact schema `/fixtures/artifact.schema.json` is missing",
            ),
            (
                FileObservation::Unreadable {
                    detail: "permission denied",
                },
                FileObservation::Readable { bytes: br#"{}"# },
                "artifact schema `/fixtures/artifact.schema.json` is unreadable: permission denied",
            ),
            (
                FileObservation::Readable { bytes: b"{" },
                FileObservation::Readable { bytes: br#"{}"# },
                "artifact schema `/fixtures/artifact.schema.json` is not valid JSON: EOF while parsing an object at line 1 column 1",
            ),
            (
                FileObservation::Readable {
                    bytes: br#"{"type":5}"#,
                },
                FileObservation::Readable { bytes: br#"{}"# },
                "artifact schema `/fixtures/artifact.schema.json` cannot be compiled: 5 is not valid under any of the schemas listed in the 'anyOf' keyword",
            ),
        ];
        for (schema, artifact, expected) in cases {
            assert_eq!(
                validate(schema, artifact)
                    .expect_err("fixture must reject")
                    .to_string(),
                expected
            );
        }
    }

    #[test]
    fn different_schema_violations_render_different_measurements() {
        let missing = validate(
            FileObservation::Readable {
                bytes: VALID_SCHEMA,
            },
            FileObservation::Readable {
                bytes: br#"{"summary":"ok"}"#,
            },
        )
        .expect_err("required property must reject")
        .to_string();
        let typed = validate(
            FileObservation::Readable {
                bytes: VALID_SCHEMA,
            },
            FileObservation::Readable {
                bytes: br#"{"verdict":"pass","summary":"ok","nested":{"count":"not-an-int"}}"#,
            },
        )
        .expect_err("nested type must reject")
        .to_string();
        assert_eq!(
            missing,
            "artifact output `/fixtures/artifact.json` violates schema keyword/location `required` at instance `<root>`: \"verdict\" is a required property"
        );
        assert_eq!(
            typed,
            "artifact output `/fixtures/artifact.json` violates schema keyword/location `type` at instance `/nested/count`: \"not-an-int\" is not of type \"integer\""
        );
        assert_ne!(missing, typed);
    }

    #[test]
    fn schema_violation_locations_have_total_fallbacks() {
        let root_instance = validate(
            FileObservation::Readable {
                bytes: VALID_SCHEMA,
            },
            FileObservation::Readable {
                bytes: br#"{"summary":"ok"}"#,
            },
        )
        .expect_err("required property must reject")
        .to_string();
        assert!(root_instance.contains("keyword/location `required` at instance `<root>`"));

        let root_schema = validate(
            FileObservation::Readable { bytes: b"false" },
            FileObservation::Readable { bytes: br#"{}"# },
        )
        .expect_err("false schema must reject")
        .to_string();
        assert!(root_schema.contains("keyword/location `<root-schema>` at instance `<root>`"));

        let property_schema = validate(
            FileObservation::Readable {
                bytes: br#"{"properties":{"blocked":false}}"#,
            },
            FileObservation::Readable {
                bytes: br#"{"blocked":1}"#,
            },
        )
        .expect_err("false property schema must reject")
        .to_string();
        assert!(property_schema.contains("keyword/location `blocked` at instance `/blocked`"));
    }

    #[test]
    fn schema_violations_are_sorted_and_joined_deterministically() {
        let schema = br#"{"type":"object","required":["a","b"],"additionalProperties":false,"properties":{"c":{"type":"integer"},"d":{"type":"integer"}}}"#;
        let rendered = validate(
            FileObservation::Readable { bytes: schema },
            FileObservation::Readable {
                bytes: br#"{"c":"x","d":"y","extra":true}"#,
            },
        )
        .expect_err("fixture must have five violations")
        .to_string();
        assert_eq!(
            rendered,
            concat!(
                "artifact output `/fixtures/artifact.json` violates schema keyword/location `additionalProperties` at instance `<root>`: Additional properties are not allowed ('extra' was unexpected); ",
                "artifact output `/fixtures/artifact.json` violates schema keyword/location `required` at instance `<root>`: \"a\" is a required property; ",
                "artifact output `/fixtures/artifact.json` violates schema keyword/location `required` at instance `<root>`: \"b\" is a required property; ",
                "artifact output `/fixtures/artifact.json` violates schema keyword/location `type` at instance `/c`: \"x\" is not of type \"integer\"; ",
                "artifact output `/fixtures/artifact.json` violates schema keyword/location `type` at instance `/d`: \"y\" is not of type \"integer\"",
            )
        );
    }

    #[test]
    fn artifact_validation_source_excludes_io_authority() {
        let source = include_str!("artifact_validation.rs");
        for forbidden in [
            ["std", "::fs"].concat(),
            ["File", "::open"].concat(),
            ["std", "::env"].concat(),
            ["SystemTime", "::now"].concat(),
            ["Instant", "::now"].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "source contains forbidden token {forbidden}"
            );
        }
    }

    #[test]
    fn observation_paths_are_preserved() {
        let (schema_path, output_path) = paths();
        assert_eq!(
            schema_path.as_path(),
            Path::new("/fixtures/artifact.schema.json")
        );
        assert_eq!(output_path.as_path(), Path::new("/fixtures/artifact.json"));
    }
}
