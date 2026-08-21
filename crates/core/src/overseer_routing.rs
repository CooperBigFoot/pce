//! routing boundary : hold × proposed action × fleet state → enforced action + retained reason
//!
//! Door kinds remain human-only. Choice reports must carry options. Learned rules are admitted
//! only when replay over retained answered holds is unambiguous and its subject is not a ruling.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::dispatch_ledger::{DispatchAccounting, DispatchLedger};
use crate::event_log::EventTimestamp;
use crate::hold_store::{Hold, HoldKey, HoldRoute, HoldStore, HoldStoreError};
use crate::overseer_rulebook::{
    RoutingRule, RoutingRuleAction, RoutingRuleClass, RoutingRuleOrigin, RoutingRuleParseError,
};

/// A hold kind whose answer exercises authority that cannot be delegated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoorKind {
    CriterionRevision,
    WorkerEnvironmentExtension,
    BaseCurrencyAcceptance,
    ParkOverrule,
    RecoverySpendAuthorization,
    FleetInstallAuthorization,
}

impl DoorKind {
    fn from_question_kind(value: &str) -> Option<Self> {
        let normalized: String = value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        match normalized.as_str() {
            "criterionrevision" | "revisecriterion" => Some(Self::CriterionRevision),
            "workerenvironmentextension" | "workerenvironment" => {
                Some(Self::WorkerEnvironmentExtension)
            }
            "basecurrencyacceptance" | "acceptbasecurrencyrisk" => {
                Some(Self::BaseCurrencyAcceptance)
            }
            "parkoverrule" | "packageparkoverrule" => Some(Self::ParkOverrule),
            "recoveryspendauthorization" | "spendauthorization" => {
                Some(Self::RecoverySpendAuthorization)
            }
            "fleetinstallauthorization" | "installauthorization" => {
                Some(Self::FleetInstallAuthorization)
            }
            _ => None,
        }
    }
}

/// One policy event retained independently of any overseer process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OverseerRecord {
    RuleAdmissionRefused {
        timestamp: EventTimestamp,
        proposal_id: String,
        reason: String,
    },
    RoutingRuleAdmitted {
        timestamp: EventTimestamp,
        rule_id: String,
    },
    InstallRequested {
        timestamp: EventTimestamp,
        request_id: String,
        version: String,
    },
    InstallCompleted {
        timestamp: EventTimestamp,
        request_id: String,
        version: String,
    },
}

/// The result of applying the route boundary to one hold.
#[derive(Clone, Debug, PartialEq)]
pub enum RouteDecision {
    Routed(Hold),
    RefusedDoor { door: DoorKind, hold: Hold },
    ReturnedForOptions { hold: Hold, request: String },
}

impl RouteDecision {
    pub fn hold(&self) -> &Hold {
        match self {
            Self::Routed(hold)
            | Self::RefusedDoor { hold, .. }
            | Self::ReturnedForOptions { hold, .. } => hold,
        }
    }
}

/// Applies non-delegable-door and option completeness policy before routing a hold.
///
/// # Errors
///
/// Returns an error when the hold or its retained routing record cannot be read or appended.
pub fn route_hold(
    store: &HoldStore,
    key: &HoldKey,
    requested: HoldRoute,
    timestamp: EventTimestamp,
) -> Result<RouteDecision, OverseerRoutingError> {
    let current = store.read(key)?;
    if let Some(door) = DoorKind::from_question_kind(current.identity().question_kind())
        && requested != HoldRoute::Human
    {
        let hold = store.refuse_route(
            key,
            requested,
            HoldRoute::Human,
            format!("door kind `{}` requires a human ruling", door_name(door)),
            timestamp,
        )?;
        return Ok(RouteDecision::RefusedDoor { door, hold });
    }
    if requested == HoldRoute::Human
        && choice_kind(current.identity().question_kind())
        && !has_named_options(current.report())
    {
        let request =
            "Name each option and its consequence before requesting a human choice.".to_owned();
        let hold = store.return_for_options(key, request.clone(), timestamp)?;
        return Ok(RouteDecision::ReturnedForOptions { hold, request });
    }
    Ok(RouteDecision::Routed(
        store.route(key, requested, timestamp)?,
    ))
}

/// A candidate learned rule and the answer it predicts on replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedRoutingRule {
    rule: RoutingRule,
    expected_answer: String,
}

impl ProposedRoutingRule {
    /// Parses a candidate rule.
    ///
    /// # Errors
    ///
    /// Returns an error when a rule field or the expected answer is blank.
    pub fn parse(
        id: impl Into<String>,
        classification: RoutingRuleClass,
        action: RoutingRuleAction,
        discriminating_fact: impl Into<String>,
        instruction: impl Into<String>,
        expected_answer: impl Into<String>,
    ) -> Result<Self, OverseerRoutingError> {
        let expected_answer = expected_answer.into();
        if expected_answer.trim().is_empty() {
            return Err(OverseerRoutingError::EmptyExpectedAnswer);
        }
        Ok(Self {
            rule: RoutingRule::parse(
                id,
                RoutingRuleOrigin::Learned,
                classification,
                action,
                discriminating_fact,
                instruction,
            )?,
            expected_answer,
        })
    }

    pub fn rule(&self) -> &RoutingRule {
        &self.rule
    }
}

/// Why a learned rule was not admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleRefusal {
    SubjectIsRuling { door: DoorKind },
    AmbiguousReplay { matching_holds: Vec<HoldKey> },
    NoReplayWitness,
}

/// Result of replay admission for one proposed rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleAdmission {
    Admitted { rule_id: String },
    Refused(RuleRefusal),
}

/// Replays a candidate against every retained answered hold before appending it to the rulebook.
///
/// # Errors
///
/// Returns an error when holds or rules cannot be read or durably appended.
pub fn admit_routing_rule(
    store: &HoldStore,
    proposal: &ProposedRoutingRule,
    timestamp: EventTimestamp,
) -> Result<RuleAdmission, OverseerRoutingError> {
    let matching: Vec<Hold> = store
        .list()?
        .into_iter()
        .filter(|hold| report_contains(hold.report(), proposal.rule.discriminating_fact()))
        .collect();

    if let Some(door) = matching
        .iter()
        .find_map(|hold| DoorKind::from_question_kind(hold.identity().question_kind()))
    {
        let refusal = RuleRefusal::SubjectIsRuling { door };
        retain_rule_refusal(store, proposal.rule.id(), &refusal, timestamp)?;
        return Ok(RuleAdmission::Refused(refusal));
    }

    let answered: Vec<(&Hold, &str)> = matching
        .iter()
        .filter_map(|hold| latest_answer(hold).map(|answer| (hold, answer)))
        .collect();
    if answered.is_empty() {
        let refusal = RuleRefusal::NoReplayWitness;
        retain_rule_refusal(store, proposal.rule.id(), &refusal, timestamp)?;
        return Ok(RuleAdmission::Refused(refusal));
    }
    let expected = normalize_answer(&proposal.expected_answer);
    if answered
        .iter()
        .any(|(_, answer)| normalize_answer(answer) != expected)
    {
        let refusal = RuleRefusal::AmbiguousReplay {
            matching_holds: answered
                .iter()
                .map(|(hold, _)| hold.key().clone())
                .collect(),
        };
        retain_rule_refusal(store, proposal.rule.id(), &refusal, timestamp)?;
        return Ok(RuleAdmission::Refused(refusal));
    }

    store.append_routing_rule(&proposal.rule)?;
    append_record(
        store,
        &OverseerRecord::RoutingRuleAdmitted {
            timestamp,
            rule_id: proposal.rule.id().to_owned(),
        },
    )?;
    Ok(RuleAdmission::Admitted {
        rule_id: proposal.rule.id().to_owned(),
    })
}

/// Stable identity and requested version of one fleet install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallRequest {
    id: String,
    version: String,
}

impl InstallRequest {
    /// Parses an install request.
    ///
    /// # Errors
    ///
    /// Returns an error when either field is blank.
    pub fn parse(
        id: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, OverseerRoutingError> {
        let request = Self {
            id: id.into(),
            version: version.into(),
        };
        if request.id.trim().is_empty() {
            return Err(OverseerRoutingError::EmptyInstallField { field: "id" });
        }
        if request.version.trim().is_empty() {
            return Err(OverseerRoutingError::EmptyInstallField { field: "version" });
        }
        Ok(request)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Whether the requested install ran at this observation of the fleet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallDecision {
    WaitingForQuietFleet,
    Installed,
    AlreadyInstalled,
}

/// Records a request once and runs the supplied installer only when every dispatch is accounted.
///
/// The installer is authority supplied by the composition root; this module never chooses a
/// binary path or command.
///
/// # Errors
///
/// Returns an error when the policy journal cannot be read or appended, or the installer fails.
pub fn request_install<F>(
    store: &HoldStore,
    request: &InstallRequest,
    ledger: &DispatchLedger,
    timestamp: EventTimestamp,
    install: F,
) -> Result<InstallDecision, OverseerRoutingError>
where
    F: FnOnce() -> Result<(), String>,
{
    let policy_path = policy_path(store);
    let Some(directory) = policy_path.parent() else {
        return Err(OverseerRoutingError::MissingPolicyParent { path: policy_path });
    };
    fs::create_dir_all(directory).map_err(|source| {
        OverseerRoutingError::CreatePolicyDirectory {
            path: directory.to_path_buf(),
            source,
        }
    })?;
    let _install_lock = PolicyLock::acquire(directory.join("install.lock"))?;
    let records = overseer_records(store)?;
    if records.iter().any(|record| {
        matches!(record, OverseerRecord::InstallCompleted { request_id, .. } if request_id == request.id())
    }) {
        return Ok(InstallDecision::AlreadyInstalled);
    }
    if !records.iter().any(|record| {
        matches!(record, OverseerRecord::InstallRequested { request_id, .. } if request_id == request.id())
    }) {
        append_record(
            store,
            &OverseerRecord::InstallRequested {
                timestamp,
                request_id: request.id.clone(),
                version: request.version.clone(),
            },
        )?;
    }
    if matches!(ledger.accounting(), DispatchAccounting::Unaccounted(_)) {
        return Ok(InstallDecision::WaitingForQuietFleet);
    }
    install().map_err(|detail| OverseerRoutingError::InstallFailed { detail })?;
    append_record(
        store,
        &OverseerRecord::InstallCompleted {
            timestamp,
            request_id: request.id.clone(),
            version: request.version.clone(),
        },
    )?;
    Ok(InstallDecision::Installed)
}

/// Reads every retained overseer policy record in append order.
///
/// # Errors
///
/// Returns an error for I/O failures, incomplete tails, or malformed records.
pub fn overseer_records(store: &HoldStore) -> Result<Vec<OverseerRecord>, OverseerRoutingError> {
    let path = policy_path(store);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(|source| OverseerRoutingError::ReadPolicyJournal {
        path: path.clone(),
        source,
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(OverseerRoutingError::IncompletePolicyTail { path });
    }
    BufReader::new(bytes.as_slice())
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.map_err(|source| OverseerRoutingError::ReadPolicyJournal {
                path: path.clone(),
                source,
            })?;
            serde_json::from_str(&line).map_err(|source| {
                OverseerRoutingError::MalformedPolicyRecord {
                    path: path.clone(),
                    line: index + 1,
                    detail: source.to_string(),
                }
            })
        })
        .collect()
}

fn append_record(store: &HoldStore, record: &OverseerRecord) -> Result<(), OverseerRoutingError> {
    let path = policy_path(store);
    let Some(directory) = path.parent() else {
        return Err(OverseerRoutingError::MissingPolicyParent { path });
    };
    fs::create_dir_all(directory).map_err(|source| {
        OverseerRoutingError::CreatePolicyDirectory {
            path: directory.to_path_buf(),
            source,
        }
    })?;
    let lock_path = directory.join("events.lock");
    let _lock = PolicyLock::acquire(lock_path)?;
    if path.exists() {
        let bytes = fs::read(&path).map_err(|source| OverseerRoutingError::ReadPolicyJournal {
            path: path.clone(),
            source,
        })?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(OverseerRoutingError::IncompletePolicyTail { path });
        }
    }
    let mut line = serde_json::to_vec(record)
        .map_err(|source| OverseerRoutingError::SerializePolicyRecord { source })?;
    line.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| OverseerRoutingError::OpenPolicyJournal {
            path: path.clone(),
            source,
        })?;
    file.write_all(&line)
        .map_err(|source| OverseerRoutingError::AppendPolicyJournal {
            path: path.clone(),
            source,
        })?;
    file.sync_data()
        .map_err(|source| OverseerRoutingError::SyncPolicyJournal { path, source })
}

fn retain_rule_refusal(
    store: &HoldStore,
    proposal_id: &str,
    refusal: &RuleRefusal,
    timestamp: EventTimestamp,
) -> Result<(), OverseerRoutingError> {
    append_record(
        store,
        &OverseerRecord::RuleAdmissionRefused {
            timestamp,
            proposal_id: proposal_id.to_owned(),
            reason: match refusal {
                RuleRefusal::SubjectIsRuling { door } => {
                    format!("door kind `{}` is a ruling", door_name(*door))
                }
                RuleRefusal::AmbiguousReplay { matching_holds } => format!(
                    "replay predicts conflicting answers for {} matching holds",
                    matching_holds.len()
                ),
                RuleRefusal::NoReplayWitness => "replay found no answered hold".to_owned(),
            },
        },
    )
}

fn latest_answer(hold: &Hold) -> Option<&str> {
    hold.records().iter().rev().find_map(|record| {
        if let crate::hold_store::HoldRecord::Answered { answer, .. } = record {
            Some(answer.as_str())
        } else {
            None
        }
    })
}

fn report_contains(value: &Value, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    match value {
        Value::String(text) => text.to_lowercase().contains(&needle),
        Value::Array(values) => values.iter().any(|value| report_contains(value, &needle)),
        Value::Object(values) => values.values().any(|value| report_contains(value, &needle)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn has_named_options(report: &Value) -> bool {
    let Value::Object(fields) = report else {
        return false;
    };
    ["options", "alternatives", "choices"].iter().any(|name| {
        fields.get(*name).is_some_and(|value| match value {
            Value::Array(values) => !values.is_empty(),
            Value::Object(values) => !values.is_empty(),
            Value::String(value) => !value.trim().is_empty(),
            Value::Null | Value::Bool(_) | Value::Number(_) => false,
        })
    })
}

fn choice_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    ["choice", "decision", "select", "option"]
        .iter()
        .any(|token| normalized.contains(token))
}

fn normalize_answer(answer: &str) -> String {
    answer
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn door_name(door: DoorKind) -> &'static str {
    match door {
        DoorKind::CriterionRevision => "criterion-revision",
        DoorKind::WorkerEnvironmentExtension => "worker-environment-extension",
        DoorKind::BaseCurrencyAcceptance => "base-currency-acceptance",
        DoorKind::ParkOverrule => "park-overrule",
        DoorKind::RecoverySpendAuthorization => "recovery-spend-authorization",
        DoorKind::FleetInstallAuthorization => "fleet-install-authorization",
    }
}

fn policy_path(store: &HoldStore) -> PathBuf {
    store.root().join("overseer/events.jsonl")
}

#[cfg(unix)]
mod policy_lock {
    use std::ffi::c_int;

    pub const EXCLUSIVE: c_int = 2;
    pub const UNLOCK: c_int = 8;

    unsafe extern "C" {
        pub fn flock(file_descriptor: c_int, operation: c_int) -> c_int;
    }
}

struct PolicyLock {
    path: PathBuf,
    file: File,
}

impl PolicyLock {
    #[cfg(unix)]
    fn acquire(path: PathBuf) -> Result<Self, OverseerRoutingError> {
        use std::os::fd::AsRawFd;

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| OverseerRoutingError::AcquirePolicyLock {
                path: path.clone(),
                source,
            })?;
        if unsafe { policy_lock::flock(file.as_raw_fd(), policy_lock::EXCLUSIVE) } == -1 {
            return Err(OverseerRoutingError::AcquirePolicyLock {
                path,
                source: std::io::Error::last_os_error(),
            });
        }
        Ok(Self { path, file })
    }

    #[cfg(not(unix))]
    fn acquire(path: PathBuf) -> Result<Self, OverseerRoutingError> {
        Err(OverseerRoutingError::PolicyLockUnsupported { path })
    }
}

impl Drop for PolicyLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { policy_lock::flock(self.file.as_raw_fd(), policy_lock::UNLOCK) } == -1 {
                tracing::error!(path = %self.path.display(), error = ?std::io::Error::last_os_error(), "failed to release overseer policy lock");
            }
        }
    }
}

/// A routing boundary operation failed without guessing an outcome.
#[derive(Debug, Error)]
pub enum OverseerRoutingError {
    /// The underlying hold store failed.
    #[error(transparent)]
    HoldStore(#[from] HoldStoreError),
    /// The proposed rule did not parse.
    #[error(transparent)]
    RuleParse(#[from] RoutingRuleParseError),
    /// A proposed rule omitted its predicted answer.
    #[error("proposed routing rule expected answer must not be empty")]
    EmptyExpectedAnswer,
    /// A required install request field is blank.
    #[error("install request field `{field}` must not be empty")]
    EmptyInstallField { field: &'static str },
    /// The policy journal has no parent directory.
    #[error("overseer policy journal `{path}` has no parent directory")]
    MissingPolicyParent { path: PathBuf },
    /// The policy journal directory could not be created.
    #[error("failed to create overseer policy directory `{path}`")]
    CreatePolicyDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Exclusive policy journal access could not be acquired.
    #[error("failed to acquire overseer policy lock `{path}`")]
    AcquirePolicyLock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Advisory policy locking is unavailable on this platform.
    #[error("advisory locking is unsupported for overseer policy journal `{path}`")]
    PolicyLockUnsupported { path: PathBuf },
    /// The policy journal could not be opened.
    #[error("failed to open overseer policy journal `{path}`")]
    OpenPolicyJournal {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The policy journal could not be read.
    #[error("failed to read overseer policy journal `{path}`")]
    ReadPolicyJournal {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A policy record could not be encoded.
    #[error("failed to serialize overseer policy record")]
    SerializePolicyRecord {
        #[source]
        source: serde_json::Error,
    },
    /// A complete policy record could not be appended.
    #[error("failed to append overseer policy journal `{path}`")]
    AppendPolicyJournal {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// An appended policy record could not be synced.
    #[error("failed to sync overseer policy journal `{path}`")]
    SyncPolicyJournal {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Existing policy bytes end in an incomplete JSONL record.
    #[error("overseer policy journal `{path}` has an incomplete final record")]
    IncompletePolicyTail { path: PathBuf },
    /// One policy journal line is malformed.
    #[error("malformed overseer policy record at `{path}` line {line}: {detail}")]
    MalformedPolicyRecord {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    /// The authority-supplied installer failed.
    #[error("fleet install failed: {detail}")]
    InstallFailed { detail: String },
}
