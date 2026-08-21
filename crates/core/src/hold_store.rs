//! hold store = fold(apply, initial hold, append-only hold events)
//!
//! A [`HoldKey`] is the SHA-256 digest of a length-delimited hold identity. One JSONL file carries
//! one hold's events. Readers derive current state by replaying the complete file.

use std::ffi::c_int;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::event_log::EventTimestamp;
use crate::overseer_registration::{RunRegistration, RunRegistrationError, RunRegistrationKey};
use crate::overseer_rulebook::{RoutingRule, initial_routing_rules};

/// The stable identity of one question within one work-graph run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldIdentity {
    repository: String,
    plan_version: String,
    package: String,
    question_kind: String,
}

impl HoldIdentity {
    /// Parses the four components which distinguish a hold.
    ///
    /// # Errors
    ///
    /// Returns [`HoldStoreError::EmptyIdentityField`] when any component is empty or whitespace.
    pub fn parse(
        repository: impl Into<String>,
        plan_version: impl Into<String>,
        package: impl Into<String>,
        question_kind: impl Into<String>,
    ) -> Result<Self, HoldStoreError> {
        let identity = Self {
            repository: repository.into(),
            plan_version: plan_version.into(),
            package: package.into(),
            question_kind: question_kind.into(),
        };
        for (field, value) in [
            ("repository", identity.repository.as_str()),
            ("plan_version", identity.plan_version.as_str()),
            ("package", identity.package.as_str()),
            ("question_kind", identity.question_kind.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(HoldStoreError::EmptyIdentityField { field });
            }
        }
        Ok(identity)
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn plan_version(&self) -> &str {
        &self.plan_version
    }
    pub fn package(&self) -> &str {
        &self.package
    }
    pub fn question_kind(&self) -> &str {
        &self.question_kind
    }

    /// Derives the collision-resistant file key for this identity.
    pub fn key(&self) -> HoldKey {
        let mut digest = Sha256::new();
        for component in [
            self.repository.as_bytes(),
            self.plan_version.as_bytes(),
            self.package.as_bytes(),
            self.question_kind.as_bytes(),
        ] {
            digest.update((component.len() as u64).to_be_bytes());
            digest.update(component);
        }
        HoldKey(format!("{:x}", digest.finalize()))
    }
}

/// A lowercase SHA-256 hold key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HoldKey(String);

impl HoldKey {
    /// Parses a key supplied by a store reader or CLI caller.
    ///
    /// # Errors
    ///
    /// Returns [`HoldStoreError::InvalidKey`] unless the value is 64 lowercase hex digits.
    pub fn parse(value: impl Into<String>) -> Result<Self, HoldStoreError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(HoldStoreError::InvalidKey { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The party currently responsible for progressing an open hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HoldRoute {
    Human,
    Overseer,
    ReportingRun,
}

impl HoldRoute {
    /// Parses the CLI spelling of a route.
    ///
    /// # Errors
    ///
    /// Returns [`HoldStoreError::InvalidRoute`] for an unknown spelling.
    pub fn parse(value: &str) -> Result<Self, HoldStoreError> {
        match value {
            "human" => Ok(Self::Human),
            "overseer" => Ok(Self::Overseer),
            "reporting-run" => Ok(Self::ReportingRun),
            _ => Err(HoldStoreError::InvalidRoute {
                value: value.to_owned(),
            }),
        }
    }
}

/// The current lifecycle state derived from a hold's event stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum HoldState {
    Open { route: HoldRoute },
    Closed,
}

/// One immutable entry in a hold file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum HoldRecord {
    Opened {
        timestamp: EventTimestamp,
        identity: HoldIdentity,
        report: Value,
    },
    Answered {
        timestamp: EventTimestamp,
        by: String,
        answer: String,
    },
    Routed {
        timestamp: EventTimestamp,
        route: HoldRoute,
    },
    RouteRefused {
        timestamp: EventTimestamp,
        requested: HoldRoute,
        enforced: HoldRoute,
        reason: String,
    },
    ReportingRunRequest {
        timestamp: EventTimestamp,
        request: String,
    },
    Closed {
        timestamp: EventTimestamp,
        reason: String,
    },
}

impl HoldRecord {
    fn timestamp(&self) -> EventTimestamp {
        match self {
            Self::Opened { timestamp, .. }
            | Self::Answered { timestamp, .. }
            | Self::Routed { timestamp, .. }
            | Self::RouteRefused { timestamp, .. }
            | Self::ReportingRunRequest { timestamp, .. }
            | Self::Closed { timestamp, .. } => *timestamp,
        }
    }
}

/// A complete hold and its state derived from all retained records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    key: HoldKey,
    identity: HoldIdentity,
    report: Value,
    state: HoldState,
    opened_at: EventTimestamp,
    updated_at: EventTimestamp,
    records: Vec<HoldRecord>,
}

impl Hold {
    pub fn key(&self) -> &HoldKey {
        &self.key
    }
    pub fn identity(&self) -> &HoldIdentity {
        &self.identity
    }
    pub fn report(&self) -> &Value {
        &self.report
    }
    pub fn state(&self) -> HoldState {
        self.state
    }
    pub const fn opened_at(&self) -> EventTimestamp {
        self.opened_at
    }
    pub const fn updated_at(&self) -> EventTimestamp {
        self.updated_at
    }
    pub fn records(&self) -> &[HoldRecord] {
        &self.records
    }
    pub fn is_open(&self) -> bool {
        matches!(self.state, HoldState::Open { .. })
    }
}

/// Whether `open` created the hold or found the identical question already present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenDisposition {
    Created,
    AlreadyExists,
}

/// The result of idempotently opening a question.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenHoldResult {
    disposition: OpenDisposition,
    hold: Hold,
}

impl OpenHoldResult {
    pub fn disposition(&self) -> OpenDisposition {
        self.disposition
    }
    pub fn hold(&self) -> &Hold {
        &self.hold
    }
}

/// Whether registration created, retained, or advanced the record for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegisterRunDisposition {
    Created,
    AlreadyCurrent,
    Updated,
}

/// The result of idempotently registering a work-graph run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRunResult {
    disposition: RegisterRunDisposition,
    registration: RunRegistration,
}

impl RegisterRunResult {
    pub fn disposition(&self) -> RegisterRunDisposition {
        self.disposition
    }
    pub fn registration(&self) -> &RunRegistration {
        &self.registration
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRegistrationRecord {
    timestamp: EventTimestamp,
    registration: RunRegistration,
}

/// Filesystem-backed append-only hold and run-registration storage.
#[derive(Clone, Debug)]
pub struct HoldStore {
    root: PathBuf,
}

impl HoldStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Loads all routing rules, installing the corpus-derived starting rulebook when needed.
    ///
    /// Initialization retains every complete record. If a process stopped after writing a prefix
    /// and part of the next record, the next query verifies the complete prefix, removes only the
    /// incomplete tail, and appends the missing records under the same advisory lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the rulebook cannot be created, locked, read, or appended, when a
    /// retained record is malformed, or when the retained starting prefix differs from the corpus.
    pub fn routing_rules(&self) -> Result<Vec<RoutingRule>, HoldStoreError> {
        self.ensure_root()?;
        let directory = self.root.join("rulebook");
        fs::create_dir_all(&directory).map_err(|source| {
            HoldStoreError::CreateRulebookDirectory {
                path: directory.clone(),
                source,
            }
        })?;
        let path = directory.join("routing-rules.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| HoldStoreError::OpenRulebook {
                path: path.clone(),
                source,
            })?;
        let _lock = AdvisoryLock::acquire(file, path.clone(), advisory_lock::EXCLUSIVE)?;
        let starting = initial_routing_rules();
        let mut retained = match read_rulebook(&path) {
            Ok(retained) => retained,
            Err(HoldStoreError::IncompleteRulebookTail { .. }) => {
                recover_incomplete_rulebook_tail(&path, &starting)?
            }
            Err(error) => return Err(error),
        };
        verify_starting_rulebook_prefix(&path, &retained, &starting)?;
        if retained.len() < starting.len() {
            for rule in &starting[retained.len()..] {
                append_rulebook_record(&path, rule)?;
            }
            retained = read_rulebook(&path)?;
        }
        Ok(retained)
    }

    /// Registers a work-graph run without duplicating an identical launch record.
    ///
    /// A changed frozen graph, journal, or Herdr session appends a new record for the same stable
    /// run. Readers return only the latest complete record.
    ///
    /// # Errors
    ///
    /// Returns an error when the registration directory cannot be created, locked, read, or
    /// durably appended, or when retained records are malformed or internally inconsistent.
    pub fn register_run(
        &self,
        registration: RunRegistration,
        timestamp: EventTimestamp,
    ) -> Result<RegisterRunResult, HoldStoreError> {
        self.ensure_root()?;
        let directory = self.registration_directory();
        fs::create_dir_all(&directory).map_err(|source| HoldStoreError::CreateDirectory {
            path: directory,
            source,
        })?;
        let key = registration.key();
        let path = self.registration_path(&key);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| HoldStoreError::OpenRegistration {
                path: path.clone(),
                source,
            })?;
        let lock = AdvisoryLock::acquire(file, path.clone(), advisory_lock::EXCLUSIVE)?;
        let current = if lock.is_empty()? {
            None
        } else {
            Some(read_registration_file(&path, &key)?)
        };
        if current
            .as_ref()
            .is_some_and(|(record, _)| record.registration == registration)
        {
            return Ok(RegisterRunResult {
                disposition: RegisterRunDisposition::AlreadyCurrent,
                registration,
            });
        }
        if let Some((record, line_count)) = &current
            && timestamp.as_datetime() < record.timestamp.as_datetime()
        {
            return Err(HoldStoreError::RegistrationTimestampWentBackwards {
                path,
                line: line_count + 1,
            });
        }
        append_registration_record(
            &path,
            &RunRegistrationRecord {
                timestamp,
                registration: registration.clone(),
            },
        )?;
        let (retained, _) = read_registration_file(&path, &key)?;
        Ok(RegisterRunResult {
            disposition: if current.is_some() {
                RegisterRunDisposition::Updated
            } else {
                RegisterRunDisposition::Created
            },
            registration: retained.registration,
        })
    }

    /// Lists the latest complete registration for every known run in stable key order.
    ///
    /// # Errors
    ///
    /// Returns an error when the registration directory or any record cannot be read exactly.
    pub fn run_registrations(&self) -> Result<Vec<RunRegistration>, HoldStoreError> {
        let directory = self.registration_directory();
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut keys = Vec::new();
        for entry in fs::read_dir(&directory).map_err(|source| HoldStoreError::ReadDirectory {
            path: directory.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| HoldStoreError::ReadDirectory {
                path: directory.clone(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| HoldStoreError::InspectPath {
                    path: entry.path(),
                    source,
                })?
                .is_file()
            {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(HoldStoreError::InvalidRegistrationFileName { path });
            };
            keys.push(
                RunRegistrationKey::parse(stem.to_owned())
                    .map_err(|source| HoldStoreError::InvalidRegistrationKey { source })?,
            );
        }
        keys.sort();
        keys.into_iter()
            .map(|key| {
                let path = self.registration_path(&key);
                let file =
                    File::open(&path).map_err(|source| HoldStoreError::OpenRegistration {
                        path: path.clone(),
                        source,
                    })?;
                let _lock = AdvisoryLock::acquire(file, path.clone(), advisory_lock::SHARED)?;
                read_registration_file(&path, &key).map(|(record, _)| record.registration)
            })
            .collect()
    }

    /// Appends one admitted routing rule without replacing the installed rulebook.
    ///
    /// # Errors
    ///
    /// Returns an error when the rulebook cannot be initialized, locked, read, or appended, or
    /// when another rule already carries the proposed identifier.
    pub(crate) fn append_routing_rule(&self, rule: &RoutingRule) -> Result<(), HoldStoreError> {
        let _ = self.routing_rules()?;
        let path = self.root.join("rulebook/routing-rules.jsonl");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| HoldStoreError::OpenRulebook {
                path: path.clone(),
                source,
            })?;
        let _lock = AdvisoryLock::acquire(file, path.clone(), advisory_lock::EXCLUSIVE)?;
        let retained = read_rulebook(&path)?;
        if retained.iter().any(|existing| existing.id() == rule.id()) {
            return Err(HoldStoreError::DuplicateRoutingRule {
                id: rule.id().to_owned(),
            });
        }
        append_rulebook_record(&path, rule)
    }

    /// Opens a hold once for its derived identity.
    ///
    /// Repeated calls return the existing hold without appending another record.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be created, locked, read, or durably appended,
    /// or when an existing file is malformed or belongs to a different identity.
    pub fn open(
        &self,
        identity: HoldIdentity,
        report: Value,
        timestamp: EventTimestamp,
    ) -> Result<OpenHoldResult, HoldStoreError> {
        self.ensure_root()?;
        let key = identity.key();
        let lock = self.lock(&key, HoldFileCreation::Create)?;
        if !lock.is_empty()? {
            let hold = self.read_unlocked(&key)?;
            if hold.identity != identity {
                return Err(HoldStoreError::KeyIdentityMismatch { key: key.0 });
            }
            return Ok(OpenHoldResult {
                disposition: OpenDisposition::AlreadyExists,
                hold,
            });
        }
        let record = HoldRecord::Opened {
            timestamp,
            identity,
            report,
        };
        self.append_unlocked(&key, &record)?;
        let hold = self.read_unlocked(&key)?;
        Ok(OpenHoldResult {
            disposition: OpenDisposition::Created,
            hold,
        })
    }

    /// Reads one complete hold by key.
    ///
    /// # Errors
    ///
    /// Returns an error when the file is absent, unreadable, malformed, or internally inconsistent.
    pub fn read(&self, key: &HoldKey) -> Result<Hold, HoldStoreError> {
        let _lock = self.lock_shared(key)?;
        self.read_unlocked(key)
    }

    /// Reads all hold files in stable key order.
    ///
    /// # Errors
    ///
    /// Returns an error when the root or any hold file cannot be read exactly.
    pub fn list(&self) -> Result<Vec<Hold>, HoldStoreError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut keys = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|source| HoldStoreError::ReadDirectory {
            path: self.root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| HoldStoreError::ReadDirectory {
                path: self.root.clone(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| HoldStoreError::InspectPath {
                    path: entry.path(),
                    source,
                })?
                .is_file()
            {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(HoldStoreError::InvalidHoldFileName { path });
            };
            keys.push(HoldKey::parse(stem.to_owned())?);
        }
        keys.sort();
        keys.into_iter().map(|key| self.read(&key)).collect()
    }

    /// Appends an answer to an open hold.
    ///
    /// # Errors
    ///
    /// Returns an error for empty fields, closed holds, or filesystem/record failures.
    pub fn answer(
        &self,
        key: &HoldKey,
        by: String,
        answer: String,
        timestamp: EventTimestamp,
    ) -> Result<Hold, HoldStoreError> {
        if by.trim().is_empty() {
            return Err(HoldStoreError::EmptyRecordField { field: "by" });
        }
        if answer.trim().is_empty() {
            return Err(HoldStoreError::EmptyRecordField { field: "answer" });
        }
        self.append_to_open(
            key,
            HoldRecord::Answered {
                timestamp,
                by,
                answer,
            },
        )
    }

    /// Routes an open hold to one named party.
    ///
    /// # Errors
    ///
    /// Returns an error for closed holds or filesystem/record failures.
    pub fn route(
        &self,
        key: &HoldKey,
        route: HoldRoute,
        timestamp: EventTimestamp,
    ) -> Result<Hold, HoldStoreError> {
        self.append_to_open(key, HoldRecord::Routed { timestamp, route })
    }

    pub(crate) fn refuse_route(
        &self,
        key: &HoldKey,
        requested: HoldRoute,
        enforced: HoldRoute,
        reason: String,
        timestamp: EventTimestamp,
    ) -> Result<Hold, HoldStoreError> {
        if reason.trim().is_empty() {
            return Err(HoldStoreError::EmptyRecordField { field: "reason" });
        }
        self.append_to_open(
            key,
            HoldRecord::RouteRefused {
                timestamp,
                requested,
                enforced,
                reason,
            },
        )
    }

    pub(crate) fn return_for_options(
        &self,
        key: &HoldKey,
        request: String,
        timestamp: EventTimestamp,
    ) -> Result<Hold, HoldStoreError> {
        if request.trim().is_empty() {
            return Err(HoldStoreError::EmptyRecordField { field: "request" });
        }
        self.append_to_open(key, HoldRecord::ReportingRunRequest { timestamp, request })
    }

    /// Closes an open hold with a retained reason.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty reason, an already closed hold, or filesystem/record failures.
    pub fn close(
        &self,
        key: &HoldKey,
        reason: String,
        timestamp: EventTimestamp,
    ) -> Result<Hold, HoldStoreError> {
        if reason.trim().is_empty() {
            return Err(HoldStoreError::EmptyRecordField { field: "reason" });
        }
        self.append_to_open(key, HoldRecord::Closed { timestamp, reason })
    }

    fn append_to_open(&self, key: &HoldKey, record: HoldRecord) -> Result<Hold, HoldStoreError> {
        self.ensure_root()?;
        let _lock = self.lock(key, HoldFileCreation::Existing)?;
        let current = self.read_unlocked(key)?;
        if !current.is_open() {
            return Err(HoldStoreError::HoldClosed { key: key.0.clone() });
        }
        self.append_unlocked(key, &record)?;
        self.read_unlocked(key)
    }

    fn ensure_root(&self) -> Result<(), HoldStoreError> {
        fs::create_dir_all(&self.root).map_err(|source| HoldStoreError::CreateDirectory {
            path: self.root.clone(),
            source,
        })
    }

    fn registration_directory(&self) -> PathBuf {
        self.root.join("runs")
    }

    fn registration_path(&self, key: &RunRegistrationKey) -> PathBuf {
        self.registration_directory()
            .join(format!("{}.jsonl", key.as_str()))
    }

    fn hold_path(&self, key: &HoldKey) -> PathBuf {
        self.root.join(format!("{}.jsonl", key.as_str()))
    }
    fn lock(
        &self,
        key: &HoldKey,
        creation: HoldFileCreation,
    ) -> Result<AdvisoryLock, HoldStoreError> {
        let path = self.hold_path(key);
        let file = OpenOptions::new()
            .create(matches!(creation, HoldFileCreation::Create))
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| HoldStoreError::OpenLock {
                path: path.clone(),
                source,
            })?;
        AdvisoryLock::acquire(file, path, advisory_lock::EXCLUSIVE)
    }

    fn lock_shared(&self, key: &HoldKey) -> Result<AdvisoryLock, HoldStoreError> {
        let path = self.hold_path(key);
        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    HoldStoreError::HoldNotFound { key: key.0.clone() }
                } else {
                    HoldStoreError::OpenLock {
                        path: path.clone(),
                        source,
                    }
                }
            })?;
        AdvisoryLock::acquire(file, path, advisory_lock::SHARED)
    }

    fn append_unlocked(&self, key: &HoldKey, record: &HoldRecord) -> Result<(), HoldStoreError> {
        let path = self.hold_path(key);
        if path.exists() {
            let bytes = fs::read(&path).map_err(|source| HoldStoreError::ReadFile {
                path: path.clone(),
                source,
            })?;
            if !bytes.is_empty() && !bytes.ends_with(b"\n") {
                return Err(HoldStoreError::IncompleteTail { path });
            }
        }
        let mut line = serde_json::to_vec(record)
            .map_err(|source| HoldStoreError::SerializeRecord { source })?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| HoldStoreError::OpenFile {
                path: path.clone(),
                source,
            })?;
        file.write_all(&line)
            .map_err(|source| HoldStoreError::AppendFile {
                path: path.clone(),
                source,
            })?;
        file.sync_data()
            .map_err(|source| HoldStoreError::SyncFile { path, source })
    }

    fn read_unlocked(&self, key: &HoldKey) -> Result<Hold, HoldStoreError> {
        let path = self.hold_path(key);
        let mut file = File::open(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                HoldStoreError::HoldNotFound { key: key.0.clone() }
            } else {
                HoldStoreError::OpenFile {
                    path: path.clone(),
                    source,
                }
            }
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|source| HoldStoreError::ReadFile {
                path: path.clone(),
                source,
            })?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(HoldStoreError::IncompleteTail { path });
        }
        let mut records = Vec::new();
        for (index, line) in BufReader::new(bytes.as_slice()).lines().enumerate() {
            let line = line.map_err(|source| HoldStoreError::ReadFile {
                path: path.clone(),
                source,
            })?;
            if line.is_empty() {
                return Err(HoldStoreError::MalformedRecord {
                    path: path.clone(),
                    line: index + 1,
                    detail: "empty JSONL line".to_owned(),
                });
            }
            let record = serde_json::from_str::<HoldRecord>(&line).map_err(|source| {
                HoldStoreError::MalformedRecord {
                    path: path.clone(),
                    line: index + 1,
                    detail: source.to_string(),
                }
            })?;
            records.push(record);
        }
        derive_hold(key.clone(), records, path)
    }
}

fn read_rulebook(path: &Path) -> Result<Vec<RoutingRule>, HoldStoreError> {
    let bytes = fs::read(path).map_err(|source| HoldStoreError::ReadRulebook {
        path: path.to_path_buf(),
        source,
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(HoldStoreError::IncompleteRulebookTail {
            path: path.to_path_buf(),
        });
    }
    parse_rulebook_records(path, &bytes)
}

fn parse_rulebook_records(path: &Path, bytes: &[u8]) -> Result<Vec<RoutingRule>, HoldStoreError> {
    BufReader::new(bytes)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.map_err(|source| HoldStoreError::ReadRulebook {
                path: path.to_path_buf(),
                source,
            })?;
            serde_json::from_str(&line).map_err(|source| HoldStoreError::MalformedRulebookRecord {
                path: path.to_path_buf(),
                line: index + 1,
                detail: source.to_string(),
            })
        })
        .collect()
}

fn verify_starting_rulebook_prefix(
    path: &Path,
    retained: &[RoutingRule],
    starting: &[RoutingRule],
) -> Result<(), HoldStoreError> {
    for (index, existing) in retained.iter().take(starting.len()).enumerate() {
        if existing != &starting[index] {
            return Err(HoldStoreError::RulebookPrefixMismatch {
                path: path.to_path_buf(),
                line: index + 1,
            });
        }
    }
    Ok(())
}

fn recover_incomplete_rulebook_tail(
    path: &Path,
    starting: &[RoutingRule],
) -> Result<Vec<RoutingRule>, HoldStoreError> {
    let bytes = fs::read(path).map_err(|source| HoldStoreError::ReadRulebook {
        path: path.to_path_buf(),
        source,
    })?;
    let complete_length = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let retained = parse_rulebook_records(path, &bytes[..complete_length])?;
    verify_starting_rulebook_prefix(path, &retained, starting)?;
    let Some(next_rule) = starting.get(retained.len()) else {
        return Err(HoldStoreError::IncompleteRulebookTail {
            path: path.to_path_buf(),
        });
    };
    let encoded_next = serde_json::to_vec(next_rule)
        .map_err(|source| HoldStoreError::SerializeRulebookRecord { source })?;
    if !encoded_next.starts_with(&bytes[complete_length..]) {
        return Err(HoldStoreError::IncompleteRulebookTail {
            path: path.to_path_buf(),
        });
    }

    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|source| HoldStoreError::OpenRulebook {
            path: path.to_path_buf(),
            source,
        })?;
    file.set_len(complete_length as u64).map_err(|source| {
        HoldStoreError::TruncateIncompleteRulebookTail {
            path: path.to_path_buf(),
            source,
        }
    })?;
    file.sync_data()
        .map_err(|source| HoldStoreError::SyncRulebook {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(retained)
}

fn append_rulebook_record(path: &Path, rule: &RoutingRule) -> Result<(), HoldStoreError> {
    let mut line = serde_json::to_vec(rule)
        .map_err(|source| HoldStoreError::SerializeRulebookRecord { source })?;
    line.push(b'\n');
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| HoldStoreError::OpenRulebook {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(&line)
        .map_err(|source| HoldStoreError::AppendRulebook {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_data()
        .map_err(|source| HoldStoreError::SyncRulebook {
            path: path.to_path_buf(),
            source,
        })
}

fn read_registration_file(
    path: &Path,
    key: &RunRegistrationKey,
) -> Result<(RunRegistrationRecord, usize), HoldStoreError> {
    let bytes = fs::read(path).map_err(|source| HoldStoreError::ReadRegistration {
        path: path.to_path_buf(),
        source,
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(HoldStoreError::IncompleteRegistrationTail {
            path: path.to_path_buf(),
        });
    }
    let mut latest = None;
    let mut previous_timestamp = None;
    let mut line_count = 0;
    for (index, line) in BufReader::new(bytes.as_slice()).lines().enumerate() {
        line_count = index + 1;
        let line = line.map_err(|source| HoldStoreError::ReadRegistration {
            path: path.to_path_buf(),
            source,
        })?;
        if line.is_empty() {
            return Err(HoldStoreError::MalformedRegistrationRecord {
                path: path.to_path_buf(),
                line: index + 1,
                detail: "empty JSONL line".to_owned(),
            });
        }
        let record = serde_json::from_str::<RunRegistrationRecord>(&line).map_err(|source| {
            HoldStoreError::MalformedRegistrationRecord {
                path: path.to_path_buf(),
                line: index + 1,
                detail: source.to_string(),
            }
        })?;
        if record.registration.key() != *key {
            return Err(HoldStoreError::RegistrationKeyMismatch {
                key: key.as_str().to_owned(),
            });
        }
        if previous_timestamp.is_some_and(|previous: EventTimestamp| {
            record.timestamp.as_datetime() < previous.as_datetime()
        }) {
            return Err(HoldStoreError::RegistrationTimestampWentBackwards {
                path: path.to_path_buf(),
                line: index + 1,
            });
        }
        previous_timestamp = Some(record.timestamp);
        latest = Some(record);
    }
    latest.map(|record| (record, line_count)).ok_or_else(|| {
        HoldStoreError::MissingRegistrationRecord {
            path: path.to_path_buf(),
        }
    })
}

fn append_registration_record(
    path: &Path,
    record: &RunRegistrationRecord,
) -> Result<(), HoldStoreError> {
    let bytes = fs::read(path).map_err(|source| HoldStoreError::ReadRegistration {
        path: path.to_path_buf(),
        source,
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(HoldStoreError::IncompleteRegistrationTail {
            path: path.to_path_buf(),
        });
    }
    let mut line = serde_json::to_vec(record)
        .map_err(|source| HoldStoreError::SerializeRegistrationRecord { source })?;
    line.push(b'\n');
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| HoldStoreError::OpenRegistration {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(&line)
        .map_err(|source| HoldStoreError::AppendRegistration {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_data()
        .map_err(|source| HoldStoreError::SyncRegistration {
            path: path.to_path_buf(),
            source,
        })
}

fn derive_hold(
    key: HoldKey,
    records: Vec<HoldRecord>,
    path: PathBuf,
) -> Result<Hold, HoldStoreError> {
    let Some(HoldRecord::Opened {
        timestamp: opened_at,
        identity,
        report,
    }) = records.first()
    else {
        return Err(HoldStoreError::MissingOpeningRecord { path });
    };
    if identity.key() != key {
        return Err(HoldStoreError::KeyIdentityMismatch { key: key.0 });
    }
    let mut state = HoldState::Open {
        route: HoldRoute::Overseer,
    };
    let mut updated_at = *opened_at;
    for (index, record) in records.iter().enumerate() {
        if index > 0 && matches!(record, HoldRecord::Opened { .. }) {
            return Err(HoldStoreError::DuplicateOpeningRecord { path });
        }
        if matches!(state, HoldState::Closed) && index > 0 {
            return Err(HoldStoreError::RecordAfterClose {
                path,
                line: index + 1,
            });
        }
        match record {
            HoldRecord::Opened { .. } | HoldRecord::Answered { .. } => {}
            HoldRecord::Routed { route, .. } => state = HoldState::Open { route: *route },
            HoldRecord::RouteRefused { enforced, .. } => {
                state = HoldState::Open { route: *enforced };
            }
            HoldRecord::ReportingRunRequest { .. } => {
                state = HoldState::Open {
                    route: HoldRoute::ReportingRun,
                };
            }
            HoldRecord::Closed { .. } => state = HoldState::Closed,
        }
        if record.timestamp().as_datetime() < updated_at.as_datetime() {
            return Err(HoldStoreError::TimestampWentBackwards {
                path,
                line: index + 1,
            });
        }
        updated_at = record.timestamp();
    }
    Ok(Hold {
        key,
        identity: identity.clone(),
        report: report.clone(),
        state,
        opened_at: *opened_at,
        updated_at,
        records,
    })
}

#[cfg(unix)]
mod advisory_lock {
    use std::ffi::c_int;

    pub const SHARED: c_int = 1;
    pub const EXCLUSIVE: c_int = 2;
    pub const UNLOCK: c_int = 8;

    unsafe extern "C" {
        pub fn flock(file_descriptor: c_int, operation: c_int) -> c_int;
    }
}

#[derive(Clone, Copy)]
enum HoldFileCreation {
    Create,
    Existing,
}

struct AdvisoryLock {
    file: File,
    path: PathBuf,
}

impl AdvisoryLock {
    fn is_empty(&self) -> Result<bool, HoldStoreError> {
        self.file
            .metadata()
            .map(|metadata| metadata.len() == 0)
            .map_err(|source| HoldStoreError::InspectPath {
                path: self.path.clone(),
                source,
            })
    }

    #[cfg(unix)]
    fn acquire(file: File, path: PathBuf, operation: c_int) -> Result<Self, HoldStoreError> {
        use std::os::fd::AsRawFd;
        let result = unsafe { advisory_lock::flock(file.as_raw_fd(), operation) };
        if result == -1 {
            return Err(HoldStoreError::AcquireLock {
                path,
                source: std::io::Error::last_os_error(),
            });
        }
        Ok(Self { file, path })
    }

    #[cfg(not(unix))]
    fn acquire(_file: File, path: PathBuf, _operation: c_int) -> Result<Self, HoldStoreError> {
        Err(HoldStoreError::LockUnsupported { path })
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { advisory_lock::flock(self.file.as_raw_fd(), advisory_lock::UNLOCK) } == -1 {
                tracing::error!(path = %self.path.display(), error = ?std::io::Error::last_os_error(), "failed to release hold lock");
            }
        }
    }
}

/// A hold store operation failed without discarding or rewriting retained state.
#[derive(Debug, Error)]
pub enum HoldStoreError {
    /// An identity component is empty.
    #[error("hold identity field `{field}` must not be empty")]
    EmptyIdentityField { field: &'static str },
    /// A record's required string field is empty.
    #[error("hold record field `{field}` must not be empty")]
    EmptyRecordField { field: &'static str },
    /// A supplied key is not a lowercase SHA-256 digest.
    #[error("invalid hold key `{value}`; expected 64 lowercase hexadecimal digits")]
    InvalidKey { value: String },
    /// A supplied route is not one of the domain routes.
    #[error("invalid hold route `{value}`; expected human, overseer, or reporting-run")]
    InvalidRoute { value: String },
    /// The store root could not be created.
    #[error("failed to create hold store directory `{path}`")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The store root could not be enumerated.
    #[error("failed to read hold store directory `{path}`")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A directory entry could not be inspected.
    #[error("failed to inspect hold store path `{path}`")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A JSONL-looking entry does not carry a valid hold key.
    #[error("hold file name `{path}` is not a derived key")]
    InvalidHoldFileName { path: PathBuf },
    /// A per-hold advisory lock file could not be opened.
    #[error("failed to open hold lock `{path}`")]
    OpenLock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The operating system refused the per-hold advisory lock.
    #[error("failed to acquire hold lock `{path}`")]
    AcquireLock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Advisory file locking is unavailable on this platform.
    #[error("advisory locking is unsupported for hold lock `{path}`")]
    LockUnsupported { path: PathBuf },
    /// A hold file could not be opened.
    #[error("failed to open hold file `{path}`")]
    OpenFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A hold file could not be read.
    #[error("failed to read hold file `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A hold record could not be encoded.
    #[error("failed to serialize hold record")]
    SerializeRecord {
        #[source]
        source: serde_json::Error,
    },
    /// A complete encoded record could not be appended.
    #[error("failed to append hold file `{path}`")]
    AppendFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// An appended record could not be synced.
    #[error("failed to sync hold file `{path}`")]
    SyncFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Existing bytes end in an incomplete JSONL record.
    #[error("hold file `{path}` has an incomplete final record")]
    IncompleteTail { path: PathBuf },
    /// One retained line is not a hold record.
    #[error("malformed hold record at `{path}` line {line}: {detail}")]
    MalformedRecord {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    /// The requested key has no hold file.
    #[error("hold `{key}` does not exist")]
    HoldNotFound { key: String },
    /// A hold file does not begin with its opening record.
    #[error("hold file `{path}` has no opening record")]
    MissingOpeningRecord { path: PathBuf },
    /// A hold file contains a second opening record.
    #[error("hold file `{path}` contains more than one opening record")]
    DuplicateOpeningRecord { path: PathBuf },
    /// The file name and opening identity derive different keys.
    #[error("hold file key `{key}` does not match its opening identity")]
    KeyIdentityMismatch { key: String },
    /// A record follows the terminal close record.
    #[error("hold file `{path}` has a record after close at line {line}")]
    RecordAfterClose { path: PathBuf, line: usize },
    /// Retained event time moved backwards.
    #[error("hold file `{path}` timestamp moves backwards at line {line}")]
    TimestampWentBackwards { path: PathBuf, line: usize },
    /// A mutating verb targeted a closed hold.
    #[error("hold `{key}` is already closed")]
    HoldClosed { key: String },
    /// The rulebook directory could not be created.
    #[error("failed to create rulebook directory `{path}`")]
    CreateRulebookDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The routing rulebook could not be opened for locking or appending.
    #[error("failed to open routing rulebook `{path}`")]
    OpenRulebook {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The routing rulebook could not be read exactly.
    #[error("failed to read routing rulebook `{path}`")]
    ReadRulebook {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A routing rule could not be encoded.
    #[error("failed to serialize routing rule")]
    SerializeRulebookRecord {
        #[source]
        source: serde_json::Error,
    },
    /// A complete routing rule could not be appended.
    #[error("failed to append routing rulebook `{path}`")]
    AppendRulebook {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// An updated routing rulebook could not be synced.
    #[error("failed to sync routing rulebook `{path}`")]
    SyncRulebook {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// An incomplete final rulebook record could not be removed during install recovery.
    #[error("failed to remove incomplete routing rulebook tail from `{path}`")]
    TruncateIncompleteRulebookTail {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Existing rulebook bytes end in an incomplete JSONL record.
    #[error("routing rulebook `{path}` has an incomplete final record")]
    IncompleteRulebookTail { path: PathBuf },
    /// One retained line is not a routing rule.
    #[error("malformed routing rule at `{path}` line {line}: {detail}")]
    MalformedRulebookRecord {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    /// The retained starting records differ from the shipped corpus rules.
    #[error("routing rulebook `{path}` differs from the starting corpus at line {line}")]
    RulebookPrefixMismatch { path: PathBuf, line: usize },
    /// A registration file name does not carry a valid run key.
    #[error("run registration file name `{path}` is not a derived key")]
    InvalidRegistrationFileName { path: PathBuf },
    /// A registration file name contains an invalid run key.
    #[error("invalid run registration key")]
    InvalidRegistrationKey {
        #[source]
        source: RunRegistrationError,
    },
    /// A run-registration stream could not be opened.
    #[error("failed to open run registration `{path}`")]
    OpenRegistration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A run-registration stream could not be read exactly.
    #[error("failed to read run registration `{path}`")]
    ReadRegistration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A run-registration record could not be encoded.
    #[error("failed to serialize run registration record")]
    SerializeRegistrationRecord {
        #[source]
        source: serde_json::Error,
    },
    /// A complete run-registration record could not be appended.
    #[error("failed to append run registration `{path}`")]
    AppendRegistration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// An appended run-registration record could not be synced.
    #[error("failed to sync run registration `{path}`")]
    SyncRegistration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Existing registration bytes end in an incomplete JSONL record.
    #[error("run registration `{path}` has an incomplete final record")]
    IncompleteRegistrationTail { path: PathBuf },
    /// One retained registration line is malformed.
    #[error("malformed run registration at `{path}` line {line}: {detail}")]
    MalformedRegistrationRecord {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    /// A registration file contains no record.
    #[error("run registration `{path}` has no record")]
    MissingRegistrationRecord { path: PathBuf },
    /// A registration stream contains a record for a different run.
    #[error("run registration file key `{key}` does not match its record")]
    RegistrationKeyMismatch { key: String },
    /// Retained registration time moved backwards.
    #[error("run registration `{path}` timestamp moves backwards at line {line}")]
    RegistrationTimestampWentBackwards { path: PathBuf, line: usize },
    /// An admitted rule attempted to reuse an existing stable identifier.
    #[error("routing rule `{id}` already exists")]
    DuplicateRoutingRule { id: String },
}
