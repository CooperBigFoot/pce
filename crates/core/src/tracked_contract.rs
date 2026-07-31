//! parse_gate_command : GateCommandText → Executable × ArgumentVector ∪ GateCommandParseError; parse_tracked_repository_contract : TrackedContractBytes → TrackedRepositoryContract ∪ TrackedContractError; serialize_tracked_repository_contract : TrackedRepositoryContract → CanonicalTrackedContractBytes ∪ serde_json::Error; admit_recurrent_finding : TrackedRepositoryContract × AppendableFinding × CurrentRunFindingTexts × PriorRunFindingTexts → TrackedRepositoryContract × FindingAdmission   (pure, deterministic)
//! This module performs no I/O.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

use crate::dispatch::{ArgumentVector, DispatchError, Executable};
use crate::run_state::VersionPolicy;

/// The typed stated and appendable halves of a tracked repository contract.
///
/// The two halves carry structurally distinct authority:
///
/// ```rust,compile_fail
/// use pce_core::{AppendableContract, StatedContract};
///
/// fn append_only(_: &AppendableContract) {}
///
/// fn cannot_append_to_stated(stated: &StatedContract) {
///     append_only(stated);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedRepositoryContract {
    stated: StatedContract,
    appendable: AppendableContract,
}

impl TrackedRepositoryContract {
    /// Return the falsifiable stated half.
    pub const fn stated(&self) -> &StatedContract {
        &self.stated
    }

    /// Return the inert appendable half.
    pub const fn appendable(&self) -> &AppendableContract {
        &self.appendable
    }
}

/// The falsifiable repository contract measured by later milestones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatedContract {
    gates: GateCommands,
    version_policy: VersionPolicy,
    branches: BranchConvention,
    pull_requests: PullRequestConvention,
    workflows: WorkflowMappings,
}

impl StatedContract {
    /// Return the five acceptance-gate commands.
    pub const fn gates(&self) -> &GateCommands {
        &self.gates
    }

    /// Return the repository's version policy.
    pub const fn version_policy(&self) -> &VersionPolicy {
        &self.version_policy
    }

    /// Return the branch convention.
    pub const fn branches(&self) -> &BranchConvention {
        &self.branches
    }

    /// Return the pull-request convention.
    pub const fn pull_requests(&self) -> &PullRequestConvention {
        &self.pull_requests
    }

    /// Return the workflow-to-local-command mappings.
    pub const fn workflows(&self) -> &WorkflowMappings {
        &self.workflows
    }
}

/// A non-empty command used by an acceptance gate or workflow stand-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommand(String);

impl GateCommand {
    /// Return the command exactly as stated.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A shell-free executable and its complete parsed argument vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGateCommand {
    executable: Executable,
    arguments: ArgumentVector,
}

impl ParsedGateCommand {
    /// Borrow the parsed executable.
    pub const fn executable(&self) -> &Executable {
        &self.executable
    }

    /// Borrow the parsed argument vector.
    pub const fn arguments(&self) -> &ArgumentVector {
        &self.arguments
    }
}

/// A gate command uses syntax that cannot be represented by direct process execution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GateCommandParseError {
    /// Fires when the command contains no word.
    #[error("gate command cannot be empty")]
    EmptyCommand,
    /// Fires when quoting constructs a zero-byte word.
    #[error("gate command contains an empty word at byte {position}")]
    EmptyWord {
        /// Byte offset where the empty word ended.
        position: usize,
    },
    /// Fires when a single or double quote has no closing delimiter.
    #[error("gate command has an unterminated {quote} quote starting at byte {position}")]
    UnterminatedQuote {
        /// The human-readable quote kind.
        quote: &'static str,
        /// Byte offset of the opening quote.
        position: usize,
    },
    /// Fires when a trailing backslash has no byte to quote.
    #[error("gate command has a dangling escape at byte {position}")]
    DanglingEscape {
        /// Byte offset of the trailing backslash.
        position: usize,
    },
    /// Fires when unquoted control or operator syntax is present.
    #[error("gate command contains shell operator {syntax:?} at byte {position}")]
    ShellOperator {
        /// The rejected syntax.
        syntax: String,
        /// Byte offset of the syntax.
        position: usize,
    },
    /// Fires when unquoted input or output redirection syntax is present.
    #[error("gate command contains redirection {syntax:?} at byte {position}")]
    Redirection {
        /// The rejected syntax.
        syntax: String,
        /// Byte offset of the syntax.
        position: usize,
    },
    /// Fires when command-substitution syntax is present.
    #[error("gate command contains command substitution {syntax:?} at byte {position}")]
    CommandSubstitution {
        /// The rejected syntax.
        syntax: String,
        /// Byte offset of the syntax.
        position: usize,
    },
    /// Fires when variable or parameter expansion syntax is present.
    #[error("gate command contains variable expansion at byte {position}")]
    VariableExpansion {
        /// Byte offset of the dollar sign.
        position: usize,
    },
    /// Fires when globbing syntax is present.
    #[error("gate command contains globbing syntax {syntax:?} at byte {position}")]
    Globbing {
        /// The rejected syntax.
        syntax: char,
        /// Byte offset of the syntax.
        position: usize,
    },
    /// Fires when the parsed executable violates its domain invariant.
    #[error("gate command executable is invalid: {source}")]
    InvalidExecutable {
        /// The executable parsing failure.
        source: DispatchError,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    Single,
    Double,
}

/// Parse lexical shell-style words without invoking or emulating a shell.
///
/// Quotes and backslashes only preserve literal word bytes. Shell operators,
/// redirections, expansions, substitutions, and globbing are rejected.
///
/// # Errors
///
/// Returns [`GateCommandParseError`] when the command is empty, creates an
/// empty word, has incomplete quoting, or contains shell execution semantics.
pub fn parse_gate_command(raw: &str) -> Result<ParsedGateCommand, GateCommandParseError> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut word_started = false;
    let mut quote = None;
    let mut quote_start = 0;
    let mut chars = raw.char_indices().peekable();

    while let Some((position, character)) = chars.next() {
        if character == '\n' {
            return Err(GateCommandParseError::ShellOperator {
                syntax: "newline".to_owned(),
                position,
            });
        }
        if character == '\\' {
            let Some((_, literal)) = chars.next() else {
                return Err(GateCommandParseError::DanglingEscape { position });
            };
            word_started = true;
            word.push(literal);
            continue;
        }
        match quote {
            Some(Quote::Single) if character == '\'' => {
                quote = None;
                continue;
            }
            Some(Quote::Double) if character == '"' => {
                quote = None;
                continue;
            }
            Some(_) => {}
            None if character == '\'' => {
                quote = Some(Quote::Single);
                quote_start = position;
                word_started = true;
                continue;
            }
            None if character == '"' => {
                quote = Some(Quote::Double);
                quote_start = position;
                word_started = true;
                continue;
            }
            None if character.is_whitespace() => {
                if word_started {
                    if word.is_empty() {
                        return Err(GateCommandParseError::EmptyWord { position });
                    }
                    words.push(std::mem::take(&mut word));
                    word_started = false;
                }
                continue;
            }
            None => {}
        }

        if character == '`' {
            return Err(GateCommandParseError::CommandSubstitution {
                syntax: "`".to_owned(),
                position,
            });
        }
        if character == '$' {
            if chars.peek().is_some_and(|(_, next)| *next == '(') {
                return Err(GateCommandParseError::CommandSubstitution {
                    syntax: "$(".to_owned(),
                    position,
                });
            }
            return Err(GateCommandParseError::VariableExpansion { position });
        }
        if quote.is_none() {
            if matches!(character, '<' | '>') {
                return Err(GateCommandParseError::Redirection {
                    syntax: character.to_string(),
                    position,
                });
            }
            if matches!(character, ';' | '&' | '|' | '(' | ')') {
                return Err(GateCommandParseError::ShellOperator {
                    syntax: character.to_string(),
                    position,
                });
            }
            if matches!(character, '*' | '?' | '[' | ']') {
                return Err(GateCommandParseError::Globbing {
                    syntax: character,
                    position,
                });
            }
        }
        word_started = true;
        word.push(character);
    }

    if let Some(open_quote) = quote {
        return Err(GateCommandParseError::UnterminatedQuote {
            quote: match open_quote {
                Quote::Single => "single",
                Quote::Double => "double",
            },
            position: quote_start,
        });
    }
    if word_started {
        if word.is_empty() {
            return Err(GateCommandParseError::EmptyWord {
                position: raw.len(),
            });
        }
        words.push(word);
    }
    if words.is_empty() {
        return Err(GateCommandParseError::EmptyCommand);
    }
    let executable = Executable::parse(&words.remove(0))
        .map_err(|source| GateCommandParseError::InvalidExecutable { source })?;
    Ok(ParsedGateCommand {
        executable,
        arguments: ArgumentVector::new(words),
    })
}

/// The role of one acceptance gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateKind {
    /// The formatting gate.
    Format,
    /// The lint gate.
    Lint,
    /// The typecheck gate.
    Typecheck,
    /// The test gate.
    Test,
    /// The build gate.
    Build,
}

/// The five required acceptance-gate commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommands {
    format: GateCommand,
    lint: GateCommand,
    typecheck: GateCommand,
    test: GateCommand,
    build: GateCommand,
}

impl GateCommands {
    /// Return the formatting command.
    pub const fn format(&self) -> &GateCommand {
        &self.format
    }

    /// Return the lint command.
    pub const fn lint(&self) -> &GateCommand {
        &self.lint
    }

    /// Return the typecheck command.
    pub const fn typecheck(&self) -> &GateCommand {
        &self.typecheck
    }

    /// Return the test command.
    pub const fn test(&self) -> &GateCommand {
        &self.test
    }

    /// Return the build command.
    pub const fn build(&self) -> &GateCommand {
        &self.build
    }

    /// Iterate over commands in format, lint, typecheck, test, build order.
    pub fn iter(&self) -> impl Iterator<Item = (GateKind, &GateCommand)> {
        [
            (GateKind::Format, &self.format),
            (GateKind::Lint, &self.lint),
            (GateKind::Typecheck, &self.typecheck),
            (GateKind::Test, &self.test),
            (GateKind::Build, &self.build),
        ]
        .into_iter()
    }
}

macro_rules! non_empty_text_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Return the text exactly as stated.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

non_empty_text_type!(
    /// The repository's non-empty default branch name.
    DefaultBranchName
);
non_empty_text_type!(
    /// The non-empty pattern for milestone branches.
    MilestoneBranchPattern
);
non_empty_text_type!(
    /// The non-empty pattern for step branches.
    StepBranchPattern
);

/// The three typed branch roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchConvention {
    default: DefaultBranchName,
    milestone: MilestoneBranchPattern,
    step: StepBranchPattern,
}

impl BranchConvention {
    /// Return the default branch name.
    pub const fn default(&self) -> &DefaultBranchName {
        &self.default
    }

    /// Return the milestone branch pattern.
    pub const fn milestone(&self) -> &MilestoneBranchPattern {
        &self.milestone
    }

    /// Return the step branch pattern.
    pub const fn step(&self) -> &StepBranchPattern {
        &self.step
    }
}

/// The required base for a step pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepPullRequestBase {
    /// Step pull requests target their milestone branch.
    Milestone,
}

/// The required base for a milestone pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MilestonePullRequestBase {
    /// Milestone pull requests target the default branch.
    Default,
}

/// The required pull-request merge method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PullRequestMergeMethod {
    /// Pull requests are squash-merged.
    Squash,
}

/// Pull-request base and merge conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestConvention {
    step_base: StepPullRequestBase,
    milestone_base: MilestonePullRequestBase,
    merge_method: PullRequestMergeMethod,
}

impl PullRequestConvention {
    /// Return the step pull-request base.
    pub const fn step_base(&self) -> StepPullRequestBase {
        self.step_base
    }

    /// Return the milestone pull-request base.
    pub const fn milestone_base(&self) -> MilestonePullRequestBase {
        self.milestone_base
    }

    /// Return the pull-request merge method.
    pub const fn merge_method(&self) -> PullRequestMergeMethod {
        self.merge_method
    }
}

non_empty_text_type!(
    /// A non-empty workflow filename or identity.
    WorkflowName
);

/// The explicit local stand-in for one tracked workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalWorkflowStandIn {
    /// Execute this local command as the workflow stand-in.
    Command(GateCommand),
    /// No local stand-in is stated.
    None,
}

/// One workflow and its explicit local stand-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowMapping {
    workflow: WorkflowName,
    stand_in: LocalWorkflowStandIn,
}

impl WorkflowMapping {
    /// Return the workflow name.
    pub const fn workflow(&self) -> &WorkflowName {
        &self.workflow
    }

    /// Return the stated local stand-in.
    pub const fn stand_in(&self) -> &LocalWorkflowStandIn {
        &self.stand_in
    }
}

/// Unique workflow-to-local-command mappings in tracked order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowMappings(Vec<WorkflowMapping>);

impl WorkflowMappings {
    /// Return the mappings in tracked order.
    pub fn as_slice(&self) -> &[WorkflowMapping] {
        &self.0
    }
}

non_empty_text_type!(
    /// A non-empty environment hazard.
    EnvironmentHazard
);
non_empty_text_type!(
    /// A non-empty acceptance-gate ordering.
    GateOrdering
);
non_empty_text_type!(
    /// A non-empty lockfile rule.
    LockfileRule
);

/// The closed set of appendable fact categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendableCategory {
    /// An environmental hazard.
    EnvironmentHazard,
    /// An acceptance-gate ordering.
    GateOrdering,
    /// A lockfile rule.
    LockfileRule,
}

impl AppendableCategory {
    /// Parse an exact appendable category spelling.
    ///
    /// # Errors
    ///
    /// Returns [`TrackedContractError::UnknownAppendableCategory`] for every
    /// string outside the closed category set.
    pub fn parse(raw: &str) -> Result<Self, TrackedContractError> {
        match raw {
            "environment-hazard" => Ok(Self::EnvironmentHazard),
            "gate-ordering" => Ok(Self::GateOrdering),
            "lockfile-rule" => Ok(Self::LockfileRule),
            _ => Err(TrackedContractError::UnknownAppendableCategory {
                category: raw.to_owned(),
            }),
        }
    }

    /// Return the exact CLI spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnvironmentHazard => "environment-hazard",
            Self::GateOrdering => "gate-ordering",
            Self::LockfileRule => "lockfile-rule",
        }
    }
}

/// A non-empty finding selected for exactly one appendable category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendableFinding {
    /// An environmental hazard.
    EnvironmentHazard(EnvironmentHazard),
    /// An acceptance-gate ordering.
    GateOrdering(GateOrdering),
    /// A lockfile rule.
    LockfileRule(LockfileRule),
}

impl AppendableFinding {
    /// Parse a finding under a typed category choice.
    ///
    /// # Errors
    ///
    /// Returns [`TrackedContractError::EmptyField`] when the finding is empty
    /// or whitespace-only.
    pub fn parse(
        category: AppendableCategory,
        finding: String,
    ) -> Result<Self, TrackedContractError> {
        match category {
            AppendableCategory::EnvironmentHazard => {
                non_empty(finding, "appendable.environment_hazards[]")
                    .map(EnvironmentHazard)
                    .map(Self::EnvironmentHazard)
            }
            AppendableCategory::GateOrdering => non_empty(finding, "appendable.gate_orderings[]")
                .map(GateOrdering)
                .map(Self::GateOrdering),
            AppendableCategory::LockfileRule => non_empty(finding, "appendable.lockfile_rules[]")
                .map(LockfileRule)
                .map(Self::LockfileRule),
        }
    }

    /// Return the finding text exactly as parsed.
    pub fn as_str(&self) -> &str {
        match self {
            Self::EnvironmentHazard(finding) => finding.as_str(),
            Self::GateOrdering(finding) => finding.as_str(),
            Self::LockfileRule(finding) => finding.as_str(),
        }
    }
}

/// The result of checking and admitting a finding by recurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingAdmission {
    /// The selected text does not occur in the current run.
    CurrentOccurrenceMissing,
    /// The selected text occurs in the current run but not the prior run.
    FirstOccurrence,
    /// The recurrent text was appended to its selected category.
    Appended,
    /// The recurrent text was already present in its selected category.
    AlreadyPresent,
}

/// Inert appendable facts that do not alter the stated contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendableContract {
    environment_hazards: Vec<EnvironmentHazard>,
    gate_orderings: Vec<GateOrdering>,
    lockfile_rules: Vec<LockfileRule>,
}

impl AppendableContract {
    /// Return the environment hazards.
    pub fn environment_hazards(&self) -> &[EnvironmentHazard] {
        &self.environment_hazards
    }

    /// Return the gate orderings.
    pub fn gate_orderings(&self) -> &[GateOrdering] {
        &self.gate_orderings
    }

    /// Return the lockfile rules.
    pub fn lockfile_rules(&self) -> &[LockfileRule] {
        &self.lockfile_rules
    }
}

/// A failure to parse or convert a tracked repository contract.
#[derive(Debug, Error)]
pub enum TrackedContractError {
    /// Fires when JSON syntax or any required closed wire shape is invalid.
    #[error("malformed tracked repository contract: {source}")]
    MalformedContract {
        /// The detailed JSON deserialization failure.
        source: serde_json::Error,
    },
    /// Fires when an invariant-bearing text field is empty or whitespace-only.
    #[error("tracked repository contract field {field} cannot be empty")]
    EmptyField {
        /// The stable path label identifying the empty field.
        field: &'static str,
    },
    /// Fires when a workflow name repeats an earlier byte-identical name.
    #[error("tracked repository contract repeats workflow {workflow:?}")]
    DuplicateWorkflow {
        /// The repeated workflow name.
        workflow: String,
    },
    /// Fires for every CLI category outside the closed appendable category set.
    #[error(
        "unknown appendable category {category:?}; expected environment-hazard, gate-ordering, or lockfile-rule"
    )]
    UnknownAppendableCategory {
        /// The rejected category string.
        category: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrackedRepositoryContract {
    stated: RawStatedContract,
    appendable: RawAppendableContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatedContract {
    gates: RawGateCommands,
    version_policy: RawVersionPolicy,
    branches: RawBranchConvention,
    pull_requests: RawPullRequestConvention,
    workflows: Vec<RawWorkflowMapping>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGateCommands {
    format: String,
    lint: String,
    typecheck: String,
    test: String,
    build: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RawVersionPolicy {
    None,
    SerializeDispatches,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBranchConvention {
    default: String,
    milestone: String,
    step: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPullRequestConvention {
    step_base: StepPullRequestBase,
    milestone_base: MilestonePullRequestBase,
    merge_method: PullRequestMergeMethod,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkflowMapping {
    workflow: String,
    stand_in: RawLocalWorkflowStandIn,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum RawLocalWorkflowStandIn {
    Command { command: String },
    None {},
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAppendableContract {
    environment_hazards: Vec<String>,
    gate_orderings: Vec<String>,
    lockfile_rules: Vec<String>,
}

fn non_empty(value: String, field: &'static str) -> Result<String, TrackedContractError> {
    if value.trim().is_empty() {
        Err(TrackedContractError::EmptyField { field })
    } else {
        Ok(value)
    }
}

impl TryFrom<RawTrackedRepositoryContract> for TrackedRepositoryContract {
    type Error = TrackedContractError;

    fn try_from(raw: RawTrackedRepositoryContract) -> Result<Self, Self::Error> {
        Ok(Self {
            stated: raw.stated.try_into()?,
            appendable: raw.appendable.try_into()?,
        })
    }
}

impl TryFrom<RawStatedContract> for StatedContract {
    type Error = TrackedContractError;

    fn try_from(raw: RawStatedContract) -> Result<Self, Self::Error> {
        Ok(Self {
            gates: raw.gates.try_into()?,
            version_policy: match raw.version_policy {
                RawVersionPolicy::None => VersionPolicy::None,
                RawVersionPolicy::SerializeDispatches => VersionPolicy::SerializeDispatches,
            },
            branches: raw.branches.try_into()?,
            pull_requests: raw.pull_requests.into(),
            workflows: raw.workflows.try_into()?,
        })
    }
}

impl TryFrom<RawGateCommands> for GateCommands {
    type Error = TrackedContractError;

    fn try_from(raw: RawGateCommands) -> Result<Self, Self::Error> {
        Ok(Self {
            format: GateCommand(non_empty(raw.format, "stated.gates.format")?),
            lint: GateCommand(non_empty(raw.lint, "stated.gates.lint")?),
            typecheck: GateCommand(non_empty(raw.typecheck, "stated.gates.typecheck")?),
            test: GateCommand(non_empty(raw.test, "stated.gates.test")?),
            build: GateCommand(non_empty(raw.build, "stated.gates.build")?),
        })
    }
}

impl TryFrom<RawBranchConvention> for BranchConvention {
    type Error = TrackedContractError;

    fn try_from(raw: RawBranchConvention) -> Result<Self, Self::Error> {
        Ok(Self {
            default: DefaultBranchName(non_empty(raw.default, "stated.branches.default")?),
            milestone: MilestoneBranchPattern(non_empty(
                raw.milestone,
                "stated.branches.milestone",
            )?),
            step: StepBranchPattern(non_empty(raw.step, "stated.branches.step")?),
        })
    }
}

impl From<RawPullRequestConvention> for PullRequestConvention {
    fn from(raw: RawPullRequestConvention) -> Self {
        Self {
            step_base: raw.step_base,
            milestone_base: raw.milestone_base,
            merge_method: raw.merge_method,
        }
    }
}

impl TryFrom<Vec<RawWorkflowMapping>> for WorkflowMappings {
    type Error = TrackedContractError;

    fn try_from(raw: Vec<RawWorkflowMapping>) -> Result<Self, Self::Error> {
        let mut names = HashSet::with_capacity(raw.len());
        let mut mappings = Vec::with_capacity(raw.len());
        for raw_mapping in raw {
            let workflow = non_empty(raw_mapping.workflow, "stated.workflows[].workflow")?;
            if !names.insert(workflow.clone()) {
                return Err(TrackedContractError::DuplicateWorkflow { workflow });
            }
            let stand_in = match raw_mapping.stand_in {
                RawLocalWorkflowStandIn::Command { command } => LocalWorkflowStandIn::Command(
                    GateCommand(non_empty(command, "stated.workflows[].stand_in.command")?),
                ),
                RawLocalWorkflowStandIn::None {} => LocalWorkflowStandIn::None,
            };
            mappings.push(WorkflowMapping {
                workflow: WorkflowName(workflow),
                stand_in,
            });
        }
        Ok(Self(mappings))
    }
}

impl TryFrom<RawAppendableContract> for AppendableContract {
    type Error = TrackedContractError;

    fn try_from(raw: RawAppendableContract) -> Result<Self, Self::Error> {
        let environment_hazards = raw
            .environment_hazards
            .into_iter()
            .map(|value| {
                non_empty(value, "appendable.environment_hazards[]").map(EnvironmentHazard)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let gate_orderings = raw
            .gate_orderings
            .into_iter()
            .map(|value| non_empty(value, "appendable.gate_orderings[]").map(GateOrdering))
            .collect::<Result<Vec<_>, _>>()?;
        let lockfile_rules = raw
            .lockfile_rules
            .into_iter()
            .map(|value| non_empty(value, "appendable.lockfile_rules[]").map(LockfileRule))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            environment_hazards,
            gate_orderings,
            lockfile_rules,
        })
    }
}

/// Admit a finding only when byte-identical text occurs in both run inputs.
pub fn admit_recurrent_finding(
    contract: &mut TrackedRepositoryContract,
    finding: AppendableFinding,
    current_run_findings: &[&str],
    prior_run_findings: &[&str],
) -> FindingAdmission {
    let selected = finding.as_str().as_bytes().to_owned();
    if !current_run_findings
        .iter()
        .any(|candidate| candidate.as_bytes() == selected)
    {
        return FindingAdmission::CurrentOccurrenceMissing;
    }
    if !prior_run_findings
        .iter()
        .any(|candidate| candidate.as_bytes() == selected)
    {
        return FindingAdmission::FirstOccurrence;
    }

    match finding {
        AppendableFinding::EnvironmentHazard(finding) => {
            if contract
                .appendable
                .environment_hazards
                .iter()
                .any(|existing| existing.as_str().as_bytes() == selected)
            {
                FindingAdmission::AlreadyPresent
            } else {
                contract.appendable.environment_hazards.push(finding);
                FindingAdmission::Appended
            }
        }
        AppendableFinding::GateOrdering(finding) => {
            if contract
                .appendable
                .gate_orderings
                .iter()
                .any(|existing| existing.as_str().as_bytes() == selected)
            {
                FindingAdmission::AlreadyPresent
            } else {
                contract.appendable.gate_orderings.push(finding);
                FindingAdmission::Appended
            }
        }
        AppendableFinding::LockfileRule(finding) => {
            if contract
                .appendable
                .lockfile_rules
                .iter()
                .any(|existing| existing.as_str().as_bytes() == selected)
            {
                FindingAdmission::AlreadyPresent
            } else {
                contract.appendable.lockfile_rules.push(finding);
                FindingAdmission::Appended
            }
        }
    }
}

/// Parse tracked repository contract bytes into the typed two-authority model.
///
/// # Errors
///
/// Returns [`TrackedContractError::MalformedContract`] for invalid JSON or wire
/// shapes, [`TrackedContractError::EmptyField`] for empty required text, and
/// [`TrackedContractError::DuplicateWorkflow`] for repeated workflow names.
#[instrument(skip(bytes))]
pub fn parse_tracked_repository_contract(
    bytes: &[u8],
) -> Result<TrackedRepositoryContract, TrackedContractError> {
    let raw = serde_json::from_slice::<RawTrackedRepositoryContract>(bytes)
        .map_err(|source| TrackedContractError::MalformedContract { source })?;
    raw.try_into()
}

#[derive(Serialize)]
struct CanonicalTrackedRepositoryContract<'a> {
    stated: CanonicalStatedContract<'a>,
    appendable: CanonicalAppendableContract<'a>,
}

#[derive(Serialize)]
struct CanonicalStatedContract<'a> {
    gates: CanonicalGateCommands<'a>,
    version_policy: CanonicalVersionPolicy,
    branches: CanonicalBranchConvention<'a>,
    pull_requests: CanonicalPullRequestConvention,
    workflows: Vec<CanonicalWorkflowMapping<'a>>,
}

#[derive(Serialize)]
struct CanonicalGateCommands<'a> {
    format: &'a str,
    lint: &'a str,
    typecheck: &'a str,
    test: &'a str,
    build: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CanonicalVersionPolicy {
    None,
    SerializeDispatches,
}

#[derive(Serialize)]
struct CanonicalBranchConvention<'a> {
    default: &'a str,
    milestone: &'a str,
    step: &'a str,
}

#[derive(Serialize)]
struct CanonicalPullRequestConvention {
    step_base: CanonicalStepPullRequestBase,
    milestone_base: CanonicalMilestonePullRequestBase,
    merge_method: CanonicalPullRequestMergeMethod,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CanonicalStepPullRequestBase {
    Milestone,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CanonicalMilestonePullRequestBase {
    Default,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CanonicalPullRequestMergeMethod {
    Squash,
}

#[derive(Serialize)]
struct CanonicalWorkflowMapping<'a> {
    workflow: &'a str,
    stand_in: CanonicalLocalWorkflowStandIn<'a>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum CanonicalLocalWorkflowStandIn<'a> {
    Command { command: &'a str },
    None {},
}

#[derive(Serialize)]
struct CanonicalAppendableContract<'a> {
    environment_hazards: Vec<&'a str>,
    gate_orderings: Vec<&'a str>,
    lockfile_rules: Vec<&'a str>,
}

impl<'a> From<&'a TrackedRepositoryContract> for CanonicalTrackedRepositoryContract<'a> {
    fn from(contract: &'a TrackedRepositoryContract) -> Self {
        let stated = contract.stated();
        let gates = stated.gates();
        let branches = stated.branches();
        let pull_requests = stated.pull_requests();
        let appendable = contract.appendable();
        Self {
            stated: CanonicalStatedContract {
                gates: CanonicalGateCommands {
                    format: gates.format().as_str(),
                    lint: gates.lint().as_str(),
                    typecheck: gates.typecheck().as_str(),
                    test: gates.test().as_str(),
                    build: gates.build().as_str(),
                },
                version_policy: match stated.version_policy() {
                    VersionPolicy::None => CanonicalVersionPolicy::None,
                    VersionPolicy::SerializeDispatches => {
                        CanonicalVersionPolicy::SerializeDispatches
                    }
                },
                branches: CanonicalBranchConvention {
                    default: branches.default().as_str(),
                    milestone: branches.milestone().as_str(),
                    step: branches.step().as_str(),
                },
                pull_requests: CanonicalPullRequestConvention {
                    step_base: match pull_requests.step_base() {
                        StepPullRequestBase::Milestone => CanonicalStepPullRequestBase::Milestone,
                    },
                    milestone_base: match pull_requests.milestone_base() {
                        MilestonePullRequestBase::Default => {
                            CanonicalMilestonePullRequestBase::Default
                        }
                    },
                    merge_method: match pull_requests.merge_method() {
                        PullRequestMergeMethod::Squash => CanonicalPullRequestMergeMethod::Squash,
                    },
                },
                workflows: stated
                    .workflows()
                    .as_slice()
                    .iter()
                    .map(|mapping| CanonicalWorkflowMapping {
                        workflow: mapping.workflow().as_str(),
                        stand_in: match mapping.stand_in() {
                            LocalWorkflowStandIn::Command(command) => {
                                CanonicalLocalWorkflowStandIn::Command {
                                    command: command.as_str(),
                                }
                            }
                            LocalWorkflowStandIn::None => CanonicalLocalWorkflowStandIn::None {},
                        },
                    })
                    .collect(),
            },
            appendable: CanonicalAppendableContract {
                environment_hazards: appendable
                    .environment_hazards()
                    .iter()
                    .map(EnvironmentHazard::as_str)
                    .collect(),
                gate_orderings: appendable
                    .gate_orderings()
                    .iter()
                    .map(GateOrdering::as_str)
                    .collect(),
                lockfile_rules: appendable
                    .lockfile_rules()
                    .iter()
                    .map(LockfileRule::as_str)
                    .collect(),
            },
        }
    }
}

/// Serialize a typed tracked repository contract into canonical JSON bytes.
///
/// # Errors
///
/// Returns [`serde_json::Error`] if canonical JSON encoding fails.
#[instrument(skip(contract))]
pub fn serialize_tracked_repository_contract(
    contract: &TrackedRepositoryContract,
) -> Result<Vec<u8>, serde_json::Error> {
    let wire = CanonicalTrackedRepositoryContract::from(contract);
    let mut bytes = serde_json::to_vec_pretty(&wire)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use serde_json::{Value, json};

    use super::{
        AppendableCategory, AppendableContract, AppendableFinding, DefaultBranchName,
        EnvironmentHazard, FindingAdmission, GateCommandParseError, GateKind, GateOrdering,
        LocalWorkflowStandIn, LockfileRule, MilestoneBranchPattern, MilestonePullRequestBase,
        PullRequestMergeMethod, StatedContract, StepBranchPattern, StepPullRequestBase,
        TrackedContractError, admit_recurrent_finding, parse_gate_command,
        parse_tracked_repository_contract, serialize_tracked_repository_contract,
    };
    use crate::run_state::VersionPolicy;

    #[test]
    fn gate_command_parser_preserves_word_boundaries() {
        for (raw, executable, arguments) in [
            ("uv build --wheel", "uv", vec!["build", "--wheel"]),
            ("uv 'quoted whitespace'", "uv", vec!["quoted whitespace"]),
            ("uv \"double whitespace\"", "uv", vec!["double whitespace"]),
            ("uv escaped\\ whitespace", "uv", vec!["escaped whitespace"]),
            ("u'v' bu\"il\"d", "uv", vec!["build"]),
        ] {
            let parsed = parse_gate_command(raw).expect("lexical command should parse");
            assert_eq!(parsed.executable().as_str(), executable);
            assert_eq!(parsed.arguments().as_slice(), arguments);
        }
    }

    #[test]
    fn gate_command_parser_rejects_empty_commands_and_words() {
        assert_eq!(
            parse_gate_command("  \t"),
            Err(GateCommandParseError::EmptyCommand)
        );
        assert!(matches!(
            parse_gate_command("uv ''"),
            Err(GateCommandParseError::EmptyWord { .. })
        ));
    }

    #[test]
    fn gate_command_parser_rejects_shell_operators_and_pipelines() {
        assert!(matches!(
            parse_gate_command("uv && build"),
            Err(GateCommandParseError::ShellOperator { syntax, .. }) if syntax == "&"
        ));
        assert!(matches!(
            parse_gate_command("uv | build"),
            Err(GateCommandParseError::ShellOperator { syntax, .. }) if syntax == "|"
        ));
        assert!(matches!(
            parse_gate_command("uv\nbuild"),
            Err(GateCommandParseError::ShellOperator { syntax, .. }) if syntax == "newline"
        ));
    }

    #[test]
    fn gate_command_parser_rejects_redirection() {
        assert!(matches!(
            parse_gate_command("uv build > artifact"),
            Err(GateCommandParseError::Redirection { syntax, .. }) if syntax == ">"
        ));
        assert!(matches!(
            parse_gate_command("uv build 2<artifact"),
            Err(GateCommandParseError::Redirection { syntax, .. }) if syntax == "<"
        ));
    }

    #[test]
    fn gate_command_parser_rejects_substitution_and_expansion() {
        assert!(matches!(
            parse_gate_command("uv `pwd`"),
            Err(GateCommandParseError::CommandSubstitution { .. })
        ));
        assert!(matches!(
            parse_gate_command("uv $(pwd)"),
            Err(GateCommandParseError::CommandSubstitution { .. })
        ));
        assert!(matches!(
            parse_gate_command("uv $HOME"),
            Err(GateCommandParseError::VariableExpansion { .. })
        ));
        assert!(matches!(
            parse_gate_command("uv ${HOME}"),
            Err(GateCommandParseError::VariableExpansion { .. })
        ));
    }

    #[test]
    fn gate_command_parser_rejects_globbing() {
        assert!(matches!(
            parse_gate_command("uv *.rs"),
            Err(GateCommandParseError::Globbing { syntax: '*', .. })
        ));
    }

    #[test]
    fn gate_command_parser_rejects_incomplete_lexical_syntax() {
        assert!(matches!(
            parse_gate_command("uv 'build"),
            Err(GateCommandParseError::UnterminatedQuote { .. })
        ));
        assert!(matches!(
            parse_gate_command("uv build\\"),
            Err(GateCommandParseError::DanglingEscape { .. })
        ));
    }

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

    const NON_EMPTY_SERIALIZATION_FIXTURE: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
    },
    "version_policy": "SERIALIZE_DISPATCHES",
    "branches": {
      "default": "trunk",
      "milestone": "integration/{vision}/{milestone}",
      "step": "work/{vision}/{milestone}/{step}"
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
      "run format before lint"
    ],
    "lockfile_rules": [
      "commit Cargo.lock with dependency changes"
    ]
  }
}"#;

    #[test]
    fn tracked_contract_serializer_is_byte_identical_to_committed_contract() {
        let committed = include_bytes!("../../../.pce/repository-contract.json");
        let parsed =
            parse_tracked_repository_contract(committed).expect("committed contract should parse");
        let serialized = serialize_tracked_repository_contract(&parsed)
            .expect("committed contract should serialize");
        assert_eq!(serialized, committed);
        let reparsed = parse_tracked_repository_contract(&serialized)
            .expect("serialized contract should parse");
        assert_eq!(reparsed, parsed);
    }

    #[test]
    fn tracked_contract_serializer_round_trips_both_authority_halves() {
        let parsed = parse_tracked_repository_contract(NON_EMPTY_SERIALIZATION_FIXTURE)
            .expect("non-empty contract should parse");
        let serialized = serialize_tracked_repository_contract(&parsed)
            .expect("non-empty contract should serialize");
        let reparsed = parse_tracked_repository_contract(&serialized)
            .expect("serialized contract should parse");
        assert_eq!(reparsed, parsed);

        let fixture_value: Value = serde_json::from_slice(NON_EMPTY_SERIALIZATION_FIXTURE)
            .expect("fixture should be JSON");
        let serialized_value: Value =
            serde_json::from_slice(&serialized).expect("serialized contract should be JSON");
        assert_eq!(serialized_value, fixture_value);
        assert!(
            serialized_value["stated"]["workflows"][1]["stand_in"]
                .get("command")
                .is_none()
        );
    }

    fn canonical_value() -> Value {
        serde_json::from_slice(VALID_TRACKED_CONTRACT).expect("canonical fixture should be JSON")
    }

    fn malformed_display(value: &Value) -> String {
        let bytes = serde_json::to_vec(value).expect("mutated fixture should serialize");
        let err = parse_tracked_repository_contract(&bytes)
            .expect_err("mutated fixture should be malformed");
        assert!(matches!(
            err,
            TrackedContractError::MalformedContract { .. }
        ));
        err.to_string()
    }

    #[test]
    fn parses_exact_two_half_tracked_contract() {
        let contract = parse_tracked_repository_contract(VALID_TRACKED_CONTRACT)
            .expect("fixture should parse");
        let stated = contract.stated();
        let gates = stated
            .gates()
            .iter()
            .map(|(kind, command)| (kind, command.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            gates,
            [
                (GateKind::Format, "cargo fmt --check"),
                (GateKind::Lint, "cargo clippy --workspace --all-targets"),
                (GateKind::Typecheck, "cargo check --workspace --all-targets"),
                (GateKind::Test, "cargo test --workspace"),
                (GateKind::Build, "cargo build --release"),
            ]
        );
        assert_eq!(stated.version_policy(), &VersionPolicy::None);
        assert_eq!(stated.branches().default().as_str(), "main");
        assert_eq!(
            stated.branches().milestone().as_str(),
            "pce/{vision}/milestone-{milestone}"
        );
        assert_eq!(
            stated.branches().step().as_str(),
            "pce/{vision}/m{milestone}-s{step}"
        );
        assert_eq!(
            stated.pull_requests().step_base(),
            StepPullRequestBase::Milestone
        );
        assert_eq!(
            stated.pull_requests().milestone_base(),
            MilestonePullRequestBase::Default
        );
        assert_eq!(
            stated.pull_requests().merge_method(),
            PullRequestMergeMethod::Squash
        );
        let workflows = stated.workflows().as_slice();
        assert_eq!(workflows.len(), 2);
        assert_eq!(workflows[0].workflow().as_str(), "ci.yml");
        match workflows[0].stand_in() {
            LocalWorkflowStandIn::Command(command) => {
                assert_eq!(command.as_str(), "cargo test --workspace");
            }
            LocalWorkflowStandIn::None => panic!("ci workflow should have a command"),
        }
        assert_eq!(workflows[1].workflow().as_str(), "release.yml");
        assert!(matches!(
            workflows[1].stand_in(),
            LocalWorkflowStandIn::None
        ));

        let appendable = contract.appendable();
        assert_eq!(appendable.environment_hazards().len(), 1);
        assert_eq!(
            appendable.environment_hazards()[0].as_str(),
            "pipe Codex stdin from /dev/null"
        );
        assert_eq!(appendable.gate_orderings().len(), 1);
        assert_eq!(
            appendable.gate_orderings()[0].as_str(),
            "run cargo fmt --check before clippy"
        );
        assert_eq!(appendable.lockfile_rules().len(), 1);
        assert_eq!(
            appendable.lockfile_rules()[0].as_str(),
            "commit Cargo.lock when dependency resolution changes"
        );
    }

    #[test]
    fn rejects_malformed_unknown_and_incomplete_contracts() {
        let err = parse_tracked_repository_contract(b"{")
            .expect_err("incomplete JSON should be malformed");
        assert!(matches!(
            err,
            TrackedContractError::MalformedContract { .. }
        ));
        assert!(err.to_string().contains(
            "malformed tracked repository contract: EOF while parsing an object at line 1 column 1"
        ));

        let mut top_level = canonical_value();
        top_level["unexpected"] = json!(true);
        assert!(
            malformed_display(&top_level)
                .contains("unknown field `unexpected`, expected `stated` or `appendable`")
        );

        let mut gates = canonical_value();
        gates["stated"]["gates"]["surprise"] = json!(true);
        assert!(malformed_display(&gates).contains(
            "unknown field `surprise`, expected one of `format`, `lint`, `typecheck`, `test`, `build`"
        ));

        let mut missing = canonical_value();
        missing
            .as_object_mut()
            .expect("fixture root should be an object")
            .remove("appendable");
        assert!(malformed_display(&missing).contains("missing field `appendable`"));

        let mut policy = canonical_value();
        policy["stated"]["version_policy"] = json!("PATCH");
        assert!(
            malformed_display(&policy)
                .contains("unknown variant `PATCH`, expected `NONE` or `SERIALIZE_DISPATCHES`")
        );

        let mut shell = canonical_value();
        shell["stated"]["workflows"][0]["stand_in"] =
            json!({"kind": "SHELL", "command": "cargo test"});
        assert!(
            malformed_display(&shell)
                .contains("unknown variant `SHELL`, expected `COMMAND` or `NONE`")
        );

        let mut none_with_command = canonical_value();
        none_with_command["stated"]["workflows"][1]["stand_in"] =
            json!({"kind": "NONE", "command": "cargo test"});
        assert!(
            malformed_display(&none_with_command)
                .contains("unknown field `command`, there are no fields")
        );
    }

    #[test]
    fn rejects_empty_fields_and_duplicate_workflows() {
        let mut empty = canonical_value();
        empty["stated"]["gates"]["test"] = json!("   ");
        let empty_bytes = serde_json::to_vec(&empty).expect("mutated fixture should serialize");
        let err = parse_tracked_repository_contract(&empty_bytes)
            .expect_err("whitespace-only gate should fail");
        assert_eq!(
            err.to_string(),
            "tracked repository contract field stated.gates.test cannot be empty"
        );

        let mut duplicate = canonical_value();
        let duplicate_workflow = duplicate["stated"]["workflows"][0].clone();
        duplicate["stated"]["workflows"]
            .as_array_mut()
            .expect("workflows should be an array")
            .push(duplicate_workflow);
        let duplicate_bytes =
            serde_json::to_vec(&duplicate).expect("mutated fixture should serialize");
        let err = parse_tracked_repository_contract(&duplicate_bytes)
            .expect_err("duplicate workflow should fail");
        assert_eq!(
            err.to_string(),
            "tracked repository contract repeats workflow \"ci.yml\""
        );
    }

    #[test]
    fn stated_and_appendable_halves_have_distinct_type_identity() {
        assert_ne!(
            TypeId::of::<StatedContract>(),
            TypeId::of::<AppendableContract>()
        );
        assert_ne!(
            TypeId::of::<DefaultBranchName>(),
            TypeId::of::<MilestoneBranchPattern>()
        );
        assert_ne!(
            TypeId::of::<DefaultBranchName>(),
            TypeId::of::<StepBranchPattern>()
        );
        assert_ne!(
            TypeId::of::<MilestoneBranchPattern>(),
            TypeId::of::<StepBranchPattern>()
        );
        assert_ne!(
            TypeId::of::<EnvironmentHazard>(),
            TypeId::of::<GateOrdering>()
        );
        assert_ne!(
            TypeId::of::<EnvironmentHazard>(),
            TypeId::of::<LockfileRule>()
        );
        assert_ne!(TypeId::of::<GateOrdering>(), TypeId::of::<LockfileRule>());
    }

    #[test]
    fn appendable_category_accepts_only_the_three_exact_spellings() {
        for (raw, expected) in [
            ("environment-hazard", AppendableCategory::EnvironmentHazard),
            ("gate-ordering", AppendableCategory::GateOrdering),
            ("lockfile-rule", AppendableCategory::LockfileRule),
        ] {
            let parsed = AppendableCategory::parse(raw).expect("exact category should parse");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), raw);
        }

        let err = AppendableCategory::parse("environment_hazards")
            .expect_err("non-canonical category should fail");
        assert_eq!(
            err.to_string(),
            "unknown appendable category \"environment_hazards\"; expected environment-hazard, \
             gate-ordering, or lockfile-rule"
        );
    }

    #[test]
    fn recurrent_finding_requires_byte_exact_text_in_both_run_inputs() {
        let mut contract = parse_tracked_repository_contract(VALID_TRACKED_CONTRACT)
            .expect("fixture should parse");
        let original = contract.clone();
        let finding = AppendableFinding::parse(
            AppendableCategory::LockfileRule,
            "regenerate Cargo.lock".to_owned(),
        )
        .expect("finding should parse");

        assert_eq!(
            admit_recurrent_finding(
                &mut contract,
                finding.clone(),
                &["other"],
                &["regenerate Cargo.lock"]
            ),
            FindingAdmission::CurrentOccurrenceMissing
        );
        assert_eq!(contract, original);
        assert_eq!(
            admit_recurrent_finding(
                &mut contract,
                finding.clone(),
                &["regenerate Cargo.lock"],
                &["Regenerate Cargo.lock"]
            ),
            FindingAdmission::FirstOccurrence
        );
        assert_eq!(contract, original);
        assert_eq!(
            admit_recurrent_finding(
                &mut contract,
                finding.clone(),
                &["regenerate Cargo.lock"],
                &["regenerate Cargo.lock "]
            ),
            FindingAdmission::FirstOccurrence
        );
        assert_eq!(contract, original);

        let composed =
            AppendableFinding::parse(AppendableCategory::EnvironmentHazard, "café".to_owned())
                .expect("Unicode finding should parse");
        assert_eq!(
            admit_recurrent_finding(&mut contract, composed, &["café"], &["cafe\u{301}"]),
            FindingAdmission::FirstOccurrence
        );
        assert_eq!(contract, original);

        assert_eq!(
            admit_recurrent_finding(
                &mut contract,
                finding,
                &["regenerate Cargo.lock"],
                &["regenerate Cargo.lock"]
            ),
            FindingAdmission::Appended
        );
        assert_eq!(contract.appendable().lockfile_rules().len(), 2);
        assert_eq!(
            contract.appendable().lockfile_rules()[1].as_str(),
            "regenerate Cargo.lock"
        );
    }

    #[test]
    fn recurrent_finding_routes_each_category_once() {
        let mut contract = parse_tracked_repository_contract(VALID_TRACKED_CONTRACT)
            .expect("fixture should parse");
        let stated_before =
            serde_json::to_value(&canonical_value()["stated"]).expect("stated should serialize");

        for (category, text) in [
            (AppendableCategory::EnvironmentHazard, "environment two"),
            (AppendableCategory::GateOrdering, "ordering two"),
            (AppendableCategory::LockfileRule, "lockfile two"),
        ] {
            let finding =
                AppendableFinding::parse(category, text.to_owned()).expect("finding should parse");
            assert_eq!(
                admit_recurrent_finding(&mut contract, finding.clone(), &[text], &[text]),
                FindingAdmission::Appended
            );
            assert_eq!(
                admit_recurrent_finding(&mut contract, finding, &[text], &[text]),
                FindingAdmission::AlreadyPresent
            );
        }

        assert_eq!(contract.appendable().environment_hazards().len(), 2);
        assert_eq!(contract.appendable().gate_orderings().len(), 2);
        assert_eq!(contract.appendable().lockfile_rules().len(), 2);
        let serialized = serialize_tracked_repository_contract(&contract)
            .expect("admitted contract should serialize");
        let serialized_value: Value =
            serde_json::from_slice(&serialized).expect("serialized contract should be JSON");
        assert_eq!(serialized_value["stated"], stated_before);
    }
}
