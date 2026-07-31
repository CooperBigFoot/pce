//! DispatchEnvelope = Executable × ArgumentVector × AbsoluteWorkingDirectory × ChildEnvironment × StdinBinding × Option<Sandbox> × Option<AbsoluteSchemaPath> × Option<AbsoluteOutputPath>   (pure, deterministic)
//! This provisional module describes child invocations; the binary adapter performs all I/O and process work and will test the shape against the real tool surface in m2-s2.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::instrument;

/// A program name passed directly to a process adapter, never to a shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executable(String);

impl Executable {
    /// Parse a non-empty program name without normalization or lookup.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::EmptyExecutable`] when `raw` has zero bytes.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, DispatchError> {
        if raw.is_empty() {
            return Err(DispatchError::EmptyExecutable {
                executable: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the program name unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The caller-supplied argument tail in caller order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArgumentVector(Vec<String>);

impl ArgumentVector {
    /// Store caller-supplied arguments without filtering or normalization.
    pub fn new(arguments: Vec<String>) -> Self {
        Self(arguments)
    }

    /// Borrow the ordered caller-supplied arguments.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Report whether the caller supplied no arguments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The complete explicit child environment.
///
/// A later process adapter must clear the inherited environment before applying these entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChildEnvironment(BTreeMap<String, String>);

impl ChildEnvironment {
    /// Store the complete child environment.
    pub fn new(environment: BTreeMap<String, String>) -> Self {
        Self(environment)
    }

    /// Iterate over environment names and values in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Report whether the explicit child environment has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A supported child sandbox capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    /// Permit writes within the configured workspace.
    WorkspaceWrite,
}

impl Sandbox {
    /// Return the tool-facing sandbox spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

/// The closed set of stdin sources expressible by a dispatch value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinBinding {
    /// Bind child stdin to a null source.
    Null,
    /// Supply approved plan bytes unchanged.
    PlanBytes(Vec<u8>),
}

/// An absolute working directory for the child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteWorkingDirectory(PathBuf);

impl AbsoluteWorkingDirectory {
    /// Parse a lexically absolute working directory without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeWorkingDirectory`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeWorkingDirectory { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// An absolute output-schema path for a structured child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteSchemaPath(PathBuf);

impl AbsoluteSchemaPath {
    /// Parse a lexically absolute schema path without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeSchemaPath`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeSchemaPath { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// An absolute result-output path for a child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteOutputPath(PathBuf);

impl AbsoluteOutputPath {
    /// Parse a lexically absolute output path without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeOutputPath`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeOutputPath { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A pure description of one shell-free child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEnvelope {
    executable: Executable,
    arguments: ArgumentVector,
    working_directory: AbsoluteWorkingDirectory,
    environment: ChildEnvironment,
    stdin: StdinBinding,
    sandbox: Option<Sandbox>,
    schema_path: Option<AbsoluteSchemaPath>,
    output_path: Option<AbsoluteOutputPath>,
}

impl DispatchEnvelope {
    /// Construct a minimal dispatch with empty arguments and explicit environment.
    pub fn new(
        executable: Executable,
        working_directory: AbsoluteWorkingDirectory,
        stdin: StdinBinding,
    ) -> Self {
        Self {
            executable,
            arguments: ArgumentVector::default(),
            working_directory,
            environment: ChildEnvironment::default(),
            stdin,
            sandbox: None,
            schema_path: None,
            output_path: None,
        }
    }

    /// Set the caller-supplied argument tail.
    pub fn with_arguments(mut self, arguments: ArgumentVector) -> Self {
        self.arguments = arguments;
        self
    }

    /// Set the complete explicit child environment.
    pub fn with_environment(mut self, environment: ChildEnvironment) -> Self {
        self.environment = environment;
        self
    }

    /// Set the optional sandbox capability.
    pub fn with_sandbox(mut self, sandbox: Sandbox) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Set the optional absolute schema path.
    pub fn with_schema_path(mut self, schema_path: AbsoluteSchemaPath) -> Self {
        self.schema_path = Some(schema_path);
        self
    }

    /// Set the optional absolute output path.
    pub fn with_output_path(mut self, output_path: AbsoluteOutputPath) -> Self {
        self.output_path = Some(output_path);
        self
    }

    /// Borrow the executable.
    pub fn executable(&self) -> &Executable {
        &self.executable
    }

    /// Borrow the caller-supplied arguments.
    pub fn arguments(&self) -> &ArgumentVector {
        &self.arguments
    }

    /// Borrow the absolute working directory.
    pub fn working_directory(&self) -> &AbsoluteWorkingDirectory {
        &self.working_directory
    }

    /// Borrow the complete explicit child environment.
    pub fn environment(&self) -> &ChildEnvironment {
        &self.environment
    }

    /// Borrow the closed stdin binding.
    pub fn stdin(&self) -> &StdinBinding {
        &self.stdin
    }

    /// Return the optional sandbox capability.
    pub fn sandbox(&self) -> Option<Sandbox> {
        self.sandbox
    }

    /// Borrow the optional absolute schema path.
    pub fn schema_path(&self) -> Option<&AbsoluteSchemaPath> {
        self.schema_path.as_ref()
    }

    /// Borrow the optional absolute output path.
    pub fn output_path(&self) -> Option<&AbsoluteOutputPath> {
        self.output_path.as_ref()
    }
}

/// A dispatch value failed pure parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchError {
    /// The executable parser received a zero-byte program name.
    #[error("executable must not be empty; rejected {executable:?}")]
    EmptyExecutable { executable: String },
    /// The working-directory parser received a relative path.
    #[error("working directory must be absolute; rejected {path:?}")]
    RelativeWorkingDirectory { path: PathBuf },
    /// The schema-path parser received a relative path.
    #[error("schema path must be absolute; rejected {path:?}")]
    RelativeSchemaPath { path: PathBuf },
    /// The output-path parser received a relative path.
    #[error("output path must be absolute; rejected {path:?}")]
    RelativeOutputPath { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::{
        AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ArgumentVector,
        ChildEnvironment, DispatchEnvelope, DispatchError, Executable, Sandbox, StdinBinding,
    };

    #[test]
    fn constructs_minimal_dispatch_envelope() -> Result<(), DispatchError> {
        let envelope = DispatchEnvelope::new(
            Executable::parse("codex")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        );

        assert_eq!(envelope.executable().as_str(), "codex");
        assert_eq!(
            envelope.working_directory().as_path(),
            Path::new("/workspace/project")
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        assert!(envelope.arguments().is_empty());
        assert!(envelope.environment().is_empty());
        assert_eq!(envelope.sandbox(), None);
        assert_eq!(envelope.schema_path(), None);
        assert_eq!(envelope.output_path(), None);
        Ok(())
    }

    #[test]
    fn rejects_empty_executable() -> Result<(), DispatchError> {
        assert_eq!(
            Executable::parse(""),
            Err(DispatchError::EmptyExecutable {
                executable: String::new()
            })
        );
        assert_eq!(Executable::parse("codex")?.as_str(), "codex");
        Ok(())
    }

    #[test]
    fn absolute_path_types_accept_absolute_paths() -> Result<(), DispatchError> {
        let working_directory = AbsoluteWorkingDirectory::parse("/workspace/./project")?;
        let schema_path = AbsoluteSchemaPath::parse("/workspace/schema.json")?;
        let output_path = AbsoluteOutputPath::parse("/workspace/output.json")?;

        assert_eq!(
            working_directory.as_path(),
            Path::new("/workspace/./project")
        );
        assert_eq!(schema_path.as_path(), Path::new("/workspace/schema.json"));
        assert_eq!(output_path.as_path(), Path::new("/workspace/output.json"));
        Ok(())
    }

    #[test]
    fn absolute_path_types_reject_relative_paths() {
        assert_eq!(
            AbsoluteWorkingDirectory::parse("relative/workspace"),
            Err(DispatchError::RelativeWorkingDirectory {
                path: PathBuf::from("relative/workspace")
            })
        );
        assert_eq!(
            AbsoluteSchemaPath::parse("relative/schema.json"),
            Err(DispatchError::RelativeSchemaPath {
                path: PathBuf::from("relative/schema.json")
            })
        );
        assert_eq!(
            AbsoluteOutputPath::parse("relative/output.json"),
            Err(DispatchError::RelativeOutputPath {
                path: PathBuf::from("relative/output.json")
            })
        );
    }

    #[test]
    fn constructs_structured_dispatch_value() -> Result<(), DispatchError> {
        let arguments = vec![
            "--caller-option".to_owned(),
            "POSITIONAL_PROMPT_PLACEHOLDER".to_owned(),
            String::new(),
        ];
        let mut environment = BTreeMap::new();
        environment.insert("LANG".to_owned(), "C.UTF-8".to_owned());
        environment.insert("PCE_MODE".to_owned(), "dispatch".to_owned());
        let plan_bytes = vec![0, 1, 2, 255];

        let envelope = DispatchEnvelope::new(
            Executable::parse("codex")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::PlanBytes(plan_bytes.clone()),
        )
        .with_arguments(ArgumentVector::new(arguments.clone()))
        .with_environment(ChildEnvironment::new(environment.clone()))
        .with_sandbox(Sandbox::WorkspaceWrite)
        .with_schema_path(AbsoluteSchemaPath::parse("/workspace/schema.json")?)
        .with_output_path(AbsoluteOutputPath::parse("/workspace/output.json")?);

        assert_eq!(envelope.executable().as_str(), "codex");
        assert_eq!(envelope.arguments().as_slice(), arguments.as_slice());
        assert_eq!(
            envelope.environment().iter().collect::<Vec<_>>(),
            environment
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            envelope.stdin(),
            &StdinBinding::PlanBytes(plan_bytes.clone())
        );
        assert_eq!(envelope.sandbox(), Some(Sandbox::WorkspaceWrite));
        assert_eq!(Sandbox::WorkspaceWrite.as_str(), "workspace-write");
        assert_eq!(
            envelope.schema_path().map(AbsoluteSchemaPath::as_path),
            Some(Path::new("/workspace/schema.json"))
        );
        assert_eq!(
            envelope.output_path().map(AbsoluteOutputPath::as_path),
            Some(Path::new("/workspace/output.json"))
        );
        Ok(())
    }

    #[test]
    fn constructs_unstructured_reusable_dispatch_value() -> Result<(), DispatchError> {
        let arguments = ArgumentVector::new(vec!["positional prompt".to_owned()]);
        let envelope = DispatchEnvelope::new(
            Executable::parse("claude")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        )
        .with_arguments(arguments);

        assert_eq!(
            envelope.arguments().as_slice(),
            &["positional prompt".to_owned()]
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        assert_eq!(envelope.sandbox(), None);
        assert_eq!(envelope.schema_path(), None);
        assert_eq!(envelope.output_path(), None);
        Ok(())
    }
}
