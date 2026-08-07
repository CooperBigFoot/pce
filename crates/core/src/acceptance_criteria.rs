//! acceptance_criteria : VisionDocument → AcceptanceCriteria; CriterionFields → AcceptanceCriterion   (fallible, deterministic)
//!
//! Parses the sole fenced JSON acceptance contract from a complete vision document.

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! strict_criterion_string_serde {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

/// A known field required on every acceptance criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionField {
    /// The plain-language identity of the criterion.
    Name,
    /// The concrete input or action supplied to the finished thing.
    Input,
    /// The externally observable result that settles the criterion.
    Observation,
}

impl Display for CriterionField {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Name => "name",
            Self::Input => "input",
            Self::Observation => "observation",
        })
    }
}

/// A non-empty plain-language acceptance-criterion name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionName(String);

impl CriterionName {
    /// Parses a criterion name and trims surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptanceCriteriaError::BlankField`] when `value` is blank.
    pub fn parse(value: &str) -> Result<Self, AcceptanceCriteriaError> {
        parse_non_empty(value, CriterionField::Name).map(Self)
    }

    /// Returns the trimmed criterion name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

strict_criterion_string_serde!(CriterionName);

/// A non-empty concrete input or action for an acceptance criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionInput(String);

impl CriterionInput {
    /// Parses a criterion input and trims surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptanceCriteriaError::BlankField`] when `value` is blank.
    pub fn parse(value: &str) -> Result<Self, AcceptanceCriteriaError> {
        parse_non_empty(value, CriterionField::Input).map(Self)
    }

    /// Returns the trimmed criterion input.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

strict_criterion_string_serde!(CriterionInput);

/// A non-empty externally observable acceptance-criterion result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionObservation(String);

impl CriterionObservation {
    /// Parses a criterion observation and trims surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptanceCriteriaError::BlankField`] when `value` is blank.
    pub fn parse(value: &str) -> Result<Self, AcceptanceCriteriaError> {
        parse_non_empty(value, CriterionField::Observation).map(Self)
    }

    /// Returns the trimmed criterion observation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

strict_criterion_string_serde!(CriterionObservation);

/// One named input-and-observation acceptance contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCriterion {
    name: CriterionName,
    input: CriterionInput,
    observation: CriterionObservation,
}

impl AcceptanceCriterion {
    /// Construct a criterion from already-parsed domain fields.
    pub const fn new(
        name: CriterionName,
        input: CriterionInput,
        observation: CriterionObservation,
    ) -> Self {
        Self {
            name,
            input,
            observation,
        }
    }
    /// Returns the criterion name.
    pub fn name(&self) -> &CriterionName {
        &self.name
    }

    /// Returns the concrete criterion input.
    pub fn input(&self) -> &CriterionInput {
        &self.input
    }

    /// Returns the externally observable criterion result.
    pub fn observation(&self) -> &CriterionObservation {
        &self.observation
    }
}

/// A non-empty acceptance-criteria collection in ratification order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceCriteria(Vec<AcceptanceCriterion>);

impl AcceptanceCriteria {
    /// Returns the number of acceptance criteria.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the criteria in ratification order.
    pub fn as_slice(&self) -> &[AcceptanceCriterion] {
        &self.0
    }
}

/// A refusal to parse the vision's strict acceptance-criteria contract.
#[derive(Debug, thiserror::Error)]
pub enum AcceptanceCriteriaError {
    /// Fires when the document does not contain exactly one exact acceptance header.
    #[error(
        "vision document must contain exactly one `## Acceptance criteria (vision-level \"done\")` section"
    )]
    HeaderCount,
    /// Fires when the acceptance section is not solely one fenced JSON block.
    #[error(
        "acceptance-criteria section must contain exactly one fenced `json` object and no other content"
    )]
    FenceShape,
    /// Fires when the decoded JSON is not an object containing only `criteria`.
    #[error("acceptance-criteria JSON must be an object with exactly the `criteria` key")]
    TopLevelShape,
    /// Fires when the criteria array has no entries.
    #[error("acceptance criteria must contain at least one criterion")]
    EmptyCriteria,
    /// Fires when an indexed criteria array entry is not an object.
    #[error("acceptance criterion {index} must be an object")]
    CriterionNotObject {
        /// The one-based array position of the invalid entry.
        index: usize,
    },
    /// Fires when an indexed criterion omits one required known field.
    #[error("acceptance criterion {index} is missing required field `{field}`")]
    MissingField {
        /// The one-based array position of the invalid entry.
        index: usize,
        /// The required field that is absent.
        field: CriterionField,
    },
    /// Fires when an indexed criterion contains a key outside the strict contract.
    #[error(
        "acceptance criterion {index} has unsupported field `{field}`; allowed fields are `name`, `input`, and `observation`"
    )]
    UnsupportedField {
        /// The one-based array position of the invalid entry.
        index: usize,
        /// The observed unsupported key.
        field: String,
    },
    /// Fires when a required field is not a non-empty string after trimming.
    #[error("acceptance criterion {index} field `{field}` must be a non-empty string")]
    BlankField {
        /// The one-based array position of the invalid entry.
        index: usize,
        /// The known field whose value is invalid.
        field: CriterionField,
    },
    /// Fires when the fenced payload is not syntactically valid JSON.
    #[error("acceptance-criteria JSON is malformed")]
    MalformedJson {
        /// The JSON decoder failure retained as the diagnostic source.
        #[source]
        source: serde_json::Error,
    },
}

/// Parses the strict acceptance-criteria contract from a complete vision document.
///
/// # Errors
///
/// Returns [`AcceptanceCriteriaError`] when the exact section, fence, JSON shape, or any
/// required non-blank criterion field violates the contract.
pub fn parse_acceptance_criteria(
    document: &str,
) -> Result<AcceptanceCriteria, AcceptanceCriteriaError> {
    const HEADER: &str = "## Acceptance criteria (vision-level \"done\")";
    let lines = lines_with_offsets(document);
    let acceptance_headers: Vec<(usize, usize)> = lines
        .iter()
        .filter(|line| line.text == HEADER)
        .map(|line| (line.start, line.end))
        .collect();
    let [(header_start, content_start)] = acceptance_headers.as_slice() else {
        return Err(AcceptanceCriteriaError::HeaderCount);
    };
    let content_end = lines
        .iter()
        .find(|line| line.start > *header_start && line.text.starts_with("## "))
        .map_or(document.len(), |line| line.start);
    let payload = fenced_json_payload(&document[*content_start..content_end])?;
    let mut duplicate_check = serde_json::Deserializer::from_str(payload);
    DuplicateCheckedValue::deserialize(&mut duplicate_check)
        .and_then(|_| duplicate_check.end())
        .map_err(|source| AcceptanceCriteriaError::MalformedJson { source })?;
    let decoded: serde_json::Value = serde_json::from_str(payload)
        .map_err(|source| AcceptanceCriteriaError::MalformedJson { source })?;
    parse_json_contract(decoded)
}

struct DuplicateCheckedValue;

impl<'de> Deserialize<'de> for DuplicateCheckedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedVisitor)?;
        Ok(Self)
    }
}

struct DuplicateCheckedVisitor;

impl<'de> serde::de::Visitor<'de> for DuplicateCheckedVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateCheckedValue>()?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut keys = std::collections::BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format_args!(
                    "duplicate object key `{key}`"
                )));
            }
            map.next_value::<DuplicateCheckedValue>()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DocumentLine<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn lines_with_offsets(document: &str) -> Vec<DocumentLine<'_>> {
    let mut start = 0;
    document
        .split_inclusive('\n')
        .map(|line| {
            let end = start + line.len();
            let text = line.strip_suffix('\n').unwrap_or(line);
            let located = DocumentLine { start, end, text };
            start = end;
            located
        })
        .collect()
}

fn fenced_json_payload(section: &str) -> Result<&str, AcceptanceCriteriaError> {
    let lines = lines_with_offsets(section);
    let openings: Vec<DocumentLine<'_>> = lines
        .iter()
        .copied()
        .filter(|line| line.text == "```json")
        .collect();
    let closings: Vec<DocumentLine<'_>> = lines
        .iter()
        .copied()
        .filter(|line| line.text == "```")
        .collect();
    let ([opening], [closing]) = (openings.as_slice(), closings.as_slice()) else {
        return Err(AcceptanceCriteriaError::FenceShape);
    };
    if closing.start < opening.end
        || !section[..opening.start].trim().is_empty()
        || !section[closing.end..].trim().is_empty()
    {
        return Err(AcceptanceCriteriaError::FenceShape);
    }
    Ok(&section[opening.end..closing.start])
}

fn parse_json_contract(
    decoded: serde_json::Value,
) -> Result<AcceptanceCriteria, AcceptanceCriteriaError> {
    let serde_json::Value::Object(mut root) = decoded else {
        return Err(AcceptanceCriteriaError::TopLevelShape);
    };
    if root.len() != 1 || !root.contains_key("criteria") {
        return Err(AcceptanceCriteriaError::TopLevelShape);
    }
    let Some(serde_json::Value::Array(entries)) = root.remove("criteria") else {
        return Err(AcceptanceCriteriaError::TopLevelShape);
    };
    if entries.is_empty() {
        return Err(AcceptanceCriteriaError::EmptyCriteria);
    }
    let criteria = entries
        .into_iter()
        .enumerate()
        .map(|(offset, entry)| parse_criterion(offset + 1, entry))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AcceptanceCriteria(criteria))
}

fn parse_criterion(
    index: usize,
    entry: serde_json::Value,
) -> Result<AcceptanceCriterion, AcceptanceCriteriaError> {
    let serde_json::Value::Object(fields) = entry else {
        return Err(AcceptanceCriteriaError::CriterionNotObject { index });
    };
    if let Some(field) = fields
        .keys()
        .find(|field| !matches!(field.as_str(), "name" | "input" | "observation"))
    {
        return Err(AcceptanceCriteriaError::UnsupportedField {
            index,
            field: field.clone(),
        });
    }
    let name = criterion_string(&fields, index, CriterionField::Name)?;
    let input = criterion_string(&fields, index, CriterionField::Input)?;
    let observation = criterion_string(&fields, index, CriterionField::Observation)?;
    Ok(AcceptanceCriterion {
        name: CriterionName(name),
        input: CriterionInput(input),
        observation: CriterionObservation(observation),
    })
}

fn criterion_string(
    fields: &serde_json::Map<String, serde_json::Value>,
    index: usize,
    field: CriterionField,
) -> Result<String, AcceptanceCriteriaError> {
    let key = field.to_string();
    let value = fields
        .get(&key)
        .ok_or(AcceptanceCriteriaError::MissingField { index, field })?;
    let serde_json::Value::String(value) = value else {
        return Err(AcceptanceCriteriaError::BlankField { index, field });
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AcceptanceCriteriaError::BlankField { index, field });
    }
    Ok(trimmed.to_owned())
}

fn parse_non_empty(value: &str, field: CriterionField) -> Result<String, AcceptanceCriteriaError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AcceptanceCriteriaError::BlankField { index: 1, field });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptanceCriteriaError, AcceptanceCriterion, CriterionField, CriterionInput,
        CriterionName, CriterionObservation, parse_acceptance_criteria,
    };

    #[test]
    fn criterion_strict_wire_shape_trims_and_rejects_invalid_objects() {
        let criterion = AcceptanceCriterion::new(
            CriterionName::parse("Runnable criterion").expect("name"),
            CriterionInput::parse("Run the finished command.").expect("input"),
            CriterionObservation::parse("It exits 0.").expect("observation"),
        );
        assert_eq!(
            serde_json::to_string(&criterion).expect("serialize criterion"),
            r#"{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."}"#
        );
        let decoded: AcceptanceCriterion = serde_json::from_str(
            r#"{"name":"  Runnable criterion  ","input":"  Run the finished command.  ","observation":"  It exits 0.  "}"#,
        )
        .expect("deserialize trimmed criterion");
        assert_eq!(decoded.name().as_str(), "Runnable criterion");
        assert_eq!(decoded.input().as_str(), "Run the finished command.");
        assert_eq!(decoded.observation().as_str(), "It exits 0.");
        for invalid in [
            r#"{"name":"   ","input":"go","observation":"done"}"#,
            r#"{"name":"one","input":"   ","observation":"done"}"#,
            r#"{"name":"one","input":"go","observation":"   "}"#,
            r#"{"name":"one","input":"go","observation":"done","extra":true}"#,
        ] {
            assert!(serde_json::from_str::<AcceptanceCriterion>(invalid).is_err());
        }
    }

    const DOCUMENT: &str = r#"# Vision: example

## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "  First criterion  ",
      "input": "  Run the finished thing.  ",
      "observation": "  It emits the result.  "
    },
    {
      "name": "Second criterion",
      "input": "Give it invalid input.",
      "observation": "It refuses the input."
    }
  ]
}
```

## Open questions / risks

None.
"#;

    #[test]
    fn parses_and_trims_all_values_in_order() {
        let criteria = parse_acceptance_criteria(DOCUMENT).expect("document should parse");

        assert_eq!(criteria.len(), 2);
        assert_eq!(criteria.as_slice()[0].name().as_str(), "First criterion");
        assert_eq!(
            criteria.as_slice()[0].input().as_str(),
            "Run the finished thing."
        );
        assert_eq!(
            criteria.as_slice()[0].observation().as_str(),
            "It emits the result."
        );
        assert_eq!(criteria.as_slice()[1].name().as_str(), "Second criterion");
    }

    #[test]
    fn reports_indexed_missing_unknown_and_blank_fields() {
        let missing = document_with_payload(r#"{"criteria":[{"name":"one","input":"go"}]}"#);
        assert!(matches!(
            parse_acceptance_criteria(&missing),
            Err(AcceptanceCriteriaError::MissingField {
                index: 1,
                field: CriterionField::Observation
            })
        ));

        let unknown = document_with_payload(
            r#"{"criteria":[{"name":"one","input":"go","observation":"done","test":"x"}]}"#,
        );
        assert!(matches!(
            parse_acceptance_criteria(&unknown),
            Err(AcceptanceCriteriaError::UnsupportedField { index: 1, field }) if field == "test"
        ));

        let blank = document_with_payload(
            r#"{"criteria":[{"name":"one","input":" ","observation":"done"}]}"#,
        );
        assert!(matches!(
            parse_acceptance_criteria(&blank),
            Err(AcceptanceCriteriaError::BlankField {
                index: 1,
                field: CriterionField::Input
            })
        ));
    }

    #[test]
    fn scans_the_next_header_without_interpreting_fences() {
        let unclosed = DOCUMENT.replacen("```\n\n## Open", "## Open", 1);

        assert!(matches!(
            parse_acceptance_criteria(&unclosed),
            Err(AcceptanceCriteriaError::FenceShape)
        ));
    }

    #[test]
    fn refuses_duplicate_json_object_keys() {
        for payload in [
            r#"{"criteria":[],"criteria":[]}"#,
            r#"{"criteria":[{"name":"one","name":"two","input":"go","observation":"done"}]}"#,
        ] {
            let document = document_with_payload(payload);
            assert!(matches!(
                parse_acceptance_criteria(&document),
                Err(AcceptanceCriteriaError::MalformedJson { .. })
            ));
        }
    }

    #[test]
    fn takes_the_acceptance_section_through_eof_when_it_is_last() {
        let end = DOCUMENT
            .find("## Open questions / risks")
            .expect("fixture next header should exist");
        let criteria =
            parse_acceptance_criteria(&DOCUMENT[..end]).expect("last section should parse");

        assert_eq!(criteria.len(), 2);
    }

    fn document_with_payload(payload: &str) -> String {
        let start = DOCUMENT
            .find("```json")
            .expect("fixture fence should exist");
        let payload_start = start + "```json\n".len();
        let payload_end = DOCUMENT[payload_start..]
            .find("\n```")
            .map(|offset| payload_start + offset)
            .expect("fixture closing fence should exist");
        format!(
            "{}{}{}",
            &DOCUMENT[..payload_start],
            payload,
            &DOCUMENT[payload_end..]
        )
    }
}
