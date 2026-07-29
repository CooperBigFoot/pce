//! validate_workflow_coverage : WorkflowMappings × ObservedWorkflowNames → () ∪ WorkflowCoverageError   (pure, deterministic)

use thiserror::Error;
use tracing::instrument;

use crate::tracked_contract::WorkflowMappings;

/// A non-empty workflow filename observed in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedWorkflowName(String);

impl ObservedWorkflowName {
    /// Parse a workflow filename observed in the repository.
    ///
    /// # Errors
    ///
    /// Returns [`ObservedWorkflowNameError::EmptyName`] when `raw` is empty or contains only
    /// whitespace.
    pub fn parse(raw: &str) -> Result<Self, ObservedWorkflowNameError> {
        if raw.trim().is_empty() {
            Err(ObservedWorkflowNameError::EmptyName {
                raw: raw.to_owned(),
            })
        } else {
            Ok(Self(raw.to_owned()))
        }
    }

    /// Return the observed workflow filename.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A failure to parse an observed workflow filename.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObservedWorkflowNameError {
    /// Fires when the observed workflow filename is empty or whitespace-only.
    #[error("observed workflow name cannot be empty")]
    EmptyName {
        /// The rejected raw filename.
        raw: String,
    },
}

/// A failure to cover observed workflows with the tracked mappings.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkflowCoverageError {
    /// Fires when an observed workflow has no byte-identical tracked mapping.
    #[error("tracked repository contract omits workflow `{workflow}` present in .github/workflows")]
    MissingMapping {
        /// The first uncovered workflow filename in observation order.
        workflow: String,
    },
}

/// Require every observed workflow to have a byte-identical tracked mapping.
///
/// # Errors
///
/// Returns [`WorkflowCoverageError::MissingMapping`] for the first observed workflow without a
/// mapping.
#[instrument(skip(mappings, observed))]
pub fn validate_workflow_coverage(
    mappings: &WorkflowMappings,
    observed: &[ObservedWorkflowName],
) -> Result<(), WorkflowCoverageError> {
    for workflow in observed {
        if !mappings
            .as_slice()
            .iter()
            .any(|mapping| mapping.workflow().as_str() == workflow.as_str())
        {
            return Err(WorkflowCoverageError::MissingMapping {
                workflow: workflow.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        ObservedWorkflowName, ObservedWorkflowNameError, WorkflowCoverageError,
        parse_tracked_repository_contract, validate_workflow_coverage,
    };

    const OMITTED_WORKFLOW_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "true",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": []
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}"#;

    const EXPLICIT_NONE_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "true",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": [
      {
        "workflow": "ci.yml",
        "stand_in": {
          "kind": "NONE"
        }
      }
    ]
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}"#;

    #[test]
    fn rejects_observed_workflow_missing_from_stated_mappings() {
        let contract = parse_tracked_repository_contract(OMITTED_WORKFLOW_CONTRACT)
            .expect("contract fixture should parse");
        let observed =
            ObservedWorkflowName::parse("ci.yml").expect("workflow fixture should parse");
        let err = validate_workflow_coverage(contract.stated().workflows(), &[observed])
            .expect_err("omitted observed workflow should fail");

        match &err {
            WorkflowCoverageError::MissingMapping { workflow } => {
                assert_eq!(workflow, "ci.yml");
            }
        }
        assert_eq!(
            err.to_string(),
            "tracked repository contract omits workflow `ci.yml` present in .github/workflows"
        );
    }

    #[test]
    fn explicit_none_mapping_covers_observed_workflow() {
        let contract = parse_tracked_repository_contract(EXPLICIT_NONE_CONTRACT)
            .expect("contract fixture should parse");
        let observed =
            ObservedWorkflowName::parse("ci.yml").expect("workflow fixture should parse");

        validate_workflow_coverage(contract.stated().workflows(), &[observed])
            .expect("explicit NONE mapping should cover observed workflow");
    }

    #[test]
    fn observed_workflow_name_rejects_empty_text() {
        for raw in ["", "   "] {
            let err = ObservedWorkflowName::parse(raw)
                .expect_err("empty observed workflow name should fail");
            assert!(matches!(err, ObservedWorkflowNameError::EmptyName { .. }));
            assert_eq!(err.to_string(), "observed workflow name cannot be empty");
        }
    }
}
