//! run registration = latest(register, repository × vision directory)
//!
//! A registration names the durable files and Herdr session of one work-graph run. Its identity is
//! stable across supervisor restarts and frozen-plan advances.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The stable key of a work-graph run.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RunRegistrationKey(String);

impl RunRegistrationKey {
    /// Parses a lowercase SHA-256 registration key.
    ///
    /// # Errors
    ///
    /// Returns [`RunRegistrationError::InvalidKey`] unless the value has the expected form.
    pub fn parse(value: impl Into<String>) -> Result<Self, RunRegistrationError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RunRegistrationError::InvalidKey { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The facts needed to locate and supervise one work-graph run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunRegistration {
    repository: String,
    vision_directory: PathBuf,
    frozen_graph: PathBuf,
    journal: PathBuf,
    herdr_session: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunRegistration {
    repository: String,
    vision_directory: PathBuf,
    frozen_graph: PathBuf,
    journal: PathBuf,
    herdr_session: Option<String>,
}

impl<'de> Deserialize<'de> for RunRegistration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRunRegistration::deserialize(deserializer)?;
        Self::parse(
            raw.repository,
            raw.vision_directory,
            raw.frozen_graph,
            raw.journal,
            raw.herdr_session,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl RunRegistration {
    /// Parses one run registration at the CLI boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty repository or Herdr session, a relative path, or a graph or
    /// journal path outside the vision directory.
    pub fn parse(
        repository: impl Into<String>,
        vision_directory: impl Into<PathBuf>,
        frozen_graph: impl Into<PathBuf>,
        journal: impl Into<PathBuf>,
        herdr_session: Option<String>,
    ) -> Result<Self, RunRegistrationError> {
        let registration = Self {
            repository: repository.into(),
            vision_directory: vision_directory.into(),
            frozen_graph: frozen_graph.into(),
            journal: journal.into(),
            herdr_session,
        };
        if registration.repository.trim().is_empty() {
            return Err(RunRegistrationError::EmptyField {
                field: "repository",
            });
        }
        if let Some(session) = &registration.herdr_session {
            if session.trim().is_empty() {
                return Err(RunRegistrationError::EmptyField {
                    field: "herdr_session",
                });
            }
            if session.len() > 64
                || matches!(session.as_str(), "." | "..")
                || !session
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err(RunRegistrationError::InvalidHerdrSession {
                    value: session.clone(),
                });
            }
        }
        for (field, path) in [
            ("vision_directory", registration.vision_directory.as_path()),
            ("frozen_graph", registration.frozen_graph.as_path()),
            ("journal", registration.journal.as_path()),
        ] {
            if !path.is_absolute() {
                return Err(RunRegistrationError::RelativePath {
                    field,
                    path: path.to_path_buf(),
                });
            }
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(RunRegistrationError::ParentPathComponent {
                    field,
                    path: path.to_path_buf(),
                });
            }
        }
        for (field, path) in [
            ("frozen_graph", registration.frozen_graph.as_path()),
            ("journal", registration.journal.as_path()),
        ] {
            if !path.starts_with(&registration.vision_directory) {
                return Err(RunRegistrationError::PathOutsideVision {
                    field,
                    path: path.to_path_buf(),
                    vision_directory: registration.vision_directory.clone(),
                });
            }
        }
        Ok(registration)
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn vision_directory(&self) -> &Path {
        &self.vision_directory
    }
    pub fn frozen_graph(&self) -> &Path {
        &self.frozen_graph
    }
    pub fn journal(&self) -> &Path {
        &self.journal
    }
    pub fn herdr_session(&self) -> Option<&str> {
        self.herdr_session.as_deref()
    }

    /// Derives the run identity from the repository and vision directory only.
    pub fn key(&self) -> RunRegistrationKey {
        let mut digest = Sha256::new();
        for component in [
            self.repository.as_bytes(),
            self.vision_directory.as_os_str().as_encoded_bytes(),
        ] {
            digest.update((component.len() as u64).to_be_bytes());
            digest.update(component);
        }
        RunRegistrationKey(format!("{:x}", digest.finalize()))
    }
}

/// A run registration could not be represented without losing an invariant.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RunRegistrationError {
    /// A required text field is empty.
    #[error("run registration field `{field}` must not be empty")]
    EmptyField { field: &'static str },
    /// A path supplied by the composition root is relative.
    #[error("run registration path `{field}` must be absolute: `{path}`")]
    RelativePath { field: &'static str, path: PathBuf },
    /// A path contains a parent component and is not lexically stable.
    #[error("run registration path `{field}` must not contain `..`: `{path}`")]
    ParentPathComponent { field: &'static str, path: PathBuf },
    /// A run artifact is not located below its declared vision directory.
    #[error(
        "run registration path `{field}` (`{path}`) is outside vision directory `{vision_directory}`"
    )]
    PathOutsideVision {
        field: &'static str,
        path: PathBuf,
        vision_directory: PathBuf,
    },
    /// A Herdr session does not use the supported portable spelling.
    #[error("invalid Herdr session in run registration: `{value}`")]
    InvalidHerdrSession { value: String },
    /// A supplied registration key is not a lowercase SHA-256 digest.
    #[error("invalid run registration key `{value}`; expected 64 lowercase hexadecimal digits")]
    InvalidKey { value: String },
}
