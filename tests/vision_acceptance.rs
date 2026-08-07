use std::io::Write;
use std::process::{Command, Output, Stdio};

use pce_core::{
    AcceptanceCriteria, AcceptanceCriterion, CriterionInput, CriterionName, CriterionObservation,
    parse_acceptance_criteria,
};

const CONFORMING_VISION: &str = r#"# Vision: authenticated dispatch

## Goal / Why

Dispatches must use the operator's authenticated subscription.

## Scope — In

Authenticated child dispatch.

## Scope — Out (explicit non-goals)

Credential installation.

## Constraints

The child starts from a cleared environment.

## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "Authenticated dispatch survives environment isolation",
      "input": "Execute a gate dispatch with PATH, HOME, and USER forwarded after clearing the child environment.",
      "observation": "The child authenticates and writes a conforming verdict artifact."
    },
    {
      "name": "Missing home is refused",
      "input": "Execute the same gate dispatch without forwarding HOME.",
      "observation": "The child reports that it is not logged in and writes no verdict artifact."
    }
  ]
}
```

## Decomposition hints

Exercise the broken route first.

## Open questions / risks

None.
"#;

const ACCEPTANCE_HEADER: &str = "## Acceptance criteria (vision-level \"done\")";
const DECOMPOSITION_HEADER: &str = "## Decomposition hints";

#[test]
fn public_parser_exposes_the_conforming_contract_in_ratification_order() {
    let criteria: AcceptanceCriteria =
        parse_acceptance_criteria(CONFORMING_VISION).expect("conforming vision should parse");
    let parsed: &[AcceptanceCriterion] = criteria.as_slice();

    assert_eq!(criteria.len(), 2);
    assert_eq!(
        parsed[0].name(),
        &CriterionName::parse("Authenticated dispatch survives environment isolation")
            .expect("name fixture should parse")
    );
    assert_eq!(
        parsed[0].input(),
        &CriterionInput::parse(
            "Execute a gate dispatch with PATH, HOME, and USER forwarded after clearing the child environment."
        )
        .expect("input fixture should parse")
    );
    assert_eq!(
        parsed[0].observation(),
        &CriterionObservation::parse(
            "The child authenticates and writes a conforming verdict artifact."
        )
        .expect("observation fixture should parse")
    );
    assert_eq!(
        parsed[1].name(),
        &CriterionName::parse("Missing home is refused").expect("name fixture should parse")
    );
    assert_eq!(
        parsed[1].input(),
        &CriterionInput::parse("Execute the same gate dispatch without forwarding HOME.")
            .expect("input fixture should parse")
    );
    assert_eq!(
        parsed[1].observation(),
        &CriterionObservation::parse(
            "The child reports that it is not logged in and writes no verdict artifact."
        )
        .expect("observation fixture should parse")
    );
}

#[test]
fn cli_accepts_the_conforming_contract_without_output() {
    let output = check_vision(CONFORMING_VISION);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_refuses_malformed_payloads_with_stable_diagnostics() {
    let cases = [
        (
            r#"{
  "criteria": [
    {
      "name": "A test is present",
      "input": "Inspect the repository."
    }
  ]
}"#,
            "acceptance criterion 1 is missing required field `observation`",
        ),
        (
            r#"{
  "criteria": [
    {
      "name": "Standard input coverage",
      "input": "Inspect the implementation.",
      "observation": "A test exists.",
      "test": "standard input must be tested"
    }
  ]
}"#,
            "acceptance criterion 1 has unsupported field `test`; allowed fields are `name`, `input`, and `observation`",
        ),
        (
            r#"{
  "criteria": [
    {
      "name": "Blank input is invalid",
      "input": "   ",
      "observation": "The checker refuses it."
    }
  ]
}"#,
            "acceptance criterion 1 field `input` must be a non-empty string",
        ),
        (
            r#"{
  "criteria": []
}"#,
            "acceptance criteria must contain at least one criterion",
        ),
        (
            r#"{
  "criteria": [
}"#,
            "acceptance-criteria JSON is malformed",
        ),
    ];

    for (payload, leaf) in cases {
        assert_refused(&vision_with_payload(payload), leaf);
    }
}

#[test]
fn cli_refuses_missing_or_duplicate_acceptance_headers() {
    let without_header = CONFORMING_VISION.replacen(ACCEPTANCE_HEADER, "", 1);
    assert_refused(
        &without_header,
        "vision document must contain exactly one `## Acceptance criteria (vision-level \"done\")` section",
    );

    let section = acceptance_section(CONFORMING_VISION);
    let duplicated = CONFORMING_VISION.replacen(
        DECOMPOSITION_HEADER,
        &format!("{section}{DECOMPOSITION_HEADER}"),
        1,
    );
    assert_refused(
        &duplicated,
        "vision document must contain exactly one `## Acceptance criteria (vision-level \"done\")` section",
    );
}

#[test]
fn cli_refuses_invalid_fence_shapes_and_content() {
    let fenced = fenced_block(CONFORMING_VISION);
    let payload = fenced
        .strip_prefix("```json\n")
        .and_then(|value| value.strip_suffix("```\n\n"))
        .expect("fixture fence should have the expected shape");
    let cases = [
        payload.to_owned(),
        format!("{fenced}{fenced}"),
        fenced.replacen("```\n\n", "", 1),
        fenced.replacen("```json", "```yaml", 1),
        format!("unexpected prose\n{fenced}"),
        format!("{fenced}unexpected prose\n"),
    ];

    for content in cases {
        assert_refused(
            &vision_with_acceptance_content(&content),
            "acceptance-criteria section must contain exactly one fenced `json` object and no other content",
        );
    }
}

#[test]
fn cli_refuses_invalid_top_level_json_shapes_before_entry_validation() {
    for payload in [
        "[]",
        "{}",
        r#"{"criteria": [], "version": 1}"#,
        r#"{"criterion": []}"#,
    ] {
        assert_refused(
            &vision_with_payload(payload),
            "acceptance-criteria JSON must be an object with exactly the `criteria` key",
        );
    }
}

#[test]
fn cli_refuses_non_object_criterion_entries() {
    assert_refused(
        &vision_with_payload(r#"{"criteria": ["input only"]}"#),
        "acceptance criterion 1 must be an object",
    );
}

#[test]
fn parser_accepts_an_acceptance_section_at_end_of_file() {
    let section = acceptance_section(CONFORMING_VISION);
    let document = format!("# Vision: last section\n\n{section}");
    let criteria =
        parse_acceptance_criteria(&document).expect("last section should parse through EOF");

    assert_eq!(criteria.len(), 2);
}

fn check_vision(document: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["vision", "check"])
        .env("RUST_LOG", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pce binary should spawn");
    child
        .stdin
        .take()
        .expect("piped stdin should exist")
        .write_all(document.as_bytes())
        .expect("vision fixture should write to stdin");
    child.wait_with_output().expect("pce binary should exit")
}

fn assert_refused(document: &str, leaf: &str) {
    let parser_error =
        parse_acceptance_criteria(document).expect_err("public parser should refuse fixture");
    let output = check_vision(document);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert_eq!(parser_error.to_string(), leaf);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("failed to check vision acceptance criteria"));
    assert!(
        stderr.contains(leaf),
        "stderr `{stderr}` missing leaf `{leaf}`"
    );
}

fn acceptance_section(document: &str) -> &str {
    let start = document
        .find(ACCEPTANCE_HEADER)
        .expect("fixture acceptance header should exist");
    let end = document[start..]
        .find(DECOMPOSITION_HEADER)
        .map(|offset| start + offset)
        .expect("fixture decomposition header should exist");
    &document[start..end]
}

fn fenced_block(document: &str) -> &str {
    acceptance_section(document)
        .strip_prefix(&format!("{ACCEPTANCE_HEADER}\n\n"))
        .expect("fixture acceptance content should follow its header")
}

fn vision_with_payload(payload: &str) -> String {
    vision_with_acceptance_content(&format!("```json\n{payload}\n```\n\n"))
}

fn vision_with_acceptance_content(content: &str) -> String {
    let replacement = format!("{ACCEPTANCE_HEADER}\n\n{content}\n");
    let section = acceptance_section(CONFORMING_VISION);
    CONFORMING_VISION.replacen(section, &replacement, 1)
}
