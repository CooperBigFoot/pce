//! measure_contract_snapshot : StatedContract × PreviousMeasuredContractSnapshot? × ExecuteGate → MeasuredContractSnapshot ∪ ContractMeasurementError
//! This module performs no ambient I/O; only the injected execution capability may observe a gate.

use std::fmt::{self, Display, Formatter};

use thiserror::Error;
use tracing::instrument;

use crate::tracked_contract::{GateCommand, GateKind, StatedContract};

/// An exit-status code observed by the injected gate execution capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedExitStatus(i32);

impl ObservedExitStatus {
    /// Construct an observation from the capability's exit-status code.
    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    /// Return the observed exit-status code.
    pub const fn code(self) -> i32 {
        self.0
    }

    const fn is_success(self) -> bool {
        self.0 == 0
    }
}

impl Display for ObservedExitStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A successful observation of one typed acceptance gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateMeasurement {
    kind: GateKind,
    command: GateCommand,
    status: ObservedExitStatus,
}

impl GateMeasurement {
    /// Return the role of the observed gate.
    pub const fn kind(&self) -> GateKind {
        self.kind
    }

    /// Return the exact typed command that was observed.
    pub const fn command(&self) -> &GateCommand {
        &self.command
    }

    /// Return the successful observed exit status.
    pub const fn status(&self) -> ObservedExitStatus {
        self.status
    }
}

/// Complete successful observations for the five required acceptance gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateMeasurements {
    format: GateMeasurement,
    lint: GateMeasurement,
    typecheck: GateMeasurement,
    test: GateMeasurement,
    build: GateMeasurement,
}

impl GateMeasurements {
    /// Return the observation for the requested gate role.
    pub const fn get(&self, kind: GateKind) -> &GateMeasurement {
        match kind {
            GateKind::Format => &self.format,
            GateKind::Lint => &self.lint,
            GateKind::Typecheck => &self.typecheck,
            GateKind::Test => &self.test,
            GateKind::Build => &self.build,
        }
    }

    /// Iterate over observations in format, lint, typecheck, test, build order.
    pub fn iter(&self) -> impl Iterator<Item = &GateMeasurement> {
        [
            &self.format,
            &self.lint,
            &self.typecheck,
            &self.test,
            &self.build,
        ]
        .into_iter()
    }
}

/// The current typed stated contract and its complete successful gate observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredContractSnapshot {
    stated: StatedContract,
    gates: GateMeasurements,
}

impl MeasuredContractSnapshot {
    /// Return the current stated contract represented by this snapshot.
    pub const fn stated(&self) -> &StatedContract {
        &self.stated
    }

    /// Return the complete successful gate observations.
    pub const fn gates(&self) -> &GateMeasurements {
        &self.gates
    }
}

/// A failure to obtain a complete successful measured-contract snapshot.
#[derive(Debug, Error)]
pub enum ContractMeasurementError<E>
where
    E: std::error::Error + 'static,
{
    /// Fires when the injected capability cannot produce an exit-status observation.
    #[error("failed to execute stated gate command `{command}`: {source}")]
    ExecutionFailed {
        /// The exact command whose execution could not be observed.
        command: String,
        /// The capability's original error.
        #[source]
        source: E,
    },
    /// Fires when a newly executed stated gate reports a non-zero exit status.
    #[error("stated gate command `{command}` exited with status {status}")]
    NonZeroGate {
        /// The exact rejected command.
        command: String,
        /// The observed non-zero status.
        status: ObservedExitStatus,
    },
}

/// Measure all changed stated gates and retain same-role byte-identical observations.
///
/// # Errors
///
/// Returns [`ContractMeasurementError::ExecutionFailed`] when the injected capability cannot
/// observe a gate. Returns [`ContractMeasurementError::NonZeroGate`] when a newly executed gate
/// reports a non-zero status.
#[instrument(skip(stated, previous, execute))]
pub fn measure_contract_snapshot<E>(
    stated: &StatedContract,
    previous: Option<&MeasuredContractSnapshot>,
    mut execute: impl FnMut(&GateCommand) -> Result<ObservedExitStatus, E>,
) -> Result<MeasuredContractSnapshot, ContractMeasurementError<E>>
where
    E: std::error::Error + 'static,
{
    let commands = stated.gates();
    let gates = GateMeasurements {
        format: measure_gate(
            GateKind::Format,
            commands.format(),
            previous.map(|snapshot| snapshot.gates().get(GateKind::Format)),
            &mut execute,
        )?,
        lint: measure_gate(
            GateKind::Lint,
            commands.lint(),
            previous.map(|snapshot| snapshot.gates().get(GateKind::Lint)),
            &mut execute,
        )?,
        typecheck: measure_gate(
            GateKind::Typecheck,
            commands.typecheck(),
            previous.map(|snapshot| snapshot.gates().get(GateKind::Typecheck)),
            &mut execute,
        )?,
        test: measure_gate(
            GateKind::Test,
            commands.test(),
            previous.map(|snapshot| snapshot.gates().get(GateKind::Test)),
            &mut execute,
        )?,
        build: measure_gate(
            GateKind::Build,
            commands.build(),
            previous.map(|snapshot| snapshot.gates().get(GateKind::Build)),
            &mut execute,
        )?,
    };

    Ok(MeasuredContractSnapshot {
        stated: stated.clone(),
        gates,
    })
}

fn measure_gate<E>(
    kind: GateKind,
    command: &GateCommand,
    prior: Option<&GateMeasurement>,
    execute: &mut impl FnMut(&GateCommand) -> Result<ObservedExitStatus, E>,
) -> Result<GateMeasurement, ContractMeasurementError<E>>
where
    E: std::error::Error + 'static,
{
    if let Some(prior) = prior
        && prior.command().as_str() == command.as_str()
    {
        return Ok(prior.clone());
    }

    let status = execute(command).map_err(|source| ContractMeasurementError::ExecutionFailed {
        command: command.as_str().to_owned(),
        source,
    })?;

    if !status.is_success() {
        return Err(ContractMeasurementError::NonZeroGate {
            command: command.as_str().to_owned(),
            status,
        });
    }

    Ok(GateMeasurement {
        kind,
        command: command.clone(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io;

    use serde_json::Value;

    use super::{ContractMeasurementError, ObservedExitStatus, measure_contract_snapshot};
    use crate::tracked_contract::{
        GateKind, TrackedRepositoryContract, parse_tracked_repository_contract,
    };

    const VALID_TRACKED_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
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
          "kind": "COMMAND",
          "command": "cargo test --workspace"
        }
      },
      {
        "workflow": "release.yml",
        "stand_in": {
          "kind": "NONE"
        }
      }
    ]
  },
  "appendable": {
    "environment_hazards": [
      "pipe Codex stdin from /dev/null"
    ],
    "gate_orderings": [
      "run cargo fmt --check before clippy"
    ],
    "lockfile_rules": [
      "commit Cargo.lock when dependency resolution changes"
    ]
  }
}"#;

    fn parsed_contract(bytes: &[u8]) -> TrackedRepositoryContract {
        parse_tracked_repository_contract(bytes).expect("fixture must parse")
    }

    fn stated_with_mutation(mut mutate: impl FnMut(&mut Value)) -> TrackedRepositoryContract {
        let mut value: Value =
            serde_json::from_slice(VALID_TRACKED_CONTRACT).expect("fixture must be valid JSON");
        mutate(&mut value);
        let bytes = serde_json::to_vec(&value).expect("mutated fixture must serialize");
        parsed_contract(&bytes)
    }

    #[test]
    fn initial_measurement_executes_every_gate_and_records_statuses() {
        let contract = parsed_contract(VALID_TRACKED_CONTRACT);
        let stated = contract.stated();
        let calls = RefCell::new(Vec::new());

        let snapshot = measure_contract_snapshot(stated, None, |command| {
            calls.borrow_mut().push(command.as_str().to_owned());
            Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
        })
        .expect("all fixture gates must succeed");

        assert_eq!(
            calls.into_inner(),
            [
                "cargo fmt --check",
                "cargo clippy --workspace --all-targets",
                "cargo check --workspace --all-targets",
                "cargo test --workspace",
                "cargo build --release",
            ]
        );
        assert_eq!(snapshot.stated(), stated);

        let observations = snapshot
            .gates()
            .iter()
            .map(|measurement| {
                (
                    measurement.kind(),
                    measurement.command().as_str(),
                    measurement.status().code(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observations,
            [
                (GateKind::Format, "cargo fmt --check", 0),
                (GateKind::Lint, "cargo clippy --workspace --all-targets", 0,),
                (
                    GateKind::Typecheck,
                    "cargo check --workspace --all-targets",
                    0,
                ),
                (GateKind::Test, "cargo test --workspace", 0),
                (GateKind::Build, "cargo build --release", 0),
            ]
        );
    }

    #[test]
    fn non_zero_gate_is_rejected_with_exact_command_and_status() {
        let contract = parsed_contract(VALID_TRACKED_CONTRACT);
        let calls = RefCell::new(Vec::new());

        let result = measure_contract_snapshot(contract.stated(), None, |command| {
            calls.borrow_mut().push(command.as_str().to_owned());
            let code = if command.as_str() == "cargo test --workspace" {
                17
            } else {
                0
            };
            Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(code))
        });
        let error = result.expect_err("the non-zero test gate must be rejected");
        let display = error.to_string();

        match error {
            ContractMeasurementError::NonZeroGate { command, status } => {
                assert_eq!(command, "cargo test --workspace");
                assert_eq!(status.code(), 17);
            }
            ContractMeasurementError::ExecutionFailed { .. } => {
                panic!("the capability returned an exit status")
            }
        }
        assert_eq!(
            display,
            "stated gate command `cargo test --workspace` exited with status 17"
        );
        assert_eq!(
            calls.into_inner(),
            [
                "cargo fmt --check",
                "cargo clippy --workspace --all-targets",
                "cargo check --workspace --all-targets",
                "cargo test --workspace",
            ]
        );
    }

    #[test]
    fn changed_command_reexecutes_only_its_gate_and_retains_prior_observations() {
        let contract = parsed_contract(VALID_TRACKED_CONTRACT);
        let calls = RefCell::new(Vec::new());
        let first_snapshot = measure_contract_snapshot(contract.stated(), None, |command| {
            calls.borrow_mut().push(command.as_str().to_owned());
            Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
        })
        .expect("initial measurement must succeed");
        let changed_contract = stated_with_mutation(|value| {
            value["stated"]["gates"]["test"] =
                Value::String("cargo test --workspace --all-targets".to_owned());
        });
        calls.borrow_mut().clear();

        let second_snapshot = measure_contract_snapshot(
            changed_contract.stated(),
            Some(&first_snapshot),
            |command| {
                calls.borrow_mut().push(command.as_str().to_owned());
                Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
            },
        )
        .expect("changed test gate must succeed");

        assert_eq!(calls.into_inner(), ["cargo test --workspace --all-targets"]);
        assert_eq!(second_snapshot.stated(), changed_contract.stated());
        let changed_test = second_snapshot.gates().get(GateKind::Test);
        assert_eq!(
            changed_test.command().as_str(),
            "cargo test --workspace --all-targets"
        );
        assert_eq!(changed_test.status().code(), 0);
        for kind in [
            GateKind::Format,
            GateKind::Lint,
            GateKind::Typecheck,
            GateKind::Build,
        ] {
            assert_eq!(
                second_snapshot.gates().get(kind),
                first_snapshot.gates().get(kind)
            );
        }
    }

    #[test]
    fn unchanged_commands_execute_zero_times_while_new_stated_contract_is_kept() {
        let contract = parsed_contract(VALID_TRACKED_CONTRACT);
        let calls = RefCell::new(Vec::new());
        let first_snapshot = measure_contract_snapshot(contract.stated(), None, |command| {
            calls.borrow_mut().push(command.as_str().to_owned());
            Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
        })
        .expect("initial measurement must succeed");
        let changed_contract = stated_with_mutation(|value| {
            value["stated"]["branches"]["default"] = Value::String("trunk".to_owned());
        });
        calls.borrow_mut().clear();

        let second_snapshot = measure_contract_snapshot(
            changed_contract.stated(),
            Some(&first_snapshot),
            |command| {
                calls.borrow_mut().push(command.as_str().to_owned());
                Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
            },
        )
        .expect("unchanged gates must be reused");

        assert!(calls.into_inner().is_empty());
        assert_eq!(second_snapshot.gates(), first_snapshot.gates());
        assert_eq!(second_snapshot.stated(), changed_contract.stated());
        assert_eq!(
            second_snapshot.stated().branches().default().as_str(),
            "trunk"
        );
    }

    #[test]
    fn execution_capability_failure_is_preserved_without_retry() {
        let contract = parsed_contract(VALID_TRACKED_CONTRACT);
        let calls = RefCell::new(Vec::new());

        let result = measure_contract_snapshot(contract.stated(), None, |command| {
            calls.borrow_mut().push(command.as_str().to_owned());
            Err(io::Error::other("executor unavailable"))
        });
        let error = result.expect_err("the capability failure must be preserved");
        let display = error.to_string();

        match error {
            ContractMeasurementError::ExecutionFailed { command, source } => {
                assert_eq!(command, "cargo fmt --check");
                assert_eq!(source.to_string(), "executor unavailable");
            }
            ContractMeasurementError::NonZeroGate { .. } => {
                panic!("the capability did not return an exit status")
            }
        }
        assert_eq!(
            display,
            "failed to execute stated gate command `cargo fmt --check`: executor unavailable"
        );
        assert_eq!(calls.borrow().len(), 1);
    }
}
