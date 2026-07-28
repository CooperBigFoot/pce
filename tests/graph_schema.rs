use std::error::Error;

use serde_json::{Value, json};

const GRAPH_SCHEMA: &str = include_str!("../skills/pce/schemas/graph.schema.json");

#[test]
fn graph_schema_requires_explained_dependencies_and_rejects_files_touched()
-> Result<(), Box<dyn Error>> {
    let schema: Value = serde_json::from_str(GRAPH_SCHEMA)?;
    let validator = jsonschema::validator_for(&schema)?;

    let valid_graph = json!({
        "nodes": [
            {
                "id": "m1-s1",
                "title": "Make graph dependencies explicit",
                "repo": "pce",
                "depends_on": [
                    {
                        "id": "m1-s0",
                        "reason": "The schema contract must exist before it is consumed"
                    }
                ],
                "summary": "Require a reason for every graph dependency"
            }
        ]
    });
    assert!(
        validator.is_valid(&valid_graph),
        "a graph with one fully explained dependency must be accepted"
    );

    let missing_reason = json!({
        "nodes": [
            {
                "id": "m1-s1",
                "title": "Make graph dependencies explicit",
                "repo": "pce",
                "depends_on": [
                    {
                        "id": "m1-s0"
                    }
                ],
                "summary": "Require a reason for every graph dependency"
            }
        ]
    });
    assert!(
        !validator.is_valid(&missing_reason),
        "a dependency without a reason must be rejected"
    );

    let empty_reason = json!({
        "nodes": [
            {
                "id": "m1-s1",
                "title": "Make graph dependencies explicit",
                "repo": "pce",
                "depends_on": [
                    {
                        "id": "m1-s0",
                        "reason": ""
                    }
                ],
                "summary": "Require a reason for every graph dependency"
            }
        ]
    });
    assert!(
        !validator.is_valid(&empty_reason),
        "an empty dependency reason must be rejected"
    );

    let files_touched = json!({
        "nodes": [
            {
                "id": "m1-s1",
                "title": "Make graph dependencies explicit",
                "repo": "pce",
                "depends_on": [
                    {
                        "id": "m1-s0",
                        "reason": "The schema contract must exist before it is consumed"
                    }
                ],
                "files_touched": ["skills/pce/schemas/graph.schema.json"],
                "summary": "Require a reason for every graph dependency"
            }
        ]
    });
    assert!(
        !validator.is_valid(&files_touched),
        "the removed files_touched field must be rejected"
    );

    Ok(())
}
