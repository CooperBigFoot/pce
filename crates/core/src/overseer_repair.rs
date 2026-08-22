//! repair pipeline = fold(dispatch × build × acceptance × fleet quiet × install, repair records)
//!
//! A repair is identified by verified code evidence. Its staged artifact cannot become installed
//! state until the human accepts its digest and every observed run is outside a dispatch.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    EventTimestamp, HoldStore, HoldStoreError, RepairDispatcherIdentity, RepairFailureStage,
    RepairRecord, RunBinaryUpdate, RunRegistrationKey,
};

/// Verified evidence and isolated paths for one binary defect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedDefect {
    source_hold: String,
    code_fact: String,
    reproduction: String,
    summary: String,
    repository_root: PathBuf,
    base_commit: String,
    worktree: PathBuf,
    target_directory: PathBuf,
    installed_binary: PathBuf,
}

impl VerifiedDefect {
    /// Parses evidence and caller-selected paths without consulting process state.
    ///
    /// # Errors
    ///
    /// Returns an error for blank evidence, malformed object identifiers, or non-absolute paths.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        source_hold: impl Into<String>,
        code_fact: impl Into<String>,
        reproduction: impl Into<String>,
        summary: impl Into<String>,
        repository_root: impl Into<PathBuf>,
        base_commit: impl Into<String>,
        worktree: impl Into<PathBuf>,
        target_directory: impl Into<PathBuf>,
        installed_binary: impl Into<PathBuf>,
    ) -> Result<Self, RepairPipelineError> {
        let defect = Self {
            source_hold: source_hold.into(),
            code_fact: code_fact.into(),
            reproduction: reproduction.into(),
            summary: summary.into(),
            repository_root: repository_root.into(),
            base_commit: base_commit.into(),
            worktree: worktree.into(),
            target_directory: target_directory.into(),
            installed_binary: installed_binary.into(),
        };
        for (field, value) in [
            ("source_hold", defect.source_hold.as_str()),
            ("code_fact", defect.code_fact.as_str()),
            ("reproduction", defect.reproduction.as_str()),
            ("summary", defect.summary.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(RepairPipelineError::EmptyField { field });
            }
        }
        validate_hex("source_hold", &defect.source_hold, 64)?;
        validate_hex("base_commit", &defect.base_commit, 40)?;
        for (field, path) in [
            ("repository_root", defect.repository_root.as_path()),
            ("worktree", defect.worktree.as_path()),
            ("target_directory", defect.target_directory.as_path()),
            ("installed_binary", defect.installed_binary.as_path()),
        ] {
            if !path.is_absolute() {
                return Err(RepairPipelineError::PathNotAbsolute {
                    field,
                    path: path.to_path_buf(),
                });
            }
        }
        if defect
            .target_directory
            .starts_with(&defect.repository_root.join("target"))
            || defect
                .installed_binary
                .starts_with(&defect.target_directory)
            || defect
                .target_directory
                .starts_with(&defect.installed_binary)
        {
            return Err(RepairPipelineError::TargetCanReachInstalledBuild {
                target: defect.target_directory,
                repository_target: defect.repository_root.join("target"),
            });
        }
        Ok(defect)
    }

    /// Derives the stable identity from the finding, not the worker attempt.
    pub fn id(&self) -> String {
        let mut digest = Sha256::new();
        for component in [
            self.source_hold.as_bytes(),
            self.code_fact.as_bytes(),
            self.reproduction.as_bytes(),
            self.base_commit.as_bytes(),
        ] {
            digest.update((component.len() as u64).to_be_bytes());
            digest.update(component);
        }
        format!("{:x}", digest.finalize())
    }
}

/// Durable dispatch coordinates supplied to the isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RepairDispatch {
    id: String,
    source_hold: String,
    code_fact: String,
    reproduction: String,
    summary: String,
    repository_root: PathBuf,
    base_commit: String,
    worktree: PathBuf,
    target_directory: PathBuf,
    installed_binary: PathBuf,
}

impl RepairDispatch {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn source_hold(&self) -> &str {
        &self.source_hold
    }
    pub fn summary(&self) -> &str {
        &self.summary
    }
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }
    pub fn base_commit(&self) -> &str {
        &self.base_commit
    }
    pub fn worktree(&self) -> &Path {
        &self.worktree
    }
    pub fn target_directory(&self) -> &Path {
        &self.target_directory
    }
    pub fn installed_binary(&self) -> &Path {
        &self.installed_binary
    }
    pub fn code_fact(&self) -> &str {
        &self.code_fact
    }
    pub fn reproduction(&self) -> &str {
        &self.reproduction
    }
}

impl From<&VerifiedDefect> for RepairDispatch {
    fn from(value: &VerifiedDefect) -> Self {
        Self {
            id: value.id(),
            source_hold: value.source_hold.clone(),
            code_fact: value.code_fact.clone(),
            reproduction: value.reproduction.clone(),
            summary: value.summary.clone(),
            repository_root: value.repository_root.clone(),
            base_commit: value.base_commit.clone(),
            worktree: value.worktree.clone(),
            target_directory: value.target_directory.clone(),
            installed_binary: value.installed_binary.clone(),
        }
    }
}

/// Exact staged artifact that passed the repair worktree's suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuiltRepair {
    commit: String,
    artifact: PathBuf,
    digest: String,
    touched_paths: Vec<PathBuf>,
}

impl BuiltRepair {
    /// Parses facts observed only after the worker and full suite completed.
    ///
    /// # Errors
    ///
    /// Returns an error when the commit, digest, artifact path, or touched-path set is invalid.
    pub fn parse(
        commit: impl Into<String>,
        artifact: impl Into<PathBuf>,
        digest: impl Into<String>,
        touched_paths: Vec<PathBuf>,
    ) -> Result<Self, RepairPipelineError> {
        let built = Self {
            commit: commit.into(),
            artifact: artifact.into(),
            digest: digest.into(),
            touched_paths,
        };
        validate_hex("commit", &built.commit, 40)?;
        validate_hex("digest", &built.digest, 64)?;
        if !built.artifact.is_absolute() {
            return Err(RepairPipelineError::PathNotAbsolute {
                field: "artifact",
                path: built.artifact,
            });
        }
        if built.touched_paths.is_empty() {
            return Err(RepairPipelineError::NoTouchedPaths);
        }
        if let Some(path) = built.touched_paths.iter().find(|path| path.is_absolute()) {
            return Err(RepairPipelineError::TouchedPathAbsolute { path: path.clone() });
        }
        Ok(built)
    }

    pub fn commit(&self) -> &str {
        &self.commit
    }
    pub fn artifact(&self) -> &Path {
        &self.artifact
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn touched_paths(&self) -> &[PathBuf] {
        &self.touched_paths
    }
}

/// Result of the idempotent dispatch boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchRepairDecision {
    Built {
        dispatch: RepairDispatch,
        built: BuiltRepair,
    },
    Failed {
        dispatch: RepairDispatch,
        detail: String,
    },
    AlreadyDispatched,
    PathConflict {
        repair_id: String,
    },
}

/// One registered run observed while the install boundary holds its journal lease.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FleetRunObservation {
    run_id: String,
    journal: PathBuf,
    dispatch: Option<(String, u64)>,
}

impl FleetRunObservation {
    pub fn quiet(run_id: impl Into<String>, journal: PathBuf) -> Self {
        Self {
            run_id: run_id.into(),
            journal,
            dispatch: None,
        }
    }
    pub fn mid_dispatch(
        run_id: impl Into<String>,
        journal: PathBuf,
        package: impl Into<String>,
        issuance: u64,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            journal,
            dispatch: Some((package.into(), issuance)),
        }
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn journal(&self) -> &Path {
        &self.journal
    }
    pub fn dispatch(&self) -> Option<(&str, u64)> {
        self.dispatch
            .as_ref()
            .map(|(package, issuance)| (package.as_str(), *issuance))
    }
}

/// Result of one safe-install observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallRepairDecision {
    NothingAccepted,
    WaitingForFleet {
        repair_id: String,
        blocking_runs: Vec<String>,
    },
    Installed {
        repair_id: String,
        digest: String,
    },
    AlreadyInstalled {
        repair_id: String,
        digest: String,
    },
    Failed {
        repair_id: String,
        detail: String,
    },
}

/// Converts a dead dispatcher's unclosed claim into a retained failure.
///
/// # Errors
///
/// Returns an error when repair state cannot be read or appended.
pub fn recover_dead_repair_dispatch<F>(
    store: &HoldStore,
    timestamp: EventTimestamp,
    is_alive: F,
) -> Result<bool, RepairPipelineError>
where
    F: Fn(&RepairDispatcherIdentity) -> Result<bool, String>,
{
    let records = store.repair_records()?;
    let active = records.iter().rev().find_map(|record| match record {
        RepairRecord::DefectBriefDispatched {
            repair_id,
            dispatcher: Some(dispatcher),
            ..
        } if records
            .iter()
            .rev()
            .find(|candidate| record_repair_id(candidate) == repair_id)
            == Some(record) =>
        {
            Some((repair_id.clone(), *dispatcher))
        }
        _ => None,
    });
    let dead = match active {
        Some((repair_id, dispatcher))
            if !is_alive(&dispatcher)
                .map_err(|detail| RepairPipelineError::DispatcherObservation { detail })? =>
        {
            Some((repair_id, dispatcher))
        }
        _ => None,
    };
    let Some((repair_id, dispatcher)) = dead else {
        return Ok(false);
    };
    let failure = RepairRecord::Failed {
        timestamp,
        repair_id: repair_id.clone(),
        stage: RepairFailureStage::Dispatch,
        detail: format!(
            "dispatcher process {}@{}.{:06} exited without closing its claim",
            dispatcher.process_number, dispatcher.started_seconds, dispatcher.started_microseconds
        ),
    };
    store
        .append_repair_record_if(&failure, |retained| {
            matches!(retained.iter().rev().find(|candidate| record_repair_id(candidate) == repair_id),
                Some(RepairRecord::DefectBriefDispatched { dispatcher: Some(current), .. }) if current == &dispatcher)
        })
        .map_err(RepairPipelineError::Store)
}

/// Claims a finding before invoking the worker, then records its built artifact or failure.
///
/// # Errors
///
/// Returns an error only when durable repair state cannot be read or appended.
pub fn dispatch_repair<F>(
    store: &HoldStore,
    defect: &VerifiedDefect,
    timestamp: EventTimestamp,
    dispatch_worker: F,
) -> Result<DispatchRepairDecision, RepairPipelineError>
where
    F: FnOnce(&RepairDispatch) -> Result<BuiltRepair, String>,
{
    dispatch_repair_with_identity(store, defect, None, timestamp, dispatch_worker)
}

/// Claims a repair on behalf of one observable dispatcher process.
pub fn dispatch_repair_with_identity<F>(
    store: &HoldStore,
    defect: &VerifiedDefect,
    dispatcher: Option<RepairDispatcherIdentity>,
    timestamp: EventTimestamp,
    dispatch_worker: F,
) -> Result<DispatchRepairDecision, RepairPipelineError>
where
    F: FnOnce(&RepairDispatch) -> Result<BuiltRepair, String>,
{
    let dispatch = RepairDispatch::from(defect);
    let record = RepairRecord::DefectBriefDispatched {
        timestamp,
        repair_id: dispatch.id.clone(),
        source_hold: dispatch.source_hold.clone(),
        code_fact: dispatch.code_fact.clone(),
        reproduction: dispatch.reproduction.clone(),
        summary: dispatch.summary.clone(),
        repository_root: dispatch.repository_root.clone(),
        base_commit: dispatch.base_commit.clone(),
        worktree: dispatch.worktree.clone(),
        target_directory: dispatch.target_directory.clone(),
        installed_binary: dispatch.installed_binary.clone(),
        dispatcher,
    };
    let mut conflicting_repair = None;
    let claimed = store.append_repair_record_if(&record, |records| {
        let latest_for_id = records
            .iter()
            .rev()
            .find(|record| record_repair_id(record) == dispatch.id);
        if latest_for_id.is_some_and(|record| {
            !matches!(
                record,
                RepairRecord::Failed {
                    stage: RepairFailureStage::Dispatch | RepairFailureStage::Build,
                    ..
                }
            )
        }) {
            return false;
        }
        conflicting_repair = records.iter().find_map(|record| match record {
            RepairRecord::DefectBriefDispatched { repair_id, .. }
                if repair_id != &dispatch.id
                    && !records
                        .iter()
                        .rev()
                        .find(|later| record_repair_id(later) == repair_id)
                        .is_some_and(|latest| {
                            matches!(
                                latest,
                                RepairRecord::Installed { .. }
                                    | RepairRecord::Failed {
                                        stage: RepairFailureStage::Dispatch
                                            | RepairFailureStage::Build,
                                        ..
                                    }
                            )
                        }) =>
            {
                Some(repair_id.clone())
            }
            _ => None,
        });
        conflicting_repair.is_none()
    })?;
    if !claimed {
        return Ok(conflicting_repair
            .map_or(DispatchRepairDecision::AlreadyDispatched, |repair_id| {
                DispatchRepairDecision::PathConflict { repair_id }
            }));
    }
    match dispatch_worker(&dispatch) {
        Ok(built) => {
            store.append_repair_record(&RepairRecord::Built {
                timestamp,
                repair_id: dispatch.id.clone(),
                commit: built.commit.clone(),
                artifact: built.artifact.clone(),
                digest: built.digest.clone(),
                touched_paths: built.touched_paths.clone(),
            })?;
            Ok(DispatchRepairDecision::Built { dispatch, built })
        }
        Err(detail) => {
            let stage = if detail.starts_with("cargo ")
                || detail.starts_with("stage repair")
                || detail.starts_with("commit repair")
                || detail.starts_with("repair worker produced")
                || detail.starts_with("failed to read")
            {
                RepairFailureStage::Build
            } else {
                RepairFailureStage::Dispatch
            };
            store.append_repair_record(&RepairRecord::Failed {
                timestamp,
                repair_id: dispatch.id.clone(),
                stage,
                detail: bounded_detail(detail.clone()),
            })?;
            Ok(DispatchRepairDecision::Failed { dispatch, detail })
        }
    }
}

/// Retains one human-attributed, digest-bound acceptance.
///
/// # Errors
///
/// Returns an error when the repair is not built, the actor is blank, or state cannot be retained.
pub fn accept_repair(
    store: &HoldStore,
    repair_id: &str,
    by: &str,
    timestamp: EventTimestamp,
) -> Result<(), RepairPipelineError> {
    if by.trim().is_empty() {
        return Err(RepairPipelineError::EmptyField { field: "by" });
    }
    let records = store.repair_records()?;
    let digest = latest_built(&records, repair_id)
        .ok_or_else(|| RepairPipelineError::RepairNotBuilt {
            repair_id: repair_id.to_owned(),
        })?
        .digest
        .clone();
    let record = RepairRecord::InstallAccepted {
        timestamp,
        repair_id: repair_id.to_owned(),
        by: by.to_owned(),
        digest,
    };
    let _ = store.append_repair_record_if(&record, |retained| {
        !retained.iter().any(|record| {
            matches!(record,
            RepairRecord::InstallAccepted { repair_id: existing, .. } if existing == repair_id)
        })
    })?;
    Ok(())
}

/// Retains an install-stage failure for the oldest accepted repair that is not installed.
///
/// # Errors
///
/// Returns an error when there is no pending accepted repair or the fact cannot be retained.
pub fn record_pending_install_failure(
    store: &HoldStore,
    timestamp: EventTimestamp,
    detail: String,
) -> Result<(), RepairPipelineError> {
    let records = store.repair_records()?;
    let repair_id = records
        .iter()
        .find_map(|record| match record {
            RepairRecord::InstallAccepted { repair_id, .. }
                if !records.iter().any(|later| {
                    matches!(later,
                    RepairRecord::Installed { repair_id: installed, .. } if installed == repair_id)
                }) =>
            {
                Some(repair_id.clone())
            }
            _ => None,
        })
        .ok_or(RepairPipelineError::NoPendingInstall)?;
    store.append_repair_record(&RepairRecord::Failed {
        timestamp,
        repair_id,
        stage: RepairFailureStage::Install,
        detail: bounded_detail(detail),
    })?;
    Ok(())
}

/// Installs the oldest accepted repair only when every leased run is quiet.
///
/// The caller must retain the journal leases represented by `fleet` until this function returns.
///
/// # Errors
///
/// Returns an error when state cannot be read, locked, or appended.
pub fn install_repair<F>(
    store: &HoldStore,
    fleet: &[FleetRunObservation],
    timestamp: EventTimestamp,
    install: F,
) -> Result<InstallRepairDecision, RepairPipelineError>
where
    F: FnOnce(&RepairInstall) -> Result<(), String>,
{
    let _lease = InstallLease::acquire(store.root().join("repairs").join("install.lock"))?;
    let records = store.repair_records()?;
    let Some((repair_id, accepted_digest)) = records.iter().find_map(|record| match record {
        RepairRecord::InstallAccepted {
            repair_id, digest, ..
        } if !records.iter().any(|later| matches!(later,
            RepairRecord::Installed { repair_id: installed, .. } if installed == repair_id))
            || fleet.iter().any(|run| !records.iter().any(|later| matches!(later,
                RepairRecord::RunNotified { repair_id: notified, run_id, digest: notified_digest, .. }
                    if notified == repair_id && run_id == run.run_id() && notified_digest == digest))) =>
            Some((repair_id.clone(), digest.clone())),
        _ => None,
    }) else {
        return Ok(InstallRepairDecision::NothingAccepted);
    };
    let prior_install = installed(&records, &repair_id);
    let dispatch = latest_dispatch(&records, &repair_id).ok_or_else(|| {
        RepairPipelineError::MissingDispatch {
            repair_id: repair_id.clone(),
        }
    })?;
    let built =
        latest_built(&records, &repair_id).ok_or_else(|| RepairPipelineError::RepairNotBuilt {
            repair_id: repair_id.clone(),
        })?;
    if built.digest != &accepted_digest {
        return Err(RepairPipelineError::AcceptedDigestMismatch {
            repair_id,
            accepted: accepted_digest,
            built: built.digest.clone(),
        });
    }
    if let Some((digest, _)) = prior_install {
        notify_runs(store, &records, fleet, &repair_id, &digest, timestamp)?;
        return Ok(InstallRepairDecision::AlreadyInstalled { repair_id, digest });
    }
    let blocking_runs: Vec<String> = fleet
        .iter()
        .filter(|run| run.dispatch.is_some())
        .map(|run| run.run_id.clone())
        .collect();
    if !blocking_runs.is_empty() {
        let deferred = RepairRecord::InstallDeferred {
            timestamp,
            repair_id: repair_id.clone(),
            blocking_runs: blocking_runs.clone(),
            touched_paths: built.touched_paths.clone(),
        };
        let _ = store.append_repair_record_if(&deferred, |retained| {
            !retained.last().is_some_and(|record| {
                matches!(record,
                RepairRecord::InstallDeferred { repair_id: existing, blocking_runs: prior, .. }
                    if existing == &repair_id && prior == &blocking_runs)
            })
        })?;
        return Ok(InstallRepairDecision::WaitingForFleet {
            repair_id,
            blocking_runs,
        });
    }
    let request = RepairInstall {
        repair_id: repair_id.clone(),
        repository_root: dispatch.repository_root.clone(),
        base_commit: dispatch.base_commit.clone(),
        commit: built.commit.clone(),
        artifact: built.artifact.clone(),
        digest: built.digest.clone(),
        installed_binary: dispatch.installed_binary.clone(),
        touched_paths: built.touched_paths.clone(),
    };
    if let Err(detail) = install(&request) {
        let detail = bounded_detail(detail);
        store.append_repair_record(&RepairRecord::Failed {
            timestamp,
            repair_id: repair_id.clone(),
            stage: RepairFailureStage::Install,
            detail: detail.clone(),
        })?;
        return Ok(InstallRepairDecision::Failed { repair_id, detail });
    }
    store.append_repair_record(&RepairRecord::Installed {
        timestamp,
        repair_id: repair_id.clone(),
        digest: built.digest.clone(),
        installed_binary: dispatch.installed_binary.clone(),
    })?;
    let records = store.repair_records()?;
    notify_runs(store, &records, fleet, &repair_id, built.digest, timestamp)?;
    Ok(InstallRepairDecision::Installed {
        repair_id,
        digest: built.digest.clone(),
    })
}

fn notify_runs(
    store: &HoldStore,
    records: &[RepairRecord],
    fleet: &[FleetRunObservation],
    repair_id: &str,
    digest: &str,
    timestamp: EventTimestamp,
) -> Result<(), RepairPipelineError> {
    for run in fleet {
        if records.iter().any(|record| {
            matches!(record,
            RepairRecord::RunNotified { repair_id: existing, run_id, digest: existing_digest, .. }
                if existing == repair_id && run_id == run.run_id() && existing_digest == digest)
        }) {
            continue;
        }
        let key = RunRegistrationKey::parse(run.run_id.clone()).map_err(|error| {
            RepairPipelineError::InvalidRunIdentity {
                run_id: run.run_id.clone(),
                detail: error.to_string(),
            }
        })?;
        if let Err(error) = store.notify_run_binary_update(
            &key,
            RunBinaryUpdate {
                timestamp,
                repair_id: repair_id.to_owned(),
                digest: digest.to_owned(),
            },
        ) {
            let detail = bounded_detail(error.to_string());
            store.append_repair_record(&RepairRecord::Failed {
                timestamp,
                repair_id: repair_id.to_owned(),
                stage: RepairFailureStage::Notify,
                detail: detail.clone(),
            })?;
            return Err(RepairPipelineError::NotificationFailed {
                run_id: run.run_id.clone(),
                detail,
            });
        }
        store.append_repair_record(&RepairRecord::RunNotified {
            timestamp,
            repair_id: repair_id.to_owned(),
            run_id: run.run_id.clone(),
            journal: run.journal.clone(),
            digest: digest.to_owned(),
        })?;
    }
    store.append_repair_record_if(
        &RepairRecord::NotificationsCompleted {
            timestamp,
            repair_id: repair_id.to_owned(),
            digest: digest.to_owned(),
        },
        |retained| !retained.iter().any(|record| matches!(record,
            RepairRecord::NotificationsCompleted { repair_id: existing, digest: existing_digest, .. }
                if existing == repair_id && existing_digest == digest)),
    )?;
    Ok(())
}

/// Exact merge and swap request admitted by the repair fold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairInstall {
    repair_id: String,
    repository_root: PathBuf,
    base_commit: String,
    commit: String,
    artifact: PathBuf,
    digest: String,
    installed_binary: PathBuf,
    touched_paths: Vec<PathBuf>,
}

impl RepairInstall {
    pub fn repair_id(&self) -> &str {
        &self.repair_id
    }
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }
    pub fn base_commit(&self) -> &str {
        &self.base_commit
    }
    pub fn commit(&self) -> &str {
        &self.commit
    }
    pub fn artifact(&self) -> &Path {
        &self.artifact
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn installed_binary(&self) -> &Path {
        &self.installed_binary
    }
    pub fn touched_paths(&self) -> &[PathBuf] {
        &self.touched_paths
    }
}

struct DispatchFact<'a> {
    repository_root: &'a PathBuf,
    base_commit: &'a String,
    installed_binary: &'a PathBuf,
}
struct BuiltFact<'a> {
    commit: &'a String,
    artifact: &'a PathBuf,
    digest: &'a String,
    touched_paths: &'a Vec<PathBuf>,
}

fn latest_dispatch<'a>(records: &'a [RepairRecord], wanted: &str) -> Option<DispatchFact<'a>> {
    records.iter().rev().find_map(|record| match record {
        RepairRecord::DefectBriefDispatched {
            repair_id,
            repository_root,
            base_commit,
            installed_binary,
            ..
        } if repair_id == wanted => Some(DispatchFact {
            repository_root,
            base_commit,
            installed_binary,
        }),
        _ => None,
    })
}

fn latest_built<'a>(records: &'a [RepairRecord], wanted: &str) -> Option<BuiltFact<'a>> {
    records.iter().rev().find_map(|record| match record {
        RepairRecord::Built {
            repair_id,
            commit,
            artifact,
            digest,
            touched_paths,
            ..
        } if repair_id == wanted => Some(BuiltFact {
            commit,
            artifact,
            digest,
            touched_paths,
        }),
        _ => None,
    })
}

fn installed(records: &[RepairRecord], wanted: &str) -> Option<(String, PathBuf)> {
    records.iter().rev().find_map(|record| match record {
        RepairRecord::Installed {
            repair_id,
            digest,
            installed_binary,
            ..
        } if repair_id == wanted => Some((digest.clone(), installed_binary.clone())),
        _ => None,
    })
}

fn record_repair_id(record: &RepairRecord) -> &str {
    match record {
        RepairRecord::DefectBriefDispatched { repair_id, .. }
        | RepairRecord::Built { repair_id, .. }
        | RepairRecord::InstallAccepted { repair_id, .. }
        | RepairRecord::InstallDeferred { repair_id, .. }
        | RepairRecord::Installed { repair_id, .. }
        | RepairRecord::RunNotified { repair_id, .. }
        | RepairRecord::NotificationsCompleted { repair_id, .. }
        | RepairRecord::Failed { repair_id, .. } => repair_id,
    }
}

fn validate_hex(
    field: &'static str,
    value: &str,
    length: usize,
) -> Result<(), RepairPipelineError> {
    if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RepairPipelineError::InvalidHex {
            field,
            value: value.to_owned(),
            length,
        });
    }
    Ok(())
}

fn bounded_detail(mut detail: String) -> String {
    const LIMIT: usize = 2_000;
    if detail.len() <= LIMIT {
        return detail;
    }
    let boundary = detail
        .char_indices()
        .take_while(|(index, _)| *index <= LIMIT)
        .last()
        .map_or(0, |(index, _)| index);
    detail.truncate(boundary);
    detail
}

#[cfg(unix)]
mod advisory {
    use std::ffi::c_int;
    pub const EXCLUSIVE: c_int = 2;
    pub const UNLOCK: c_int = 8;
    unsafe extern "C" {
        pub fn flock(fd: c_int, operation: c_int) -> c_int;
    }
}

struct InstallLease {
    file: File,
    path: PathBuf,
}
impl InstallLease {
    #[cfg(unix)]
    fn acquire(path: PathBuf) -> Result<Self, RepairPipelineError> {
        use std::os::fd::AsRawFd;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                RepairPipelineError::InstallLease {
                    path: path.clone(),
                    source,
                }
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| RepairPipelineError::InstallLease {
                path: path.clone(),
                source,
            })?;
        if unsafe { advisory::flock(file.as_raw_fd(), advisory::EXCLUSIVE) } == -1 {
            return Err(RepairPipelineError::InstallLease {
                path,
                source: std::io::Error::last_os_error(),
            });
        }
        Ok(Self { file, path })
    }
    #[cfg(not(unix))]
    fn acquire(path: PathBuf) -> Result<Self, RepairPipelineError> {
        Err(RepairPipelineError::InstallLeaseUnsupported { path })
    }
}
impl Drop for InstallLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { advisory::flock(self.file.as_raw_fd(), advisory::UNLOCK) } == -1 {
                tracing::error!(path = %self.path.display(), error = ?std::io::Error::last_os_error(), "failed to release repair install lease");
            }
        }
    }
}

/// A repair pipeline input or retained state is invalid.
#[derive(Debug, Error)]
pub enum RepairPipelineError {
    /// A required text field is blank.
    #[error("repair field `{field}` must not be empty")]
    EmptyField { field: &'static str },
    /// A hexadecimal identity has the wrong shape.
    #[error("repair field `{field}` must be {length} hexadecimal characters, got `{value}`")]
    InvalidHex {
        field: &'static str,
        value: String,
        length: usize,
    },
    /// A path crossing the composition boundary is not absolute.
    #[error("repair path `{field}` must be absolute, got `{path}`")]
    PathNotAbsolute { field: &'static str, path: PathBuf },
    /// The isolated target could mutate the repository's installed build.
    #[error("repair target `{target}` must not be inside repository target `{repository_target}`")]
    TargetCanReachInstalledBuild {
        target: PathBuf,
        repository_target: PathBuf,
    },
    /// A successful repair commit changed no tracked path.
    #[error("built repair must name at least one touched path")]
    NoTouchedPaths,
    /// A changed path escaped repository-relative form.
    #[error("repair touched path must be repository-relative, got `{path}`")]
    TouchedPathAbsolute { path: PathBuf },
    /// Acceptance named a repair that has no built artifact.
    #[error("repair `{repair_id}` has no built artifact")]
    RepairNotBuilt { repair_id: String },
    /// Retained state lost the dispatch coordinates for an accepted repair.
    #[error("repair `{repair_id}` has no dispatch record")]
    MissingDispatch { repair_id: String },
    /// Acceptance and staged artifact name different bytes.
    #[error("repair `{repair_id}` accepted digest {accepted} but built digest is {built}")]
    AcceptedDigestMismatch {
        repair_id: String,
        accepted: String,
        built: String,
    },
    /// A recorded dispatcher process identity could not be observed.
    #[error("failed to observe repair dispatcher: {detail}")]
    DispatcherObservation { detail: String },
    /// No accepted repair exists to receive an install failure.
    #[error("there is no pending accepted repair")]
    NoPendingInstall,
    /// A fleet observation did not carry a valid registration identity.
    #[error("invalid fleet run identity `{run_id}`: {detail}")]
    InvalidRunIdentity { run_id: String, detail: String },
    /// A run update inbox could not receive the installed digest.
    #[error("failed to notify run `{run_id}`: {detail}")]
    NotificationFailed { run_id: String, detail: String },
    /// The append-only store could not retain or read repair state.
    #[error(transparent)]
    Store(#[from] HoldStoreError),
    /// The cross-process install lease could not be acquired.
    #[error("failed to acquire repair install lease `{path}`: {source}")]
    InstallLease {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Advisory locking is unavailable on this platform.
    #[error("repair install lease is unsupported on `{path}`")]
    InstallLeaseUnsupported { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Utc;

    use super::{RepairDispatcherIdentity, recover_dead_repair_dispatch};
    use crate::{EventTimestamp, HoldStore, RepairFailureStage, RepairRecord};

    #[test]
    fn dead_dispatcher_becomes_a_retryable_failure() {
        let root = std::env::temp_dir().join(format!(
            "pce-repair-recovery-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let store = HoldStore::new(&root);
        let dispatcher = RepairDispatcherIdentity {
            process_number: 42,
            started_seconds: 100,
            started_microseconds: 7,
        };
        store
            .append_repair_record(&RepairRecord::DefectBriefDispatched {
                timestamp: EventTimestamp::new(Utc::now()),
                repair_id: "a".repeat(64),
                source_hold: "b".repeat(64),
                code_fact: "src/main.rs:1 fixture".to_owned(),
                reproduction: "fixture".to_owned(),
                summary: "fixture".to_owned(),
                repository_root: root.join("repository"),
                base_commit: "c".repeat(40),
                worktree: root.join("worktree"),
                target_directory: root.join("target"),
                installed_binary: root.join("installed/pce"),
                dispatcher: Some(dispatcher),
            })
            .expect("dispatch claim");
        assert!(
            recover_dead_repair_dispatch(&store, EventTimestamp::new(Utc::now()), |_| Ok(false))
                .expect("recover dead dispatcher")
        );
        assert!(
            store
                .repair_records()
                .expect("records")
                .iter()
                .any(|record| {
                    matches!(record, RepairRecord::Failed {
                stage: RepairFailureStage::Dispatch,
                detail,
                ..
            } if detail.contains("42@100.000007"))
                })
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
