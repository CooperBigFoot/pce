//! dispatch_process_identity : IssuanceSequence × ProcessNumber × DarwinStartIdentity × AbsoluteRequiredArtifactPath → DispatchProcessIdentity   (pure, deterministic)
//! This module defines and parses the binary-owned identity sidecar without observing a process or touching a file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::event_log::{EventLogError, Sequence};

const SCHEMA_ID: &str = "pce.dispatch-process-identity";
const SCHEMA_VERSION: u32 = 1;

/// A positive operating-system process number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessNumber(u32);

impl ProcessNumber {
    /// Construct a positive process number.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchProcessIdentityError::ZeroProcessNumber`] when `value` is zero.
    pub fn new(value: u32) -> Result<Self, DispatchProcessIdentityError> {
        if value == 0 {
            return Err(DispatchProcessIdentityError::ZeroProcessNumber { value });
        }
        Ok(Self(value))
    }

    /// Return the numeric process number.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The Darwin kernel process start time at microsecond precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessStartIdentity {
    seconds_since_unix_epoch: u64,
    microseconds: u32,
}

impl ProcessStartIdentity {
    /// Construct a normalized Darwin start identity.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchProcessIdentityError::InvalidStartMicroseconds`] when `microseconds`
    /// is not a valid subsecond component.
    pub fn new(
        seconds_since_unix_epoch: u64,
        microseconds: u32,
    ) -> Result<Self, DispatchProcessIdentityError> {
        if microseconds >= 1_000_000 {
            return Err(DispatchProcessIdentityError::InvalidStartMicroseconds { microseconds });
        }
        Ok(Self {
            seconds_since_unix_epoch,
            microseconds,
        })
    }

    /// Return the whole seconds since the Unix epoch.
    pub const fn seconds_since_unix_epoch(self) -> u64 {
        self.seconds_since_unix_epoch
    }

    /// Return the microsecond subsecond component.
    pub const fn microseconds(self) -> u32 {
        self.microseconds
    }
}

/// A lexically absolute, exactly UTF-8 required artifact path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteRequiredArtifactPath(PathBuf);

impl AbsoluteRequiredArtifactPath {
    /// Parse an absolute path without lossy text conversion.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the path is relative or is not valid UTF-8.
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchProcessIdentityError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchProcessIdentityError::RelativeRequiredArtifactPath { path });
        }
        if path.to_str().is_none() {
            return Err(DispatchProcessIdentityError::NonUtf8RequiredArtifactPath);
        }
        Ok(Self(path))
    }

    /// Borrow the exact absolute path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    fn as_str(&self) -> Result<&str, DispatchProcessIdentityError> {
        self.0
            .to_str()
            .ok_or(DispatchProcessIdentityError::NonUtf8RequiredArtifactPath)
    }
}

/// One complete binary-owned dispatch identity sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchProcessIdentity {
    schema_id: String,
    schema_version: u32,
    issuance_sequence: Sequence,
    process_number: ProcessNumber,
    process_start_identity: ProcessStartIdentity,
    required_artifact_path: AbsoluteRequiredArtifactPath,
}

impl DispatchProcessIdentity {
    /// Author a version-one dispatch identity.
    pub fn new(
        issuance_sequence: Sequence,
        process_number: ProcessNumber,
        process_start_identity: ProcessStartIdentity,
        required_artifact_path: AbsoluteRequiredArtifactPath,
    ) -> Self {
        Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION,
            issuance_sequence,
            process_number,
            process_start_identity,
            required_artifact_path,
        }
    }

    /// Return the schema identifier.
    pub fn schema_id(&self) -> &str {
        &self.schema_id
    }

    /// Return the schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Return the issuance sequence named by this identity.
    pub const fn issuance_sequence(&self) -> Sequence {
        self.issuance_sequence
    }

    /// Return the operating-system process number.
    pub const fn process_number(&self) -> ProcessNumber {
        self.process_number
    }

    /// Return the Darwin process start identity.
    pub const fn process_start_identity(&self) -> ProcessStartIdentity {
        self.process_start_identity
    }

    /// Borrow the required artifact path.
    pub fn required_artifact_path(&self) -> &AbsoluteRequiredArtifactPath {
        &self.required_artifact_path
    }
}

/// The issuance and artifact authority a sidecar reader expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchProcessIdentityExpectation {
    issuance_sequence: Sequence,
    required_artifact_path: AbsoluteRequiredArtifactPath,
}

impl DispatchProcessIdentityExpectation {
    /// Construct an exact sidecar expectation.
    pub fn new(
        issuance_sequence: Sequence,
        required_artifact_path: AbsoluteRequiredArtifactPath,
    ) -> Self {
        Self {
            issuance_sequence,
            required_artifact_path,
        }
    }
}

#[derive(Serialize)]
struct AuthoredIdentity<'a> {
    schema_id: &'a str,
    schema_version: u32,
    issuance_sequence: u64,
    process_number: u32,
    process_start_identity: AuthoredStartIdentity,
    required_artifact_path: &'a str,
}

#[derive(Serialize)]
struct AuthoredStartIdentity {
    seconds_since_unix_epoch: u64,
    microseconds: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdentity {
    schema_id: String,
    schema_version: u32,
    issuance_sequence: u64,
    process_number: u32,
    process_start_identity: RawStartIdentity,
    required_artifact_path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStartIdentity {
    seconds_since_unix_epoch: u64,
    microseconds: u32,
}

/// Serialize one canonical compact identity document followed by exactly one LF.
///
/// # Errors
///
/// Returns [`DispatchProcessIdentityError::Serialization`] if JSON serialization fails.
pub fn serialize_dispatch_process_identity(
    identity: &DispatchProcessIdentity,
) -> Result<Vec<u8>, DispatchProcessIdentityError> {
    let start = identity.process_start_identity;
    let authored = AuthoredIdentity {
        schema_id: &identity.schema_id,
        schema_version: identity.schema_version,
        issuance_sequence: identity.issuance_sequence.get(),
        process_number: identity.process_number.get(),
        process_start_identity: AuthoredStartIdentity {
            seconds_since_unix_epoch: start.seconds_since_unix_epoch(),
            microseconds: start.microseconds(),
        },
        required_artifact_path: identity.required_artifact_path.as_str()?,
    };
    let mut bytes = serde_json::to_vec(&authored)
        .map_err(|source| DispatchProcessIdentityError::Serialization { source })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Parse exactly one complete strict identity document and reconstruct its domain values.
///
/// # Errors
///
/// Returns a typed schema, JSON, sequence, process-number, start-time, or artifact-path error.
pub fn parse_dispatch_process_identity(
    bytes: &[u8],
) -> Result<DispatchProcessIdentity, DispatchProcessIdentityError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let raw = RawIdentity::deserialize(&mut deserializer)
        .map_err(|source| DispatchProcessIdentityError::InvalidJson { source })?;
    deserializer
        .end()
        .map_err(|source| DispatchProcessIdentityError::InvalidJson { source })?;
    if raw.schema_id != SCHEMA_ID {
        return Err(DispatchProcessIdentityError::UnexpectedSchemaId {
            found: raw.schema_id,
        });
    }
    if raw.schema_version != SCHEMA_VERSION {
        return Err(DispatchProcessIdentityError::UnexpectedSchemaVersion {
            found: raw.schema_version,
        });
    }
    let issuance_sequence = Sequence::parse(raw.issuance_sequence)
        .map_err(|source| DispatchProcessIdentityError::InvalidIssuanceSequence { source })?;
    let process_number = ProcessNumber::new(raw.process_number)?;
    let process_start_identity = ProcessStartIdentity::new(
        raw.process_start_identity.seconds_since_unix_epoch,
        raw.process_start_identity.microseconds,
    )?;
    let required_artifact_path = AbsoluteRequiredArtifactPath::parse(raw.required_artifact_path)?;
    Ok(DispatchProcessIdentity {
        schema_id: raw.schema_id,
        schema_version: raw.schema_version,
        issuance_sequence,
        process_number,
        process_start_identity,
        required_artifact_path,
    })
}

/// Require a sidecar to name the expected issuance and required artifact.
///
/// # Errors
///
/// Returns distinct typed errors for an issuance mismatch and an artifact-path mismatch.
pub fn require_dispatch_process_identity_match(
    identity: &DispatchProcessIdentity,
    expected: &DispatchProcessIdentityExpectation,
) -> Result<(), DispatchProcessIdentityError> {
    if identity.issuance_sequence != expected.issuance_sequence {
        return Err(DispatchProcessIdentityError::IssuanceSequenceMismatch {
            found: identity.issuance_sequence.get(),
            expected: expected.issuance_sequence.get(),
        });
    }
    if identity.required_artifact_path.as_path().as_os_str()
        != expected.required_artifact_path.as_path().as_os_str()
    {
        return Err(DispatchProcessIdentityError::ArtifactPathMismatch {
            found: identity.required_artifact_path.as_path().to_path_buf(),
            expected: expected.required_artifact_path.as_path().to_path_buf(),
        });
    }
    Ok(())
}

/// A dispatch process identity could not be constructed, encoded, parsed, or matched.
#[derive(Debug, Error)]
pub enum DispatchProcessIdentityError {
    /// A process-number construction was attempted with zero.
    #[error("process number must be positive; found {value}")]
    ZeroProcessNumber { value: u32 },
    /// A start identity carried a microsecond component outside its normalized range.
    #[error("process start microseconds must be less than 1000000; found {microseconds}")]
    InvalidStartMicroseconds { microseconds: u32 },
    /// A required artifact path was not lexically absolute.
    #[error("required artifact path must be absolute: {}", path.display())]
    RelativeRequiredArtifactPath { path: PathBuf },
    /// A required artifact path could not be represented exactly as UTF-8.
    #[error("required artifact path is not valid UTF-8")]
    NonUtf8RequiredArtifactPath,
    /// Canonical identity serialization failed.
    #[error("failed to serialize dispatch process identity")]
    Serialization { source: serde_json::Error },
    /// The bytes were not exactly one strict complete JSON document.
    #[error("invalid dispatch process identity JSON")]
    InvalidJson { source: serde_json::Error },
    /// The document named an unsupported schema identifier.
    #[error(
        "dispatch process identity has schema id `{found}`; expected `pce.dispatch-process-identity`"
    )]
    UnexpectedSchemaId { found: String },
    /// The document named an unsupported schema version.
    #[error("dispatch process identity has schema version {found}; expected 1")]
    UnexpectedSchemaVersion { found: u32 },
    /// The document carried an invalid first-based issuance sequence.
    #[error("dispatch process identity has invalid issuance sequence")]
    InvalidIssuanceSequence { source: EventLogError },
    /// A valid sidecar named a different issuance sequence than its reader expected.
    #[error(
        "dispatch process identity issuance sequence {found} does not match expected sequence {expected}"
    )]
    IssuanceSequenceMismatch { found: u64, expected: u64 },
    /// A valid sidecar named a different required artifact than its reader expected.
    #[error("dispatch process identity artifact path `{}` does not match expected path `{}`", found.display(), expected.display())]
    ArtifactPathMismatch { found: PathBuf, expected: PathBuf },
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStringExt;

    use super::{
        AbsoluteRequiredArtifactPath, DispatchProcessIdentity, DispatchProcessIdentityError,
        DispatchProcessIdentityExpectation, ProcessNumber, ProcessStartIdentity,
        parse_dispatch_process_identity, require_dispatch_process_identity_match,
        serialize_dispatch_process_identity,
    };
    use crate::event_log::Sequence;

    const CANONICAL: &[u8] = b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n";

    #[test]
    fn canonical_dispatch_process_identity_bytes_round_trip() {
        let identity = DispatchProcessIdentity::new(
            Sequence::parse(42).expect("sequence"),
            ProcessNumber::new(4242).expect("process number"),
            ProcessStartIdentity::new(1_723_200_000, 123_456).expect("start identity"),
            AbsoluteRequiredArtifactPath::parse("/tmp/pce/result.json").expect("artifact path"),
        );
        assert_eq!(
            serialize_dispatch_process_identity(&identity).expect("serialize"),
            CANONICAL
        );
        let parsed = parse_dispatch_process_identity(CANONICAL).expect("parse");
        assert_eq!(parsed.schema_id(), "pce.dispatch-process-identity");
        assert_eq!(parsed.schema_version(), 1);
        assert_eq!(parsed.issuance_sequence().get(), 42);
        assert_eq!(parsed.process_number().get(), 4242);
        assert_eq!(
            parsed.process_start_identity().seconds_since_unix_epoch(),
            1_723_200_000
        );
        assert_eq!(parsed.process_start_identity().microseconds(), 123_456);
        assert_eq!(
            parsed.required_artifact_path().as_path(),
            std::path::Path::new("/tmp/pce/result.json")
        );
    }

    #[test]
    fn dispatch_process_identity_rejects_noncanonical_documents() {
        let fixtures: &[(&[u8], fn(&DispatchProcessIdentityError) -> bool)] = &[
            (b"{\"schema_id\":\"wrong\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::UnexpectedSchemaId { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":2,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::UnexpectedSchemaVersion { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":0,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::ZeroProcessNumber { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":1000000},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::InvalidStartMicroseconds { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456,\"ticks\":1},\"required_artifact_path\":\"/tmp/pce/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::InvalidJson { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\",\"extra\":true}\n", |error| matches!(error, DispatchProcessIdentityError::InvalidJson { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"relative/result.json\"}\n", |error| matches!(error, DispatchProcessIdentityError::RelativeRequiredArtifactPath { .. })),
            (b"{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":42,\"process_number\":4242,\"process_start_identity\":{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456},\"required_artifact_path\":\"/tmp/pce/result.json\"}{\"second\":true}\n", |error| matches!(error, DispatchProcessIdentityError::InvalidJson { .. })),
        ];
        for (bytes, expected) in fixtures {
            let error = parse_dispatch_process_identity(bytes).expect_err("fixture must fail");
            assert!(expected(&error), "unexpected error: {error:?}");
        }
    }

    #[test]
    fn dispatch_process_identity_rejects_expected_sequence_or_artifact_mismatch() {
        let identity = parse_dispatch_process_identity(CANONICAL).expect("parse canonical");
        let wrong_sequence = DispatchProcessIdentityExpectation::new(
            Sequence::parse(43).expect("sequence"),
            AbsoluteRequiredArtifactPath::parse("/tmp/pce/result.json").expect("artifact"),
        );
        let error = require_dispatch_process_identity_match(&identity, &wrong_sequence)
            .expect_err("sequence mismatch");
        assert_eq!(
            error.to_string(),
            "dispatch process identity issuance sequence 42 does not match expected sequence 43"
        );

        let wrong_artifact = DispatchProcessIdentityExpectation::new(
            Sequence::parse(42).expect("sequence"),
            AbsoluteRequiredArtifactPath::parse("/tmp/pce/other.json").expect("artifact"),
        );
        let error = require_dispatch_process_identity_match(&identity, &wrong_artifact)
            .expect_err("artifact mismatch");
        assert_eq!(
            error.to_string(),
            "dispatch process identity artifact path `/tmp/pce/result.json` does not match expected path `/tmp/pce/other.json`"
        );
    }

    #[test]
    fn invariant_construction_diagnostics_are_exact() {
        assert_eq!(
            ProcessNumber::new(0).expect_err("zero PID").to_string(),
            "process number must be positive; found 0"
        );
        assert_eq!(
            ProcessStartIdentity::new(1, 1_000_000)
                .expect_err("invalid microseconds")
                .to_string(),
            "process start microseconds must be less than 1000000; found 1000000"
        );
        assert_eq!(
            AbsoluteRequiredArtifactPath::parse("relative/result.json")
                .expect_err("relative path")
                .to_string(),
            "required artifact path must be absolute: relative/result.json"
        );
        let non_utf8 = std::ffi::OsString::from_vec(vec![b'/', 0xff]);
        assert_eq!(
            AbsoluteRequiredArtifactPath::parse(std::path::PathBuf::from(non_utf8))
                .expect_err("non-UTF-8 path")
                .to_string(),
            "required artifact path is not valid UTF-8"
        );
    }
}
