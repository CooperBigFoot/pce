//! run_state : Ordered<EventRecord> × DispatchRequiredArtifactObservation* × AcceptanceCriteria × VisionSlug × RecoveryLogPath × CurrentArtifactObservation* × RepositoryObservation* × StepMergeRouteObservation* → DerivedRunState × Ordered<CriterionExecutionObservation> ∪ RunStateError; snapshot_v1 : DerivedRunState → RunSnapshot; human_status : RunSnapshot → String; compute_dispatchability : ArtifactProvenance × Ordered<DispatchCandidate> × OrderingEdge* × (CanonicalNode → MergeStatus) × (RepositoryName → VersionPolicy) → Ordered<DispatchabilityResult> ∪ RunStateError   (pure, deterministic)
//! This module performs no I/O.

use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

use crate::dispatch_ledger::{DispatchAccounting, fold_dispatch_ledger};
use crate::dispatch_process_identity::AbsoluteRequiredArtifactPath;
use crate::event_log::DispatchCompletionOutcomeRef;
use crate::event_log::{
    ArtifactOutcome, ArtifactPath, ArtifactProduction, ChangeOfCourse, CriterionExecutionOutcome,
    DispatchDuration, DispatchExitStatus, DispatchRef, DispatchRole, DispatchRootCause,
    DispatchTokenUsage, EscalationKey, EventBodyRef, EventRecord, EventTimestamp, Evidence,
    FinishedResult, KnownPayload, NodeId, NonProductionHoldResolution, NonProductionKey,
    ReconciledDispatchOutcome, RepositoryName, RequiredArtifactPresence, Sequence, Sha256Digest,
};
use crate::{AcceptanceCriteria, AcceptanceCriterion};

/// A vision-directory basename suffix with its leading date prefix removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionSlug(String);

impl VisionSlug {
    /// Parse a `VISION_DIR` basename and preserve the suffix after `YYYY-MM-DD-`.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::MalformedVisionBasename`] when the basename lacks the
    /// literal ASCII date-prefix shape, or [`RunStateError::EmptyVisionSlug`] when its
    /// suffix is empty.
    #[instrument(skip(basename))]
    pub fn parse(basename: &str) -> Result<Self, RunStateError> {
        let bytes = basename.as_bytes();
        let date_shaped = bytes.len() >= 11
            && bytes[0..4].iter().all(u8::is_ascii_digit)
            && bytes[4] == b'-'
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[7] == b'-'
            && bytes[8..10].iter().all(u8::is_ascii_digit)
            && bytes[10] == b'-';
        if !date_shaped {
            return Err(RunStateError::MalformedVisionBasename {
                value: basename.to_owned(),
            });
        }

        let Some(suffix) = basename.get(11..) else {
            return Err(RunStateError::MalformedVisionBasename {
                value: basename.to_owned(),
            });
        };
        if suffix.is_empty() {
            return Err(RunStateError::EmptyVisionSlug {
                value: basename.to_owned(),
            });
        }
        Ok(Self(suffix.to_owned()))
    }

    /// Return the preserved vision slug.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The exact event-log path spelling used in recovery commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryLogPath(String);

impl RecoveryLogPath {
    /// Preserve a caller-provided log path without filesystem interpretation.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the exact caller-provided path spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A positive milestone number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MilestoneNumber(u64);

impl MilestoneNumber {
    /// Return the milestone number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A canonical `m<milestone>` event-log node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneNode {
    milestone: MilestoneNumber,
}

impl MilestoneNode {
    /// Parse a canonical milestone node from a general event-log node identifier.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::MalformedMilestoneNode`] for a non-canonical shape,
    /// [`RunStateError::LeadingZeroMilestoneNodeComponent`] for a leading zero,
    /// [`RunStateError::ZeroMilestoneNodeComponent`] for zero, or
    /// [`RunStateError::MilestoneNodeComponentOverflow`] when the component exceeds `u64`.
    #[instrument]
    pub fn parse(node: &NodeId) -> Result<Self, RunStateError> {
        let raw = node.as_str();
        let Some(component) = raw.strip_prefix('m') else {
            return Err(RunStateError::MalformedMilestoneNode {
                value: raw.to_owned(),
            });
        };
        if component.is_empty() || component.bytes().any(|byte| !byte.is_ascii_digit()) {
            return Err(RunStateError::MalformedMilestoneNode {
                value: raw.to_owned(),
            });
        }
        if component.len() > 1 && component.starts_with('0') {
            return Err(RunStateError::LeadingZeroMilestoneNodeComponent {
                value: raw.to_owned(),
            });
        }
        let milestone = component.parse::<u64>().map_err(|_| {
            RunStateError::MilestoneNodeComponentOverflow {
                value: raw.to_owned(),
            }
        })?;
        if milestone == 0 {
            return Err(RunStateError::ZeroMilestoneNodeComponent {
                value: raw.to_owned(),
            });
        }
        Ok(Self {
            milestone: MilestoneNumber(milestone),
        })
    }

    /// Return the milestone number.
    pub const fn milestone(&self) -> MilestoneNumber {
        self.milestone
    }
}

/// A positive step number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepNumber(u64);

impl StepNumber {
    /// Return the step number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A canonical `m<milestone>-s<step>` event-log node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepNode {
    milestone: MilestoneNumber,
    step: StepNumber,
}

impl StepNode {
    /// Parse a canonical step node from a general event-log node identifier.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::MalformedStepNode`] for a non-canonical shape,
    /// [`RunStateError::LeadingZeroStepNodeComponent`] for a leading zero,
    /// [`RunStateError::ZeroStepNodeComponent`] for zero, or
    /// [`RunStateError::StepNodeComponentOverflow`] when a component exceeds `u64`.
    #[instrument]
    pub fn parse(node: &NodeId) -> Result<Self, RunStateError> {
        let raw = node.as_str();
        let Some(rest) = raw.strip_prefix('m') else {
            return Err(RunStateError::MalformedStepNode {
                value: raw.to_owned(),
            });
        };
        let Some((milestone, step)) = rest.split_once("-s") else {
            return Err(RunStateError::MalformedStepNode {
                value: raw.to_owned(),
            });
        };
        if milestone.is_empty()
            || step.is_empty()
            || milestone.bytes().any(|byte| !byte.is_ascii_digit())
            || step.bytes().any(|byte| !byte.is_ascii_digit())
        {
            return Err(RunStateError::MalformedStepNode {
                value: raw.to_owned(),
            });
        }

        let milestone = parse_component(raw, "milestone", milestone)?;
        let step = parse_component(raw, "step", step)?;
        Ok(Self {
            milestone: MilestoneNumber(milestone),
            step: StepNumber(step),
        })
    }

    /// Return the milestone number.
    pub const fn milestone(&self) -> MilestoneNumber {
        self.milestone
    }

    /// Return the step number.
    pub const fn step(&self) -> StepNumber {
        self.step
    }
}

/// Either valid form of canonical event-log node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalNode {
    /// A canonical bare milestone node.
    Milestone(MilestoneNode),
    /// A canonical milestone step node.
    Step(StepNode),
}

/// A typed dependency requiring one canonical node to merge before another dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingEdge {
    dependent: CanonicalNode,
    dependency: CanonicalNode,
}

impl OrderingEdge {
    /// Construct an edge from a dependent node to its required dependency.
    pub const fn new(dependent: CanonicalNode, dependency: CanonicalNode) -> Self {
        Self {
            dependent,
            dependency,
        }
    }

    /// Return the node whose dispatch is constrained.
    pub const fn dependent(&self) -> &CanonicalNode {
        &self.dependent
    }

    /// Return the node that must be conclusively merged.
    pub const fn dependency(&self) -> &CanonicalNode {
        &self.dependency
    }
}

/// One caller-ordered undispatched canonical node and its repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchCandidate {
    node: CanonicalNode,
    repository: RepositoryName,
}

impl DispatchCandidate {
    /// Construct an undispatched candidate in its repository.
    pub const fn new(node: CanonicalNode, repository: RepositoryName) -> Self {
        Self { node, repository }
    }

    /// Return the candidate's canonical node.
    pub const fn node(&self) -> &CanonicalNode {
        &self.node
    }

    /// Return the candidate's repository.
    pub const fn repository(&self) -> &RepositoryName {
        &self.repository
    }
}

/// The live version policy governing dispatch admission in one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionPolicy {
    /// The live `NONE` policy, which does not narrow otherwise-dispatchable candidates.
    #[serde(rename = "NONE")]
    None,
    /// A representative non-`NONE` policy admitting only the first otherwise-dispatchable candidate.
    #[serde(rename = "SERIALIZE_DISPATCHES")]
    SerializeDispatches,
}

/// The three-valued dispatch classification for one complete candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchabilityResult {
    /// The candidate may dispatch now.
    Dispatchable { candidate: DispatchCandidate },
    /// The candidate must wait for a conclusive merge or repository admission.
    Waiting { candidate: DispatchCandidate },
    /// At least one relevant merge status is inconclusive.
    DependencyInconclusive { candidate: DispatchCandidate },
}

impl DispatchabilityResult {
    /// Return the complete classified candidate.
    pub const fn candidate(&self) -> &DispatchCandidate {
        match self {
            Self::Dispatchable { candidate }
            | Self::Waiting { candidate }
            | Self::DependencyInconclusive { candidate } => candidate,
        }
    }
}

fn parse_component(node: &str, component: &'static str, raw: &str) -> Result<u64, RunStateError> {
    if raw.len() > 1 && raw.starts_with('0') {
        return Err(RunStateError::LeadingZeroStepNodeComponent {
            value: node.to_owned(),
            component,
        });
    }
    let value = raw
        .parse::<u64>()
        .map_err(|_| RunStateError::StepNodeComponentOverflow {
            value: node.to_owned(),
            component,
        })?;
    if value == 0 {
        return Err(RunStateError::ZeroStepNodeComponent {
            value: node.to_owned(),
            component,
        });
    }
    Ok(value)
}

/// A derived pull-request head branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadBranch(String);

impl HeadBranch {
    /// Return the complete head branch name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A derived pull-request base branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationBranch(String);

impl IntegrationBranch {
    /// Return the complete integration branch name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A non-blank integration branch named by an exceptional merge-chain declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclaredIntegrationBranch(String);

impl DeclaredIntegrationBranch {
    /// Parse a non-blank declared integration branch without normalization.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::BlankDeclaredIntegrationBranch`] when `raw` is empty or whitespace.
    #[instrument(skip(raw))]
    pub fn parse(raw: &str) -> Result<Self, RunStateError> {
        if raw.trim().is_empty() {
            return Err(RunStateError::BlankDeclaredIntegrationBranch {
                value: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the declared branch unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DeclaredIntegrationBranch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// The exact head/base pair that uniquely selects a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSelector {
    head: HeadBranch,
    base: IntegrationBranch,
}

impl PullRequestSelector {
    /// Return the exact head branch.
    pub const fn head(&self) -> &HeadBranch {
        &self.head
    }

    /// Return the exact base branch.
    pub const fn base(&self) -> &IntegrationBranch {
        &self.base
    }
}

/// A step node and its convention-derived merge identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSubject {
    node: StepNode,
    selector: PullRequestSelector,
}

impl MergeSubject {
    /// Derive the merge subject from a parsed vision slug and canonical step node.
    pub fn derive(vision: &VisionSlug, node: StepNode) -> Self {
        let head = HeadBranch(format!(
            "pce/{}/m{}-s{}",
            vision.as_str(),
            node.milestone().get(),
            node.step().get()
        ));
        let base = IntegrationBranch(format!(
            "pce/{}/milestone-{}",
            vision.as_str(),
            node.milestone().get()
        ));
        Self {
            node,
            selector: PullRequestSelector { head, base },
        }
    }

    /// Return the canonical step node.
    pub const fn node(&self) -> &StepNode {
        &self.node
    }

    /// Return the convention-derived exact pull-request selector.
    pub const fn selector(&self) -> &PullRequestSelector {
        &self.selector
    }

    /// Return the derived head branch.
    pub const fn head(&self) -> &HeadBranch {
        self.selector.head()
    }

    /// Return the derived integration branch.
    pub const fn integration_branch(&self) -> &IntegrationBranch {
        self.selector.base()
    }
}

/// A milestone node and its convention-derived merge identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneMergeSubject {
    node: MilestoneNode,
    selector: PullRequestSelector,
}

impl MilestoneMergeSubject {
    /// Derive the merge subject from a parsed vision slug and canonical milestone node.
    pub fn derive(vision: &VisionSlug, node: MilestoneNode) -> Self {
        let head = HeadBranch(format!(
            "pce/{}/milestone-{}",
            vision.as_str(),
            node.milestone().get()
        ));
        let base = IntegrationBranch("main".to_owned());
        Self {
            node,
            selector: PullRequestSelector { head, base },
        }
    }

    /// Return the canonical milestone node.
    pub const fn node(&self) -> &MilestoneNode {
        &self.node
    }

    /// Return the convention-derived exact pull-request selector.
    pub const fn selector(&self) -> &PullRequestSelector {
        &self.selector
    }

    /// Return the derived head branch.
    pub const fn head(&self) -> &HeadBranch {
        self.selector.head()
    }

    /// Return the derived base branch.
    pub const fn base(&self) -> &IntegrationBranch {
        self.selector.base()
    }
}

/// A positive GitHub pull-request number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PullRequestNumber(u64);

impl PullRequestNumber {
    /// Parse a positive GitHub pull-request number.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::ZeroPullRequestNumber`] when `value` is zero.
    #[instrument]
    pub fn parse(value: u64) -> Result<Self, RunStateError> {
        if value == 0 {
            return Err(RunStateError::ZeroPullRequestNumber { value });
        }
        Ok(Self(value))
    }

    /// Return the pull-request number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for PullRequestNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::parse(u64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The declared selector authority for one exceptional two-hop step merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionalMergeChain {
    node: StepNode,
    subject: MergeSubject,
    integration_branch: DeclaredIntegrationBranch,
    step_pull_request: ExactPullRequestIdentity,
    promotion_pull_request: ExactPullRequestIdentity,
}

impl ExceptionalMergeChain {
    /// Construct the two exact declared identities while deriving the step head and fixing `main`.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::EqualExceptionalMergeChainPullRequestNumbers`] when both hops name
    /// the same pull request.
    pub fn new(
        vision: &VisionSlug,
        node: StepNode,
        integration_branch: DeclaredIntegrationBranch,
        step_pull_request_number: PullRequestNumber,
        promotion_pull_request_number: PullRequestNumber,
    ) -> Result<Self, RunStateError> {
        if step_pull_request_number == promotion_pull_request_number {
            return Err(
                RunStateError::EqualExceptionalMergeChainPullRequestNumbers {
                    step: step_pull_request_number.get(),
                    promotion: promotion_pull_request_number.get(),
                },
            );
        }
        let derived = MergeSubject::derive(vision, node.clone());
        let step_selector = PullRequestSelector {
            head: derived.head().clone(),
            base: IntegrationBranch(integration_branch.as_str().to_owned()),
        };
        let subject = MergeSubject {
            node: node.clone(),
            selector: step_selector.clone(),
        };
        let promotion_selector = PullRequestSelector {
            head: HeadBranch(integration_branch.as_str().to_owned()),
            base: IntegrationBranch("main".to_owned()),
        };
        Ok(Self {
            node,
            subject,
            integration_branch,
            step_pull_request: ExactPullRequestIdentity::from_selector(
                step_pull_request_number,
                &step_selector,
            ),
            promotion_pull_request: ExactPullRequestIdentity::from_selector(
                promotion_pull_request_number,
                &promotion_selector,
            ),
        })
    }

    pub const fn node(&self) -> &StepNode {
        &self.node
    }
    pub const fn subject(&self) -> &MergeSubject {
        &self.subject
    }
    pub const fn integration_branch(&self) -> &DeclaredIntegrationBranch {
        &self.integration_branch
    }
    pub const fn step_pull_request(&self) -> &ExactPullRequestIdentity {
        &self.step_pull_request
    }
    pub const fn promotion_pull_request(&self) -> &ExactPullRequestIdentity {
        &self.promotion_pull_request
    }
}

/// The selected merge-proof route for one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepMergeRoute {
    /// Use the convention-derived single-hop selector.
    Derived,
    /// Use the declared two-hop selector chain.
    Exceptional(ExceptionalMergeChain),
}

/// Both fresh authority observations for one pull-request hop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestAuthorityObservation {
    github: GitHubAuthorityObservation,
    git: GitAuthorityObservation,
}

impl PullRequestAuthorityObservation {
    pub const fn new(github: GitHubAuthorityObservation, git: GitAuthorityObservation) -> Self {
        Self { github, git }
    }
    pub const fn github(&self) -> &GitHubAuthorityObservation {
        &self.github
    }
    pub const fn git(&self) -> &GitAuthorityObservation {
        &self.git
    }
}

/// The two independently observed hops required by an exceptional merge chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionalMergeChainObservation {
    step_to_integration: PullRequestAuthorityObservation,
    integration_to_default: PullRequestAuthorityObservation,
}

impl ExceptionalMergeChainObservation {
    pub const fn new(
        step_to_integration: PullRequestAuthorityObservation,
        integration_to_default: PullRequestAuthorityObservation,
    ) -> Self {
        Self {
            step_to_integration,
            integration_to_default,
        }
    }
    pub const fn step_to_integration(&self) -> &PullRequestAuthorityObservation {
        &self.step_to_integration
    }
    pub const fn integration_to_default(&self) -> &PullRequestAuthorityObservation {
        &self.integration_to_default
    }
}

/// A unique pull request carrying the exact selector used to find it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactPullRequestIdentity {
    number: PullRequestNumber,
    selector: PullRequestSelector,
}

impl ExactPullRequestIdentity {
    /// Construct an exact identity from an already-derived selector.
    pub fn from_selector(number: PullRequestNumber, selector: &PullRequestSelector) -> Self {
        Self {
            number,
            selector: selector.clone(),
        }
    }

    /// Return the GitHub pull-request number.
    pub const fn number(&self) -> PullRequestNumber {
        self.number
    }

    /// Return the exact selector used to find this pull request.
    pub const fn selector(&self) -> &PullRequestSelector {
        &self.selector
    }
}

/// A non-empty, byte-preserving GitHub `mergeCommit.oid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquashCommitOid(String);

impl SquashCommitOid {
    /// Parse a non-empty squash commit OID without normalization.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::EmptySquashCommitOid`] when `raw` has zero bytes.
    #[instrument(skip(raw))]
    pub fn parse(raw: &str) -> Result<Self, RunStateError> {
        if raw.is_empty() {
            return Err(RunStateError::EmptySquashCommitOid {
                value: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the squash commit OID unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-empty, byte-preserving diagnostic detail for an unreachable authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityFailure(String);

impl AuthorityFailure {
    /// Parse non-empty authority failure detail without normalization.
    ///
    /// # Errors
    ///
    /// Returns [`RunStateError::EmptyAuthorityFailure`] when `raw` has zero bytes.
    #[instrument(skip(raw))]
    pub fn parse(raw: &str) -> Result<Self, RunStateError> {
        if raw.is_empty() {
            return Err(RunStateError::EmptyAuthorityFailure {
                value: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the authority failure detail unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The merge state reported for one unique exact pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactPullRequestState {
    /// GitHub reports the exact pull request as not merged.
    NotMerged,
    /// GitHub reports the exact pull request merged with this squash commit.
    Merged { squash_commit: SquashCommitOid },
}

/// The cardinality and state of a reachable exact GitHub PR lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubPullRequestObservation {
    /// No pull request has the exact head/base pair.
    ZeroExactMatches,
    /// Exactly one pull request has the exact head/base pair.
    OneExactMatch {
        identity: ExactPullRequestIdentity,
        state: ExactPullRequestState,
    },
    /// More than one pull request has the exact head/base pair.
    MultipleExactMatches,
}

/// Availability and result of the GitHub authority lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubAuthorityObservation {
    /// GitHub could not be reached or queried reliably.
    Unreachable { failure: AuthorityFailure },
    /// GitHub returned a complete exact-selector lookup result.
    Reachable {
        observation: GitHubPullRequestObservation,
    },
}

/// The merge observation made from the fetched integration branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitMergeObservation {
    /// Git reports no relevant squash commit reachable from the integration branch.
    NotMerged,
    /// Git reports this exact squash commit reachable from the integration branch.
    SquashCommitReachable { squash_commit: SquashCommitOid },
}

/// Availability and result of the git authority observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAuthorityObservation {
    /// Git could not provide a reliable reachability observation.
    Unreachable { failure: AuthorityFailure },
    /// Git returned a complete observation from the fetched integration branch.
    Reachable { observation: GitMergeObservation },
}

/// Three-valued merge status derived from both authorities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStatus {
    /// Both authorities prove the exact pull request's squash commit is merged.
    Merged,
    /// Both authorities report that the exact pull request is not merged.
    NotMerged,
    /// An authority is unreachable, authorities disagree, or identity is ambiguous.
    Inconclusive,
}

/// Derive three-valued merge status from the exact subject and both authorities.
pub fn derive_merge_status(
    subject: &MergeSubject,
    github: &GitHubAuthorityObservation,
    git: &GitAuthorityObservation,
) -> MergeStatus {
    derive_merge_status_for_selector(
        ExpectedPullRequestIdentity::Selector(subject.selector()),
        github,
        git,
    )
}

/// Derive three-valued merge status from the exact milestone subject and both authorities.
pub fn derive_milestone_merge_status(
    subject: &MilestoneMergeSubject,
    github: &GitHubAuthorityObservation,
    git: &GitAuthorityObservation,
) -> MergeStatus {
    derive_merge_status_for_selector(
        ExpectedPullRequestIdentity::Selector(subject.selector()),
        github,
        git,
    )
}

#[derive(Debug, Clone, Copy)]
enum ExpectedPullRequestIdentity<'a> {
    Selector(&'a PullRequestSelector),
    Declared(&'a ExactPullRequestIdentity),
}

impl ExpectedPullRequestIdentity<'_> {
    fn matches(self, actual: &ExactPullRequestIdentity) -> bool {
        match self {
            Self::Selector(selector) => actual.selector() == selector,
            Self::Declared(identity) => actual == identity,
        }
    }
}

fn derive_merge_status_for_selector(
    expected: ExpectedPullRequestIdentity<'_>,
    github: &GitHubAuthorityObservation,
    git: &GitAuthorityObservation,
) -> MergeStatus {
    use GitAuthorityObservation::{Reachable as GitReachable, Unreachable as GitUnreachable};
    use GitHubAuthorityObservation::{Reachable as GhReachable, Unreachable as GhUnreachable};
    use GitHubPullRequestObservation::{MultipleExactMatches, OneExactMatch, ZeroExactMatches};

    match (github, git) {
        (GhUnreachable { .. }, GitUnreachable { .. })
        | (
            GhUnreachable { .. },
            GitReachable {
                observation: GitMergeObservation::NotMerged,
            },
        )
        | (
            GhUnreachable { .. },
            GitReachable {
                observation: GitMergeObservation::SquashCommitReachable { .. },
            },
        )
        | (
            GhReachable {
                observation: ZeroExactMatches,
            },
            GitUnreachable { .. },
        )
        | (
            GhReachable {
                observation: ZeroExactMatches,
            },
            GitReachable {
                observation: GitMergeObservation::SquashCommitReachable { .. },
            },
        )
        | (
            GhReachable {
                observation: MultipleExactMatches,
            },
            GitUnreachable { .. },
        )
        | (
            GhReachable {
                observation: MultipleExactMatches,
            },
            GitReachable {
                observation: GitMergeObservation::NotMerged,
            },
        )
        | (
            GhReachable {
                observation: MultipleExactMatches,
            },
            GitReachable {
                observation: GitMergeObservation::SquashCommitReachable { .. },
            },
        )
        | (
            GhReachable {
                observation: OneExactMatch { .. },
            },
            GitUnreachable { .. },
        )
        | (
            GhReachable {
                observation:
                    OneExactMatch {
                        state: ExactPullRequestState::NotMerged,
                        ..
                    },
            },
            GitReachable {
                observation: GitMergeObservation::SquashCommitReachable { .. },
            },
        )
        | (
            GhReachable {
                observation:
                    OneExactMatch {
                        state: ExactPullRequestState::Merged { .. },
                        ..
                    },
            },
            GitReachable {
                observation: GitMergeObservation::NotMerged,
            },
        ) => MergeStatus::Inconclusive,
        (
            GhReachable {
                observation: ZeroExactMatches,
            },
            GitReachable {
                observation: GitMergeObservation::NotMerged,
            },
        ) => MergeStatus::NotMerged,
        (
            GhReachable {
                observation:
                    OneExactMatch {
                        identity,
                        state: ExactPullRequestState::NotMerged,
                    },
            },
            GitReachable {
                observation: GitMergeObservation::NotMerged,
            },
        ) => {
            if expected.matches(identity) {
                MergeStatus::NotMerged
            } else {
                MergeStatus::Inconclusive
            }
        }
        (
            GhReachable {
                observation:
                    OneExactMatch {
                        identity,
                        state:
                            ExactPullRequestState::Merged {
                                squash_commit: github_oid,
                            },
                    },
            },
            GitReachable {
                observation:
                    GitMergeObservation::SquashCommitReachable {
                        squash_commit: git_oid,
                    },
            },
        ) => {
            if expected.matches(identity) && github_oid == git_oid {
                MergeStatus::Merged
            } else {
                MergeStatus::Inconclusive
            }
        }
    }
}

fn derive_exceptional_merge_status(
    chain: &ExceptionalMergeChain,
    observation: &ExceptionalMergeChainObservation,
) -> (MergeStatus, MergeStatus, MergeStatus) {
    let step = derive_merge_status_for_selector(
        ExpectedPullRequestIdentity::Declared(chain.step_pull_request()),
        observation.step_to_integration().github(),
        observation.step_to_integration().git(),
    );
    let promotion = derive_merge_status_for_selector(
        ExpectedPullRequestIdentity::Declared(chain.promotion_pull_request()),
        observation.integration_to_default().github(),
        observation.integration_to_default().git(),
    );
    let aggregate = combine_exceptional_merge_statuses(step, promotion);
    (step, promotion, aggregate)
}

fn combine_exceptional_merge_statuses(step: MergeStatus, promotion: MergeStatus) -> MergeStatus {
    match (step, promotion) {
        (MergeStatus::Merged, MergeStatus::Merged) => MergeStatus::Merged,
        (MergeStatus::Inconclusive, _) | (_, MergeStatus::Inconclusive) => {
            MergeStatus::Inconclusive
        }
        _ => MergeStatus::NotMerged,
    }
}

macro_rules! repository_string_type {
    ($name:ident, $domain:literal, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Parse a non-empty value without normalization.
            ///
            /// # Errors
            ///
            /// Returns [`RunStateError::EmptyRepositoryObservationValue`] when `raw` is empty.
            #[instrument(skip(raw))]
            pub fn parse(raw: &str) -> Result<Self, RunStateError> {
                if raw.is_empty() {
                    return Err(RunStateError::EmptyRepositoryObservationValue {
                        domain: $domain,
                        value: raw.to_owned(),
                    });
                }
                Ok(Self(raw.to_owned()))
            }

            /// Return the stored value unchanged.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

repository_string_type!(
    RepositoryObservationRef,
    "repository observation ref",
    "The exact fetched repository ref supporting an observation."
);
repository_string_type!(
    RepositoryObservationFailure,
    "repository observation failure",
    "Diagnostic detail explaining why a repository observation was unavailable."
);
repository_string_type!(
    RepositoryBranchName,
    "repository branch name",
    "A repository branch whose existence was observed."
);
repository_string_type!(
    WorktreeIdentity,
    "worktree identity",
    "A repository worktree whose existence was observed."
);
repository_string_type!(
    TagName,
    "tag name",
    "A repository tag whose target was observed."
);
repository_string_type!(
    TagTarget,
    "tag target",
    "The exact target of an observed repository tag."
);

/// Fetch metadata supporting one repository observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryFetchObservation {
    /// The repository was observed at this exact ref and supplied fetch time.
    Observed {
        observation_ref: RepositoryObservationRef,
        fetched_at: EventTimestamp,
    },
    /// A reliable repository observation was unavailable.
    Unavailable {
        failure: RepositoryObservationFailure,
    },
}

/// Whether a named repository branch exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchState {
    /// The branch exists.
    Present,
    /// The branch does not exist.
    Absent,
}

/// Whether a named worktree exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeState {
    /// The worktree exists.
    Present,
    /// The worktree does not exist.
    Absent,
}

/// The observed state of a named tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagState {
    /// The tag does not exist.
    Absent,
    /// The tag points to the exact supplied target.
    PointsTo { target: TagTarget },
}

/// A typed, caller-ordered observation of one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryObservation {
    repository: RepositoryName,
    fetch: RepositoryFetchObservation,
    branch_name: RepositoryBranchName,
    branch_state: BranchState,
    worktree: WorktreeIdentity,
    worktree_state: WorktreeState,
    tag_name: TagName,
    tag_state: TagState,
}

impl RepositoryObservation {
    /// Construct one complete typed repository observation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repository: RepositoryName,
        fetch: RepositoryFetchObservation,
        branch_name: RepositoryBranchName,
        branch_state: BranchState,
        worktree: WorktreeIdentity,
        worktree_state: WorktreeState,
        tag_name: TagName,
        tag_state: TagState,
    ) -> Self {
        Self {
            repository,
            fetch,
            branch_name,
            branch_state,
            worktree,
            worktree_state,
            tag_name,
            tag_state,
        }
    }

    /// Return the exact repository name.
    pub const fn repository(&self) -> &RepositoryName {
        &self.repository
    }

    /// Return the fetch metadata.
    pub const fn fetch(&self) -> &RepositoryFetchObservation {
        &self.fetch
    }

    /// Return the observed branch name.
    pub const fn branch_name(&self) -> &RepositoryBranchName {
        &self.branch_name
    }

    /// Return the observed branch state.
    pub const fn branch_state(&self) -> BranchState {
        self.branch_state
    }

    /// Return the observed worktree identity.
    pub const fn worktree(&self) -> &WorktreeIdentity {
        &self.worktree
    }

    /// Return the observed worktree state.
    pub const fn worktree_state(&self) -> WorktreeState {
        self.worktree_state
    }

    /// Return the observed tag name.
    pub const fn tag_name(&self) -> &TagName {
        &self.tag_name
    }

    /// Return the observed tag state.
    pub const fn tag_state(&self) -> &TagState {
        &self.tag_state
    }
}

/// The supplied current state of one approved artifact path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentArtifactState {
    /// The artifact is present with this typed digest.
    Present { digest: Sha256Digest },
    /// The artifact is missing.
    Missing,
}

/// A typed current observation for one artifact path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentArtifactObservation {
    path: ArtifactPath,
    state: CurrentArtifactState,
}

impl CurrentArtifactObservation {
    /// Construct a current artifact observation.
    pub fn new(path: ArtifactPath, state: CurrentArtifactState) -> Self {
        Self { path, state }
    }

    /// Return the exact artifact path.
    pub const fn path(&self) -> &ArtifactPath {
        &self.path
    }

    /// Return the current artifact state.
    pub const fn state(&self) -> &CurrentArtifactState {
        &self.state
    }
}

/// Typed merge-authority observations for one event-log step node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepAuthorityObservation {
    node: NodeId,
    github: GitHubAuthorityObservation,
    git: GitAuthorityObservation,
}

impl StepAuthorityObservation {
    /// Construct one step's typed authority observations.
    pub fn new(
        node: NodeId,
        github: GitHubAuthorityObservation,
        git: GitAuthorityObservation,
    ) -> Self {
        Self { node, github, git }
    }

    /// Return the exact event-log node.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the GitHub authority observation.
    pub const fn github(&self) -> &GitHubAuthorityObservation {
        &self.github
    }

    /// Return the git authority observation.
    pub const fn git(&self) -> &GitAuthorityObservation {
        &self.git
    }
}

/// The exact round-bearing classification of a dispatch role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchRoleClass {
    /// A role producing a plan artifact.
    PlanProducing,
    /// A role producing a critique artifact, including PR review.
    CritiqueProducing,
    /// The step execution role.
    Execution,
    /// The repository analyst, explicitly outside round series.
    ExplicitlyNonRoundBearing,
    /// Any other exact spelling, also outside round series.
    Unrecognized,
}

impl DispatchRoleClass {
    /// Classify the exact, byte-preserved role spelling.
    pub fn classify(role: &DispatchRole) -> Self {
        match role.as_str() {
            "milestone-planner" | "step-planner" | "step-plan-writer" => Self::PlanProducing,
            "milestone-critic"
            | "step-critic"
            | "step-plan-critic"
            | "falsification-critic"
            | "pr-reviewer" => Self::CritiqueProducing,
            "step-executor" => Self::Execution,
            "repository-analyst" => Self::ExplicitlyNonRoundBearing,
            _ => Self::Unrecognized,
        }
    }

    fn round_classification(self) -> Option<RoundClassification> {
        match self {
            Self::PlanProducing => Some(RoundClassification::PlanProducing),
            Self::CritiqueProducing => Some(RoundClassification::CritiqueProducing),
            Self::Execution => Some(RoundClassification::Execution),
            Self::ExplicitlyNonRoundBearing | Self::Unrecognized => None,
        }
    }
}

/// A classification whose carrier contains only round-bearing roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoundClassification {
    /// A role producing a plan artifact.
    PlanProducing,
    /// A role producing a critique artifact.
    CritiqueProducing,
    /// The step execution role.
    Execution,
}

impl From<RoundClassification> for DispatchRoleClass {
    fn from(value: RoundClassification) -> Self {
        match value {
            RoundClassification::PlanProducing => Self::PlanProducing,
            RoundClassification::CritiqueProducing => Self::CritiqueProducing,
            RoundClassification::Execution => Self::Execution,
        }
    }
}

/// One exact dispatch record retained in sequence order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchObservation {
    sequence: Sequence,
    node: NodeId,
    role: DispatchRole,
    dispatch_ref: DispatchRef,
}

/// One completion correlated to its exact earlier issuance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchLifecycleObservation {
    ObservedChild(ObservedDispatchLifecycleObservation),
    ReconciledDead(ReconciledDeadDispatchLifecycleObservation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDispatchLifecycleObservation {
    completion_sequence: Sequence,
    completion_timestamp: EventTimestamp,
    issuance: DispatchObservation,
    duration: DispatchDuration,
    usage: DispatchTokenUsage,
    exit_status: DispatchExitStatus,
    artifact_outcome: ArtifactOutcome,
    required_artifact_presence: Option<RequiredArtifactPresence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciledDeadDispatchLifecycleObservation {
    completion_sequence: Sequence,
    completion_timestamp: EventTimestamp,
    issuance: DispatchObservation,
    outcome: ReconciledDispatchOutcome,
    artifact_production: ArtifactProduction,
}

impl DispatchLifecycleObservation {
    pub const fn completion_sequence(&self) -> Sequence {
        match self {
            Self::ObservedChild(value) => value.completion_sequence,
            Self::ReconciledDead(value) => value.completion_sequence,
        }
    }
    pub const fn completion_timestamp(&self) -> EventTimestamp {
        match self {
            Self::ObservedChild(value) => value.completion_timestamp,
            Self::ReconciledDead(value) => value.completion_timestamp,
        }
    }
    pub const fn issuance(&self) -> &DispatchObservation {
        match self {
            Self::ObservedChild(value) => &value.issuance,
            Self::ReconciledDead(value) => &value.issuance,
        }
    }
}

impl ObservedDispatchLifecycleObservation {
    pub const fn completion_sequence(&self) -> Sequence {
        self.completion_sequence
    }
    pub const fn completion_timestamp(&self) -> EventTimestamp {
        self.completion_timestamp
    }
    pub const fn issuance(&self) -> &DispatchObservation {
        &self.issuance
    }
    pub const fn duration(&self) -> DispatchDuration {
        self.duration
    }
    pub const fn usage(&self) -> &DispatchTokenUsage {
        &self.usage
    }
    pub const fn exit_status(&self) -> DispatchExitStatus {
        self.exit_status
    }
    pub const fn artifact_outcome(&self) -> ArtifactOutcome {
        self.artifact_outcome
    }
    pub const fn required_artifact_presence(&self) -> Option<RequiredArtifactPresence> {
        self.required_artifact_presence
    }
}

impl ReconciledDeadDispatchLifecycleObservation {
    pub const fn outcome(&self) -> ReconciledDispatchOutcome {
        self.outcome
    }
    pub const fn artifact_production(&self) -> ArtifactProduction {
        self.artifact_production
    }
}

impl DispatchObservation {
    /// Return the dispatch sequence.
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// Return the exact dispatch node.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the exact dispatch role.
    pub const fn role(&self) -> &DispatchRole {
        &self.role
    }

    /// Return the exact dispatch ref.
    pub const fn dispatch_ref(&self) -> &DispatchRef {
        &self.dispatch_ref
    }
}

/// A checked issuance ordinal for one exact `(node, role)` series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssuanceOrdinal(u64);

impl IssuanceOrdinal {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuanceOrdinalSeries {
    node: NodeId,
    role: DispatchRole,
    ordinal: IssuanceOrdinal,
}

impl IssuanceOrdinalSeries {
    pub const fn node(&self) -> &NodeId {
        &self.node
    }
    pub const fn role(&self) -> &DispatchRole {
        &self.role
    }
    pub const fn ordinal(&self) -> IssuanceOrdinal {
        self.ordinal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedProductionCount(u64);

impl ValidatedProductionCount {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProductionSeries {
    node: NodeId,
    role: DispatchRole,
    count: ValidatedProductionCount,
}

impl ValidatedProductionSeries {
    pub const fn node(&self) -> &NodeId {
        &self.node
    }
    pub const fn role(&self) -> &DispatchRole {
        &self.role
    }
    pub const fn count(&self) -> ValidatedProductionCount {
        self.count
    }
}

/// Compatibility name for replays and callers compiled against the former terminology.
pub type DefectRoundCount = ValidatedProductionCount;
/// Compatibility name for replays and callers compiled against the former terminology.
pub type DefectRoundSeries = ValidatedProductionSeries;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsecutiveNonProduction(u64);
impl ConsecutiveNonProduction {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonProductionSeries {
    key: NonProductionKey,
    consecutive: ConsecutiveNonProduction,
}
impl NonProductionSeries {
    pub const fn key(&self) -> &NonProductionKey {
        &self.key
    }
    pub const fn consecutive(&self) -> ConsecutiveNonProduction {
        self.consecutive
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonProductionHoldStatus {
    Open {
        sequence: Sequence,
    },
    Closed {
        sequence: Sequence,
        resolution: NonProductionHoldResolution,
        /// Issuance ordinal at authorization time, used to consume one retry.
        issuance_ordinal: IssuanceOrdinal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonProductionHoldObservation {
    key: NonProductionKey,
    status: NonProductionHoldStatus,
}
impl NonProductionHoldObservation {
    pub const fn key(&self) -> &NonProductionKey {
        &self.key
    }
    pub const fn status(&self) -> &NonProductionHoldStatus {
        &self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequiredArtifactObservation {
    issuance_sequence: Sequence,
    required_artifact_path: AbsoluteRequiredArtifactPath,
}
impl DispatchRequiredArtifactObservation {
    pub fn new(
        issuance_sequence: Sequence,
        required_artifact_path: AbsoluteRequiredArtifactPath,
    ) -> Self {
        Self {
            issuance_sequence,
            required_artifact_path,
        }
    }
    pub const fn issuance_sequence(&self) -> Sequence {
        self.issuance_sequence
    }
    pub const fn required_artifact_path(&self) -> &AbsoluteRequiredArtifactPath {
        &self.required_artifact_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchOutcomeState {
    issuance_ordinals: Vec<IssuanceOrdinalSeries>,
    rounds: Vec<ValidatedProductionSeries>,
    non_production_streaks: Vec<NonProductionSeries>,
    non_production_holds: Vec<NonProductionHoldObservation>,
}
impl DispatchOutcomeState {
    pub fn issuance_ordinals(&self) -> &[IssuanceOrdinalSeries] {
        &self.issuance_ordinals
    }
    /// Return validated production counted against the spending limit.
    pub fn validated_production_counts(&self) -> &[ValidatedProductionSeries] {
        &self.rounds
    }

    /// Return validated production under the legacy accessor name.
    pub fn rounds(&self) -> &[ValidatedProductionSeries] {
        self.validated_production_counts()
    }
    pub fn non_production_streaks(&self) -> &[NonProductionSeries] {
        &self.non_production_streaks
    }
    pub fn non_production_holds(&self) -> &[NonProductionHoldObservation] {
        &self.non_production_holds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchAdmission {
    Admit,
    DefectRoundCapExhausted {
        count: ValidatedProductionCount,
    },
    OpenNonProductionHold {
        consecutive: ConsecutiveNonProduction,
    },
    NonProductionHoldOpen,
    NonProductionResolutionClosesAdmission {
        resolution: NonProductionHoldResolution,
    },
}

/// Latest ordered escalation state for one exact key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldStatus {
    /// The latest record for the key is an open.
    Open {
        node: NodeId,
        sequence: Sequence,
        question: String,
    },
    /// The latest record for the key is a close.
    Closed {
        node: NodeId,
        sequence: Sequence,
        resolution: String,
    },
}

/// A first-seen-ordered escalation key and its latest record-derived state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldObservation {
    key: EscalationKey,
    status: HoldStatus,
}

impl HoldObservation {
    /// Return the exact escalation key.
    pub const fn key(&self) -> &EscalationKey {
        &self.key
    }

    /// Return the latest ordered status.
    pub const fn status(&self) -> &HoldStatus {
        &self.status
    }
}

/// The provenance relationship between an approval and current artifact state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactProvenanceCondition {
    /// The current digest exactly matches the approved digest.
    DigestMatches,
    /// The current digest differs from the approved digest.
    DigestMismatch {
        approved: Sha256Digest,
        current: Sha256Digest,
    },
    /// The approved artifact is currently missing.
    ArtifactMissing,
}

/// Current provenance for one latest approved artifact path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactProvenance {
    path: ArtifactPath,
    approved_digest: Sha256Digest,
    approval_node: NodeId,
    approval_sequence: Sequence,
    condition: ArtifactProvenanceCondition,
}

impl ArtifactProvenance {
    /// Return the exact approved artifact path.
    pub const fn path(&self) -> &ArtifactPath {
        &self.path
    }

    /// Return the latest approved digest.
    pub const fn approved_digest(&self) -> &Sha256Digest {
        &self.approved_digest
    }

    /// Return the node recording the latest approval.
    pub const fn approval_node(&self) -> &NodeId {
        &self.approval_node
    }

    /// Return the latest approval sequence.
    pub const fn approval_sequence(&self) -> Sequence {
        self.approval_sequence
    }

    /// Return the independent current provenance condition.
    pub const fn condition(&self) -> &ArtifactProvenanceCondition {
        &self.condition
    }
}

/// Classify every supplied undispatched candidate while preserving caller order.
///
/// The composition root must filter dispatch history and supply only undispatched
/// candidates. The merge-status arrow must be an exhaustive lookup over every
/// candidate and every ordering-edge endpoint. The version-policy arrow must be
/// an exhaustive lookup over every candidate repository. Both lookups are
/// explicit inputs, not hidden global state.
///
/// # Errors
///
/// Returns [`RunStateError::PlanningArtifactDigestMismatch`] or
/// [`RunStateError::PlanningArtifactMissing`] when selected-artifact provenance
/// is not current. Returns malformed-input errors when candidates repeat, either
/// lookup contains duplicate keys, or either lookup omits a required key.
#[instrument(skip(
    provenance,
    candidates,
    ordering_edges,
    merge_statuses,
    version_policies
))]
pub fn compute_dispatchability(
    provenance: &ArtifactProvenance,
    candidates: &[DispatchCandidate],
    ordering_edges: &[OrderingEdge],
    merge_statuses: &[(CanonicalNode, MergeStatus)],
    version_policies: &[(RepositoryName, VersionPolicy)],
) -> Result<Vec<DispatchabilityResult>, RunStateError> {
    match provenance.condition() {
        ArtifactProvenanceCondition::DigestMatches => {}
        ArtifactProvenanceCondition::DigestMismatch { current, .. } => {
            return Err(RunStateError::PlanningArtifactDigestMismatch {
                path: provenance.path().clone(),
                approved_digest: provenance.approved_digest().clone(),
                current_digest: current.clone(),
            });
        }
        ArtifactProvenanceCondition::ArtifactMissing => {
            return Err(RunStateError::PlanningArtifactMissing {
                path: provenance.path().clone(),
                approved_digest: provenance.approved_digest().clone(),
                current_digest: None,
            });
        }
    }

    for (index, candidate) in candidates.iter().enumerate() {
        if candidates[..index]
            .iter()
            .any(|earlier| earlier.node() == candidate.node())
        {
            return Err(RunStateError::DuplicateDispatchCandidate {
                node: candidate.node().clone(),
            });
        }
    }
    for (index, (node, _)) in merge_statuses.iter().enumerate() {
        if merge_statuses[..index]
            .iter()
            .any(|(earlier, _)| earlier == node)
        {
            return Err(RunStateError::DuplicateMergeStatus { node: node.clone() });
        }
    }
    for (index, (repository, _)) in version_policies.iter().enumerate() {
        if version_policies[..index]
            .iter()
            .any(|(earlier, _)| earlier == repository)
        {
            return Err(RunStateError::DuplicateVersionPolicy {
                repository: repository.clone(),
            });
        }
    }

    let status_for = |node: &CanonicalNode| {
        merge_statuses
            .iter()
            .find(|(candidate, _)| candidate == node)
            .map(|(_, status)| status)
            .ok_or_else(|| RunStateError::MissingMergeStatus { node: node.clone() })
    };
    for candidate in candidates {
        status_for(candidate.node())?;
    }
    for edge in ordering_edges {
        status_for(edge.dependent())?;
        status_for(edge.dependency())?;
    }

    let policy_for = |repository: &RepositoryName| {
        version_policies
            .iter()
            .find(|(candidate, _)| candidate == repository)
            .map(|(_, policy)| policy)
            .ok_or_else(|| RunStateError::MissingVersionPolicy {
                repository: repository.clone(),
            })
    };
    for candidate in candidates {
        policy_for(candidate.repository())?;
    }

    let mut results = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let result = match status_for(candidate.node())? {
            MergeStatus::Merged => DispatchabilityResult::Waiting {
                candidate: candidate.clone(),
            },
            MergeStatus::Inconclusive => DispatchabilityResult::DependencyInconclusive {
                candidate: candidate.clone(),
            },
            MergeStatus::NotMerged => {
                let mut any_not_merged = false;
                let mut any_inconclusive = false;
                for edge in ordering_edges
                    .iter()
                    .filter(|edge| edge.dependent() == candidate.node())
                {
                    match status_for(edge.dependency())? {
                        MergeStatus::Merged => {}
                        MergeStatus::NotMerged => any_not_merged = true,
                        MergeStatus::Inconclusive => any_inconclusive = true,
                    }
                }
                if any_inconclusive {
                    DispatchabilityResult::DependencyInconclusive {
                        candidate: candidate.clone(),
                    }
                } else if any_not_merged {
                    DispatchabilityResult::Waiting {
                        candidate: candidate.clone(),
                    }
                } else {
                    DispatchabilityResult::Dispatchable {
                        candidate: candidate.clone(),
                    }
                }
            }
        };
        results.push(result);
    }

    let mut admitted_repositories = Vec::<RepositoryName>::new();
    for result in &mut results {
        let DispatchabilityResult::Dispatchable { candidate } = result else {
            continue;
        };
        if policy_for(candidate.repository())? == &VersionPolicy::SerializeDispatches {
            if admitted_repositories
                .iter()
                .any(|repository| repository == candidate.repository())
            {
                *result = DispatchabilityResult::Waiting {
                    candidate: candidate.clone(),
                };
            } else {
                admitted_repositories.push(candidate.repository().clone());
            }
        }
    }

    Ok(results)
}

/// Derived merge result for one supplied log-visible canonical step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepMergeResult {
    node: NodeId,
    subject: MergeSubject,
    observation: StepMergeResultObservation,
    status: MergeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StepMergeResultObservation {
    Derived(PullRequestAuthorityObservation),
    Exceptional(ExceptionalStepMergeResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExceptionalStepMergeResult {
    chain: ExceptionalMergeChain,
    observation: ExceptionalMergeChainObservation,
    step_to_integration_status: MergeStatus,
    integration_to_default_status: MergeStatus,
}

impl StepMergeResult {
    /// Return the original exact event-log node.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the core-derived merge subject.
    pub const fn subject(&self) -> &MergeSubject {
        &self.subject
    }

    /// Return the retained GitHub authority observation.
    pub const fn github(&self) -> &GitHubAuthorityObservation {
        match &self.observation {
            StepMergeResultObservation::Derived(observation) => observation.github(),
            StepMergeResultObservation::Exceptional(result) => {
                result.observation.step_to_integration().github()
            }
        }
    }

    /// Return the retained git authority observation.
    pub const fn git(&self) -> &GitAuthorityObservation {
        match &self.observation {
            StepMergeResultObservation::Derived(observation) => observation.git(),
            StepMergeResultObservation::Exceptional(result) => {
                result.observation.step_to_integration().git()
            }
        }
    }

    /// Return the independent merge status.
    pub const fn status(&self) -> MergeStatus {
        self.status
    }
}

/// The selected node's latest observed position in the dispatch cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclePosition {
    /// No recognized round-bearing dispatch exists for the node.
    NoRoundDispatch,
    /// The latest recognized dispatch is a plan-producing role.
    PlanDispatched { sequence: Sequence },
    /// The latest recognized dispatch is a plan-critique role.
    CritiqueDispatched { sequence: Sequence },
    /// The latest recognized dispatch is step execution.
    ExecutionDispatched { sequence: Sequence },
    /// The latest recognized dispatch is pre-PR falsification.
    FalsificationDispatched { sequence: Sequence },
    /// The latest recognized dispatch is PR review.
    ReviewDispatched { sequence: Sequence },
}

/// Conservative resume observation over log-visible candidates only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeObservation {
    /// No log-visible node remains a candidate.
    NoLogVisibleCandidate,
    /// The latest remaining log-visible candidate and its observed cycle position.
    Candidate {
        node: NodeId,
        latest_sequence: Sequence,
        cycle_position: CyclePosition,
    },
}

/// One exact criterion execution retained in event-sequence order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionExecutionObservation {
    sequence: Sequence,
    node: NodeId,
    criterion: AcceptanceCriterion,
    finished_result: FinishedResult,
    outcome: CriterionExecutionOutcome,
    evidence: Evidence,
}

impl CriterionExecutionObservation {
    /// Return the source event sequence.
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// Return the source event node.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the exact criterion supplied for execution.
    pub const fn criterion(&self) -> &AcceptanceCriterion {
        &self.criterion
    }

    /// Return the exact supplied finished result.
    pub const fn finished_result(&self) -> &FinishedResult {
        &self.finished_result
    }

    /// Return the distinct execution outcome.
    pub const fn outcome(&self) -> &CriterionExecutionOutcome {
        &self.outcome
    }

    /// Return the execution evidence.
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
}

/// The immutable source of one effective blocking criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockingCriterionOrigin {
    /// The criterion was ratified in the vision.
    Ratified,
    /// The criterion was accepted during the run as a change of course.
    Added {
        sequence: Sequence,
        node: NodeId,
        change_of_course: ChangeOfCourse,
    },
}

/// One effective blocking criterion and its immutable origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingCriterion {
    criterion: AcceptanceCriterion,
    origin: BlockingCriterionOrigin,
}

impl BlockingCriterion {
    /// Return the criterion contract.
    pub const fn criterion(&self) -> &AcceptanceCriterion {
        &self.criterion
    }

    /// Return the criterion's immutable origin.
    pub const fn origin(&self) -> &BlockingCriterionOrigin {
        &self.origin
    }
}

/// Pure derived run state with deterministic collection order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedRunState {
    blocking_criteria: Vec<BlockingCriterion>,
    repositories: Vec<RepositoryObservation>,
    steps: Vec<StepMergeResult>,
    dispatches: Vec<DispatchObservation>,
    dispatch_lifecycles: Vec<DispatchLifecycleObservation>,
    dispatch_accounting: DispatchAccounting,
    criterion_executions: Vec<CriterionExecutionObservation>,
    issuance_ordinals: Vec<IssuanceOrdinalSeries>,
    rounds: Vec<ValidatedProductionSeries>,
    non_production_streaks: Vec<NonProductionSeries>,
    non_production_holds: Vec<NonProductionHoldObservation>,
    holds: Vec<HoldObservation>,
    provenance: Vec<ArtifactProvenance>,
    resume: ResumeObservation,
    recovery_digest: RecoveryDigest,
}

impl DerivedRunState {
    /// Return the ratified floor followed by accepted additions in event order.
    pub fn blocking_criteria(&self) -> &[BlockingCriterion] {
        &self.blocking_criteria
    }
    /// Return repository observations in caller order.
    pub fn repositories(&self) -> &[RepositoryObservation] {
        &self.repositories
    }

    /// Return step merge results in authority-input order.
    pub fn steps(&self) -> &[StepMergeResult] {
        &self.steps
    }

    /// Return every exact dispatch in record order.
    pub fn dispatches(&self) -> &[DispatchObservation] {
        &self.dispatches
    }

    /// Return measured dispatch lifecycles in completion-record order.
    pub fn dispatch_lifecycles(&self) -> &[DispatchLifecycleObservation] {
        &self.dispatch_lifecycles
    }
    pub const fn dispatch_accounting(&self) -> &DispatchAccounting {
        &self.dispatch_accounting
    }

    /// Return every criterion execution in event-sequence order.
    pub fn criterion_executions(&self) -> &[CriterionExecutionObservation] {
        &self.criterion_executions
    }

    /// Return first-seen-ordered exact `(node, role)` round series.
    pub fn issuance_ordinals(&self) -> &[IssuanceOrdinalSeries] {
        &self.issuance_ordinals
    }
    /// Return validated production counted against the spending limit.
    pub fn validated_production_counts(&self) -> &[ValidatedProductionSeries] {
        &self.rounds
    }

    /// Return validated production under the legacy accessor name.
    pub fn rounds(&self) -> &[ValidatedProductionSeries] {
        self.validated_production_counts()
    }
    pub fn non_production_streaks(&self) -> &[NonProductionSeries] {
        &self.non_production_streaks
    }
    pub fn non_production_holds(&self) -> &[NonProductionHoldObservation] {
        &self.non_production_holds
    }

    /// Return first-seen-ordered escalation keys and their latest status.
    pub fn holds(&self) -> &[HoldObservation] {
        &self.holds
    }

    /// Return first-seen-ordered latest artifact provenance results.
    pub fn provenance(&self) -> &[ArtifactProvenance] {
        &self.provenance
    }

    /// Return the conservative resume observation.
    pub const fn resume(&self) -> &ResumeObservation {
        &self.resume
    }

    /// Return the bounded recovery digest derived by the ordered fold.
    pub const fn recovery_digest(&self) -> &RecoveryDigest {
        &self.recovery_digest
    }
}

/// The compiled version-1 serialized projection of one derived run state.
#[derive(Debug, Serialize)]
pub struct RunSnapshot<'a> {
    schema_id: &'static str,
    schema_version: u64,
    repositories: Vec<RepositorySnapshot<'a>>,
    steps: Vec<StepSnapshot<'a>>,
    dispatches: Vec<DispatchSnapshot<'a>>,
    dispatch_accounting: DispatchAccountingSnapshot,
    issuance_ordinals: Vec<IssuanceOrdinalSeriesSnapshot<'a>>,
    rounds: Vec<DefectRoundSeriesSnapshot<'a>>,
    non_production_streaks: Vec<NonProductionSeriesSnapshot<'a>>,
    non_production_holds: Vec<NonProductionHoldSnapshot<'a>>,
    holds: Vec<HoldSnapshot<'a>>,
    provenance: Vec<ProvenanceSnapshot<'a>>,
    resume: ResumeSnapshot<'a>,
    recovery_digest: RecoveryDigest,
}

impl<'a> RunSnapshot<'a> {
    /// Return the fixed schema identifier.
    pub const fn schema_id(&self) -> &'static str {
        self.schema_id
    }

    /// Return the fixed schema version.
    pub const fn schema_version(&self) -> u64 {
        self.schema_version
    }
}

impl<'a> From<&'a DerivedRunState> for RunSnapshot<'a> {
    fn from(state: &'a DerivedRunState) -> Self {
        Self {
            schema_id: "pce.run-snapshot",
            schema_version: 1,
            repositories: state
                .repositories()
                .iter()
                .map(RepositorySnapshot::from)
                .collect(),
            steps: state.steps().iter().map(StepSnapshot::from).collect(),
            dispatches: state
                .dispatches()
                .iter()
                .map(|dispatch| DispatchSnapshot::new(dispatch, state.dispatch_lifecycles()))
                .collect(),
            dispatch_accounting: DispatchAccountingSnapshot::from(state.dispatch_accounting()),
            issuance_ordinals: state
                .issuance_ordinals()
                .iter()
                .map(IssuanceOrdinalSeriesSnapshot::from)
                .collect(),
            rounds: state
                .rounds()
                .iter()
                .map(DefectRoundSeriesSnapshot::from)
                .collect(),
            non_production_streaks: state
                .non_production_streaks()
                .iter()
                .map(NonProductionSeriesSnapshot::from)
                .collect(),
            non_production_holds: state
                .non_production_holds()
                .iter()
                .map(NonProductionHoldSnapshot::from)
                .collect(),
            holds: state.holds().iter().map(HoldSnapshot::from).collect(),
            provenance: state
                .provenance()
                .iter()
                .map(ProvenanceSnapshot::from)
                .collect(),
            resume: ResumeSnapshot::from(state.resume()),
            recovery_digest: state.recovery_digest().clone(),
        }
    }
}

/// Serialized repository observation.
#[derive(Debug, Serialize)]
pub struct RepositorySnapshot<'a> {
    repository: &'a str,
    fetch: RepositoryFetchSnapshot<'a>,
    branch: BranchSnapshot<'a>,
    worktree: WorktreeSnapshot<'a>,
    tag: TagSnapshot<'a>,
}

impl<'a> From<&'a RepositoryObservation> for RepositorySnapshot<'a> {
    fn from(value: &'a RepositoryObservation) -> Self {
        Self {
            repository: value.repository().as_str(),
            fetch: RepositoryFetchSnapshot::from(value.fetch()),
            branch: BranchSnapshot {
                name: value.branch_name().as_str(),
                state: value.branch_state(),
            },
            worktree: WorktreeSnapshot {
                identity: value.worktree().as_str(),
                state: value.worktree_state(),
            },
            tag: TagSnapshot::from((value.tag_name(), value.tag_state())),
        }
    }
}

/// Serialized repository fetch availability.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RepositoryFetchSnapshot<'a> {
    /// A successful observation.
    Observed {
        observation_ref: &'a str,
        fetched_at: &'a EventTimestamp,
    },
    /// An unavailable observation.
    Unavailable { failure: &'a str },
}

impl<'a> From<&'a RepositoryFetchObservation> for RepositoryFetchSnapshot<'a> {
    fn from(value: &'a RepositoryFetchObservation) -> Self {
        match value {
            RepositoryFetchObservation::Observed {
                observation_ref,
                fetched_at,
            } => Self::Observed {
                observation_ref: observation_ref.as_str(),
                fetched_at,
            },
            RepositoryFetchObservation::Unavailable { failure } => Self::Unavailable {
                failure: failure.as_str(),
            },
        }
    }
}

/// Serialized branch name and state.
#[derive(Debug, Serialize)]
pub struct BranchSnapshot<'a> {
    name: &'a str,
    state: BranchState,
}

impl Serialize for BranchState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Present => "present",
            Self::Absent => "absent",
        })
    }
}

/// Serialized worktree identity and state.
#[derive(Debug, Serialize)]
pub struct WorktreeSnapshot<'a> {
    identity: &'a str,
    state: WorktreeState,
}

impl Serialize for WorktreeState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Present => "present",
            Self::Absent => "absent",
        })
    }
}

/// Serialized tag state.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum TagSnapshot<'a> {
    /// An absent tag.
    Absent { name: &'a str },
    /// A tag pointing to an exact target.
    PointsTo { name: &'a str, target: &'a str },
}

impl<'a> From<(&'a TagName, &'a TagState)> for TagSnapshot<'a> {
    fn from((name, state): (&'a TagName, &'a TagState)) -> Self {
        match state {
            TagState::Absent => Self::Absent {
                name: name.as_str(),
            },
            TagState::PointsTo { target } => Self::PointsTo {
                name: name.as_str(),
                target: target.as_str(),
            },
        }
    }
}

/// Serialized step result and its explanatory observations.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum StepSnapshot<'a> {
    Derived {
        node: &'a str,
        subject: MergeSubjectSnapshot<'a>,
        github: GitHubObservationSnapshot<'a>,
        git: GitObservationSnapshot<'a>,
        merge_status: MergeStatus,
    },
    Exceptional {
        node: &'a str,
        subject: MergeSubjectSnapshot<'a>,
        exceptional_merge_chain: ExceptionalMergeChainSnapshot<'a>,
        merge_status: MergeStatus,
    },
}

impl<'a> StepSnapshot<'a> {
    fn node(&self) -> &'a str {
        match self {
            Self::Derived { node, .. } | Self::Exceptional { node, .. } => node,
        }
    }
    fn subject(&self) -> &MergeSubjectSnapshot<'a> {
        match self {
            Self::Derived { subject, .. } | Self::Exceptional { subject, .. } => subject,
        }
    }
    fn github(&self) -> &GitHubObservationSnapshot<'a> {
        match self {
            Self::Derived { github, .. } => github,
            Self::Exceptional {
                exceptional_merge_chain,
                ..
            } => &exceptional_merge_chain.step_to_integration.github,
        }
    }
    fn git(&self) -> &GitObservationSnapshot<'a> {
        match self {
            Self::Derived { git, .. } => git,
            Self::Exceptional {
                exceptional_merge_chain,
                ..
            } => &exceptional_merge_chain.step_to_integration.git,
        }
    }
    const fn merge_status(&self) -> MergeStatus {
        match self {
            Self::Derived { merge_status, .. } | Self::Exceptional { merge_status, .. } => {
                *merge_status
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ExceptionalMergeChainSnapshot<'a> {
    step_pull_request_number: u64,
    promotion_pull_request_number: u64,
    promotion_selector: SelectorSnapshot<'a>,
    step_to_integration: ExceptionalMergeHopSnapshot<'a>,
    integration_to_default: ExceptionalMergeHopSnapshot<'a>,
}

#[derive(Debug, Serialize)]
pub struct ExceptionalMergeHopSnapshot<'a> {
    github: GitHubObservationSnapshot<'a>,
    git: GitObservationSnapshot<'a>,
    merge_status: MergeStatus,
}

impl<'a> From<&'a StepMergeResult> for StepSnapshot<'a> {
    fn from(value: &'a StepMergeResult) -> Self {
        match &value.observation {
            StepMergeResultObservation::Derived(observation) => Self::Derived {
                node: value.node().as_str(),
                subject: MergeSubjectSnapshot::from(value.subject()),
                github: GitHubObservationSnapshot::from(observation.github()),
                git: GitObservationSnapshot::from(observation.git()),
                merge_status: value.status(),
            },
            StepMergeResultObservation::Exceptional(result) => Self::Exceptional {
                node: value.node().as_str(),
                subject: MergeSubjectSnapshot::from(value.subject()),
                exceptional_merge_chain: ExceptionalMergeChainSnapshot {
                    step_pull_request_number: result.chain.step_pull_request().number().get(),
                    promotion_pull_request_number: result
                        .chain
                        .promotion_pull_request()
                        .number()
                        .get(),
                    promotion_selector: SelectorSnapshot::from(
                        result.chain.promotion_pull_request().selector(),
                    ),
                    step_to_integration: ExceptionalMergeHopSnapshot {
                        github: GitHubObservationSnapshot::from(
                            result.observation.step_to_integration().github(),
                        ),
                        git: GitObservationSnapshot::from(
                            result.observation.step_to_integration().git(),
                        ),
                        merge_status: result.step_to_integration_status,
                    },
                    integration_to_default: ExceptionalMergeHopSnapshot {
                        github: GitHubObservationSnapshot::from(
                            result.observation.integration_to_default().github(),
                        ),
                        git: GitObservationSnapshot::from(
                            result.observation.integration_to_default().git(),
                        ),
                        merge_status: result.integration_to_default_status,
                    },
                },
                merge_status: value.status(),
            },
        }
    }
}

impl Serialize for MergeStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Merged => "merged",
            Self::NotMerged => "not-merged",
            Self::Inconclusive => "inconclusive",
        })
    }
}

/// Serialized convention-derived merge subject.
#[derive(Debug, Serialize)]
pub struct MergeSubjectSnapshot<'a> {
    milestone: u64,
    step: u64,
    head_branch: &'a str,
    integration_branch: &'a str,
    pull_request_selector: SelectorSnapshot<'a>,
}

impl<'a> From<&'a MergeSubject> for MergeSubjectSnapshot<'a> {
    fn from(value: &'a MergeSubject) -> Self {
        Self {
            milestone: value.node().milestone().get(),
            step: value.node().step().get(),
            head_branch: value.head().as_str(),
            integration_branch: value.integration_branch().as_str(),
            pull_request_selector: SelectorSnapshot::from(value.selector()),
        }
    }
}

/// Serialized exact pull-request selector.
#[derive(Debug, Serialize)]
pub struct SelectorSnapshot<'a> {
    head: &'a str,
    base: &'a str,
}

impl<'a> From<&'a PullRequestSelector> for SelectorSnapshot<'a> {
    fn from(value: &'a PullRequestSelector) -> Self {
        Self {
            head: value.head().as_str(),
            base: value.base().as_str(),
        }
    }
}

/// Serialized GitHub lookup availability and cardinality.
#[derive(Debug, Serialize)]
#[serde(tag = "availability", rename_all = "kebab-case")]
pub enum GitHubObservationSnapshot<'a> {
    /// GitHub was unreachable.
    Unreachable { failure: &'a str },
    /// GitHub returned no exact match.
    #[serde(rename = "reachable")]
    ReachableZero { cardinality: ExactMatchCardinality },
    /// GitHub returned multiple exact matches.
    #[serde(rename = "reachable")]
    ReachableMultiple { cardinality: ExactMatchCardinality },
    /// GitHub returned one exact match.
    #[serde(rename = "reachable")]
    ReachableOne {
        cardinality: ExactMatchCardinality,
        pull_request: PullRequestSnapshot<'a>,
    },
}

/// Serialized exact-match cardinality.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExactMatchCardinality {
    /// No exact matches.
    ZeroExactMatches,
    /// One exact match.
    OneExactMatch,
    /// Multiple exact matches.
    MultipleExactMatches,
}

impl<'a> From<&'a GitHubAuthorityObservation> for GitHubObservationSnapshot<'a> {
    fn from(value: &'a GitHubAuthorityObservation) -> Self {
        match value {
            GitHubAuthorityObservation::Unreachable { failure } => Self::Unreachable {
                failure: failure.as_str(),
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            } => Self::ReachableZero {
                cardinality: ExactMatchCardinality::ZeroExactMatches,
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::MultipleExactMatches,
            } => Self::ReachableMultiple {
                cardinality: ExactMatchCardinality::MultipleExactMatches,
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch { identity, state },
            } => Self::ReachableOne {
                cardinality: ExactMatchCardinality::OneExactMatch,
                pull_request: PullRequestSnapshot::from((identity, state)),
            },
        }
    }
}

/// Serialized exact pull request.
#[derive(Debug, Serialize)]
pub struct PullRequestSnapshot<'a> {
    number: u64,
    selector: SelectorSnapshot<'a>,
    state: PullRequestStateSnapshot<'a>,
}

impl<'a> From<(&'a ExactPullRequestIdentity, &'a ExactPullRequestState)>
    for PullRequestSnapshot<'a>
{
    fn from((identity, state): (&'a ExactPullRequestIdentity, &'a ExactPullRequestState)) -> Self {
        Self {
            number: identity.number().get(),
            selector: SelectorSnapshot::from(identity.selector()),
            state: PullRequestStateSnapshot::from(state),
        }
    }
}

/// Serialized exact pull-request state.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum PullRequestStateSnapshot<'a> {
    /// The exact pull request is not merged.
    NotMerged,
    /// The exact pull request is merged with this squash commit.
    Merged { squash_commit_oid: &'a str },
}

impl<'a> From<&'a ExactPullRequestState> for PullRequestStateSnapshot<'a> {
    fn from(value: &'a ExactPullRequestState) -> Self {
        match value {
            ExactPullRequestState::NotMerged => Self::NotMerged,
            ExactPullRequestState::Merged { squash_commit } => Self::Merged {
                squash_commit_oid: squash_commit.as_str(),
            },
        }
    }
}

/// Serialized git reachability observation.
#[derive(Debug, Serialize)]
#[serde(tag = "availability", rename_all = "kebab-case")]
pub enum GitObservationSnapshot<'a> {
    /// Git was unreachable.
    Unreachable { failure: &'a str },
    /// Git found no reachable relevant squash commit.
    #[serde(rename = "reachable")]
    ReachableNotMerged { state: GitReachableState },
    /// Git found the exact squash commit reachable.
    #[serde(rename = "reachable")]
    ReachableSquashCommit {
        state: GitReachableState,
        squash_commit_oid: &'a str,
    },
}

/// Serialized state of a reachable git authority.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitReachableState {
    /// No merge was observed.
    NotMerged,
    /// The exact squash commit is reachable.
    SquashCommitReachable,
}

impl<'a> From<&'a GitAuthorityObservation> for GitObservationSnapshot<'a> {
    fn from(value: &'a GitAuthorityObservation) -> Self {
        match value {
            GitAuthorityObservation::Unreachable { failure } => Self::Unreachable {
                failure: failure.as_str(),
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            } => Self::ReachableNotMerged {
                state: GitReachableState::NotMerged,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable { squash_commit },
            } => Self::ReachableSquashCommit {
                state: GitReachableState::SquashCommitReachable,
                squash_commit_oid: squash_commit.as_str(),
            },
        }
    }
}

/// Serialized dispatch observation.
#[derive(Debug, Serialize)]
pub struct DispatchSnapshot<'a> {
    sequence: u64,
    node: &'a str,
    role: &'a str,
    #[serde(rename = "ref")]
    dispatch_ref: &'a str,
    completion: Option<DispatchCompletionSnapshot<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum DispatchCompletionSnapshot<'a> {
    ObservedChild {
        sequence: u64,
        timestamp: EventTimestamp,
        duration_ms: DispatchDuration,
        usage: &'a DispatchTokenUsage,
        exit_status: DispatchExitStatus,
        artifact_outcome: ArtifactOutcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        required_artifact_presence: Option<RequiredArtifactPresence>,
    },
    ReconciledDead {
        sequence: u64,
        timestamp: EventTimestamp,
        artifact_production: ArtifactProduction,
    },
}

#[derive(Debug, Serialize)]
pub struct DispatchAccountingSnapshot {
    state: DispatchAccountingStateSnapshot,
    issuance_sequences: Vec<u64>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DispatchAccountingStateSnapshot {
    AllAccounted,
    Unaccounted,
}

impl From<&DispatchAccounting> for DispatchAccountingSnapshot {
    fn from(value: &DispatchAccounting) -> Self {
        match value {
            DispatchAccounting::AllAccounted => Self {
                state: DispatchAccountingStateSnapshot::AllAccounted,
                issuance_sequences: Vec::new(),
            },
            DispatchAccounting::Unaccounted(ledger) => Self {
                state: DispatchAccountingStateSnapshot::Unaccounted,
                issuance_sequences: ledger
                    .entries()
                    .iter()
                    .map(|entry| entry.sequence().get())
                    .collect(),
            },
        }
    }
}

impl<'a> From<&'a DispatchObservation> for DispatchSnapshot<'a> {
    fn from(value: &'a DispatchObservation) -> Self {
        Self {
            sequence: value.sequence().get(),
            node: value.node().as_str(),
            role: value.role().as_str(),
            dispatch_ref: value.dispatch_ref().as_str(),
            completion: None,
        }
    }
}

impl<'a> DispatchSnapshot<'a> {
    fn new(value: &'a DispatchObservation, lifecycles: &'a [DispatchLifecycleObservation]) -> Self {
        let completion = lifecycles
            .iter()
            .find(|lifecycle| lifecycle.issuance().sequence() == value.sequence())
            .map(|lifecycle| match lifecycle {
                DispatchLifecycleObservation::ObservedChild(observed) => {
                    DispatchCompletionSnapshot::ObservedChild {
                        sequence: observed.completion_sequence.get(),
                        timestamp: observed.completion_timestamp,
                        duration_ms: observed.duration,
                        usage: &observed.usage,
                        exit_status: observed.exit_status,
                        artifact_outcome: observed.artifact_outcome,
                        required_artifact_presence: observed.required_artifact_presence,
                    }
                }
                DispatchLifecycleObservation::ReconciledDead(reconciled) => {
                    DispatchCompletionSnapshot::ReconciledDead {
                        sequence: reconciled.completion_sequence.get(),
                        timestamp: reconciled.completion_timestamp,
                        artifact_production: reconciled.artifact_production,
                    }
                }
            });
        Self {
            sequence: value.sequence().get(),
            node: value.node().as_str(),
            role: value.role().as_str(),
            dispatch_ref: value.dispatch_ref().as_str(),
            completion,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct IssuanceOrdinalSeriesSnapshot<'a> {
    node: &'a str,
    role: &'a str,
    ordinal: u64,
}
impl<'a> From<&'a IssuanceOrdinalSeries> for IssuanceOrdinalSeriesSnapshot<'a> {
    fn from(value: &'a IssuanceOrdinalSeries) -> Self {
        Self {
            node: value.node().as_str(),
            role: value.role().as_str(),
            ordinal: value.ordinal().get(),
        }
    }
}

/// Serialized exact defect-round series.
#[derive(Debug, Serialize)]
pub struct DefectRoundSeriesSnapshot<'a> {
    node: &'a str,
    role: &'a str,
    count: u64,
}

impl<'a> From<&'a ValidatedProductionSeries> for DefectRoundSeriesSnapshot<'a> {
    fn from(value: &'a ValidatedProductionSeries) -> Self {
        Self {
            node: value.node().as_str(),
            role: value.role().as_str(),
            count: value.count().get(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NonProductionSeriesSnapshot<'a> {
    key: &'a NonProductionKey,
    consecutive: u64,
}
impl<'a> From<&'a NonProductionSeries> for NonProductionSeriesSnapshot<'a> {
    fn from(value: &'a NonProductionSeries) -> Self {
        Self {
            key: value.key(),
            consecutive: value.consecutive().get(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NonProductionHoldSnapshot<'a> {
    key: &'a NonProductionKey,
    status: NonProductionHoldStatusSnapshot,
}
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum NonProductionHoldStatusSnapshot {
    Open {
        sequence: u64,
    },
    Closed {
        sequence: u64,
        resolution: NonProductionHoldResolution,
    },
}
impl<'a> From<&'a NonProductionHoldObservation> for NonProductionHoldSnapshot<'a> {
    fn from(value: &'a NonProductionHoldObservation) -> Self {
        let status = match value.status() {
            NonProductionHoldStatus::Open { sequence } => NonProductionHoldStatusSnapshot::Open {
                sequence: sequence.get(),
            },
            NonProductionHoldStatus::Closed {
                sequence,
                resolution,
                ..
            } => NonProductionHoldStatusSnapshot::Closed {
                sequence: sequence.get(),
                resolution: *resolution,
            },
        };
        Self {
            key: value.key(),
            status,
        }
    }
}

/// Serialized latest hold observation.
#[derive(Debug, Serialize)]
pub struct HoldSnapshot<'a> {
    key: &'a str,
    status: HoldStatusSnapshot<'a>,
}

impl<'a> From<&'a HoldObservation> for HoldSnapshot<'a> {
    fn from(value: &'a HoldObservation) -> Self {
        Self {
            key: value.key().as_str(),
            status: HoldStatusSnapshot::from(value.status()),
        }
    }
}

/// Serialized open or closed hold state.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum HoldStatusSnapshot<'a> {
    /// The hold is open.
    Open {
        node: &'a str,
        sequence: u64,
        question: &'a str,
    },
    /// The hold is closed.
    Closed {
        node: &'a str,
        sequence: u64,
        resolution: &'a str,
    },
}

impl<'a> From<&'a HoldStatus> for HoldStatusSnapshot<'a> {
    fn from(value: &'a HoldStatus) -> Self {
        match value {
            HoldStatus::Open {
                node,
                sequence,
                question,
            } => Self::Open {
                node: node.as_str(),
                sequence: sequence.get(),
                question,
            },
            HoldStatus::Closed {
                node,
                sequence,
                resolution,
            } => Self::Closed {
                node: node.as_str(),
                sequence: sequence.get(),
                resolution,
            },
        }
    }
}

/// Serialized artifact provenance result.
#[derive(Debug, Serialize)]
pub struct ProvenanceSnapshot<'a> {
    path: &'a str,
    approved_sha256: &'a str,
    approval_node: &'a str,
    approval_sequence: u64,
    condition: ProvenanceConditionSnapshot<'a>,
}

impl<'a> From<&'a ArtifactProvenance> for ProvenanceSnapshot<'a> {
    fn from(value: &'a ArtifactProvenance) -> Self {
        Self {
            path: value.path().as_str(),
            approved_sha256: value.approved_digest().as_str(),
            approval_node: value.approval_node().as_str(),
            approval_sequence: value.approval_sequence().get(),
            condition: ProvenanceConditionSnapshot::from(value.condition()),
        }
    }
}

/// Serialized provenance condition independent of merge status.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ProvenanceConditionSnapshot<'a> {
    /// Current and approved digests match.
    DigestMatches,
    /// Current and approved digests differ.
    DigestMismatch {
        approved_sha256: &'a str,
        current_sha256: &'a str,
    },
    /// The approved artifact is missing.
    ArtifactMissing,
}

impl<'a> From<&'a ArtifactProvenanceCondition> for ProvenanceConditionSnapshot<'a> {
    fn from(value: &'a ArtifactProvenanceCondition) -> Self {
        match value {
            ArtifactProvenanceCondition::DigestMatches => Self::DigestMatches,
            ArtifactProvenanceCondition::DigestMismatch { approved, current } => {
                Self::DigestMismatch {
                    approved_sha256: approved.as_str(),
                    current_sha256: current.as_str(),
                }
            }
            ArtifactProvenanceCondition::ArtifactMissing => Self::ArtifactMissing,
        }
    }
}

/// Serialized conservative resume observation.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ResumeSnapshot<'a> {
    /// No log-visible candidate remains.
    NoLogVisibleCandidate,
    /// The latest log-visible candidate.
    Candidate {
        node: &'a str,
        latest_sequence: u64,
        cycle_position: CyclePositionSnapshot,
    },
}

impl<'a> From<&'a ResumeObservation> for ResumeSnapshot<'a> {
    fn from(value: &'a ResumeObservation) -> Self {
        match value {
            ResumeObservation::NoLogVisibleCandidate => Self::NoLogVisibleCandidate,
            ResumeObservation::Candidate {
                node,
                latest_sequence,
                cycle_position,
            } => Self::Candidate {
                node: node.as_str(),
                latest_sequence: latest_sequence.get(),
                cycle_position: CyclePositionSnapshot::from(*cycle_position),
            },
        }
    }
}

/// Serialized dispatch-cycle position.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum CyclePositionSnapshot {
    /// No round dispatch has occurred.
    NoRoundDispatch,
    /// A plan-producing role was dispatched.
    PlanDispatched { sequence: u64 },
    /// A critique-producing role was dispatched.
    CritiqueDispatched { sequence: u64 },
    /// Execution was dispatched.
    ExecutionDispatched { sequence: u64 },
    /// Pre-PR falsification was dispatched.
    FalsificationDispatched { sequence: u64 },
    /// PR review was dispatched.
    ReviewDispatched { sequence: u64 },
}

impl From<CyclePosition> for CyclePositionSnapshot {
    fn from(value: CyclePosition) -> Self {
        match value {
            CyclePosition::NoRoundDispatch => Self::NoRoundDispatch,
            CyclePosition::PlanDispatched { sequence } => Self::PlanDispatched {
                sequence: sequence.get(),
            },
            CyclePosition::CritiqueDispatched { sequence } => Self::CritiqueDispatched {
                sequence: sequence.get(),
            },
            CyclePosition::ExecutionDispatched { sequence } => Self::ExecutionDispatched {
                sequence: sequence.get(),
            },
            CyclePosition::FalsificationDispatched { sequence } => Self::FalsificationDispatched {
                sequence: sequence.get(),
            },
            CyclePosition::ReviewDispatched { sequence } => Self::ReviewDispatched {
                sequence: sequence.get(),
            },
        }
    }
}

/// Versioned recovery-digest carriers populated by the bounded digest fold in m2-s5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryDigest {
    rounds: RecoveryCategory<RecoveryRoundEntry>,
    open_holds: RecoveryCategory<RecoveryOpenHoldEntry>,
    deltas: RecoveryCategory<RecoveryDeltaEntry>,
    facts: RecoveryCategory<RecoveryFactEntry>,
}

/// One typed recovery category with explicit elisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryCategory<T> {
    entries: Vec<T>,
    elisions: Vec<RecoveryElision>,
}

/// One recovery round entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryRoundEntry {
    sequence: u64,
    node: String,
    role: String,
    round_number: u64,
}

/// One recovery open-hold entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryOpenHoldEntry {
    sequence: u64,
    node: String,
    key: String,
    question: String,
}

/// One recovery delta entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryDeltaEntry {
    sequence: u64,
    node: String,
    message: String,
}

/// One recovery fact entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryFactEntry {
    sequence: u64,
    node: String,
    kind: String,
    evidence: String,
}

/// One explicit omitted recovery range and its retrieval commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryElision {
    omitted_count: u64,
    start_sequence: u64,
    end_sequence: u64,
    retrieval_commands: Vec<String>,
}

/// Render the versioned snapshot as deterministic human-readable status.
#[instrument(skip(snapshot))]
pub fn render_human_snapshot(snapshot: &RunSnapshot<'_>) -> String {
    let mut output = String::new();
    output.push_str("pce status (pce.run-snapshot v1)\n");
    output.push_str(&format!("repositories ({})\n", snapshot.repositories.len()));
    for (index, repository) in snapshot.repositories.iter().enumerate() {
        output.push_str(&format!(
            "  repository {}: name={}\n",
            index + 1,
            quoted(repository.repository)
        ));
        match &repository.fetch {
            RepositoryFetchSnapshot::Observed {
                observation_ref,
                fetched_at,
            } => output.push_str(&format!(
                "    fetch observed: ref={} fetched-at={}\n",
                quoted(observation_ref),
                quoted(
                    &fetched_at
                        .as_datetime()
                        .to_rfc3339_opts(SecondsFormat::Millis, true)
                )
            )),
            RepositoryFetchSnapshot::Unavailable { failure } => output.push_str(&format!(
                "    fetch unavailable: failure={}\n",
                quoted(failure)
            )),
        }
        match repository.branch.state {
            BranchState::Present => output.push_str(&format!(
                "    branch: name={} state=present\n",
                quoted(repository.branch.name)
            )),
            BranchState::Absent => output.push_str(&format!(
                "    branch: name={} state=absent\n",
                quoted(repository.branch.name)
            )),
        }
        match repository.worktree.state {
            WorktreeState::Present => output.push_str(&format!(
                "    worktree: identity={} state=present\n",
                quoted(repository.worktree.identity)
            )),
            WorktreeState::Absent => output.push_str(&format!(
                "    worktree: identity={} state=absent\n",
                quoted(repository.worktree.identity)
            )),
        }
        match &repository.tag {
            TagSnapshot::Absent { name } => {
                output.push_str(&format!("    tag: name={} state=absent\n", quoted(name)))
            }
            TagSnapshot::PointsTo { name, target } => output.push_str(&format!(
                "    tag: name={} state=points-to target={}\n",
                quoted(name),
                quoted(target)
            )),
        }
    }

    output.push_str(&format!("steps ({})\n", snapshot.steps.len()));
    for (index, step) in snapshot.steps.iter().enumerate() {
        let merge_status = match step.merge_status() {
            MergeStatus::Merged => "merged",
            MergeStatus::NotMerged => "not-merged",
            MergeStatus::Inconclusive => "inconclusive",
        };
        output.push_str(&format!(
            "  step {}: node={} merge-status={merge_status}\n",
            index + 1,
            quoted(step.node())
        ));
        output.push_str(&format!(
            "    subject: milestone={} step={} head={} integration={}\n",
            step.subject().milestone,
            step.subject().step,
            quoted(step.subject().head_branch),
            quoted(step.subject().integration_branch)
        ));
        output.push_str(&format!(
            "    selector: head={} base={}\n",
            quoted(step.subject().pull_request_selector.head),
            quoted(step.subject().pull_request_selector.base)
        ));
        render_github(&mut output, step.github());
        render_git(&mut output, step.git());
    }

    match &snapshot.dispatch_accounting {
        DispatchAccountingSnapshot {
            state: DispatchAccountingStateSnapshot::AllAccounted,
            ..
        } => output.push_str("dispatch-accounting state=all-accounted issuance-sequences=-\n"),
        DispatchAccountingSnapshot {
            state: DispatchAccountingStateSnapshot::Unaccounted,
            issuance_sequences,
        } => output.push_str(&format!(
            "dispatch-accounting state=unaccounted issuance-sequences={}\n",
            issuance_sequences
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )),
    }
    output.push_str(&format!("dispatches ({})\n", snapshot.dispatches.len()));
    for (index, dispatch) in snapshot.dispatches.iter().enumerate() {
        output.push_str(&format!(
            "  dispatch {}: sequence={} node={} role={} ref={}",
            index + 1,
            dispatch.sequence,
            quoted(dispatch.node),
            quoted(dispatch.role),
            quoted(dispatch.dispatch_ref)
        ));
        match &dispatch.completion {
            None => output.push_str(" completion=unaccounted"),
            Some(DispatchCompletionSnapshot::ObservedChild {
                sequence,
                duration_ms,
                usage,
                exit_status,
                artifact_outcome,
                required_artifact_presence,
                ..
            }) => {
                let presence = match required_artifact_presence {
                    Some(RequiredArtifactPresence::Present) => "present",
                    Some(RequiredArtifactPresence::Absent) => "absent",
                    None => "unknown",
                };
                output.push_str(&format!(" completion=observed-child completion-sequence={} duration-ms={} exit-status={} artifact-outcome={} required-artifact-presence={} usage={}", sequence, duration_ms.get(), render_dispatch_exit(*exit_status), render_artifact_outcome(*artifact_outcome), presence, render_dispatch_usage(usage)));
            }
            Some(DispatchCompletionSnapshot::ReconciledDead {
                sequence,
                artifact_production,
                ..
            }) => {
                let production = match artifact_production {
                    ArtifactProduction::Produced => "produced",
                    ArtifactProduction::NotProduced => "not-produced",
                };
                output.push_str(&format!(" completion=reconciled-dead completion-sequence={sequence} artifact-production={production}"));
            }
        }
        output.push('\n');
    }

    output.push_str(&format!(
        "issuance-ordinals ({})\n",
        snapshot.issuance_ordinals.len()
    ));
    for (index, series) in snapshot.issuance_ordinals.iter().enumerate() {
        output.push_str(&format!(
            "  issuance-ordinal {}: node={} role={} ordinal={}\n",
            index + 1,
            quoted(series.node),
            quoted(series.role),
            series.ordinal
        ));
    }
    output.push_str(&format!("rounds ({})\n", snapshot.rounds.len()));
    for (index, round) in snapshot.rounds.iter().enumerate() {
        output.push_str(&format!(
            "  round {}: node={} role={} count={}\n",
            index + 1,
            quoted(round.node),
            quoted(round.role),
            round.count
        ));
    }
    output.push_str(&format!(
        "non-production-streaks ({})\n",
        snapshot.non_production_streaks.len()
    ));
    for (index, series) in snapshot.non_production_streaks.iter().enumerate() {
        output.push_str(&format!(
            "  non-production-streak {}: node={} role={} required-artifact={} consecutive={}\n",
            index + 1,
            quoted(series.key.node.as_str()),
            quoted(series.key.role.as_str()),
            quoted(series.key.required_artifact_path.as_str()),
            series.consecutive
        ));
    }
    output.push_str(&format!(
        "non-production-holds ({})\n",
        snapshot.non_production_holds.len()
    ));
    for (index, hold) in snapshot.non_production_holds.iter().enumerate() {
        match hold.status {
            NonProductionHoldStatusSnapshot::Open { sequence } => output.push_str(&format!("  non-production-hold {}: node={} role={} required-artifact={} state=open sequence={}\n", index + 1, quoted(hold.key.node.as_str()), quoted(hold.key.role.as_str()), quoted(hold.key.required_artifact_path.as_str()), sequence)),
            NonProductionHoldStatusSnapshot::Closed { sequence, resolution } => output.push_str(&format!("  non-production-hold {}: node={} role={} required-artifact={} state=closed sequence={} resolution={}\n", index + 1, quoted(hold.key.node.as_str()), quoted(hold.key.role.as_str()), quoted(hold.key.required_artifact_path.as_str()), sequence, match resolution { NonProductionHoldResolution::Retry => "retry", NonProductionHoldResolution::RePlan => "re-plan", NonProductionHoldResolution::Abandon => "abandon" })),
        }
    }

    output.push_str(&format!("holds ({})\n", snapshot.holds.len()));
    for (index, hold) in snapshot.holds.iter().enumerate() {
        match &hold.status {
            HoldStatusSnapshot::Open {
                node,
                sequence,
                question,
            } => output.push_str(&format!(
                "  hold {}: key={} state=open node={} sequence={} question={}\n",
                index + 1,
                quoted(hold.key),
                quoted(node),
                sequence,
                quoted(question)
            )),
            HoldStatusSnapshot::Closed {
                node,
                sequence,
                resolution,
            } => output.push_str(&format!(
                "  hold {}: key={} state=closed node={} sequence={} resolution={}\n",
                index + 1,
                quoted(hold.key),
                quoted(node),
                sequence,
                quoted(resolution)
            )),
        }
    }

    output.push_str(&format!("provenance ({})\n", snapshot.provenance.len()));
    for (index, artifact) in snapshot.provenance.iter().enumerate() {
        match &artifact.condition {
            ProvenanceConditionSnapshot::DigestMatches => output.push_str(&format!(
                "  artifact {}: path={} approved-sha256={} approval-node={} approval-sequence={} condition=digest-matches\n",
                index + 1,
                quoted(artifact.path),
                quoted(artifact.approved_sha256),
                quoted(artifact.approval_node),
                artifact.approval_sequence
            )),
            ProvenanceConditionSnapshot::DigestMismatch {
                approved_sha256,
                current_sha256,
            } => output.push_str(&format!(
                "  artifact {}: path={} approved-sha256={} approval-node={} approval-sequence={} condition=digest-mismatch current-sha256={}\n",
                index + 1,
                quoted(artifact.path),
                quoted(approved_sha256),
                quoted(artifact.approval_node),
                artifact.approval_sequence,
                quoted(current_sha256)
            )),
            ProvenanceConditionSnapshot::ArtifactMissing => output.push_str(&format!(
                "  artifact {}: path={} approved-sha256={} approval-node={} approval-sequence={} condition=artifact-missing\n",
                index + 1,
                quoted(artifact.path),
                quoted(artifact.approved_sha256),
                quoted(artifact.approval_node),
                artifact.approval_sequence
            )),
        }
    }

    render_resume(&mut output, &snapshot.resume);
    output.push_str("recovery-digest\n");
    render_recovery_rounds(&mut output, &snapshot.recovery_digest.rounds);
    render_recovery_holds(&mut output, &snapshot.recovery_digest.open_holds);
    render_recovery_deltas(&mut output, &snapshot.recovery_digest.deltas);
    render_recovery_facts(&mut output, &snapshot.recovery_digest.facts);
    output
}

fn render_github(output: &mut String, github: &GitHubObservationSnapshot<'_>) {
    match github {
        GitHubObservationSnapshot::Unreachable { failure } => output.push_str(&format!(
            "    github unreachable: failure={}\n",
            quoted(failure)
        )),
        GitHubObservationSnapshot::ReachableZero { cardinality } => {
            output.push_str(&format!(
                "    github reachable: cardinality={}\n",
                exact_match_cardinality(cardinality)
            ));
        }
        GitHubObservationSnapshot::ReachableMultiple { cardinality } => {
            output.push_str(&format!(
                "    github reachable: cardinality={}\n",
                exact_match_cardinality(cardinality)
            ));
        }
        GitHubObservationSnapshot::ReachableOne {
            cardinality,
            pull_request,
        } => {
            let cardinality = exact_match_cardinality(cardinality);
            match &pull_request.state {
                PullRequestStateSnapshot::NotMerged => output.push_str(&format!(
                    "    github reachable: cardinality={cardinality} pr={} head={} base={} status=not-merged\n",
                    pull_request.number,
                    quoted(pull_request.selector.head),
                    quoted(pull_request.selector.base)
                )),
                PullRequestStateSnapshot::Merged { squash_commit_oid } => {
                    output.push_str(&format!(
                        "    github reachable: cardinality={cardinality} pr={} head={} base={} status=merged squash={}\n",
                        pull_request.number,
                        quoted(pull_request.selector.head),
                        quoted(pull_request.selector.base),
                        quoted(squash_commit_oid)
                    ));
                }
            }
        }
    }
}

fn exact_match_cardinality(cardinality: &ExactMatchCardinality) -> &'static str {
    match cardinality {
        ExactMatchCardinality::ZeroExactMatches => "zero-exact-matches",
        ExactMatchCardinality::OneExactMatch => "one-exact-match",
        ExactMatchCardinality::MultipleExactMatches => "multiple-exact-matches",
    }
}

fn render_git(output: &mut String, git: &GitObservationSnapshot<'_>) {
    match git {
        GitObservationSnapshot::Unreachable { failure } => output.push_str(&format!(
            "    git unreachable: failure={}\n",
            quoted(failure)
        )),
        GitObservationSnapshot::ReachableNotMerged { state } => {
            let state = match state {
                GitReachableState::NotMerged => "not-merged",
                GitReachableState::SquashCommitReachable => "squash-commit-reachable",
            };
            output.push_str(&format!("    git reachable: state={state}\n"));
        }
        GitObservationSnapshot::ReachableSquashCommit {
            state,
            squash_commit_oid,
        } => {
            let state = match state {
                GitReachableState::NotMerged => "not-merged",
                GitReachableState::SquashCommitReachable => "squash-commit-reachable",
            };
            output.push_str(&format!(
                "    git reachable: state={state} squash={}\n",
                quoted(squash_commit_oid)
            ));
        }
    }
}

fn render_resume(output: &mut String, resume: &ResumeSnapshot<'_>) {
    match resume {
        ResumeSnapshot::NoLogVisibleCandidate => {
            output.push_str("resume state=no-log-visible-candidate\n");
        }
        ResumeSnapshot::Candidate {
            node,
            latest_sequence,
            cycle_position,
        } => {
            output.push_str(&format!(
                "resume state=candidate node={} latest-sequence={latest_sequence}",
                quoted(node)
            ));
            match cycle_position {
                CyclePositionSnapshot::NoRoundDispatch => {
                    output.push_str(" cycle=no-round-dispatch\n");
                }
                CyclePositionSnapshot::PlanDispatched { sequence } => output.push_str(&format!(
                    " cycle=plan-dispatched cycle-sequence={sequence}\n"
                )),
                CyclePositionSnapshot::CritiqueDispatched { sequence } => output.push_str(
                    &format!(" cycle=critique-dispatched cycle-sequence={sequence}\n"),
                ),
                CyclePositionSnapshot::ExecutionDispatched { sequence } => output.push_str(
                    &format!(" cycle=execution-dispatched cycle-sequence={sequence}\n"),
                ),
                CyclePositionSnapshot::FalsificationDispatched { sequence } => output.push_str(
                    &format!(" cycle=falsification-dispatched cycle-sequence={sequence}\n"),
                ),
                CyclePositionSnapshot::ReviewDispatched { sequence } => output.push_str(&format!(
                    " cycle=review-dispatched cycle-sequence={sequence}\n"
                )),
            }
        }
    }
}

fn render_recovery_rounds(output: &mut String, category: &RecoveryCategory<RecoveryRoundEntry>) {
    render_recovery_header(output, "rounds", category);
    for entry in &category.entries {
        output.push_str(&format!(
            "    entry: sequence={} node={} role={} round-number={}\n",
            entry.sequence,
            quoted(&entry.node),
            quoted(&entry.role),
            entry.round_number
        ));
    }
    render_elisions(output, &category.elisions);
}

fn render_recovery_holds(output: &mut String, category: &RecoveryCategory<RecoveryOpenHoldEntry>) {
    render_recovery_header(output, "open-holds", category);
    for entry in &category.entries {
        output.push_str(&format!(
            "    entry: sequence={} node={} key={} question={}\n",
            entry.sequence,
            quoted(&entry.node),
            quoted(&entry.key),
            quoted(&entry.question)
        ));
    }
    render_elisions(output, &category.elisions);
}

fn render_recovery_deltas(output: &mut String, category: &RecoveryCategory<RecoveryDeltaEntry>) {
    render_recovery_header(output, "deltas", category);
    for entry in &category.entries {
        output.push_str(&format!(
            "    entry: sequence={} node={} message={}\n",
            entry.sequence,
            quoted(&entry.node),
            quoted(&entry.message)
        ));
    }
    render_elisions(output, &category.elisions);
}

fn render_recovery_facts(output: &mut String, category: &RecoveryCategory<RecoveryFactEntry>) {
    render_recovery_header(output, "facts", category);
    for entry in &category.entries {
        output.push_str(&format!(
            "    entry: sequence={} node={} kind={} evidence={}\n",
            entry.sequence,
            quoted(&entry.node),
            quoted(&entry.kind),
            quoted(&entry.evidence)
        ));
    }
    render_elisions(output, &category.elisions);
}

fn render_recovery_header<T>(output: &mut String, name: &str, category: &RecoveryCategory<T>) {
    output.push_str(&format!(
        "  {name} (entries={}, elisions={})\n",
        category.entries.len(),
        category.elisions.len()
    ));
}

fn render_elisions(output: &mut String, elisions: &[RecoveryElision]) {
    for elision in elisions {
        output.push_str(&format!(
            "    elision: omitted-count={} sequence={}..={}\n",
            elision.omitted_count, elision.start_sequence, elision.end_sequence
        ));
        for command in &elision.retrieval_commands {
            output.push_str(&format!("      retrieve: {}\n", quoted(command)));
        }
    }
}

fn quoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        if character == '\'' {
            escaped.push(character);
        } else {
            escaped.extend(character.escape_default());
        }
    }
    escaped.push('"');
    escaped
}

fn render_dispatch_exit(value: DispatchExitStatus) -> String {
    match value {
        DispatchExitStatus::Exited { code } => format!("exited:{}", code.get()),
        DispatchExitStatus::Signaled { signal } => format!("signaled:{}", signal.get()),
    }
}

fn render_artifact_outcome(value: ArtifactOutcome) -> &'static str {
    match value {
        ArtifactOutcome::NotValidated => "not-validated",
        ArtifactOutcome::Validated => "validated",
        ArtifactOutcome::Missing => "missing",
        ArtifactOutcome::Truncated => "truncated",
        ArtifactOutcome::SchemaInvalid => "schema-invalid",
        ArtifactOutcome::SchemaViolating => "schema-violating",
    }
}

fn render_dispatch_usage(value: &DispatchTokenUsage) -> String {
    match value {
        DispatchTokenUsage::Measured {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
        } => format!(
            "measured:input={},cached-input={},output={},reasoning-output={}",
            input_tokens.get(),
            cached_input_tokens.get(),
            output_tokens.get(),
            reasoning_output_tokens.get()
        ),
        DispatchTokenUsage::ClaudeMeasured {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        } => format!(
            "claude-measured:input={},output={},cache-creation-input={},cache-read-input={}",
            input_tokens.get(),
            output_tokens.get(),
            cache_creation_input_tokens.get(),
            cache_read_input_tokens.get()
        ),
        DispatchTokenUsage::Absent { reason } => format!(
            "absent:{}",
            match reason {
                crate::event_log::UsageAbsenceReason::TurnFailed => "turn-failed",
                crate::event_log::UsageAbsenceReason::NoTerminalTurn => "no-terminal-turn",
                crate::event_log::UsageAbsenceReason::MalformedTerminalData =>
                    "malformed-terminal-data",
                crate::event_log::UsageAbsenceReason::DuplicateTerminalData =>
                    "duplicate-terminal-data",
                crate::event_log::UsageAbsenceReason::ContradictoryTerminalData =>
                    "contradictory-terminal-data",
                crate::event_log::UsageAbsenceReason::ClaudeMalformedResult =>
                    "claude-malformed-result",
                crate::event_log::UsageAbsenceReason::ClaudeMissingUsage => "claude-missing-usage",
                crate::event_log::UsageAbsenceReason::ClaudeErrorEnvelope =>
                    "claude-error-envelope",
                crate::event_log::UsageAbsenceReason::ClaudeExitEnvelopeContradiction =>
                    "claude-exit-envelope-contradiction",
            }
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleNode {
    node: NodeId,
    latest_sequence: Sequence,
    cycle_position: CyclePosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestApproval {
    path: ArtifactPath,
    digest: Sha256Digest,
    node: NodeId,
    sequence: Sequence,
}

#[derive(Debug)]
struct RecoveryCandidate<T> {
    sequence: u64,
    node: NodeId,
    kind: &'static str,
    entry: T,
}

/// Derive only binary-owned dispatch outcome state.
fn root_cause_is_upstream(
    reporting_role: &DispatchRole,
    root_cause: Option<DispatchRootCause>,
) -> bool {
    match (reporting_role.as_str(), root_cause) {
        (_, None) => false,
        ("step-executor", Some(DispatchRootCause::Execution))
        | ("step-plan-writer" | "step-planner", Some(DispatchRootCause::StepPlan))
        | ("milestone-planner", Some(DispatchRootCause::MilestonePlan)) => false,
        ("step-executor" | "step-plan-writer" | "step-planner" | "milestone-planner", Some(_))
        | (
            "step-plan-critic"
            | "step-critic"
            | "milestone-critic"
            | "falsification-critic"
            | "pr-reviewer",
            Some(_),
        ) => true,
        (_, Some(_)) => false,
    }
}

pub fn derive_dispatch_outcome_state(
    records: &[EventRecord],
    dispatch_artifacts: &[DispatchRequiredArtifactObservation],
) -> Result<DispatchOutcomeState, RunStateError> {
    let ledger =
        fold_dispatch_ledger(records).map_err(|source| RunStateError::DispatchLedger { source })?;
    for (index, observation) in dispatch_artifacts.iter().enumerate() {
        if dispatch_artifacts[..index]
            .iter()
            .any(|candidate| candidate.issuance_sequence == observation.issuance_sequence)
        {
            return Err(RunStateError::DuplicateDispatchArtifactObservation {
                issuance_sequence: observation.issuance_sequence,
            });
        }
        if ledger.issuance(observation.issuance_sequence).is_none() {
            return Err(RunStateError::DispatchArtifactObservationNamesNonDispatch {
                issuance_sequence: observation.issuance_sequence,
            });
        }
    }

    let mut state = DispatchOutcomeState {
        issuance_ordinals: Vec::new(),
        rounds: Vec::new(),
        non_production_streaks: Vec::new(),
        non_production_holds: Vec::new(),
    };
    let mut open_escalations = Vec::<(EscalationKey, NodeId)>::new();
    for record in records {
        match record.body_ref() {
            EventBodyRef::Known(KnownPayload::Dispatch(payload)) => {
                increment_issuance_ordinal(
                    &mut state.issuance_ordinals,
                    record.node(),
                    &payload.role,
                )?;
            }
            EventBodyRef::Known(KnownPayload::DispatchCompletion(payload)) => {
                let Some(entry) = ledger.issuance(payload.issuance_sequence()) else {
                    continue;
                };
                let Some(observation) = dispatch_artifacts
                    .iter()
                    .find(|candidate| candidate.issuance_sequence == payload.issuance_sequence())
                else {
                    continue;
                };
                let key = NonProductionKey {
                    node: entry.issuance().node().clone(),
                    role: entry.issuance().role().clone(),
                    required_artifact_path: observation.required_artifact_path.clone(),
                };
                match payload.outcome() {
                    DispatchCompletionOutcomeRef::ObservedChildWithArtifactPresence(current) => {
                        match (current.artifact_outcome, current.required_artifact_presence) {
                            (ArtifactOutcome::Validated, _) => {
                                if !root_cause_is_upstream(&key.role, current.root_cause) {
                                    increment_validated_production_count(
                                        &mut state.rounds,
                                        &key.node,
                                        &key.role,
                                    )?;
                                }
                                set_non_production(&mut state.non_production_streaks, key, 0)?;
                            }
                            (ArtifactOutcome::NotValidated, RequiredArtifactPresence::Present) => {
                                set_non_production(&mut state.non_production_streaks, key, 0)?;
                            }
                            (ArtifactOutcome::NotValidated, RequiredArtifactPresence::Absent)
                            | (
                                ArtifactOutcome::Missing
                                | ArtifactOutcome::Truncated
                                | ArtifactOutcome::SchemaInvalid
                                | ArtifactOutcome::SchemaViolating,
                                _,
                            ) => {
                                increment_non_production(&mut state.non_production_streaks, key)?;
                            }
                        }
                    }
                    DispatchCompletionOutcomeRef::ObservedChild(_) => {}
                    DispatchCompletionOutcomeRef::ReconciledDead(reconciled) => {
                        match reconciled.artifact_production {
                            ArtifactProduction::Produced => {
                                set_non_production(&mut state.non_production_streaks, key, 0)?
                            }
                            ArtifactProduction::NotProduced => {
                                increment_non_production(&mut state.non_production_streaks, key)?
                            }
                        }
                    }
                }
            }
            EventBodyRef::Known(KnownPayload::EscalationOpen(payload)) => {
                if let Some(existing) = open_escalations
                    .iter_mut()
                    .find(|(key, _)| key == &payload.key)
                {
                    *existing = (payload.key.clone(), record.node().clone());
                } else {
                    open_escalations.push((payload.key.clone(), record.node().clone()));
                }
            }
            EventBodyRef::Known(KnownPayload::EscalationClose(payload)) => {
                if let Some(position) = open_escalations
                    .iter()
                    .position(|(key, node)| key == &payload.key && node == record.node())
                {
                    open_escalations.remove(position);
                    let matching_keys = dispatch_artifacts
                        .iter()
                        .filter_map(|artifact| {
                            let issuance = ledger.issuance(artifact.issuance_sequence)?;
                            let exhausted = state.rounds.iter().any(|series| {
                                series.node == *issuance.issuance().node()
                                    && series.role == *issuance.issuance().role()
                                    && series.count.get() >= 12
                            });
                            (issuance.issuance().node() == record.node() && exhausted).then(|| {
                                NonProductionKey {
                                    node: issuance.issuance().node().clone(),
                                    role: issuance.issuance().role().clone(),
                                    required_artifact_path: artifact.required_artifact_path.clone(),
                                }
                            })
                        })
                        .collect::<Vec<_>>();
                    for key in matching_keys {
                        let issuance_ordinal = state
                            .issuance_ordinals
                            .iter()
                            .find(|series| series.node == key.node && series.role == key.role)
                            .map_or(IssuanceOrdinal(0), |series| series.ordinal);
                        let status = NonProductionHoldStatus::Closed {
                            sequence: record.sequence(),
                            resolution: NonProductionHoldResolution::Retry,
                            issuance_ordinal,
                        };
                        if let Some(existing) = state
                            .non_production_holds
                            .iter_mut()
                            .find(|hold| hold.key == key)
                        {
                            existing.status = status;
                        } else {
                            state
                                .non_production_holds
                                .push(NonProductionHoldObservation { key, status });
                        }
                    }
                }
            }
            EventBodyRef::Known(KnownPayload::NonProductionHoldOpen(payload)) => {
                let status = NonProductionHoldStatus::Open {
                    sequence: record.sequence(),
                };
                if let Some(existing) = state
                    .non_production_holds
                    .iter_mut()
                    .find(|candidate| candidate.key == payload.key)
                {
                    if matches!(
                        existing.status,
                        NonProductionHoldStatus::Closed {
                            resolution: NonProductionHoldResolution::Retry,
                            ..
                        }
                    ) {
                        existing.status = status;
                    }
                } else {
                    state
                        .non_production_holds
                        .push(NonProductionHoldObservation {
                            key: payload.key.clone(),
                            status,
                        });
                }
            }
            EventBodyRef::Known(KnownPayload::NonProductionHoldClose(payload)) => {
                if let Some(existing) = state
                    .non_production_holds
                    .iter_mut()
                    .find(|candidate| candidate.key == payload.key)
                    && matches!(existing.status, NonProductionHoldStatus::Open { .. })
                {
                    let issuance_ordinal = state
                        .issuance_ordinals
                        .iter()
                        .find(|series| {
                            series.node == payload.key.node && series.role == payload.key.role
                        })
                        .map_or(IssuanceOrdinal(0), |series| series.ordinal);
                    existing.status = NonProductionHoldStatus::Closed {
                        sequence: record.sequence(),
                        resolution: payload.resolution,
                        issuance_ordinal,
                    };
                    if payload.resolution == NonProductionHoldResolution::Retry {
                        set_non_production(
                            &mut state.non_production_streaks,
                            payload.key.clone(),
                            0,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(state)
}

pub fn classify_dispatch_admission(
    state: &DispatchOutcomeState,
    key: &NonProductionKey,
) -> DispatchAdmission {
    let count = state
        .rounds
        .iter()
        .find(|series| series.node == key.node && series.role == key.role)
        .map(|series| series.count)
        .unwrap_or(ValidatedProductionCount(0));
    let consecutive = state
        .non_production_streaks
        .iter()
        .find(|series| series.key == *key)
        .map(|series| series.consecutive)
        .unwrap_or(ConsecutiveNonProduction(0));
    let status = state
        .non_production_holds
        .iter()
        .find(|hold| hold.key == *key)
        .map(|hold| &hold.status);
    const VALIDATED_PRODUCTION_SPENDING_LIMIT: u64 = 12;
    if count.get() >= VALIDATED_PRODUCTION_SPENDING_LIMIT {
        match status {
            Some(NonProductionHoldStatus::Open { .. }) => {
                return DispatchAdmission::NonProductionHoldOpen;
            }
            Some(NonProductionHoldStatus::Closed {
                resolution: NonProductionHoldResolution::Retry,
                issuance_ordinal,
                ..
            }) => {
                let current_ordinal = state
                    .issuance_ordinals
                    .iter()
                    .find(|series| series.node == key.node && series.role == key.role)
                    .map_or(IssuanceOrdinal(0), |series| series.ordinal);
                if current_ordinal == *issuance_ordinal {
                    return DispatchAdmission::Admit;
                }
                return DispatchAdmission::OpenNonProductionHold {
                    consecutive: ConsecutiveNonProduction(0),
                };
            }
            Some(NonProductionHoldStatus::Closed { .. }) => {}
            None => {
                return DispatchAdmission::OpenNonProductionHold {
                    consecutive: ConsecutiveNonProduction(0),
                };
            }
        }
    }
    if matches!(status, Some(NonProductionHoldStatus::Open { .. })) {
        return DispatchAdmission::NonProductionHoldOpen;
    }
    if let Some(NonProductionHoldStatus::Closed {
        resolution:
            resolution @ (NonProductionHoldResolution::RePlan | NonProductionHoldResolution::Abandon),
        ..
    }) = status
    {
        return DispatchAdmission::NonProductionResolutionClosesAdmission {
            resolution: *resolution,
        };
    }
    if consecutive.get() >= 2 {
        return DispatchAdmission::OpenNonProductionHold { consecutive };
    }
    DispatchAdmission::Admit
}

fn increment_issuance_ordinal(
    series: &mut Vec<IssuanceOrdinalSeries>,
    node: &NodeId,
    role: &DispatchRole,
) -> Result<IssuanceOrdinal, RunStateError> {
    if let Some(existing) = series
        .iter_mut()
        .find(|item| item.node == *node && item.role == *role)
    {
        existing.ordinal = IssuanceOrdinal(existing.ordinal.0.checked_add(1).ok_or_else(|| {
            RunStateError::IssuanceOrdinalOverflow {
                node: node.clone(),
                role: role.clone(),
            }
        })?);
        Ok(existing.ordinal)
    } else {
        let value = IssuanceOrdinal(1);
        series.push(IssuanceOrdinalSeries {
            node: node.clone(),
            role: role.clone(),
            ordinal: value,
        });
        Ok(value)
    }
}

fn increment_validated_production_count(
    series: &mut Vec<ValidatedProductionSeries>,
    node: &NodeId,
    role: &DispatchRole,
) -> Result<(), RunStateError> {
    if let Some(existing) = series
        .iter_mut()
        .find(|item| item.node == *node && item.role == *role)
    {
        existing.count =
            ValidatedProductionCount(existing.count.0.checked_add(1).ok_or_else(|| {
                RunStateError::ValidatedProductionCountOverflow {
                    node: node.clone(),
                    role: role.clone(),
                }
            })?);
    } else {
        series.push(ValidatedProductionSeries {
            node: node.clone(),
            role: role.clone(),
            count: ValidatedProductionCount(1),
        });
    }
    Ok(())
}

fn set_non_production(
    series: &mut Vec<NonProductionSeries>,
    key: NonProductionKey,
    value: u64,
) -> Result<(), RunStateError> {
    if let Some(existing) = series.iter_mut().find(|item| item.key == key) {
        existing.consecutive = ConsecutiveNonProduction(value);
    } else {
        series.push(NonProductionSeries {
            key,
            consecutive: ConsecutiveNonProduction(value),
        });
    }
    Ok(())
}

fn increment_non_production(
    series: &mut Vec<NonProductionSeries>,
    key: NonProductionKey,
) -> Result<(), RunStateError> {
    if let Some(existing) = series.iter_mut().find(|item| item.key == key) {
        existing.consecutive =
            ConsecutiveNonProduction(
                existing.consecutive.0.checked_add(1).ok_or_else(|| {
                    RunStateError::NonProductionStreakOverflow { key: key.clone() }
                })?,
            );
    } else {
        series.push(NonProductionSeries {
            key,
            consecutive: ConsecutiveNonProduction(1),
        });
    }
    Ok(())
}

/// Fold ordered records and typed authority observations into deterministic run state.
///
/// # Errors
///
/// Returns a named [`RunStateError`] for non-increasing records, checked round overflow,
/// close-before-open history, duplicate or missing cross-input identities, malformed or
/// absent authority nodes, or empty raw values passed to typed constructors.
#[instrument(skip(
    records,
    ratified_criteria,
    vision,
    recovery_log_path,
    artifacts,
    repositories,
    authorities
))]
pub fn derive_run_state(
    records: &[EventRecord],
    ratified_criteria: &AcceptanceCriteria,
    vision: &VisionSlug,
    recovery_log_path: &RecoveryLogPath,
    artifacts: &[CurrentArtifactObservation],
    repositories: &[RepositoryObservation],
    authorities: &[StepAuthorityObservation],
) -> Result<DerivedRunState, RunStateError> {
    derive_run_state_with_dispatch_artifacts(
        records,
        &[],
        ratified_criteria,
        vision,
        recovery_log_path,
        artifacts,
        repositories,
        authorities,
    )
}

#[instrument(skip(
    records,
    dispatch_artifacts,
    ratified_criteria,
    vision,
    recovery_log_path,
    artifacts,
    repositories,
    authorities
))]
pub fn derive_run_state_with_dispatch_artifacts(
    records: &[EventRecord],
    dispatch_artifacts: &[DispatchRequiredArtifactObservation],
    ratified_criteria: &AcceptanceCriteria,
    vision: &VisionSlug,
    recovery_log_path: &RecoveryLogPath,
    artifacts: &[CurrentArtifactObservation],
    repositories: &[RepositoryObservation],
    authorities: &[StepAuthorityObservation],
) -> Result<DerivedRunState, RunStateError> {
    derive_run_state_internal(
        records,
        dispatch_artifacts,
        ratified_criteria,
        vision,
        recovery_log_path,
        artifacts,
        repositories,
        authorities,
        None,
    )
}

/// Fold run state while requiring complete two-hop observations for every declared exception.
///
/// # Errors
///
/// Returns the ordinary run-state errors plus exact declaration/observation route-agreement errors.
#[instrument(skip(
    records,
    ratified_criteria,
    vision,
    recovery_log_path,
    artifacts,
    repositories,
    authorities,
    exceptional
))]
pub fn derive_run_state_with_exceptional_merge_chains(
    records: &[EventRecord],
    ratified_criteria: &AcceptanceCriteria,
    vision: &VisionSlug,
    recovery_log_path: &RecoveryLogPath,
    artifacts: &[CurrentArtifactObservation],
    repositories: &[RepositoryObservation],
    authorities: &[StepAuthorityObservation],
    exceptional: &[(NodeId, ExceptionalMergeChainObservation)],
) -> Result<DerivedRunState, RunStateError> {
    derive_run_state_internal(
        records,
        &[],
        ratified_criteria,
        vision,
        recovery_log_path,
        artifacts,
        repositories,
        authorities,
        Some(exceptional),
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_run_state_internal(
    records: &[EventRecord],
    dispatch_artifacts: &[DispatchRequiredArtifactObservation],
    ratified_criteria: &AcceptanceCriteria,
    vision: &VisionSlug,
    recovery_log_path: &RecoveryLogPath,
    artifacts: &[CurrentArtifactObservation],
    repositories: &[RepositoryObservation],
    authorities: &[StepAuthorityObservation],
    exceptional: Option<&[(NodeId, ExceptionalMergeChainObservation)]>,
) -> Result<DerivedRunState, RunStateError> {
    validate_repository_inputs(repositories)?;
    validate_artifact_inputs(artifacts)?;
    validate_authority_duplicates(authorities)?;
    let dispatch_ledger =
        fold_dispatch_ledger(records).map_err(|source| RunStateError::DispatchLedger { source })?;
    let outcome_state = derive_dispatch_outcome_state(records, dispatch_artifacts)?;

    let mut blocking_criteria = ratified_criteria
        .as_slice()
        .iter()
        .cloned()
        .map(|criterion| BlockingCriterion {
            criterion,
            origin: BlockingCriterionOrigin::Ratified,
        })
        .collect::<Vec<_>>();
    let mut visible_nodes = Vec::<VisibleNode>::new();
    let mut dispatches = Vec::<DispatchObservation>::new();
    let mut dispatch_lifecycles = Vec::<DispatchLifecycleObservation>::new();
    let mut criterion_executions = Vec::<CriterionExecutionObservation>::new();
    let mut holds = Vec::<HoldObservation>::new();
    let mut approvals = Vec::<LatestApproval>::new();
    let mut recovery_rounds = Vec::<RecoveryCandidate<RecoveryRoundEntry>>::new();
    let mut recovery_deltas = Vec::<RecoveryCandidate<RecoveryDeltaEntry>>::new();
    let mut recovery_facts = Vec::<RecoveryCandidate<RecoveryFactEntry>>::new();
    let mut exceptional_chains = Vec::<(NodeId, ExceptionalMergeChain)>::new();
    let mut previous = None::<Sequence>;

    for record in records {
        if let Some(previous_sequence) = previous
            && record.sequence().get() <= previous_sequence.get()
        {
            return Err(RunStateError::NonIncreasingSequence {
                previous: previous_sequence,
                current: record.sequence(),
            });
        }
        previous = Some(record.sequence());
        update_visible_node(&mut visible_nodes, record);

        match record.body_ref() {
            EventBodyRef::Known(KnownPayload::Dispatch(payload)) => {
                dispatches.push(DispatchObservation {
                    sequence: record.sequence(),
                    node: record.node().clone(),
                    role: payload.role.clone(),
                    dispatch_ref: payload.r#ref.clone(),
                });
                let classification = DispatchRoleClass::classify(&payload.role);
                let ordinal = records.iter()
                    .filter(|candidate| candidate.sequence().get() <= record.sequence().get())
                    .filter(|candidate| matches!(candidate.body_ref(), EventBodyRef::Known(KnownPayload::Dispatch(candidate_payload)) if candidate.node() == record.node() && candidate_payload.role == payload.role))
                    .count() as u64;
                if classification.round_classification().is_some() {
                    recovery_rounds.push(RecoveryCandidate {
                        sequence: record.sequence().get(),
                        node: record.node().clone(),
                        kind: "dispatch",
                        entry: RecoveryRoundEntry {
                            sequence: record.sequence().get(),
                            node: record.node().as_str().to_owned(),
                            role: payload.role.as_str().to_owned(),
                            round_number: ordinal,
                        },
                    });
                }
                update_cycle_position(
                    &mut visible_nodes,
                    record.node(),
                    &payload.role,
                    record.sequence(),
                );
                recovery_facts.push(recovery_fact(record, "dispatch", payload.evidence.as_str()));
            }
            EventBodyRef::Known(KnownPayload::DispatchCompletion(payload)) => {
                if payload.issuance_sequence().get() >= record.sequence().get() {
                    return Err(RunStateError::CompletionPointsForward {
                        completion_sequence: record.sequence(),
                        issuance_sequence: payload.issuance_sequence(),
                    });
                }
                let Some(issuance) = dispatches
                    .iter()
                    .find(|item| item.sequence == payload.issuance_sequence())
                    .cloned()
                else {
                    if records
                        .iter()
                        .any(|candidate| candidate.sequence() == payload.issuance_sequence())
                    {
                        return Err(RunStateError::CompletionPointsToNonDispatch {
                            completion_sequence: record.sequence(),
                            issuance_sequence: payload.issuance_sequence(),
                        });
                    }
                    return Err(RunStateError::CompletionIssuanceMissing {
                        completion_sequence: record.sequence(),
                        issuance_sequence: payload.issuance_sequence(),
                    });
                };
                if dispatch_lifecycles
                    .iter()
                    .any(|item| item.issuance().sequence == payload.issuance_sequence())
                {
                    return Err(RunStateError::DispatchAlreadyCompleted {
                        completion_sequence: record.sequence(),
                        issuance_sequence: payload.issuance_sequence(),
                    });
                }
                if issuance.node != *record.node() {
                    return Err(RunStateError::CompletionNodeMismatch {
                        completion_sequence: record.sequence(),
                        issuance_sequence: payload.issuance_sequence(),
                        issuance_node: issuance.node,
                        completion_node: record.node().clone(),
                    });
                }
                dispatch_lifecycles.push(match payload.outcome() {
                    DispatchCompletionOutcomeRef::ObservedChildWithArtifactPresence(payload) => {
                        DispatchLifecycleObservation::ObservedChild(
                            ObservedDispatchLifecycleObservation {
                                completion_sequence: record.sequence(),
                                completion_timestamp: *record.timestamp(),
                                issuance,
                                duration: payload.duration_ms,
                                usage: payload.usage.clone(),
                                exit_status: payload.exit_status,
                                artifact_outcome: payload.artifact_outcome,
                                required_artifact_presence: Some(
                                    payload.required_artifact_presence,
                                ),
                            },
                        )
                    }
                    DispatchCompletionOutcomeRef::ObservedChild(payload) => {
                        DispatchLifecycleObservation::ObservedChild(
                            ObservedDispatchLifecycleObservation {
                                completion_sequence: record.sequence(),
                                completion_timestamp: *record.timestamp(),
                                issuance,
                                duration: payload.duration_ms,
                                usage: payload.usage.clone(),
                                exit_status: payload.exit_status,
                                artifact_outcome: payload.artifact_outcome,
                                required_artifact_presence: None,
                            },
                        )
                    }
                    DispatchCompletionOutcomeRef::ReconciledDead(payload) => {
                        DispatchLifecycleObservation::ReconciledDead(
                            ReconciledDeadDispatchLifecycleObservation {
                                completion_sequence: record.sequence(),
                                completion_timestamp: *record.timestamp(),
                                issuance,
                                outcome: payload.outcome,
                                artifact_production: payload.artifact_production,
                            },
                        )
                    }
                });
            }
            EventBodyRef::Known(KnownPayload::EscalationOpen(payload)) => {
                let status = HoldStatus::Open {
                    node: record.node().clone(),
                    sequence: record.sequence(),
                    question: payload.question.clone(),
                };
                if let Some(existing) = holds.iter_mut().find(|item| item.key == payload.key) {
                    existing.status = status;
                } else {
                    holds.push(HoldObservation {
                        key: payload.key.clone(),
                        status,
                    });
                }
            }
            EventBodyRef::Known(KnownPayload::EscalationClose(payload)) => {
                let Some(existing) = holds.iter_mut().find(|item| item.key == payload.key) else {
                    return Err(RunStateError::EscalationCloseBeforeOpen {
                        key: payload.key.clone(),
                        sequence: record.sequence(),
                    });
                };
                existing.status = HoldStatus::Closed {
                    node: record.node().clone(),
                    sequence: record.sequence(),
                    resolution: payload.resolution.clone(),
                };
            }
            EventBodyRef::Known(KnownPayload::PlanningArtifactApproved(payload)) => {
                let latest = LatestApproval {
                    path: payload.path.clone(),
                    digest: payload.sha256.clone(),
                    node: record.node().clone(),
                    sequence: record.sequence(),
                };
                if let Some(existing) = approvals.iter_mut().find(|item| item.path == payload.path)
                {
                    *existing = latest;
                } else {
                    approvals.push(latest);
                }
                recovery_facts.push(recovery_fact(
                    record,
                    "planning-artifact-approved",
                    payload.evidence.as_str(),
                ));
            }
            EventBodyRef::Known(KnownPayload::CriterionExecution(payload)) => {
                criterion_executions.push(CriterionExecutionObservation {
                    sequence: record.sequence(),
                    node: record.node().clone(),
                    criterion: payload.criterion.clone(),
                    finished_result: payload.finished_result.clone(),
                    outcome: payload.outcome.clone(),
                    evidence: payload.evidence.clone(),
                });
            }
            EventBodyRef::Known(KnownPayload::Delta(payload)) => {
                recovery_deltas.push(RecoveryCandidate {
                    sequence: record.sequence().get(),
                    node: record.node().clone(),
                    kind: "delta",
                    entry: RecoveryDeltaEntry {
                        sequence: record.sequence().get(),
                        node: record.node().as_str().to_owned(),
                        message: payload.message.clone(),
                    },
                });
            }
            EventBodyRef::Known(KnownPayload::KeyFinding(payload)) => {
                recovery_facts.push(recovery_fact(
                    record,
                    "key-finding",
                    payload.evidence.as_str(),
                ));
            }
            EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) => {
                recovery_facts.push(recovery_fact(
                    record,
                    "repository-contract",
                    payload.evidence.as_str(),
                ));
            }
            EventBodyRef::Known(KnownPayload::LegacyRepositoryContract(payload)) => {
                recovery_facts.push(recovery_fact(
                    record,
                    "repository-contract",
                    payload.evidence.as_str(),
                ));
            }
            EventBodyRef::Known(KnownPayload::CriterionAdded(payload)) => {
                blocking_criteria.push(BlockingCriterion {
                    criterion: payload.criterion.clone(),
                    origin: BlockingCriterionOrigin::Added {
                        sequence: record.sequence(),
                        node: record.node().clone(),
                        change_of_course: payload.change_of_course.clone(),
                    },
                });
            }
            EventBodyRef::Known(KnownPayload::NonProductionHoldOpen(_))
            | EventBodyRef::Known(KnownPayload::NonProductionHoldClose(_)) => {}
            EventBodyRef::Known(KnownPayload::ExceptionalMergeChainDeclared(payload)) => {
                let step = StepNode::parse(record.node()).map_err(|_| {
                    RunStateError::ExceptionalMergeChainDeclarationOnNonStepNode {
                        node: record.node().clone(),
                    }
                })?;
                if exceptional_chains
                    .iter()
                    .any(|(node, _)| node == record.node())
                {
                    return Err(RunStateError::DuplicateExceptionalMergeChainDeclaration {
                        node: record.node().clone(),
                    });
                }
                let chain = ExceptionalMergeChain::new(
                    vision,
                    step,
                    payload.integration_branch.clone(),
                    payload.step_pull_request_number,
                    payload.promotion_pull_request_number,
                )?;
                exceptional_chains.push((record.node().clone(), chain));
            }
            EventBodyRef::Unknown { .. } => {}
        }
    }

    let provenance = derive_provenance(&approvals, artifacts)?;
    let steps = match exceptional {
        Some(observations) => {
            validate_exceptional_routes(&exceptional_chains, observations)?;
            derive_step_results_with_exceptional(
                &visible_nodes,
                vision,
                authorities,
                &exceptional_chains,
                observations,
            )?
        }
        None => derive_step_results(&visible_nodes, vision, authorities)?,
    };
    let resume = derive_resume(&visible_nodes, &steps);
    let mut recovery_open_holds = holds
        .iter()
        .filter_map(|hold| match &hold.status {
            HoldStatus::Open {
                node,
                sequence,
                question,
            } => Some(RecoveryCandidate {
                sequence: sequence.get(),
                node: node.clone(),
                kind: "escalation-open",
                entry: RecoveryOpenHoldEntry {
                    sequence: sequence.get(),
                    node: node.as_str().to_owned(),
                    key: hold.key.as_str().to_owned(),
                    question: question.clone(),
                },
            }),
            HoldStatus::Closed { .. } => None,
        })
        .collect::<Vec<_>>();
    recovery_open_holds.sort_by_key(|candidate| candidate.sequence);
    let recovery_digest = RecoveryDigest {
        rounds: finalize_recovery_category(recovery_rounds, recovery_log_path),
        open_holds: finalize_recovery_category(recovery_open_holds, recovery_log_path),
        deltas: finalize_recovery_category(recovery_deltas, recovery_log_path),
        facts: finalize_recovery_category(recovery_facts, recovery_log_path),
    };

    let dispatch_accounting = dispatch_ledger.accounting();
    Ok(DerivedRunState {
        blocking_criteria,
        repositories: repositories.to_vec(),
        steps,
        dispatches,
        dispatch_lifecycles,
        dispatch_accounting,
        criterion_executions,
        issuance_ordinals: outcome_state.issuance_ordinals,
        rounds: outcome_state.rounds,
        non_production_streaks: outcome_state.non_production_streaks,
        non_production_holds: outcome_state.non_production_holds,
        holds,
        provenance,
        resume,
        recovery_digest,
    })
}

fn recovery_fact(
    record: &EventRecord,
    kind: &'static str,
    evidence: &str,
) -> RecoveryCandidate<RecoveryFactEntry> {
    RecoveryCandidate {
        sequence: record.sequence().get(),
        node: record.node().clone(),
        kind,
        entry: RecoveryFactEntry {
            sequence: record.sequence().get(),
            node: record.node().as_str().to_owned(),
            kind: kind.to_owned(),
            evidence: evidence.to_owned(),
        },
    }
}

fn finalize_recovery_category<T>(
    mut candidates: Vec<RecoveryCandidate<T>>,
    recovery_log_path: &RecoveryLogPath,
) -> RecoveryCategory<T> {
    const CAP: usize = 20;
    let omitted_count = candidates.len().saturating_sub(CAP);
    let retained = candidates.split_off(omitted_count);
    let omitted = candidates;
    let mut elisions = Vec::new();
    let mut start = 0;
    while start < omitted.len() {
        let mut end = start + 1;
        while end < omitted.len()
            && omitted[end - 1].sequence.checked_add(1) == Some(omitted[end].sequence)
        {
            end += 1;
        }
        let range = &omitted[start..end];
        let mut commands = Vec::new();
        for candidate in range {
            let command = recovery_command(recovery_log_path, candidate.kind, &candidate.node);
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
        elisions.push(RecoveryElision {
            omitted_count: range.len() as u64,
            start_sequence: range[0].sequence,
            end_sequence: range[range.len() - 1].sequence,
            retrieval_commands: commands,
        });
        start = end;
    }
    RecoveryCategory {
        entries: retained
            .into_iter()
            .map(|candidate| candidate.entry)
            .collect(),
        elisions,
    }
}

fn recovery_command(path: &RecoveryLogPath, kind: &str, node: &NodeId) -> String {
    format!(
        "pce log read --file {} --kind {kind} --node {}",
        shell_quote(path.as_str()),
        shell_quote(node.as_str())
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn validate_repository_inputs(repositories: &[RepositoryObservation]) -> Result<(), RunStateError> {
    for (index, repository) in repositories.iter().enumerate() {
        if repositories[..index]
            .iter()
            .any(|earlier| earlier.repository == repository.repository)
        {
            return Err(RunStateError::DuplicateRepositoryObservation {
                repository: repository.repository.clone(),
            });
        }
    }
    Ok(())
}

fn validate_artifact_inputs(artifacts: &[CurrentArtifactObservation]) -> Result<(), RunStateError> {
    for (index, artifact) in artifacts.iter().enumerate() {
        if artifacts[..index]
            .iter()
            .any(|earlier| earlier.path == artifact.path)
        {
            return Err(RunStateError::DuplicateCurrentArtifactObservation {
                path: artifact.path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_authority_duplicates(
    authorities: &[StepAuthorityObservation],
) -> Result<(), RunStateError> {
    for (index, authority) in authorities.iter().enumerate() {
        if authorities[..index]
            .iter()
            .any(|earlier| earlier.node == authority.node)
        {
            return Err(RunStateError::DuplicateStepAuthorityObservation {
                node: authority.node.clone(),
            });
        }
    }
    Ok(())
}

fn update_visible_node(visible_nodes: &mut Vec<VisibleNode>, record: &EventRecord) {
    if let Some(existing) = visible_nodes
        .iter_mut()
        .find(|item| item.node == *record.node())
    {
        existing.latest_sequence = record.sequence();
    } else {
        visible_nodes.push(VisibleNode {
            node: record.node().clone(),
            latest_sequence: record.sequence(),
            cycle_position: CyclePosition::NoRoundDispatch,
        });
    }
}

fn update_cycle_position(
    visible_nodes: &mut [VisibleNode],
    node: &NodeId,
    role: &DispatchRole,
    sequence: Sequence,
) {
    let position = match role.as_str() {
        "milestone-planner" | "step-planner" | "step-plan-writer" => {
            CyclePosition::PlanDispatched { sequence }
        }
        "milestone-critic" | "step-critic" | "step-plan-critic" => {
            CyclePosition::CritiqueDispatched { sequence }
        }
        "step-executor" => CyclePosition::ExecutionDispatched { sequence },
        "falsification-critic" => CyclePosition::FalsificationDispatched { sequence },
        "pr-reviewer" => CyclePosition::ReviewDispatched { sequence },
        _ => return,
    };
    if let Some(existing) = visible_nodes.iter_mut().find(|item| item.node == *node) {
        existing.cycle_position = position;
    }
}

fn derive_provenance(
    approvals: &[LatestApproval],
    artifacts: &[CurrentArtifactObservation],
) -> Result<Vec<ArtifactProvenance>, RunStateError> {
    let mut provenance = Vec::with_capacity(approvals.len());
    for approval in approvals {
        let observation = artifacts
            .iter()
            .find(|artifact| artifact.path == approval.path)
            .ok_or_else(|| RunStateError::MissingCurrentArtifactObservation {
                path: approval.path.clone(),
            })?;
        let condition = match &observation.state {
            CurrentArtifactState::Present { digest } if *digest == approval.digest => {
                ArtifactProvenanceCondition::DigestMatches
            }
            CurrentArtifactState::Present { digest } => {
                ArtifactProvenanceCondition::DigestMismatch {
                    approved: approval.digest.clone(),
                    current: digest.clone(),
                }
            }
            CurrentArtifactState::Missing => ArtifactProvenanceCondition::ArtifactMissing,
        };
        provenance.push(ArtifactProvenance {
            path: approval.path.clone(),
            approved_digest: approval.digest.clone(),
            approval_node: approval.node.clone(),
            approval_sequence: approval.sequence,
            condition,
        });
    }
    Ok(provenance)
}

fn derive_step_results(
    visible_nodes: &[VisibleNode],
    vision: &VisionSlug,
    authorities: &[StepAuthorityObservation],
) -> Result<Vec<StepMergeResult>, RunStateError> {
    let mut steps = Vec::with_capacity(authorities.len());
    for authority in authorities {
        let step_node = StepNode::parse(&authority.node)?;
        if !visible_nodes.iter().any(|item| item.node == authority.node) {
            return Err(RunStateError::AuthorityNodeAbsentFromLog {
                node: authority.node.clone(),
            });
        }
        let subject = MergeSubject::derive(vision, step_node);
        let status = derive_merge_status(&subject, &authority.github, &authority.git);
        steps.push(StepMergeResult {
            node: authority.node.clone(),
            subject,
            observation: StepMergeResultObservation::Derived(PullRequestAuthorityObservation::new(
                authority.github.clone(),
                authority.git.clone(),
            )),
            status,
        });
    }
    Ok(steps)
}

fn validate_exceptional_routes(
    chains: &[(NodeId, ExceptionalMergeChain)],
    observations: &[(NodeId, ExceptionalMergeChainObservation)],
) -> Result<(), RunStateError> {
    let missing = chains
        .iter()
        .filter(|(node, _)| !observations.iter().any(|(actual, _)| actual == node))
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    let surplus = observations
        .iter()
        .filter(|(node, _)| !chains.iter().any(|(expected, _)| expected == node))
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    if missing.len() == 1 && surplus.len() == 1 {
        return Err(
            RunStateError::ExceptionalMergeChainObservationNodeMismatch {
                expected: missing[0].clone(),
                actual: surplus[0].clone(),
            },
        );
    }
    if let Some(node) = missing.first() {
        return Err(RunStateError::MissingExceptionalMergeChainObservation {
            node: (*node).clone(),
        });
    }
    if let Some(node) = surplus.first() {
        return Err(
            RunStateError::ExceptionalMergeChainObservationWithoutDeclaration {
                node: (*node).clone(),
            },
        );
    }
    for (index, (node, _)) in observations.iter().enumerate() {
        if observations[..index]
            .iter()
            .any(|(earlier, _)| earlier == node)
        {
            return Err(RunStateError::SurplusExceptionalMergeChainObservation {
                node: node.clone(),
            });
        }
    }
    Ok(())
}

fn derive_step_results_with_exceptional(
    visible_nodes: &[VisibleNode],
    vision: &VisionSlug,
    authorities: &[StepAuthorityObservation],
    chains: &[(NodeId, ExceptionalMergeChain)],
    observations: &[(NodeId, ExceptionalMergeChainObservation)],
) -> Result<Vec<StepMergeResult>, RunStateError> {
    let mut steps = derive_step_results(visible_nodes, vision, authorities)?;
    for (node, chain) in chains {
        if !visible_nodes.iter().any(|item| item.node == *node) {
            return Err(RunStateError::AuthorityNodeAbsentFromLog { node: node.clone() });
        }
        let observation = observations
            .iter()
            .find(|(actual, _)| actual == node)
            .map(|(_, observation)| observation)
            .ok_or_else(|| RunStateError::MissingExceptionalMergeChainObservation {
                node: node.clone(),
            })?;
        let (step_status, promotion_status, status) =
            derive_exceptional_merge_status(chain, observation);
        steps.push(StepMergeResult {
            node: node.clone(),
            subject: chain.subject().clone(),
            observation: StepMergeResultObservation::Exceptional(ExceptionalStepMergeResult {
                chain: chain.clone(),
                observation: observation.clone(),
                step_to_integration_status: step_status,
                integration_to_default_status: promotion_status,
            }),
            status,
        });
    }
    Ok(steps)
}

fn derive_resume(visible_nodes: &[VisibleNode], steps: &[StepMergeResult]) -> ResumeObservation {
    let mut selected = None::<&VisibleNode>;
    for visible in visible_nodes {
        let conclusively_merged = steps
            .iter()
            .any(|step| step.node == visible.node && step.status == MergeStatus::Merged);
        if conclusively_merged {
            continue;
        }
        if selected
            .is_none_or(|current| visible.latest_sequence.get() > current.latest_sequence.get())
        {
            selected = Some(visible);
        }
    }
    match selected {
        Some(visible) => ResumeObservation::Candidate {
            node: visible.node.clone(),
            latest_sequence: visible.latest_sequence,
            cycle_position: visible.cycle_position,
        },
        None => ResumeObservation::NoLogVisibleCandidate,
    }
}

/// Errors parsing typed inputs and folding run state at the domain boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunStateError {
    /// Dispatch issuance/completion correlation is untrustworthy.
    #[error(transparent)]
    DispatchLedger {
        source: crate::dispatch_ledger::DispatchLedgerError,
    },
    /// Returned when a vision basename does not begin with ASCII `YYYY-MM-DD-` shape.
    #[error("vision directory basename lacks YYYY-MM-DD- prefix shape: {value}")]
    MalformedVisionBasename { value: String },
    /// Returned when a date-shaped vision basename has no suffix.
    #[error("vision directory basename has an empty slug suffix: {value}")]
    EmptyVisionSlug { value: String },
    /// Returned when a milestone node does not have canonical `m<digits>` shape.
    #[error("milestone node is not canonical m<milestone>: {value}")]
    MalformedMilestoneNode { value: String },
    /// Returned when a milestone-node component contains a leading zero.
    #[error("milestone node component has a leading zero: {value}")]
    LeadingZeroMilestoneNodeComponent { value: String },
    /// Returned when a milestone-node component is numerically zero.
    #[error("milestone node component is zero: {value}")]
    ZeroMilestoneNodeComponent { value: String },
    /// Returned when a milestone-node component exceeds the `u64` range.
    #[error("milestone node component overflows u64: {value}")]
    MilestoneNodeComponentOverflow { value: String },
    /// Returned when a node does not have canonical `m<digits>-s<digits>` shape.
    #[error("step node is not canonical m<milestone>-s<step>: {value}")]
    MalformedStepNode { value: String },
    /// Returned when a step-node component contains a leading zero.
    #[error("step node {component} component has a leading zero: {value}")]
    LeadingZeroStepNodeComponent {
        value: String,
        component: &'static str,
    },
    /// Returned when a step-node component is numerically zero.
    #[error("step node {component} component is zero: {value}")]
    ZeroStepNodeComponent {
        value: String,
        component: &'static str,
    },
    /// Returned when a step-node component exceeds the `u64` range.
    #[error("step node {component} component overflows u64: {value}")]
    StepNodeComponentOverflow {
        value: String,
        component: &'static str,
    },
    /// Returned when a GitHub pull-request number is zero.
    #[error("pull-request number must be positive, got {value}")]
    ZeroPullRequestNumber { value: u64 },
    /// Returned when a declared integration branch is blank.
    #[error("declared integration branch cannot be blank: {value:?}")]
    BlankDeclaredIntegrationBranch { value: String },
    /// Returned when both exceptional hops name the same pull request.
    #[error(
        "exceptional merge chain pull-request numbers must differ: step {step}, promotion {promotion}"
    )]
    EqualExceptionalMergeChainPullRequestNumbers { step: u64, promotion: u64 },
    /// Returned when an exceptional declaration is not attached to a canonical step node.
    #[error("exceptional merge chain declaration requires a canonical step node, got {node:?}")]
    ExceptionalMergeChainDeclarationOnNonStepNode { node: NodeId },
    /// Returned when a node has more than one exceptional declaration.
    #[error("duplicate exceptional merge chain declaration for node {node:?}")]
    DuplicateExceptionalMergeChainDeclaration { node: NodeId },
    /// Returned when a declaration has no supplied exceptional observation.
    #[error("exceptional merge chain declaration has no observation for node {node:?}")]
    MissingExceptionalMergeChainObservation { node: NodeId },
    /// Returned for the sole unmatched declaration and observation pair.
    #[error(
        "exceptional merge chain observation node mismatch: expected {expected:?}, got {actual:?}"
    )]
    ExceptionalMergeChainObservationNodeMismatch { expected: NodeId, actual: NodeId },
    /// Returned when an exceptional observation has no declaration.
    #[error("exceptional merge chain observation has no declaration for node {node:?}")]
    ExceptionalMergeChainObservationWithoutDeclaration { node: NodeId },
    /// Returned when an exceptional observation key repeats.
    #[error("surplus exceptional merge chain observation for node {node:?}")]
    SurplusExceptionalMergeChainObservation { node: NodeId },
    /// Returned when GitHub supplies an empty `mergeCommit.oid`.
    #[error("squash commit OID cannot be empty: {value}")]
    EmptySquashCommitOid { value: String },
    /// Returned when an unreachable authority supplies empty failure detail.
    #[error("authority failure detail cannot be empty: {value}")]
    EmptyAuthorityFailure { value: String },
    /// Returned when a repository-observation string domain is supplied empty.
    #[error("{domain} cannot be empty: {value:?}")]
    EmptyRepositoryObservationValue { domain: &'static str, value: String },
    /// Returned when ordered records contain a non-increasing adjacent sequence.
    #[error("event sequences must strictly increase, got {current:?} after {previous:?}")]
    NonIncreasingSequence {
        previous: Sequence,
        current: Sequence,
    },
    /// A completion names an issuance identity absent from the record set.
    #[error("completion {completion_sequence:?} names missing issuance {issuance_sequence:?}")]
    CompletionIssuanceMissing {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// A completion identity resolves to a record that is not a dispatch.
    #[error("completion {completion_sequence:?} points to non-dispatch {issuance_sequence:?}")]
    CompletionPointsToNonDispatch {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// A completion points to itself or a later record.
    #[error("completion {completion_sequence:?} points forward to {issuance_sequence:?}")]
    CompletionPointsForward {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// More than one completion names the same issuance.
    #[error("completion {completion_sequence:?} repeats completed issuance {issuance_sequence:?}")]
    DispatchAlreadyCompleted {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// Completion and issuance nodes differ.
    #[error(
        "completion {completion_sequence:?} node {completion_node:?} differs from issuance {issuance_sequence:?} node {issuance_node:?}"
    )]
    CompletionNodeMismatch {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
        issuance_node: NodeId,
        completion_node: NodeId,
    },
    /// Returned when an exact `(node, role)` issuance ordinal cannot be incremented.
    #[error("issuance ordinal overflow for node {node:?} and role {role:?}")]
    IssuanceOrdinalOverflow { node: NodeId, role: DispatchRole },
    /// Returned when an exact `(node, role)` validated-production count cannot be incremented.
    #[error("validated production count overflow for node {node:?} and role {role:?}")]
    ValidatedProductionCountOverflow { node: NodeId, role: DispatchRole },
    /// Returned when an exact non-production streak cannot be incremented.
    #[error("non-production streak overflow for key {key:?}")]
    NonProductionStreakOverflow { key: NonProductionKey },
    /// Returned when dispatch artifact observations repeat an issuance.
    #[error("duplicate required-artifact observation for issuance {issuance_sequence:?}")]
    DuplicateDispatchArtifactObservation { issuance_sequence: Sequence },
    /// Returned when a dispatch artifact observation names a non-dispatch record.
    #[error("required-artifact observation names non-dispatch issuance {issuance_sequence:?}")]
    DispatchArtifactObservationNamesNonDispatch { issuance_sequence: Sequence },
    /// Returned when an escalation close has no earlier open for the exact key.
    #[error("escalation key {key:?} closes at sequence {sequence:?} before any open")]
    EscalationCloseBeforeOpen {
        key: EscalationKey,
        sequence: Sequence,
    },
    /// Returned when repository inputs repeat an exact repository name.
    #[error("duplicate repository observation for {repository:?}")]
    DuplicateRepositoryObservation { repository: RepositoryName },
    /// Returned when current artifact inputs repeat an exact artifact path.
    #[error("duplicate current artifact observation for {path:?}")]
    DuplicateCurrentArtifactObservation { path: ArtifactPath },
    /// Returned when the log approves an artifact path without a supplied current observation.
    #[error("approved artifact has no current observation: {path:?}")]
    MissingCurrentArtifactObservation { path: ArtifactPath },
    /// Returned when authority inputs repeat an exact event-log node.
    #[error("duplicate step authority observation for node {node:?}")]
    DuplicateStepAuthorityObservation { node: NodeId },
    /// Returned when an authority observation names a node absent from the supplied log.
    #[error("step authority observation names node absent from log: {node:?}")]
    AuthorityNodeAbsentFromLog { node: NodeId },
    /// Returned when the selected planning artifact's current digest differs from its approval.
    #[error(
        "planning artifact {path:?} digest mismatch: approved {approved_digest:?}, current {current_digest:?}"
    )]
    PlanningArtifactDigestMismatch {
        path: ArtifactPath,
        approved_digest: Sha256Digest,
        current_digest: Sha256Digest,
    },
    /// Returned when the selected approved planning artifact is currently missing.
    #[error(
        "planning artifact {path:?} is missing: approved {approved_digest:?}, current {current_digest:?}"
    )]
    PlanningArtifactMissing {
        path: ArtifactPath,
        approved_digest: Sha256Digest,
        current_digest: Option<Sha256Digest>,
    },
    /// Returned when caller-ordered candidates repeat an exact canonical node.
    #[error("duplicate dispatch candidate for node {node:?}")]
    DuplicateDispatchCandidate { node: CanonicalNode },
    /// Returned when the merge-status lookup repeats an exact canonical node.
    #[error("duplicate merge status for node {node:?}")]
    DuplicateMergeStatus { node: CanonicalNode },
    /// Returned when the exhaustive merge-status lookup omits a required canonical node.
    #[error("missing merge status for required node {node:?}")]
    MissingMergeStatus { node: CanonicalNode },
    /// Returned when the version-policy lookup repeats an exact repository.
    #[error("duplicate version policy for repository {repository:?}")]
    DuplicateVersionPolicy { repository: RepositoryName },
    /// Returned when the exhaustive version-policy lookup omits a candidate repository.
    #[error("missing version policy for candidate repository {repository:?}")]
    MissingVersionPolicy { repository: RepositoryName },
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::acceptance_criteria::{AcceptanceCriteria, parse_acceptance_criteria};
    use crate::dispatch_process_identity::AbsoluteRequiredArtifactPath;
    use crate::event_log::{
        ArtifactOutcome, ArtifactPath, CachedInputTokens, CriterionExecutionOutcome, DeltaPayload,
        DispatchCompletionPayload, DispatchDuration, DispatchExitStatus, DispatchPayload,
        DispatchRef, DispatchRole, DispatchTokenUsage, EscalationClosePayload, EscalationKey,
        EscalationOpenPayload, EventBodyRef, EventRecord, EventTimestamp, Evidence,
        ExceptionalMergeChainDeclaredPayload, ExitCode, InputTokens, KnownPayload, NodeId,
        NonProductionHoldResolution, NonProductionKey, OutputTokens,
        PlanningArtifactApprovedPayload, ReasoningOutputTokens, RepositoryName, Sequence,
        Sha256Digest, parse_event_line,
    };
    use crate::run_state::{
        ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BlockingCriterionOrigin,
        BranchState, CanonicalNode, ConsecutiveNonProduction, CriterionExecutionObservation,
        CurrentArtifactObservation, CurrentArtifactState, CyclePosition, DeclaredIntegrationBranch,
        DerivedRunState, DispatchAdmission, DispatchCandidate, DispatchOutcomeState,
        DispatchRequiredArtifactObservation, DispatchRoleClass, DispatchabilityResult,
        ExactPullRequestIdentity, ExactPullRequestState, ExceptionalMergeChain,
        ExceptionalMergeChainObservation, GitAuthorityObservation, GitHubAuthorityObservation,
        GitHubPullRequestObservation, GitMergeObservation, HoldStatus, MergeStatus, MergeSubject,
        MilestoneMergeSubject, MilestoneNode, NonProductionHoldObservation,
        NonProductionHoldStatus, NonProductionSeries, OrderingEdge,
        PullRequestAuthorityObservation, PullRequestNumber, PullRequestSelector, RecoveryLogPath,
        RepositoryBranchName, RepositoryFetchObservation, RepositoryObservation,
        RepositoryObservationFailure, RepositoryObservationRef, ResumeObservation, RunSnapshot,
        RunStateError, SquashCommitOid, StepAuthorityObservation, StepNode, TagName, TagState,
        TagTarget, ValidatedProductionCount, ValidatedProductionSeries, VersionPolicy, VisionSlug,
        WorktreeIdentity, WorktreeState, classify_dispatch_admission,
        combine_exceptional_merge_statuses, compute_dispatchability, derive_dispatch_outcome_state,
        derive_merge_status, derive_milestone_merge_status, derive_run_state,
        derive_run_state_with_exceptional_merge_chains, recovery_command, render_human_snapshot,
    };

    const RUN_SNAPSHOT_SCHEMA: &str =
        include_str!("../../../skills/pce/schemas/run-snapshot.schema.json");

    fn ratified_floor() -> AcceptanceCriteria {
        parse_acceptance_criteria(
            r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Ratified one","input":"Run input one.","observation":"Observe one."},{"name":"Ratified two","input":"Run input two.","observation":"Observe two."}]}
```

## Decomposition hints

None.
"#,
        )
        .expect("ratified floor fixture")
    }

    fn criterion_added_record(
        sequence: u64,
        node: &str,
        name: &str,
        input: &str,
        observation: &str,
        change: &str,
    ) -> EventRecord {
        parse_event_line(&format!(
            r#"{{"sequence":{sequence},"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-added","node":"{node}","payload":{{"criterion":{{"name":"{name}","input":"{input}","observation":"{observation}"}},"change_of_course":"{change}"}}}}"#
        ))
        .expect("criterion-added fixture")
    }

    fn floor_state(records: &[EventRecord]) -> Result<DerivedRunState, RunStateError> {
        derive_run_state(
            records,
            &ratified_floor(),
            &VisionSlug::parse("2026-07-27-example")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )
    }

    #[test]
    fn ratified_criteria_are_the_initial_blocking_floor() -> Result<(), RunStateError> {
        let state = floor_state(&[])?;
        assert_eq!(state.blocking_criteria().len(), 2);
        assert_eq!(
            state
                .blocking_criteria()
                .iter()
                .map(|item| item.criterion().name().as_str())
                .collect::<Vec<_>>(),
            ["Ratified one", "Ratified two"]
        );
        assert!(
            state
                .blocking_criteria()
                .iter()
                .all(|item| matches!(item.origin(), BlockingCriterionOrigin::Ratified))
        );
        Ok(())
    }

    #[test]
    fn additions_append_in_sequence_order_with_exact_origin() -> Result<(), RunStateError> {
        let records = [
            criterion_added_record(
                2,
                "m2-s1",
                "Added one",
                "Run added one.",
                "Observe added one.",
                "First course change.",
            ),
            criterion_added_record(
                4,
                "m3-s1",
                "Added two",
                "Run added two.",
                "Observe added two.",
                "Second course change.",
            ),
        ];
        let state = floor_state(&records)?;
        assert_eq!(
            state
                .blocking_criteria()
                .iter()
                .map(|item| item.criterion().name().as_str())
                .collect::<Vec<_>>(),
            ["Ratified one", "Ratified two", "Added one", "Added two"]
        );
        assert!(matches!(
            state.blocking_criteria()[2].origin(),
            BlockingCriterionOrigin::Added { sequence, node, change_of_course }
                if sequence.get() == 2 && node.as_str() == "m2-s1"
                    && change_of_course.as_str() == "First course change."
        ));
        assert!(matches!(
            state.blocking_criteria()[3].origin(),
            BlockingCriterionOrigin::Added { sequence, node, change_of_course }
                if sequence.get() == 4 && node.as_str() == "m3-s1"
                    && change_of_course.as_str() == "Second course change."
        ));
        Ok(())
    }

    #[test]
    fn duplicate_additions_remain_blocking_and_unknown_mutations_are_inert()
    -> Result<(), RunStateError> {
        let duplicate = criterion_added_record(
            1,
            "m2-s1",
            "Ratified one",
            "Run input one.",
            "Observe one.",
            "Duplicate exposed.",
        );
        let duplicate_state = floor_state(&[duplicate])?;
        assert_eq!(
            duplicate_state
                .blocking_criteria()
                .iter()
                .map(|item| item.criterion().name().as_str())
                .collect::<Vec<_>>(),
            ["Ratified one", "Ratified two", "Ratified one"]
        );

        let records = [
            criterion_added_record(1, "m2-s1", "Added one", "Run added one.", "Observe added one.", "First course change."),
            parse_event_line(r#"{"sequence":2,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-removed","node":"m2-s1","payload":{"name":"Ratified one"}}"#).expect("unknown removal"),
            parse_event_line(r#"{"sequence":3,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-updated","node":"m2-s1","payload":{"name":"Ratified one"}}"#).expect("unknown update"),
            parse_event_line(r#"{"sequence":4,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-weakened","node":"m2-s1","payload":{"name":"Ratified one"}}"#).expect("unknown weakening"),
            criterion_added_record(5, "m3-s1", "Added two", "Run added two.", "Observe added two.", "Second course change."),
        ];
        let state = floor_state(&records)?;
        assert_eq!(
            state
                .blocking_criteria()
                .iter()
                .map(|item| item.criterion().name().as_str())
                .collect::<Vec<_>>(),
            ["Ratified one", "Ratified two", "Added one", "Added two"]
        );
        Ok(())
    }

    #[test]
    fn blocking_floor_does_not_relax_sequence_refusal() {
        for (records, expected) in [
            (
                [
                    criterion_added_record(2, "m2-s1", "A", "I", "O", "C"),
                    criterion_added_record(1, "m2-s1", "B", "I", "O", "C"),
                ],
                (2, 1),
            ),
            (
                [
                    criterion_added_record(1, "m2-s1", "A", "I", "O", "C"),
                    criterion_added_record(1, "m2-s1", "B", "I", "O", "C"),
                ],
                (1, 1),
            ),
        ] {
            assert!(matches!(
                floor_state(&records),
                Err(RunStateError::DispatchLedger { source: crate::dispatch_ledger::DispatchLedgerError::NonIncreasingSequence { previous_sequence, sequence } })
                    if (previous_sequence.get(), sequence.get()) == expected
            ));
        }
    }

    fn subject(slug: &str, node: &str) -> Result<MergeSubject, Box<dyn Error>> {
        let slug = VisionSlug::parse(slug)?;
        let node = NodeId::parse(node)?;
        let node = StepNode::parse(&node)?;
        Ok(MergeSubject::derive(&slug, node))
    }

    fn identity(
        selector: &PullRequestSelector,
    ) -> Result<ExactPullRequestIdentity, Box<dyn Error>> {
        Ok(ExactPullRequestIdentity::from_selector(
            PullRequestNumber::parse(17)?,
            selector,
        ))
    }

    fn event(
        sequence: u64,
        node: &str,
        payload: KnownPayload,
    ) -> Result<EventRecord, Box<dyn Error>> {
        Ok(EventRecord::known(
            Sequence::parse(sequence)?,
            EventTimestamp::parse("2026-07-27T12:34:56.000Z")?,
            NodeId::parse(node)?,
            payload,
        ))
    }

    fn dispatch(
        sequence: u64,
        node: &str,
        role: &str,
        dispatch_ref: &str,
    ) -> Result<EventRecord, Box<dyn Error>> {
        event(
            sequence,
            node,
            KnownPayload::Dispatch(DispatchPayload {
                role: DispatchRole::new(role),
                r#ref: DispatchRef::new(dispatch_ref),
                evidence: Evidence::parse("dispatch evidence")?,
            }),
        )
    }

    fn delta(sequence: u64, node: &str) -> Result<EventRecord, Box<dyn Error>> {
        event(
            sequence,
            node,
            KnownPayload::Delta(DeltaPayload {
                message: "changed".to_owned(),
            }),
        )
    }

    #[test]
    fn completion_correlates_exact_identity_without_incrementing_rounds()
    -> Result<(), Box<dyn Error>> {
        let records = vec![
            dispatch(1, "m3-s1", "step-executor", "abc")?,
            delta(2, "m3-s1")?,
            event(
                3,
                "m3-s1",
                KnownPayload::DispatchCompletion(DispatchCompletionPayload::ObservedChild(
                    crate::event_log::ObservedDispatchCompletionPayload {
                        issuance_sequence: Sequence::parse(1)?,
                        duration_ms: DispatchDuration::new(200),
                        usage: DispatchTokenUsage::Measured {
                            input_tokens: InputTokens::new(101),
                            cached_input_tokens: CachedInputTokens::new(23),
                            output_tokens: OutputTokens::new(17),
                            reasoning_output_tokens: ReasoningOutputTokens::new(5),
                        },
                        exit_status: DispatchExitStatus::Exited {
                            code: ExitCode::new(0),
                        },
                        artifact_outcome: ArtifactOutcome::NotValidated,
                    },
                )),
            )?,
        ];
        let state = derive(&records, &[], &[], &[])?;
        assert_eq!(state.dispatch_lifecycles().len(), 1);
        assert_eq!(
            state.dispatch_lifecycles()[0].issuance().sequence().get(),
            1
        );
        assert_eq!(
            state.dispatch_lifecycles()[0].completion_sequence().get(),
            3
        );
        assert_eq!(state.issuance_ordinals()[0].ordinal().get(), 1);
        assert!(state.rounds().is_empty());
        assert!(state.non_production_streaks().is_empty());
        Ok(())
    }

    #[test]
    fn dispatch_outcome_transition_table_is_exhaustive() -> Result<(), Box<dyn Error>> {
        let outcomes = [
            ("validated", "present", Some(1), Some(0)),
            ("validated", "absent", Some(1), Some(0)),
            ("not-validated", "present", None, Some(0)),
            ("not-validated", "absent", None, Some(1)),
            ("missing", "present", None, Some(1)),
            ("missing", "absent", None, Some(1)),
            ("truncated", "present", None, Some(1)),
            ("truncated", "absent", None, Some(1)),
            ("schema-invalid", "present", None, Some(1)),
            ("schema-invalid", "absent", None, Some(1)),
            ("schema-violating", "present", None, Some(1)),
            ("schema-violating", "absent", None, Some(1)),
        ];
        for (outcome, presence, rounds, streak) in outcomes {
            let records = vec![
                parse_event_line(
                    r#"{"sequence":1,"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#,
                )?,
                parse_event_line(&format!(
                    r#"{{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{{"issuance_sequence":1,"duration_ms":1,"usage":{{"availability":"absent","reason":"no-terminal-turn"}},"exit_status":{{"kind":"exited","code":0}},"artifact_outcome":"{outcome}","required_artifact_presence":"{presence}"}}}}"#
                ))?,
            ];
            let observations = [DispatchRequiredArtifactObservation::new(
                Sequence::parse(1)?,
                AbsoluteRequiredArtifactPath::parse("/workspace/plan.md")?,
            )];
            let state = derive_dispatch_outcome_state(&records, &observations)?;
            assert_eq!(
                state.rounds().first().map(|series| series.count().get()),
                rounds,
                "{outcome}/{presence}"
            );
            assert_eq!(
                state
                    .non_production_streaks()
                    .first()
                    .map(|series| series.consecutive().get()),
                streak,
                "{outcome}/{presence}"
            );
        }
        for (payload, streak) in [
            (
                r#"{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}"#,
                None,
            ),
            (
                r#"{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"produced"}"#,
                Some(0),
            ),
            (
                r#"{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"not-produced"}"#,
                Some(1),
            ),
        ] {
            let records = vec![
                dispatch(1, "m4-s1", "step-plan-writer", "abc")?,
                parse_event_line(&format!(
                    r#"{{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{payload}}}"#
                ))?,
            ];
            let observations = [DispatchRequiredArtifactObservation::new(
                Sequence::parse(1)?,
                AbsoluteRequiredArtifactPath::parse("/workspace/plan.md")?,
            )];
            let state = derive_dispatch_outcome_state(&records, &observations)?;
            assert!(state.rounds().is_empty());
            assert_eq!(
                state
                    .non_production_streaks()
                    .first()
                    .map(|series| series.consecutive().get()),
                streak
            );
        }
        let unresolved = [dispatch(1, "m4-s1", "step-plan-writer", "abc")?];
        let observations = [DispatchRequiredArtifactObservation::new(
            Sequence::parse(1)?,
            AbsoluteRequiredArtifactPath::parse("/workspace/plan.md")?,
        )];
        let state = derive_dispatch_outcome_state(&unresolved, &observations)?;
        assert!(state.rounds().is_empty() && state.non_production_streaks().is_empty());
        let current_without_observation = vec![
            unresolved[0].clone(),
            parse_event_line(
                r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated","required_artifact_presence":"absent"}}"#,
            )?,
        ];
        let state = derive_dispatch_outcome_state(&current_without_observation, &[])?;
        assert!(state.rounds().is_empty() && state.non_production_streaks().is_empty());
        Ok(())
    }

    #[test]
    fn dispatch_admission_classifier_covers_all_45_ordered_cells() -> Result<(), Box<dyn Error>> {
        let key = NonProductionKey {
            node: NodeId::parse("m4-s1")?,
            role: DispatchRole::new("step-plan-writer"),
            required_artifact_path: AbsoluteRequiredArtifactPath::parse("/workspace/plan.md")?,
        };
        for count in [11_u64, 12, 13] {
            for streak in [1_u64, 2, 3] {
                for status in [
                    None,
                    Some(NonProductionHoldStatus::Open {
                        sequence: Sequence::parse(9)?,
                    }),
                    Some(NonProductionHoldStatus::Closed {
                        sequence: Sequence::parse(10)?,
                        resolution: NonProductionHoldResolution::Retry,
                        issuance_ordinal: super::IssuanceOrdinal(0),
                    }),
                    Some(NonProductionHoldStatus::Closed {
                        sequence: Sequence::parse(10)?,
                        resolution: NonProductionHoldResolution::RePlan,
                        issuance_ordinal: super::IssuanceOrdinal(0),
                    }),
                    Some(NonProductionHoldStatus::Closed {
                        sequence: Sequence::parse(10)?,
                        resolution: NonProductionHoldResolution::Abandon,
                        issuance_ordinal: super::IssuanceOrdinal(0),
                    }),
                ] {
                    let state = DispatchOutcomeState {
                        issuance_ordinals: vec![],
                        rounds: vec![ValidatedProductionSeries {
                            node: key.node.clone(),
                            role: key.role.clone(),
                            count: ValidatedProductionCount(count),
                        }],
                        non_production_streaks: vec![NonProductionSeries {
                            key: key.clone(),
                            consecutive: ConsecutiveNonProduction(streak),
                        }],
                        non_production_holds: status
                            .clone()
                            .map(|status| NonProductionHoldObservation {
                                key: key.clone(),
                                status,
                            })
                            .into_iter()
                            .collect(),
                    };
                    let admission = classify_dispatch_admission(&state, &key);
                    if count >= 12 {
                        match status {
                            None => assert_eq!(
                                admission,
                                DispatchAdmission::OpenNonProductionHold {
                                    consecutive: ConsecutiveNonProduction(0)
                                }
                            ),
                            Some(NonProductionHoldStatus::Open { .. }) => {
                                assert_eq!(admission, DispatchAdmission::NonProductionHoldOpen)
                            }
                            Some(NonProductionHoldStatus::Closed {
                                resolution: NonProductionHoldResolution::Retry,
                                ..
                            }) => assert_eq!(admission, DispatchAdmission::Admit),
                            Some(NonProductionHoldStatus::Closed { resolution, .. }) => {
                                assert_eq!(
                                    admission,
                                    DispatchAdmission::NonProductionResolutionClosesAdmission {
                                        resolution
                                    }
                                )
                            }
                        }
                    } else {
                        match status {
                            Some(NonProductionHoldStatus::Open { .. }) => {
                                assert_eq!(admission, DispatchAdmission::NonProductionHoldOpen)
                            }
                            Some(NonProductionHoldStatus::Closed {
                                resolution: NonProductionHoldResolution::RePlan,
                                ..
                            }) => assert_eq!(
                                admission,
                                DispatchAdmission::NonProductionResolutionClosesAdmission {
                                    resolution: NonProductionHoldResolution::RePlan
                                }
                            ),
                            Some(NonProductionHoldStatus::Closed {
                                resolution: NonProductionHoldResolution::Abandon,
                                ..
                            }) => assert_eq!(
                                admission,
                                DispatchAdmission::NonProductionResolutionClosesAdmission {
                                    resolution: NonProductionHoldResolution::Abandon
                                }
                            ),
                            _ if streak >= 2 => assert_eq!(
                                admission,
                                DispatchAdmission::OpenNonProductionHold {
                                    consecutive: ConsecutiveNonProduction(streak)
                                }
                            ),
                            _ => assert_eq!(admission, DispatchAdmission::Admit),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn typed_retry_alone_resets_window_and_free_text_escalations_are_inert()
    -> Result<(), Box<dyn Error>> {
        let key = NonProductionKey {
            node: NodeId::parse("m4-s1")?,
            role: DispatchRole::new("step-plan-writer"),
            required_artifact_path: AbsoluteRequiredArtifactPath::parse("/workspace/plan.md")?,
        };
        let dispatch_line = |sequence: u64| {
            parse_event_line(&format!(
                r#"{{"sequence":{sequence},"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m4-s1","payload":{{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}}}"#
            ))
        };
        let completion_line = |sequence: u64, issuance: u64| {
            parse_event_line(&format!(
                r#"{{"sequence":{sequence},"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{{"issuance_sequence":{issuance},"duration_ms":1,"usage":{{"availability":"absent","reason":"no-terminal-turn"}},"exit_status":{{"kind":"exited","code":0}},"artifact_outcome":"not-validated","required_artifact_presence":"absent"}}}}"#
            ))
        };
        let mut records = vec![
            dispatch_line(1)?,
            completion_line(2, 1)?,
            dispatch_line(3)?,
            completion_line(4, 3)?,
        ];
        let mut observations = vec![
            DispatchRequiredArtifactObservation::new(
                Sequence::parse(1)?,
                key.required_artifact_path.clone(),
            ),
            DispatchRequiredArtifactObservation::new(
                Sequence::parse(3)?,
                key.required_artifact_path.clone(),
            ),
        ];
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(state.non_production_streaks()[0].consecutive().get(), 2);
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::OpenNonProductionHold {
                consecutive: ConsecutiveNonProduction(2)
            }
        );
        records.push(parse_event_line(r#"{"sequence":5,"timestamp":"2026-08-09T12:00:05.000Z","kind":"non-production-hold-open","node":"m4-s1","payload":{"key":{"node":"m4-s1","role":"step-plan-writer","required_artifact_path":"/workspace/plan.md"}}}"#)?);
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::NonProductionHoldOpen
        );
        records.push(parse_event_line(r#"{"sequence":6,"timestamp":"2026-08-09T12:00:06.000Z","kind":"escalation-close","node":"m4-s1","payload":{"key":"review","resolution":"retry"}}"#)?);
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::NonProductionHoldOpen
        );
        records.push(parse_event_line(r#"{"sequence":7,"timestamp":"2026-08-09T12:00:07.000Z","kind":"non-production-hold-close","node":"m4-s1","payload":{"key":{"node":"m4-s1","role":"step-plan-writer","required_artifact_path":"/workspace/plan.md"},"resolution":"retry"}}"#)?);
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(state.non_production_streaks()[0].consecutive().get(), 0);
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::Admit
        );
        records.extend([dispatch_line(8)?, completion_line(9, 8)?]);
        observations.push(DispatchRequiredArtifactObservation::new(
            Sequence::parse(8)?,
            key.required_artifact_path.clone(),
        ));
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(state.non_production_streaks()[0].consecutive().get(), 1);
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::Admit
        );
        records.extend([dispatch_line(10)?, completion_line(11, 10)?]);
        observations.push(DispatchRequiredArtifactObservation::new(
            Sequence::parse(10)?,
            key.required_artifact_path.clone(),
        ));
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        assert_eq!(state.non_production_streaks()[0].consecutive().get(), 2);
        assert_eq!(
            classify_dispatch_admission(&state, &key),
            DispatchAdmission::OpenNonProductionHold {
                consecutive: ConsecutiveNonProduction(2)
            }
        );
        Ok(())
    }

    fn completion(sequence: u64, node: &str, issuance: u64) -> Result<EventRecord, Box<dyn Error>> {
        event(
            sequence,
            node,
            KnownPayload::DispatchCompletion(DispatchCompletionPayload::ObservedChild(
                crate::event_log::ObservedDispatchCompletionPayload {
                    issuance_sequence: Sequence::parse(issuance)?,
                    duration_ms: DispatchDuration::new(1),
                    usage: DispatchTokenUsage::Absent {
                        reason: crate::event_log::UsageAbsenceReason::NoTerminalTurn,
                    },
                    exit_status: DispatchExitStatus::Exited {
                        code: ExitCode::new(42),
                    },
                    artifact_outcome: ArtifactOutcome::NotValidated,
                },
            )),
        )
    }

    #[test]
    fn completion_rejects_every_invalid_correlation() -> Result<(), Box<dyn Error>> {
        assert!(matches!(
            derive(&[completion(2, "m3-s1", 1)?], &[], &[], &[]),
            Err(RunStateError::DispatchLedger { .. })
        ));
        assert!(matches!(
            derive(
                &[delta(1, "m3-s1")?, completion(2, "m3-s1", 1)?],
                &[],
                &[],
                &[]
            ),
            Err(RunStateError::DispatchLedger { .. })
        ));
        assert!(matches!(
            derive(
                &[
                    completion(1, "m3-s1", 2)?,
                    dispatch(2, "m3-s1", "step-executor", "abc")?
                ],
                &[],
                &[],
                &[]
            ),
            Err(RunStateError::DispatchLedger { .. })
        ));
        assert!(matches!(
            derive(
                &[
                    dispatch(1, "m3-s1", "step-executor", "abc")?,
                    completion(2, "m3-s1", 1)?,
                    completion(3, "m3-s1", 1)?
                ],
                &[],
                &[],
                &[]
            ),
            Err(RunStateError::DispatchLedger { .. })
        ));
        assert!(matches!(
            derive(
                &[
                    dispatch(1, "m3-s1", "step-executor", "abc")?,
                    completion(2, "m3-s2", 1)?
                ],
                &[],
                &[],
                &[]
            ),
            Err(RunStateError::DispatchLedger { .. })
        ));
        Ok(())
    }

    fn digest(byte: char) -> Result<Sha256Digest, Box<dyn Error>> {
        Ok(Sha256Digest::parse(&byte.to_string().repeat(64))?)
    }

    fn canonical_node(value: &str) -> Result<CanonicalNode, Box<dyn Error>> {
        let node = NodeId::parse(value)?;
        Ok(CanonicalNode::Step(StepNode::parse(&node)?))
    }

    fn candidate(node: &str, repository: &str) -> Result<DispatchCandidate, Box<dyn Error>> {
        Ok(DispatchCandidate::new(
            canonical_node(node)?,
            RepositoryName::new(repository),
        ))
    }

    fn provenance(
        path: &str,
        approved_digest: Sha256Digest,
        condition: ArtifactProvenanceCondition,
    ) -> Result<ArtifactProvenance, Box<dyn Error>> {
        Ok(ArtifactProvenance {
            path: ArtifactPath::new(path),
            approved_digest,
            approval_node: NodeId::parse("m2-s1")?,
            approval_sequence: Sequence::parse(1)?,
            condition,
        })
    }

    fn approval(
        sequence: u64,
        node: &str,
        path: &str,
        sha256: Sha256Digest,
    ) -> Result<EventRecord, Box<dyn Error>> {
        event(
            sequence,
            node,
            KnownPayload::PlanningArtifactApproved(PlanningArtifactApprovedPayload {
                path: ArtifactPath::new(path),
                sha256,
                evidence: Evidence::parse("digest evidence")?,
            }),
        )
    }

    fn not_merged_authority(node: &str) -> Result<StepAuthorityObservation, Box<dyn Error>> {
        Ok(StepAuthorityObservation::new(
            NodeId::parse(node)?,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        ))
    }

    fn merged_authority(
        vision: &VisionSlug,
        node: &str,
    ) -> Result<StepAuthorityObservation, Box<dyn Error>> {
        let node_id = NodeId::parse(node)?;
        let merge_subject = MergeSubject::derive(vision, StepNode::parse(&node_id)?);
        let squash_commit = SquashCommitOid::parse("merged-oid")?;
        Ok(StepAuthorityObservation::new(
            node_id,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: identity(merge_subject.selector())?,
                    state: ExactPullRequestState::Merged {
                        squash_commit: squash_commit.clone(),
                    },
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable { squash_commit },
            },
        ))
    }

    fn derive(
        records: &[EventRecord],
        artifacts: &[CurrentArtifactObservation],
        repositories: &[RepositoryObservation],
        authorities: &[StepAuthorityObservation],
    ) -> Result<crate::run_state::DerivedRunState, RunStateError> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        derive_run_state(
            records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            artifacts,
            repositories,
            authorities,
        )
    }

    fn rich_render_state() -> Result<crate::run_state::DerivedRunState, Box<dyn Error>> {
        let mut records = vec![
            event(
                1,
                "m2-s1",
                KnownPayload::Dispatch(DispatchPayload {
                    role: DispatchRole::new("step-executor"),
                    r#ref: DispatchRef::new("dispatch-1"),
                    evidence: Evidence::parse("dispatch evidence 1")?,
                }),
            )?,
            event(
                2,
                "m2-s3",
                KnownPayload::Dispatch(DispatchPayload {
                    role: DispatchRole::new("pr-reviewer"),
                    r#ref: DispatchRef::new("review-1"),
                    evidence: Evidence::parse("dispatch evidence 2")?,
                }),
            )?,
            event(
                3,
                "m2-s2",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("release"),
                    question: "Approve?".to_owned(),
                }),
            )?,
            event(
                4,
                "m2-s2",
                KnownPayload::EscalationClose(EscalationClosePayload {
                    key: EscalationKey::new("release"),
                    resolution: "approved".to_owned(),
                }),
            )?,
            event(
                5,
                "m2-s3",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("network"),
                    question: "Retry?".to_owned(),
                }),
            )?,
        ];
        for (sequence, node, path, digest_byte, evidence) in [
            (6, "m2-s1", "planning/match.md", 'a', "approve match"),
            (7, "m2-s2", "planning/mismatch.md", 'b', "approve mismatch"),
            (8, "m2-s3", "planning/missing.md", 'd', "approve missing"),
        ] {
            records.push(event(
                sequence,
                node,
                KnownPayload::PlanningArtifactApproved(PlanningArtifactApprovedPayload {
                    path: ArtifactPath::new(path),
                    sha256: digest(digest_byte)?,
                    evidence: Evidence::parse(evidence)?,
                }),
            )?);
        }
        records.push(event(
            9,
            "m2-s3",
            KnownPayload::KeyFinding(crate::event_log::KeyFindingPayload {
                finding: "fact".to_owned(),
                evidence: Evidence::parse("git rev-parse HEAD\ncargo test --workspace")?,
            }),
        )?);
        for sequence in 10..=30 {
            records.push(event(
                sequence,
                "m2-s3",
                KnownPayload::Delta(DeltaPayload {
                    message: format!("delta-{sequence}"),
                }),
            )?);
        }

        let artifacts = [
            CurrentArtifactObservation::new(
                ArtifactPath::new("planning/match.md"),
                CurrentArtifactState::Present {
                    digest: digest('a')?,
                },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("planning/mismatch.md"),
                CurrentArtifactState::Present {
                    digest: digest('c')?,
                },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("planning/missing.md"),
                CurrentArtifactState::Missing,
            ),
        ];
        let fetched_at = EventTimestamp::parse("2026-07-27T12:34:56.123Z")?;
        let repositories = [
            RepositoryObservation::new(
                RepositoryName::new("pce"),
                RepositoryFetchObservation::Observed {
                    observation_ref: RepositoryObservationRef::parse(
                        "origin/pce/event-log-and-derived-run-state/milestone-2",
                    )?,
                    fetched_at,
                },
                RepositoryBranchName::parse("pce/event-log-and-derived-run-state/milestone-2")?,
                BranchState::Present,
                WorktreeIdentity::parse("pce/event-log-and-derived-run-state/m2-s6")?,
                WorktreeState::Absent,
                TagName::parse("v0.1.16")?,
                TagState::PointsTo {
                    target: TagTarget::parse("release-oid")?,
                },
            ),
            RepositoryObservation::new(
                RepositoryName::new("docs"),
                RepositoryFetchObservation::Unavailable {
                    failure: RepositoryObservationFailure::parse("offline")?,
                },
                RepositoryBranchName::parse("pce/event-log-and-derived-run-state/milestone-2")?,
                BranchState::Absent,
                WorktreeIdentity::parse("pce/event-log-and-derived-run-state/m2-s6")?,
                WorktreeState::Present,
                TagName::parse("v0.1.16")?,
                TagState::Absent,
            ),
        ];
        let vision = VisionSlug::parse("2026-07-27-event-log-and-derived-run-state")?;
        let merged_subject =
            MergeSubject::derive(&vision, StepNode::parse(&NodeId::parse("m2-s1")?)?);
        let merged_oid = SquashCommitOid::parse("merge-oid")?;
        let authorities = [
            StepAuthorityObservation::new(
                NodeId::parse("m2-s1")?,
                GitHubAuthorityObservation::Reachable {
                    observation: GitHubPullRequestObservation::OneExactMatch {
                        identity: ExactPullRequestIdentity::from_selector(
                            PullRequestNumber::parse(53)?,
                            merged_subject.selector(),
                        ),
                        state: ExactPullRequestState::Merged {
                            squash_commit: merged_oid.clone(),
                        },
                    },
                },
                GitAuthorityObservation::Reachable {
                    observation: GitMergeObservation::SquashCommitReachable {
                        squash_commit: merged_oid,
                    },
                },
            ),
            not_merged_authority("m2-s2")?,
            StepAuthorityObservation::new(
                NodeId::parse("m2-s3")?,
                GitHubAuthorityObservation::Unreachable {
                    failure: AuthorityFailure::parse("gh offline")?,
                },
                GitAuthorityObservation::Unreachable {
                    failure: AuthorityFailure::parse(
                        "GitHub authority unavailable before git reachability selection",
                    )?,
                },
            ),
        ];
        Ok(derive_run_state(
            &records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &artifacts,
            &repositories,
            &authorities,
        )?)
    }

    #[test]
    fn returns_multiple_dispatchable_candidates_when_edges_permit() -> Result<(), Box<dyn Error>> {
        let first = candidate("m2-s1", "pce")?;
        let second = candidate("m2-s2", "pce")?;
        let third = candidate("m2-s3", "pce")?;
        let fourth = candidate("m2-s4", "pce")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            &[first.clone(), second.clone(), third.clone(), fourth.clone()],
            &[],
            &[
                (first.node().clone(), MergeStatus::NotMerged),
                (second.node().clone(), MergeStatus::NotMerged),
                (third.node().clone(), MergeStatus::NotMerged),
                (fourth.node().clone(), MergeStatus::NotMerged),
            ],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(
            results,
            vec![
                DispatchabilityResult::Dispatchable { candidate: first },
                DispatchabilityResult::Dispatchable { candidate: second },
                DispatchabilityResult::Dispatchable { candidate: third },
                DispatchabilityResult::Dispatchable { candidate: fourth },
            ]
        );
        Ok(())
    }

    #[test]
    fn preserves_all_three_dependency_classifications_with_inconclusive_precedence()
    -> Result<(), Box<dyn Error>> {
        let ready = candidate("m2-s1", "pce")?;
        let waiting = candidate("m2-s2", "pce")?;
        let inconclusive = candidate("m2-s3", "pce")?;
        let merged_dependency = canonical_node("m1-s1")?;
        let not_merged_dependency = canonical_node("m1-s2")?;
        let inconclusive_dependency = canonical_node("m1-s3")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            &[ready.clone(), waiting.clone(), inconclusive.clone()],
            &[
                OrderingEdge::new(ready.node().clone(), merged_dependency.clone()),
                OrderingEdge::new(waiting.node().clone(), not_merged_dependency.clone()),
                OrderingEdge::new(inconclusive.node().clone(), not_merged_dependency.clone()),
                OrderingEdge::new(inconclusive.node().clone(), inconclusive_dependency.clone()),
            ],
            &[
                (ready.node().clone(), MergeStatus::NotMerged),
                (waiting.node().clone(), MergeStatus::NotMerged),
                (inconclusive.node().clone(), MergeStatus::NotMerged),
                (merged_dependency, MergeStatus::Merged),
                (not_merged_dependency, MergeStatus::NotMerged),
                (inconclusive_dependency, MergeStatus::Inconclusive),
            ],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(
            results,
            vec![
                DispatchabilityResult::Dispatchable { candidate: ready },
                DispatchabilityResult::Waiting { candidate: waiting },
                DispatchabilityResult::DependencyInconclusive {
                    candidate: inconclusive
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn merged_zero_dependency_candidate_waits() -> Result<(), Box<dyn Error>> {
        let candidate = candidate("m2-s1", "pce")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[],
            &[(candidate.node().clone(), MergeStatus::Merged)],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(results, vec![DispatchabilityResult::Waiting { candidate }]);
        Ok(())
    }

    #[test]
    fn resolves_non_candidate_merged_predecessor_from_status_lookup() -> Result<(), Box<dyn Error>>
    {
        let candidate = candidate("m2-s1", "pce")?;
        let predecessor = canonical_node("m1-s1")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[OrderingEdge::new(
                candidate.node().clone(),
                predecessor.clone(),
            )],
            &[
                (candidate.node().clone(), MergeStatus::NotMerged),
                (predecessor, MergeStatus::Merged),
            ],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(
            results,
            vec![DispatchabilityResult::Dispatchable { candidate }]
        );
        Ok(())
    }

    #[test]
    fn serializes_dispatches_per_repository_without_rewriting_inconclusive()
    -> Result<(), Box<dyn Error>> {
        let a1 = candidate("m3-s1", "repository-a")?;
        let a_inconclusive = candidate("m2-s1", "repository-a")?;
        let a2 = candidate("m1-s1", "repository-a")?;
        let b1 = candidate("m3-s2", "repository-b")?;
        let b2 = candidate("m1-s2", "repository-b")?;
        let dependency = canonical_node("m4-s1")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            &[
                a1.clone(),
                a_inconclusive.clone(),
                a2.clone(),
                b1.clone(),
                b2.clone(),
            ],
            &[OrderingEdge::new(
                a_inconclusive.node().clone(),
                dependency.clone(),
            )],
            &[
                (a1.node().clone(), MergeStatus::NotMerged),
                (a_inconclusive.node().clone(), MergeStatus::NotMerged),
                (a2.node().clone(), MergeStatus::NotMerged),
                (b1.node().clone(), MergeStatus::NotMerged),
                (b2.node().clone(), MergeStatus::NotMerged),
                (dependency, MergeStatus::Inconclusive),
            ],
            &[
                (
                    RepositoryName::new("repository-a"),
                    VersionPolicy::SerializeDispatches,
                ),
                (
                    RepositoryName::new("repository-b"),
                    VersionPolicy::SerializeDispatches,
                ),
            ],
        )?;

        assert_eq!(
            results,
            vec![
                DispatchabilityResult::Dispatchable { candidate: a1 },
                DispatchabilityResult::DependencyInconclusive {
                    candidate: a_inconclusive
                },
                DispatchabilityResult::Waiting { candidate: a2 },
                DispatchabilityResult::Dispatchable { candidate: b1 },
                DispatchabilityResult::Waiting { candidate: b2 },
            ]
        );
        Ok(())
    }

    #[test]
    fn matching_provenance_allows_dispatchability_classification() -> Result<(), Box<dyn Error>> {
        let candidate = candidate("m2-s1", "pce")?;
        let provenance = provenance(
            "planning/selected.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[],
            &[(candidate.node().clone(), MergeStatus::NotMerged)],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(
            results,
            vec![DispatchabilityResult::Dispatchable { candidate }]
        );
        Ok(())
    }

    #[test]
    fn mismatched_provenance_returns_typed_digest_error() -> Result<(), Box<dyn Error>> {
        let candidate = candidate("m2-s1", "pce")?;
        let path = ArtifactPath::new("planning/recognizable-mismatch.md");
        let approved = digest('a')?;
        let current = digest('b')?;
        let provenance = provenance(
            path.as_str(),
            approved.clone(),
            ArtifactProvenanceCondition::DigestMismatch {
                approved: approved.clone(),
                current: current.clone(),
            },
        )?;
        let error = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[],
            &[(candidate.node().clone(), MergeStatus::NotMerged)],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )
        .unwrap_err();

        assert_eq!(
            error,
            RunStateError::PlanningArtifactDigestMismatch {
                path,
                approved_digest: approved,
                current_digest: current,
            }
        );
        Ok(())
    }

    #[test]
    fn missing_provenance_returns_typed_missing_artifact_error() -> Result<(), Box<dyn Error>> {
        let candidate = candidate("m2-s1", "pce")?;
        let path = ArtifactPath::new("planning/recognizable-missing.md");
        let approved = digest('c')?;
        let provenance = provenance(
            path.as_str(),
            approved.clone(),
            ArtifactProvenanceCondition::ArtifactMissing,
        )?;
        let error = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[],
            &[(candidate.node().clone(), MergeStatus::NotMerged)],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )
        .unwrap_err();

        assert_eq!(
            error,
            RunStateError::PlanningArtifactMissing {
                path,
                approved_digest: approved,
                current_digest: None,
            }
        );
        Ok(())
    }

    #[test]
    fn inconclusive_zero_dependency_candidate_remains_inconclusive() -> Result<(), Box<dyn Error>> {
        let candidate = candidate("m2-s1", "pce")?;
        let provenance = provenance(
            "planning/plan.md",
            digest('a')?,
            ArtifactProvenanceCondition::DigestMatches,
        )?;
        let results = compute_dispatchability(
            &provenance,
            std::slice::from_ref(&candidate),
            &[],
            &[(candidate.node().clone(), MergeStatus::Inconclusive)],
            &[(RepositoryName::new("pce"), VersionPolicy::None)],
        )?;

        assert_eq!(
            results,
            vec![DispatchabilityResult::DependencyInconclusive { candidate }]
        );
        Ok(())
    }

    #[test]
    fn derives_complete_worked_merge_subject_example() -> Result<(), Box<dyn Error>> {
        let subject = subject("2026-07-27-event-log-and-derived-run-state", "m2-s1")?;

        assert_eq!(subject.node().milestone().get(), 2);
        assert_eq!(subject.node().step().get(), 1);
        assert_eq!(
            subject.head().as_str(),
            "pce/event-log-and-derived-run-state/m2-s1"
        );
        assert_eq!(
            subject.integration_branch().as_str(),
            "pce/event-log-and-derived-run-state/milestone-2"
        );
        assert_eq!(subject.selector().head(), subject.head());
        assert_eq!(subject.selector().base(), subject.integration_branch());
        Ok(())
    }

    #[test]
    fn milestone_node_parses_only_strict_canonical_identifiers() -> Result<(), Box<dyn Error>> {
        let first = MilestoneNode::parse(&NodeId::parse("m1")?)?;
        assert_eq!(first.milestone().get(), 1);

        let maximum = MilestoneNode::parse(&NodeId::parse("m18446744073709551615")?)?;
        assert_eq!(maximum.milestone().get(), u64::MAX);

        assert!(matches!(
            MilestoneNode::parse(&NodeId::parse("m0")?),
            Err(RunStateError::ZeroMilestoneNodeComponent { value }) if value == "m0"
        ));
        for raw in ["m00", "m01"] {
            assert!(matches!(
                MilestoneNode::parse(&NodeId::parse(raw)?),
                Err(RunStateError::LeadingZeroMilestoneNodeComponent { value })
                    if value == raw
            ));
        }
        for raw in ["m", "m1-s1", "M1", "x1", "mA", "m+1", "m 1", "m١"] {
            assert!(matches!(
                MilestoneNode::parse(&NodeId::parse(raw)?),
                Err(RunStateError::MalformedMilestoneNode { value }) if value == raw
            ));
        }
        assert!(matches!(
            MilestoneNode::parse(&NodeId::parse("m18446744073709551616")?),
            Err(RunStateError::MilestoneNodeComponentOverflow { value })
                if value == "m18446744073709551616"
        ));
        Ok(())
    }

    #[test]
    fn canonical_node_variants_carry_typed_nodes() -> Result<(), Box<dyn Error>> {
        let milestone = CanonicalNode::Milestone(MilestoneNode::parse(&NodeId::parse("m2")?)?);
        let step = CanonicalNode::Step(StepNode::parse(&NodeId::parse("m2-s3")?)?);

        match milestone {
            CanonicalNode::Milestone(node) => assert_eq!(node.milestone().get(), 2),
            CanonicalNode::Step(_) => panic!("milestone variant changed"),
        }
        match step {
            CanonicalNode::Step(node) => {
                assert_eq!(node.milestone().get(), 2);
                assert_eq!(node.step().get(), 3);
            }
            CanonicalNode::Milestone(_) => panic!("step variant changed"),
        }
        Ok(())
    }

    #[test]
    fn derives_exact_milestone_merge_subject() -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-event-log-and-derived-run-state")?;
        let node = MilestoneNode::parse(&NodeId::parse("m2")?)?;
        let subject = MilestoneMergeSubject::derive(&vision, node);

        assert_eq!(subject.node().milestone().get(), 2);
        assert_eq!(
            subject.head().as_str(),
            "pce/event-log-and-derived-run-state/milestone-2"
        );
        assert_eq!(subject.base().as_str(), "main");
        assert_eq!(subject.selector().head(), subject.head());
        assert_eq!(subject.selector().base(), subject.base());
        Ok(())
    }

    #[test]
    fn milestone_merge_subject_selectors_differ_across_visions() -> Result<(), Box<dyn Error>> {
        let first_vision = VisionSlug::parse("2026-07-27-first")?;
        let second_vision = VisionSlug::parse("2026-07-27-second")?;
        let node = MilestoneNode::parse(&NodeId::parse("m2")?)?;
        let first = MilestoneMergeSubject::derive(&first_vision, node.clone());
        let second = MilestoneMergeSubject::derive(&second_vision, node);

        assert_ne!(first.selector(), second.selector());
        Ok(())
    }

    #[test]
    fn vision_slug_rejects_bad_prefix_and_empty_suffix() {
        for malformed in [
            "event-log",
            "2026-7-27-event-log",
            "202x-07-27-event-log",
            "2026_07-27-event-log",
            "é026-07-27-event-log",
        ] {
            assert!(
                VisionSlug::parse(malformed).is_err(),
                "accepted {malformed}"
            );
        }
        assert!(VisionSlug::parse("2026-07-27-").is_err());
        assert_eq!(
            VisionSlug::parse("2026-07-27-Raw_Suffix").unwrap().as_str(),
            "Raw_Suffix"
        );
    }

    #[test]
    fn step_node_rejects_malformed_zero_overflow_and_leading_zero() {
        let invalid = [
            "m0-s1",
            "m1-s0",
            "m01-s1",
            "m1-s01",
            "m-s1",
            "m1-s",
            "m1",
            "m1-s1-extra",
            "m1-s1-s2",
            "x1-s1",
            "mA-s1",
            "m1-sA",
            "m18446744073709551616-s1",
            "m1-s18446744073709551616",
        ];
        for raw in invalid {
            let node = NodeId::parse(raw).unwrap();
            assert!(StepNode::parse(&node).is_err(), "accepted {raw}");
        }
        let valid = NodeId::parse("m1-s1").unwrap();
        assert!(StepNode::parse(&valid).is_ok());
    }

    #[test]
    fn step_node_remains_strict_for_step_identifiers() -> Result<(), Box<dyn Error>> {
        let step = StepNode::parse(&NodeId::parse("m1-s1")?)?;
        assert_eq!(step.milestone().get(), 1);
        assert_eq!(step.step().get(), 1);

        assert!(matches!(
            StepNode::parse(&NodeId::parse("m1")?),
            Err(RunStateError::MalformedStepNode { value }) if value == "m1"
        ));
        assert!(matches!(
            StepNode::parse(&NodeId::parse("m0-s1")?),
            Err(RunStateError::ZeroStepNodeComponent {
                value,
                component: "milestone"
            }) if value == "m0-s1"
        ));
        assert!(matches!(
            StepNode::parse(&NodeId::parse("m1-s0")?),
            Err(RunStateError::ZeroStepNodeComponent {
                value,
                component: "step"
            }) if value == "m1-s0"
        ));
        assert!(matches!(
            StepNode::parse(&NodeId::parse("m01-s1")?),
            Err(RunStateError::LeadingZeroStepNodeComponent {
                value,
                component: "milestone"
            }) if value == "m01-s1"
        ));
        assert!(matches!(
            StepNode::parse(&NodeId::parse("m1-s01")?),
            Err(RunStateError::LeadingZeroStepNodeComponent {
                value,
                component: "step"
            }) if value == "m1-s01"
        ));
        Ok(())
    }

    #[test]
    fn exact_identity_is_constructed_from_subject_selector() -> Result<(), Box<dyn Error>> {
        let subject = subject("2026-07-27-example", "m3-s4")?;
        let identity = identity(subject.selector())?;

        assert_eq!(identity.number().get(), 17);
        assert_eq!(identity.selector(), subject.selector());
        Ok(())
    }

    #[test]
    fn boundary_values_reject_empty_and_preserve_bytes() {
        assert!(PullRequestNumber::parse(0).is_err());
        assert!(SquashCommitOid::parse("").is_err());
        assert!(AuthorityFailure::parse("").is_err());
        assert_eq!(
            SquashCommitOid::parse("not hex / still exact")
                .unwrap()
                .as_str(),
            "not hex / still exact"
        );
        assert_eq!(
            AuthorityFailure::parse("  detail\n").unwrap().as_str(),
            "  detail\n"
        );
    }

    #[test]
    fn merge_status_covers_complete_authority_truth_table() -> Result<(), Box<dyn Error>> {
        let subject = subject("2026-07-27-example", "m2-s1")?;
        let failure = AuthorityFailure::parse("unavailable")?;
        let squash_a = SquashCommitOid::parse("A")?;
        let squash_b = SquashCommitOid::parse("B")?;
        let exact_identity = identity(subject.selector())?;
        let github = [
            GitHubAuthorityObservation::Unreachable {
                failure: failure.clone(),
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::MultipleExactMatches,
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: exact_identity.clone(),
                    state: ExactPullRequestState::NotMerged,
                },
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: exact_identity,
                    state: ExactPullRequestState::Merged {
                        squash_commit: squash_a.clone(),
                    },
                },
            },
        ];
        let git = [
            GitAuthorityObservation::Unreachable { failure },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable {
                    squash_commit: squash_a,
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable {
                    squash_commit: squash_b,
                },
            },
        ];
        let expected = [
            [MergeStatus::Inconclusive; 4],
            [
                MergeStatus::Inconclusive,
                MergeStatus::NotMerged,
                MergeStatus::Inconclusive,
                MergeStatus::Inconclusive,
            ],
            [MergeStatus::Inconclusive; 4],
            [
                MergeStatus::Inconclusive,
                MergeStatus::NotMerged,
                MergeStatus::Inconclusive,
                MergeStatus::Inconclusive,
            ],
            [
                MergeStatus::Inconclusive,
                MergeStatus::Inconclusive,
                MergeStatus::Merged,
                MergeStatus::Inconclusive,
            ],
        ];

        for (github_index, github_observation) in github.iter().enumerate() {
            for (git_index, git_observation) in git.iter().enumerate() {
                assert_eq!(
                    derive_merge_status(&subject, github_observation, git_observation),
                    expected[github_index][git_index],
                    "truth-table cell ({github_index}, {git_index})"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn identity_mismatch_is_inconclusive() -> Result<(), Box<dyn Error>> {
        let merge_subject = subject("2026-07-27-example", "m2-s1")?;
        let other_subject = subject("2026-07-27-other", "m2-s1")?;
        let github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::OneExactMatch {
                identity: identity(other_subject.selector())?,
                state: ExactPullRequestState::NotMerged,
            },
        };
        let git = GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::NotMerged,
        };

        assert_eq!(
            derive_merge_status(&merge_subject, &github, &git),
            MergeStatus::Inconclusive
        );
        Ok(())
    }

    #[test]
    fn milestone_merge_status_uses_shared_three_valued_rules() -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let subject =
            MilestoneMergeSubject::derive(&vision, MilestoneNode::parse(&NodeId::parse("m2")?)?);
        let squash_a = SquashCommitOid::parse("A")?;
        let squash_b = SquashCommitOid::parse("B")?;
        let exact_identity = identity(subject.selector())?;

        let merged_github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::OneExactMatch {
                identity: exact_identity.clone(),
                state: ExactPullRequestState::Merged {
                    squash_commit: squash_a.clone(),
                },
            },
        };
        let merged_git = GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::SquashCommitReachable {
                squash_commit: squash_a,
            },
        };
        assert_eq!(
            derive_milestone_merge_status(&subject, &merged_github, &merged_git),
            MergeStatus::Merged
        );

        let not_merged_github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::OneExactMatch {
                identity: exact_identity.clone(),
                state: ExactPullRequestState::NotMerged,
            },
        };
        let not_merged_git = GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::NotMerged,
        };
        assert_eq!(
            derive_milestone_merge_status(&subject, &not_merged_github, &not_merged_git),
            MergeStatus::NotMerged
        );

        let oid_mismatch_github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::OneExactMatch {
                identity: exact_identity,
                state: ExactPullRequestState::Merged {
                    squash_commit: SquashCommitOid::parse("A")?,
                },
            },
        };
        let oid_mismatch_git = GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::SquashCommitReachable {
                squash_commit: squash_b,
            },
        };
        assert_eq!(
            derive_milestone_merge_status(&subject, &oid_mismatch_github, &oid_mismatch_git),
            MergeStatus::Inconclusive
        );
        Ok(())
    }

    #[test]
    fn issuance_ordinals_join_exact_node_and_role_and_preserve_all_dispatches()
    -> Result<(), Box<dyn Error>> {
        let records = vec![
            dispatch(1, "m2-s1", "step-planner", "plan-1")?,
            dispatch(2, "m2-s1", "step-critic", "critic-1")?,
            dispatch(3, "m2-s1", "step-planner", "plan-2")?,
            dispatch(4, "m2-s1", "step-critic", "critic-2")?,
            dispatch(5, "m2-s1", "step-critic", "critic-3")?,
            dispatch(6, "m2-s2", "step-planner", "other-node")?,
            dispatch(7, "m2-s1", "repository-analyst", "analyst-ref")?,
            dispatch(8, "m2-s1", "step-planer", "misspelled-ref")?,
            dispatch(9, "m2-s1", "falsification-critic", "falsification-1")?,
            dispatch(10, "m2-s1", "falsification-critic", "falsification-2")?,
        ];

        let state = derive(&records, &[], &[], &[])?;
        assert_eq!(state.dispatches().len(), 10);
        assert_eq!(state.dispatches()[6].role().as_str(), "repository-analyst");
        assert_eq!(state.dispatches()[6].sequence().get(), 7);
        assert_eq!(state.dispatches()[6].node().as_str(), "m2-s1");
        assert_eq!(state.dispatches()[6].dispatch_ref().as_str(), "analyst-ref");
        assert_eq!(state.dispatches()[7].role().as_str(), "step-planer");
        assert_eq!(state.dispatches()[7].sequence().get(), 8);
        assert_eq!(state.dispatches()[7].node().as_str(), "m2-s1");
        assert_eq!(
            state.dispatches()[7].dispatch_ref().as_str(),
            "misspelled-ref"
        );
        assert_eq!(state.issuance_ordinals().len(), 6);
        assert_eq!(state.issuance_ordinals()[0].ordinal().get(), 2);
        assert_eq!(state.issuance_ordinals()[1].ordinal().get(), 3);
        assert_eq!(state.issuance_ordinals()[2].ordinal().get(), 1);
        assert_eq!(state.issuance_ordinals()[3].ordinal().get(), 1);
        assert_eq!(state.issuance_ordinals()[4].ordinal().get(), 1);
        assert_eq!(state.issuance_ordinals()[5].ordinal().get(), 2);
        assert!(state.rounds().is_empty());
        Ok(())
    }

    #[test]
    fn pinned_role_vocabulary_classifies_without_creating_unknown_series()
    -> Result<(), Box<dyn Error>> {
        let fixtures = [
            ("milestone-planner", DispatchRoleClass::PlanProducing),
            ("step-planner", DispatchRoleClass::PlanProducing),
            ("step-plan-writer", DispatchRoleClass::PlanProducing),
            ("milestone-critic", DispatchRoleClass::CritiqueProducing),
            ("step-critic", DispatchRoleClass::CritiqueProducing),
            ("step-plan-critic", DispatchRoleClass::CritiqueProducing),
            ("falsification-critic", DispatchRoleClass::CritiqueProducing),
            ("pr-reviewer", DispatchRoleClass::CritiqueProducing),
            ("step-executor", DispatchRoleClass::Execution),
            (
                "repository-analyst",
                DispatchRoleClass::ExplicitlyNonRoundBearing,
            ),
            ("Step-Executor", DispatchRoleClass::Unrecognized),
        ];
        let mut records = Vec::new();
        for (index, (role, expected)) in fixtures.iter().enumerate() {
            assert_eq!(
                DispatchRoleClass::classify(&DispatchRole::new(*role)),
                *expected
            );
            records.push(dispatch((index + 1) as u64, "m2-s1", role, role)?);
        }

        let state = derive(&records, &[], &[], &[])?;
        assert_eq!(state.issuance_ordinals().len(), 11);
        assert!(state.rounds().is_empty());
        assert_eq!(state.dispatches().len(), 11);
        Ok(())
    }

    #[test]
    fn hold_fold_tracks_open_close_reopen_and_rejects_close_before_open()
    -> Result<(), Box<dyn Error>> {
        let records = vec![
            event(
                1,
                "m2-s1",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("first"),
                    question: "q1".to_owned(),
                }),
            )?,
            event(
                2,
                "m2-s2",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("second"),
                    question: "q2".to_owned(),
                }),
            )?,
            event(
                3,
                "m2-s1",
                KnownPayload::EscalationClose(EscalationClosePayload {
                    key: EscalationKey::new("first"),
                    resolution: "done".to_owned(),
                }),
            )?,
            event(
                4,
                "m2-s3",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("first"),
                    question: "again".to_owned(),
                }),
            )?,
        ];
        let state = derive(&records, &[], &[], &[])?;
        assert_eq!(state.holds().len(), 2);
        assert_eq!(state.holds()[0].key().as_str(), "first");
        assert!(matches!(
            state.holds()[0].status(),
            HoldStatus::Open { node, sequence, question }
                if node.as_str() == "m2-s3" && sequence.get() == 4 && question == "again"
        ));
        assert_eq!(state.holds()[1].key().as_str(), "second");

        let invalid = vec![event(
            9,
            "m2-s1",
            KnownPayload::EscalationClose(EscalationClosePayload {
                key: EscalationKey::new("missing"),
                resolution: "none".to_owned(),
            }),
        )?];
        assert!(matches!(
            derive(&invalid, &[], &[], &[]),
            Err(RunStateError::EscalationCloseBeforeOpen { key, sequence })
                if key.as_str() == "missing" && sequence.get() == 9
        ));
        Ok(())
    }

    #[test]
    fn provenance_distinguishes_match_mismatch_missing_and_latest_approval()
    -> Result<(), Box<dyn Error>> {
        let a = digest('a')?;
        let b = digest('b')?;
        let c = digest('c')?;
        let records = vec![
            approval(1, "m2-s1", "match", a.clone())?,
            approval(2, "m2-s1", "mismatch", a.clone())?,
            approval(3, "m2-s1", "missing", a.clone())?,
            approval(4, "m2-s2", "mismatch", b.clone())?,
        ];
        let artifacts = vec![
            CurrentArtifactObservation::new(
                ArtifactPath::new("match"),
                CurrentArtifactState::Present { digest: a },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("mismatch"),
                CurrentArtifactState::Present { digest: c.clone() },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("missing"),
                CurrentArtifactState::Missing,
            ),
        ];
        let authority = not_merged_authority("m2-s1")?;
        let state = derive(&records, &artifacts, &[], &[authority])?;
        assert_eq!(state.provenance().len(), 3);
        assert_eq!(
            state.provenance()[0].condition(),
            &ArtifactProvenanceCondition::DigestMatches
        );
        assert_eq!(state.provenance()[1].approval_sequence().get(), 4);
        assert_eq!(state.provenance()[1].approval_node().as_str(), "m2-s2");
        assert_eq!(state.provenance()[1].approved_digest(), &b);
        assert_eq!(
            state.provenance()[1].condition(),
            &ArtifactProvenanceCondition::DigestMismatch {
                approved: b,
                current: c,
            }
        );
        assert_eq!(
            state.provenance()[2].condition(),
            &ArtifactProvenanceCondition::ArtifactMissing
        );
        assert_eq!(state.steps()[0].status(), MergeStatus::NotMerged);
        Ok(())
    }

    #[test]
    fn artifact_cross_input_errors_fail_loudly() -> Result<(), Box<dyn Error>> {
        let records = vec![approval(1, "m2-s1", "approved", digest('a')?)?];
        assert!(matches!(
            derive(&records, &[], &[], &[]),
            Err(RunStateError::MissingCurrentArtifactObservation { path })
                if path.as_str() == "approved"
        ));

        let observations = vec![
            CurrentArtifactObservation::new(
                ArtifactPath::new("approved"),
                CurrentArtifactState::Missing,
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("approved"),
                CurrentArtifactState::Missing,
            ),
        ];
        assert!(matches!(
            derive(&records, &observations, &[], &[]),
            Err(RunStateError::DuplicateCurrentArtifactObservation { path })
                if path.as_str() == "approved"
        ));
        Ok(())
    }

    #[test]
    fn repository_observations_project_in_order_and_reject_duplicates() -> Result<(), Box<dyn Error>>
    {
        let timestamp = EventTimestamp::parse("2026-07-27T12:34:56.000Z")?;
        let first = RepositoryObservation::new(
            RepositoryName::new("pce"),
            RepositoryFetchObservation::Observed {
                observation_ref: RepositoryObservationRef::parse("abc")?,
                fetched_at: timestamp,
            },
            RepositoryBranchName::parse("pce/example/milestone-2")?,
            BranchState::Present,
            WorktreeIdentity::parse("m2-s2")?,
            WorktreeState::Present,
            TagName::parse("v1")?,
            TagState::PointsTo {
                target: TagTarget::parse("def")?,
            },
        );
        let second = RepositoryObservation::new(
            RepositoryName::new("other"),
            RepositoryFetchObservation::Unavailable {
                failure: RepositoryObservationFailure::parse("offline")?,
            },
            RepositoryBranchName::parse("main")?,
            BranchState::Absent,
            WorktreeIdentity::parse("other-worktree")?,
            WorktreeState::Absent,
            TagName::parse("v2")?,
            TagState::Absent,
        );
        let state = derive(&[], &[], &[first.clone(), second.clone()], &[])?;
        assert_eq!(state.repositories(), &[first.clone(), second]);
        assert_eq!(first.branch_state(), BranchState::Present);
        assert_eq!(first.worktree_state(), WorktreeState::Present);
        assert!(
            matches!(first.tag_state(), TagState::PointsTo { target } if target.as_str() == "def")
        );

        assert!(matches!(
            derive(&[], &[], &[first.clone(), first], &[]),
            Err(RunStateError::DuplicateRepositoryObservation { repository })
                if repository.as_str() == "pce"
        ));
        Ok(())
    }

    #[test]
    fn step_results_derive_identity_truth_and_validate_authority_inputs()
    -> Result<(), Box<dyn Error>> {
        let records = vec![dispatch(
            1,
            "m2-s3",
            "step-executor",
            "payload-not-identity",
        )?];
        let authority = not_merged_authority("m2-s3")?;
        let state = derive(&records, &[], &[], std::slice::from_ref(&authority))?;
        let step = &state.steps()[0];
        assert_eq!(step.node().as_str(), "m2-s3");
        assert_eq!(step.subject().head().as_str(), "pce/example/m2-s3");
        assert_eq!(
            step.subject().integration_branch().as_str(),
            "pce/example/milestone-2"
        );
        assert_eq!(step.status(), MergeStatus::NotMerged);

        assert!(matches!(
            derive(&records, &[], &[], &[authority.clone(), authority]),
            Err(RunStateError::DuplicateStepAuthorityObservation { node })
                if node.as_str() == "m2-s3"
        ));
        assert!(matches!(
            derive(&records, &[], &[], &[not_merged_authority("m2-s4")?]),
            Err(RunStateError::AuthorityNodeAbsentFromLog { node })
                if node.as_str() == "m2-s4"
        ));
        let malformed = StepAuthorityObservation::new(
            NodeId::parse("not-a-step")?,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        );
        assert!(matches!(
            derive(&records, &[], &[], &[malformed]),
            Err(RunStateError::MalformedStepNode { value }) if value == "not-a-step"
        ));
        Ok(())
    }

    #[test]
    fn resume_excludes_only_merged_and_selects_latest_visible_candidate()
    -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let records = vec![
            delta(1, "m2-s1")?,
            delta(2, "m2-s2")?,
            delta(3, "general-node")?,
            delta(4, "m2-s1")?,
            delta(5, "m2-s3")?,
        ];
        let failure = AuthorityFailure::parse("unavailable")?;
        let authorities = vec![
            merged_authority(&vision, "m2-s1")?,
            not_merged_authority("m2-s2")?,
            StepAuthorityObservation::new(
                NodeId::parse("m2-s3")?,
                GitHubAuthorityObservation::Unreachable {
                    failure: failure.clone(),
                },
                GitAuthorityObservation::Unreachable { failure },
            ),
        ];
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &authorities,
        )?;
        assert!(matches!(
            state.resume(),
            ResumeObservation::Candidate { node, latest_sequence, cycle_position: CyclePosition::NoRoundDispatch }
                if node.as_str() == "m2-s3" && latest_sequence.get() == 5
        ));
        assert_eq!(state.steps()[1].status(), MergeStatus::NotMerged);
        assert_eq!(state.steps()[2].status(), MergeStatus::Inconclusive);

        let no_authority = derive(&[delta(1, "m9-s9")?], &[], &[], &[])?;
        assert!(matches!(
            no_authority.resume(),
            ResumeObservation::Candidate { node, .. } if node.as_str() == "m9-s9"
        ));

        let all_merged_records = vec![delta(1, "m2-s1")?];
        let all_merged = derive_run_state(
            &all_merged_records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[merged_authority(&vision, "m2-s1")?],
        )?;
        assert_eq!(
            all_merged.resume(),
            &ResumeObservation::NoLogVisibleCandidate
        );
        assert_eq!(
            derive(&[], &[], &[], &[])?.resume(),
            &ResumeObservation::NoLogVisibleCandidate
        );
        Ok(())
    }

    #[test]
    fn resume_cycle_position_uses_latest_recognized_exact_role_mapping()
    -> Result<(), Box<dyn Error>> {
        let cases = [
            (
                "milestone-planner",
                CyclePosition::PlanDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "step-planner",
                CyclePosition::PlanDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "step-plan-writer",
                CyclePosition::PlanDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "milestone-critic",
                CyclePosition::CritiqueDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "step-critic",
                CyclePosition::CritiqueDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "step-plan-critic",
                CyclePosition::CritiqueDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "step-executor",
                CyclePosition::ExecutionDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "pr-reviewer",
                CyclePosition::ReviewDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
            (
                "falsification-critic",
                CyclePosition::FalsificationDispatched {
                    sequence: Sequence::parse(1)?,
                },
            ),
        ];
        for (role, expected) in cases {
            let records = vec![
                dispatch(1, "node", role, "ref")?,
                dispatch(2, "node", "repository-analyst", "analyst")?,
                dispatch(3, "node", "unknown", "unknown")?,
                delta(4, "node")?,
            ];
            let state = derive(&records, &[], &[], &[])?;
            assert!(matches!(
                state.resume(),
                ResumeObservation::Candidate { cycle_position, latest_sequence, .. }
                    if *cycle_position == expected && latest_sequence.get() == 4
            ));
            if role == "falsification-critic" {
                let snapshot = RunSnapshot::from(&state);
                let serialized = serde_json::to_value(&snapshot)?;
                assert_eq!(
                    serialized["resume"]["cycle_position"],
                    serde_json::json!({
                        "state": "falsification-dispatched",
                        "sequence": 1
                    })
                );
                assert!(snapshot_validator()?.is_valid(&serialized));
            }
        }
        Ok(())
    }

    #[test]
    fn sequence_order_is_strict_and_core_never_sorts() -> Result<(), Box<dyn Error>> {
        let records = vec![delta(2, "later")?, delta(1, "earlier")?];
        assert!(matches!(
            derive(&records, &[], &[], &[]),
            Err(RunStateError::DispatchLedger { source: crate::dispatch_ledger::DispatchLedgerError::NonIncreasingSequence { previous_sequence, sequence } })
                if previous_sequence.get() == 2 && sequence.get() == 1
        ));
        let duplicate = vec![delta(1, "a")?, delta(1, "b")?];
        assert!(matches!(
            derive(&duplicate, &[], &[], &[]),
            Err(RunStateError::DispatchLedger { source: crate::dispatch_ledger::DispatchLedgerError::NonIncreasingSequence { previous_sequence, sequence } })
                if previous_sequence.get() == 1 && sequence.get() == 1
        ));
        Ok(())
    }

    #[test]
    fn merged_identity_mismatch_is_inconclusive() -> Result<(), Box<dyn Error>> {
        let merge_subject = subject("2026-07-27-example", "m2-s1")?;
        let other_subject = subject("2026-07-27-other", "m2-s1")?;
        let squash_commit = SquashCommitOid::parse("A")?;
        let github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::OneExactMatch {
                identity: identity(other_subject.selector())?,
                state: ExactPullRequestState::Merged {
                    squash_commit: squash_commit.clone(),
                },
            },
        };
        let git = GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::SquashCommitReachable { squash_commit },
        };

        assert_eq!(
            derive_merge_status(&merge_subject, &github, &git),
            MergeStatus::Inconclusive
        );
        Ok(())
    }

    fn snapshot_validator() -> Result<jsonschema::Validator, Box<dyn Error>> {
        let schema = serde_json::from_str(RUN_SNAPSHOT_SCHEMA)?;
        Ok(jsonschema::validator_for(&schema)?)
    }

    #[test]
    fn recovery_digest_uses_same_fold_content_and_schema() -> Result<(), Box<dyn Error>> {
        let lines = [
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch","node":"m2-s1","payload":{"role":"step-planner","ref":"plan-ref","evidence":"git rev-parse HEAD"}}"#,
            r#"{"sequence":2,"timestamp":"2026-07-27T12:34:56.000Z","kind":"escalation-open","node":"m2-s1","payload":{"key":"review","question":"Proceed?"}}"#,
            r#"{"sequence":3,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m2-s1","payload":{"message":"changed"}}"#,
            r#"{"sequence":4,"timestamp":"2026-07-27T12:34:56.000Z","kind":"key-finding","node":"m2-s1","payload":{"finding":"measured","evidence":"git status --short"}}"#,
            r#"{"sequence":5,"timestamp":"2026-07-27T12:34:56.000Z","kind":"repository-contract","node":"m2-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"all gates pass","install":"none","evidence":"git rev-parse --show-toplevel"}}"#,
            r#"{"sequence":6,"timestamp":"2026-07-27T12:34:56.000Z","kind":"planning-artifact-approved","node":"m2-s1","payload":{"path":"planning/steps.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","evidence":"shasum -a 256 planning/steps.json"}}"#,
            r#"{"sequence":7,"timestamp":"2026-07-27T12:34:56.000Z","kind":"escalation-close","node":"m2-s1","payload":{"key":"review","resolution":"done"}}"#,
            r#"{"sequence":8,"timestamp":"2026-07-27T12:34:56.000Z","kind":"escalation-open","node":"m2-s2","payload":{"key":"review","question":"Again?"}}"#,
        ];
        let records = lines
            .into_iter()
            .map(crate::event_log::parse_event_line)
            .collect::<Result<Vec<_>, _>>()?;
        let artifacts = [CurrentArtifactObservation::new(
            ArtifactPath::new("planning/steps.json"),
            CurrentArtifactState::Present {
                digest: digest('a')?,
            },
        )];
        let authorities = [
            not_merged_authority("m2-s1")?,
            not_merged_authority("m2-s2")?,
        ];
        let state = derive(&records, &artifacts, &[], &authorities)?;
        let value = serde_json::to_value(RunSnapshot::from(&state))?;
        let recovery = &value["recovery_digest"];

        assert_eq!(
            recovery["rounds"]["entries"],
            serde_json::json!([{
                "sequence": 1, "node": "m2-s1", "role": "step-planner", "round_number": 1
            }])
        );
        assert_eq!(
            recovery["open_holds"]["entries"],
            serde_json::json!([{
                "sequence": 8, "node": "m2-s2", "key": "review", "question": "Again?"
            }])
        );
        assert_eq!(
            recovery["deltas"]["entries"],
            serde_json::json!([{
                "sequence": 3, "node": "m2-s1", "message": "changed"
            }])
        );
        assert_eq!(
            recovery["facts"]["entries"],
            serde_json::json!([
                {"sequence": 1, "node": "m2-s1", "kind": "dispatch", "evidence": "git rev-parse HEAD"},
                {"sequence": 4, "node": "m2-s1", "kind": "key-finding", "evidence": "git status --short"},
                {"sequence": 5, "node": "m2-s1", "kind": "repository-contract", "evidence": "git rev-parse --show-toplevel"},
                {"sequence": 6, "node": "m2-s1", "kind": "planning-artifact-approved", "evidence": "shasum -a 256 planning/steps.json"}
            ])
        );
        for category in ["rounds", "open_holds", "deltas", "facts"] {
            assert_eq!(recovery[category]["elisions"], serde_json::json!([]));
        }
        assert!(snapshot_validator()?.is_valid(&value));
        Ok(())
    }

    #[test]
    fn recovery_digest_caps_each_category_independently() -> Result<(), Box<dyn Error>> {
        let mut records = Vec::new();
        for sequence in 1..=22 {
            records.push(dispatch(
                sequence,
                "m2-s1",
                "step-planner",
                &format!("ref-{sequence}"),
            )?);
        }
        for sequence in 23..=44 {
            records.push(event(
                sequence,
                "m2-s2",
                KnownPayload::Delta(DeltaPayload {
                    message: format!("delta-{sequence}"),
                }),
            )?);
        }
        for sequence in 45..=66 {
            records.push(event(
                sequence,
                if sequence % 2 == 1 { "m2-s1" } else { "m2-s2" },
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new(format!("hold-{sequence}")),
                    question: format!("question-{sequence}"),
                }),
            )?);
        }
        let authorities = [
            not_merged_authority("m2-s1")?,
            not_merged_authority("m2-s2")?,
        ];
        let state = derive(&records, &[], &[], &authorities)?;
        let value = serde_json::to_value(RunSnapshot::from(&state))?;
        let recovery = &value["recovery_digest"];

        for category in ["rounds", "facts"] {
            let entries = recovery[category]["entries"].as_array().expect("entries");
            assert_eq!(entries.len(), 20);
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| entry["sequence"].as_u64().expect("sequence"))
                    .collect::<Vec<_>>(),
                (3..=22).collect::<Vec<_>>()
            );
            assert_eq!(
                recovery[category]["elisions"],
                serde_json::json!([{
                    "omitted_count": 2,
                    "start_sequence": 1,
                    "end_sequence": 2,
                    "retrieval_commands": [
                        "pce log read --file 'events.jsonl' --kind dispatch --node 'm2-s1'"
                    ]
                }])
            );
        }
        assert_eq!(
            recovery["rounds"]["entries"]
                .as_array()
                .expect("round entries")
                .iter()
                .map(|entry| entry["round_number"].as_u64().expect("round number"))
                .collect::<Vec<_>>(),
            (3..=22).collect::<Vec<_>>()
        );
        assert!(
            recovery["facts"]["entries"]
                .as_array()
                .expect("fact entries")
                .iter()
                .all(|entry| entry["evidence"] == "dispatch evidence")
        );
        assert_eq!(
            recovery["deltas"]["entries"]
                .as_array()
                .expect("delta entries")
                .iter()
                .map(|entry| entry["sequence"].as_u64().expect("sequence"))
                .collect::<Vec<_>>(),
            (25..=44).collect::<Vec<_>>()
        );
        assert_eq!(recovery["deltas"]["entries"][0]["message"], "delta-25");
        assert_eq!(recovery["deltas"]["entries"][19]["message"], "delta-44");
        assert_eq!(
            recovery["deltas"]["elisions"],
            serde_json::json!([{
                "omitted_count": 2,
                "start_sequence": 23,
                "end_sequence": 24,
                "retrieval_commands": [
                    "pce log read --file 'events.jsonl' --kind delta --node 'm2-s2'"
                ]
            }])
        );
        assert_eq!(
            recovery["open_holds"]["entries"]
                .as_array()
                .expect("hold entries")
                .iter()
                .map(|entry| entry["sequence"].as_u64().expect("sequence"))
                .collect::<Vec<_>>(),
            (47..=66).collect::<Vec<_>>()
        );
        assert_eq!(
            recovery["open_holds"]["elisions"],
            serde_json::json!([{
                "omitted_count": 2,
                "start_sequence": 45,
                "end_sequence": 46,
                "retrieval_commands": [
                    "pce log read --file 'events.jsonl' --kind escalation-open --node 'm2-s1'",
                    "pce log read --file 'events.jsonl' --kind escalation-open --node 'm2-s2'"
                ]
            }])
        );
        assert!(snapshot_validator()?.is_valid(&value));
        Ok(())
    }

    #[test]
    fn recovery_elisions_use_actual_sequence_gaps_and_quote_shell_values()
    -> Result<(), Box<dyn Error>> {
        let mut records = vec![delta(1, "node")?, delta(3, "node")?, delta(4, "node")?];
        for sequence in 5..=24 {
            records.push(delta(sequence, "node")?);
        }
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &VisionSlug::parse("2026-07-27-example")?,
            &RecoveryLogPath::new("dir/it's log.jsonl"),
            &[],
            &[],
            &[],
        )?;
        let value = serde_json::to_value(RunSnapshot::from(&state))?;
        let elisions = &value["recovery_digest"]["deltas"]["elisions"];
        assert_eq!(elisions[0]["omitted_count"], 1);
        assert_eq!(elisions[0]["start_sequence"], 1);
        assert_eq!(elisions[0]["end_sequence"], 1);
        assert_eq!(elisions[1]["omitted_count"], 2);
        assert_eq!(elisions[1]["start_sequence"], 3);
        assert_eq!(elisions[1]["end_sequence"], 4);
        assert_eq!(
            elisions[0]["retrieval_commands"][0],
            "pce log read --file 'dir/it'\\''s log.jsonl' --kind delta --node 'node'"
        );
        assert_eq!(
            recovery_command(
                &RecoveryLogPath::new("dir/it's log.jsonl"),
                "delta",
                &NodeId::parse("m2-s1")?
            ),
            "pce log read --file 'dir/it'\\''s log.jsonl' --kind delta --node 'm2-s1'"
        );
        Ok(())
    }

    #[test]
    fn typed_rich_snapshot_conforms_and_pins_identity() -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let records = vec![
            dispatch(1, "m2-s1", "step-executor", "dispatch-ref")?,
            event(
                2,
                "m2-s1",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("review"),
                    question: "Proceed?".to_owned(),
                }),
            )?,
            approval(3, "m2-s1", "plan.json", digest('a')?)?,
            dispatch(4, "m2-s2", "step-planner", "next-ref")?,
        ];
        let repository = RepositoryObservation::new(
            RepositoryName::new("pce"),
            RepositoryFetchObservation::Observed {
                observation_ref: RepositoryObservationRef::parse("origin/pce/example/milestone-2")?,
                fetched_at: EventTimestamp::parse("2026-07-27T12:34:56.123Z")?,
            },
            RepositoryBranchName::parse("pce/example/milestone-2")?,
            BranchState::Present,
            WorktreeIdentity::parse("m2-s3")?,
            WorktreeState::Present,
            TagName::parse("v0.1.16")?,
            TagState::PointsTo {
                target: TagTarget::parse("commit-oid")?,
            },
        );
        let artifacts = [CurrentArtifactObservation::new(
            ArtifactPath::new("plan.json"),
            CurrentArtifactState::Present {
                digest: digest('a')?,
            },
        )];
        let authority = merged_authority(&vision, "m2-s1")?;
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &artifacts,
            &[repository],
            std::slice::from_ref(&authority),
        )?;
        assert_eq!(state.steps()[0].github(), authority.github());
        assert_eq!(state.steps()[0].git(), authority.git());

        let snapshot = RunSnapshot::from(&state);
        assert_eq!(snapshot.schema_id(), "pce.run-snapshot");
        assert_eq!(snapshot.schema_version(), 1);
        let value = serde_json::to_value(&snapshot)?;
        assert_eq!(value["schema_id"], "pce.run-snapshot");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(
            value["repositories"][0]["fetch"]["fetched_at"],
            "2026-07-27T12:34:56.123Z"
        );
        assert!(snapshot_validator()?.is_valid(&value));
        Ok(())
    }

    #[test]
    fn typed_snapshot_covers_unavailable_absent_and_not_merged_arms() -> Result<(), Box<dyn Error>>
    {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let records = vec![delta(1, "m2-s1")?];
        let subject = MergeSubject::derive(&vision, StepNode::parse(&NodeId::parse("m2-s1")?)?);
        let one_not_merged = StepAuthorityObservation::new(
            NodeId::parse("m2-s1")?,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: identity(subject.selector())?,
                    state: ExactPullRequestState::NotMerged,
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        );
        let repository = RepositoryObservation::new(
            RepositoryName::new(""),
            RepositoryFetchObservation::Unavailable {
                failure: RepositoryObservationFailure::parse("offline")?,
            },
            RepositoryBranchName::parse("missing-branch")?,
            BranchState::Absent,
            WorktreeIdentity::parse("missing-worktree")?,
            WorktreeState::Absent,
            TagName::parse("missing-tag")?,
            TagState::Absent,
        );
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[repository],
            &[one_not_merged],
        )?;
        let value = serde_json::to_value(RunSnapshot::from(&state))?;
        assert_eq!(
            value["steps"][0]["github"]["pull_request"]["state"]["status"],
            "not-merged"
        );
        assert_eq!(value["steps"][0]["git"]["state"], "not-merged");
        assert_eq!(value["steps"][0]["merge_status"], "not-merged");
        assert!(snapshot_validator()?.is_valid(&value));

        for github in [
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::MultipleExactMatches,
            },
            GitHubAuthorityObservation::Unreachable {
                failure: AuthorityFailure::parse("github offline")?,
            },
        ] {
            let authority = StepAuthorityObservation::new(
                NodeId::parse("m2-s1")?,
                github,
                GitAuthorityObservation::Unreachable {
                    failure: AuthorityFailure::parse("git offline")?,
                },
            );
            let state = derive_run_state(
                &records,
                &ratified_floor(),
                &vision,
                &RecoveryLogPath::new("events.jsonl"),
                &[],
                &[],
                &[authority],
            )?;
            assert!(
                snapshot_validator()?.is_valid(&serde_json::to_value(RunSnapshot::from(&state))?)
            );
        }
        Ok(())
    }

    #[test]
    fn recovery_carriers_and_negative_contract_boundaries() -> Result<(), Box<dyn Error>> {
        let records = (1..=21)
            .map(|sequence| dispatch(sequence, "m2-s1", "step-planner", "ref"))
            .collect::<Result<Vec<_>, _>>()?;
        let state = derive(&records, &[], &[], &[not_merged_authority("m2-s1")?])?;
        let valid = serde_json::to_value(RunSnapshot::from(&state))?;
        let validator = snapshot_validator()?;
        assert!(validator.is_valid(&valid));
        assert_eq!(
            valid["recovery_digest"]["rounds"]["elisions"][0]["retrieval_commands"],
            serde_json::json!([
                "pce log read --file 'events.jsonl' --kind dispatch --node 'm2-s1'"
            ])
        );

        let mut invalid_values = Vec::new();
        let mut wrong_id = valid.clone();
        wrong_id["schema_id"] = serde_json::json!("wrong");
        invalid_values.push(wrong_id);
        let mut wrong_version = valid.clone();
        wrong_version["schema_version"] = serde_json::json!(2);
        invalid_values.push(wrong_version);
        let mut missing = valid.clone();
        missing.as_object_mut().expect("object").remove("steps");
        invalid_values.push(missing);
        let mut extra = valid.clone();
        extra["extra"] = serde_json::json!(true);
        invalid_values.push(extra);
        let mut invalid_enum = valid.clone();
        invalid_enum["resume"]["state"] = serde_json::json!("invalid");
        invalid_values.push(invalid_enum);
        let mut zero = valid.clone();
        zero["recovery_digest"]["rounds"]["entries"][0]["sequence"] = serde_json::json!(0);
        invalid_values.push(zero);
        let mut malformed_digest = valid.clone();
        malformed_digest["provenance"] = serde_json::json!([{"path":"x","approved_sha256":"bad","approval_node":"m2-s1","approval_sequence":1,"condition":{"state":"digest-matches"}}]);
        invalid_values.push(malformed_digest);
        let mut too_many = valid.clone();
        let extra_entry = too_many["recovery_digest"]["rounds"]["entries"][0].clone();
        too_many["recovery_digest"]["rounds"]["entries"]
            .as_array_mut()
            .expect("array")
            .push(extra_entry);
        invalid_values.push(too_many);
        let mut empty_commands = valid.clone();
        empty_commands["recovery_digest"]["rounds"]["elisions"][0]["retrieval_commands"] =
            serde_json::json!([]);
        invalid_values.push(empty_commands);
        assert!(
            invalid_values
                .iter()
                .all(|value| !validator.is_valid(value))
        );
        Ok(())
    }

    #[test]
    fn typed_snapshots_cover_hold_provenance_and_cycle_variants() -> Result<(), Box<dyn Error>> {
        let records = vec![
            event(
                1,
                "m2-s1",
                KnownPayload::EscalationOpen(EscalationOpenPayload {
                    key: EscalationKey::new("hold"),
                    question: String::new(),
                }),
            )?,
            event(
                2,
                "m2-s1",
                KnownPayload::EscalationClose(EscalationClosePayload {
                    key: EscalationKey::new("hold"),
                    resolution: String::new(),
                }),
            )?,
            approval(3, "m2-s1", "matching", digest('a')?)?,
            approval(4, "m2-s1", "mismatch", digest('b')?)?,
            approval(5, "m2-s1", "missing", digest('c')?)?,
        ];
        let artifacts = [
            CurrentArtifactObservation::new(
                ArtifactPath::new("matching"),
                CurrentArtifactState::Present {
                    digest: digest('a')?,
                },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("mismatch"),
                CurrentArtifactState::Present {
                    digest: digest('d')?,
                },
            ),
            CurrentArtifactObservation::new(
                ArtifactPath::new("missing"),
                CurrentArtifactState::Missing,
            ),
        ];
        let state = derive(&records, &artifacts, &[], &[])?;
        let value = serde_json::to_value(RunSnapshot::from(&state))?;
        assert!(snapshot_validator()?.is_valid(&value));
        assert_eq!(value["holds"][0]["status"]["state"], "closed");
        assert_eq!(
            value["provenance"][1]["condition"]["state"],
            "digest-mismatch"
        );
        assert_eq!(
            value["provenance"][2]["condition"]["state"],
            "artifact-missing"
        );

        for role in [
            "step-planner",
            "step-critic",
            "step-executor",
            "pr-reviewer",
            "repository-analyst",
        ] {
            let state = derive(&[dispatch(1, "m2-s1", role, "ref")?], &[], &[], &[])?;
            assert!(
                snapshot_validator()?.is_valid(&serde_json::to_value(RunSnapshot::from(&state))?)
            );
        }
        Ok(())
    }

    #[test]
    fn renders_exact_empty_human_snapshot() -> Result<(), Box<dyn Error>> {
        let state = derive(&[], &[], &[], &[])?;
        let snapshot = RunSnapshot::from(&state);
        assert_eq!(
            render_human_snapshot(&snapshot),
            concat!(
                "pce status (pce.run-snapshot v1)\n",
                "repositories (0)\n",
                "steps (0)\n",
                "dispatch-accounting state=all-accounted issuance-sequences=-\n",
                "dispatches (0)\n",
                "issuance-ordinals (0)\n",
                "rounds (0)\n",
                "non-production-streaks (0)\n",
                "non-production-holds (0)\n",
                "holds (0)\n",
                "provenance (0)\n",
                "resume state=no-log-visible-candidate\n",
                "recovery-digest\n",
                "  rounds (entries=0, elisions=0)\n",
                "  open-holds (entries=0, elisions=0)\n",
                "  deltas (entries=0, elisions=0)\n",
                "  facts (entries=0, elisions=0)\n",
            )
        );
        Ok(())
    }

    #[test]
    fn renders_exact_rich_snapshot_from_one_real_fold() -> Result<(), Box<dyn Error>> {
        let state = rich_render_state()?;
        let snapshot = RunSnapshot::from(&state);
        let json = serde_json::to_vec(&snapshot)?;
        assert_eq!(
            json,
            serde_json::to_string(&snapshot)?.into_bytes(),
            "the compact JSON path must retain identical bytes"
        );
        let value: serde_json::Value = serde_json::from_slice(&json)?;
        assert_eq!(value["schema_id"], "pce.run-snapshot");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["steps"][0]["merge_status"], "merged");
        assert_eq!(
            value["provenance"][1]["condition"]["state"],
            "digest-mismatch"
        );
        assert_eq!(
            render_human_snapshot(&snapshot),
            concat!(
                "pce status (pce.run-snapshot v1)\n",
                "repositories (2)\n",
                "  repository 1: name=\"pce\"\n",
                "    fetch observed: ref=\"origin/pce/event-log-and-derived-run-state/milestone-2\" fetched-at=\"2026-07-27T12:34:56.123Z\"\n",
                "    branch: name=\"pce/event-log-and-derived-run-state/milestone-2\" state=present\n",
                "    worktree: identity=\"pce/event-log-and-derived-run-state/m2-s6\" state=absent\n",
                "    tag: name=\"v0.1.16\" state=points-to target=\"release-oid\"\n",
                "  repository 2: name=\"docs\"\n",
                "    fetch unavailable: failure=\"offline\"\n",
                "    branch: name=\"pce/event-log-and-derived-run-state/milestone-2\" state=absent\n",
                "    worktree: identity=\"pce/event-log-and-derived-run-state/m2-s6\" state=present\n",
                "    tag: name=\"v0.1.16\" state=absent\n",
                "steps (3)\n",
                "  step 1: node=\"m2-s1\" merge-status=merged\n",
                "    subject: milestone=2 step=1 head=\"pce/event-log-and-derived-run-state/m2-s1\" integration=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    selector: head=\"pce/event-log-and-derived-run-state/m2-s1\" base=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    github reachable: cardinality=one-exact-match pr=53 head=\"pce/event-log-and-derived-run-state/m2-s1\" base=\"pce/event-log-and-derived-run-state/milestone-2\" status=merged squash=\"merge-oid\"\n",
                "    git reachable: state=squash-commit-reachable squash=\"merge-oid\"\n",
                "  step 2: node=\"m2-s2\" merge-status=not-merged\n",
                "    subject: milestone=2 step=2 head=\"pce/event-log-and-derived-run-state/m2-s2\" integration=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    selector: head=\"pce/event-log-and-derived-run-state/m2-s2\" base=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    github reachable: cardinality=zero-exact-matches\n",
                "    git reachable: state=not-merged\n",
                "  step 3: node=\"m2-s3\" merge-status=inconclusive\n",
                "    subject: milestone=2 step=3 head=\"pce/event-log-and-derived-run-state/m2-s3\" integration=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    selector: head=\"pce/event-log-and-derived-run-state/m2-s3\" base=\"pce/event-log-and-derived-run-state/milestone-2\"\n",
                "    github unreachable: failure=\"gh offline\"\n",
                "    git unreachable: failure=\"GitHub authority unavailable before git reachability selection\"\n",
                "dispatch-accounting state=unaccounted issuance-sequences=1,2\n",
                "dispatches (2)\n",
                "  dispatch 1: sequence=1 node=\"m2-s1\" role=\"step-executor\" ref=\"dispatch-1\" completion=unaccounted\n",
                "  dispatch 2: sequence=2 node=\"m2-s3\" role=\"pr-reviewer\" ref=\"review-1\" completion=unaccounted\n",
                "issuance-ordinals (2)\n",
                "  issuance-ordinal 1: node=\"m2-s1\" role=\"step-executor\" ordinal=1\n",
                "  issuance-ordinal 2: node=\"m2-s3\" role=\"pr-reviewer\" ordinal=1\n",
                "rounds (0)\n",
                "non-production-streaks (0)\n",
                "non-production-holds (0)\n",
                "holds (2)\n",
                "  hold 1: key=\"release\" state=closed node=\"m2-s2\" sequence=4 resolution=\"approved\"\n",
                "  hold 2: key=\"network\" state=open node=\"m2-s3\" sequence=5 question=\"Retry?\"\n",
                "provenance (3)\n",
                "  artifact 1: path=\"planning/match.md\" approved-sha256=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" approval-node=\"m2-s1\" approval-sequence=6 condition=digest-matches\n",
                "  artifact 2: path=\"planning/mismatch.md\" approved-sha256=\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\" approval-node=\"m2-s2\" approval-sequence=7 condition=digest-mismatch current-sha256=\"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\"\n",
                "  artifact 3: path=\"planning/missing.md\" approved-sha256=\"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\" approval-node=\"m2-s3\" approval-sequence=8 condition=artifact-missing\n",
                "resume state=candidate node=\"m2-s3\" latest-sequence=30 cycle=review-dispatched cycle-sequence=2\n",
                "recovery-digest\n",
                "  rounds (entries=2, elisions=0)\n",
                "    entry: sequence=1 node=\"m2-s1\" role=\"step-executor\" round-number=1\n",
                "    entry: sequence=2 node=\"m2-s3\" role=\"pr-reviewer\" round-number=1\n",
                "  open-holds (entries=1, elisions=0)\n",
                "    entry: sequence=5 node=\"m2-s3\" key=\"network\" question=\"Retry?\"\n",
                "  deltas (entries=20, elisions=1)\n",
                "    entry: sequence=11 node=\"m2-s3\" message=\"delta-11\"\n",
                "    entry: sequence=12 node=\"m2-s3\" message=\"delta-12\"\n",
                "    entry: sequence=13 node=\"m2-s3\" message=\"delta-13\"\n",
                "    entry: sequence=14 node=\"m2-s3\" message=\"delta-14\"\n",
                "    entry: sequence=15 node=\"m2-s3\" message=\"delta-15\"\n",
                "    entry: sequence=16 node=\"m2-s3\" message=\"delta-16\"\n",
                "    entry: sequence=17 node=\"m2-s3\" message=\"delta-17\"\n",
                "    entry: sequence=18 node=\"m2-s3\" message=\"delta-18\"\n",
                "    entry: sequence=19 node=\"m2-s3\" message=\"delta-19\"\n",
                "    entry: sequence=20 node=\"m2-s3\" message=\"delta-20\"\n",
                "    entry: sequence=21 node=\"m2-s3\" message=\"delta-21\"\n",
                "    entry: sequence=22 node=\"m2-s3\" message=\"delta-22\"\n",
                "    entry: sequence=23 node=\"m2-s3\" message=\"delta-23\"\n",
                "    entry: sequence=24 node=\"m2-s3\" message=\"delta-24\"\n",
                "    entry: sequence=25 node=\"m2-s3\" message=\"delta-25\"\n",
                "    entry: sequence=26 node=\"m2-s3\" message=\"delta-26\"\n",
                "    entry: sequence=27 node=\"m2-s3\" message=\"delta-27\"\n",
                "    entry: sequence=28 node=\"m2-s3\" message=\"delta-28\"\n",
                "    entry: sequence=29 node=\"m2-s3\" message=\"delta-29\"\n",
                "    entry: sequence=30 node=\"m2-s3\" message=\"delta-30\"\n",
                "    elision: omitted-count=1 sequence=10..=10\n",
                "      retrieve: \"pce log read --file 'events.jsonl' --kind delta --node 'm2-s3'\"\n",
                "  facts (entries=6, elisions=0)\n",
                "    entry: sequence=1 node=\"m2-s1\" kind=\"dispatch\" evidence=\"dispatch evidence 1\"\n",
                "    entry: sequence=2 node=\"m2-s3\" kind=\"dispatch\" evidence=\"dispatch evidence 2\"\n",
                "    entry: sequence=6 node=\"m2-s1\" kind=\"planning-artifact-approved\" evidence=\"approve match\"\n",
                "    entry: sequence=7 node=\"m2-s2\" kind=\"planning-artifact-approved\" evidence=\"approve mismatch\"\n",
                "    entry: sequence=8 node=\"m2-s3\" kind=\"planning-artifact-approved\" evidence=\"approve missing\"\n",
                "    entry: sequence=9 node=\"m2-s3\" kind=\"key-finding\" evidence=\"git rev-parse HEAD\\ncargo test --workspace\"\n",
            )
        );
        Ok(())
    }

    #[test]
    fn human_renderer_escapes_arbitrary_text_to_one_physical_line() -> Result<(), Box<dyn Error>> {
        let text = "quote\" slash\\ newline\nreturn\rtab\tcontrol\u{7}";
        let state = derive(
            &[event(
                1,
                "m2-s1",
                KnownPayload::Delta(DeltaPayload {
                    message: text.to_owned(),
                }),
            )?],
            &[],
            &[],
            &[],
        )?;
        let rendered = render_human_snapshot(&RunSnapshot::from(&state));
        assert!(
            rendered
                .contains("message=\"quote\\\" slash\\\\ newline\\nreturn\\rtab\\tcontrol\\u{7}\"")
        );
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.contains("message="))
                .count(),
            1
        );
        assert!(rendered.ends_with('\n'));
        assert!(!rendered.ends_with("\n\n"));
        Ok(())
    }

    #[test]
    fn human_renderer_spells_remaining_authority_round_and_cycle_variants()
    -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let node = NodeId::parse("m2-s1")?;
        let subject = MergeSubject::derive(&vision, StepNode::parse(&node)?);
        let one_not_merged = StepAuthorityObservation::new(
            node.clone(),
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: ExactPullRequestIdentity::from_selector(
                        PullRequestNumber::parse(19)?,
                        subject.selector(),
                    ),
                    state: ExactPullRequestState::NotMerged,
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        );
        let multiple = StepAuthorityObservation::new(
            node.clone(),
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::MultipleExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        );
        let disagreement = StepAuthorityObservation::new(
            node,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: ExactPullRequestIdentity::from_selector(
                        PullRequestNumber::parse(20)?,
                        subject.selector(),
                    ),
                    state: ExactPullRequestState::Merged {
                        squash_commit: SquashCommitOid::parse("github-oid")?,
                    },
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable {
                    squash_commit: SquashCommitOid::parse("git-oid")?,
                },
            },
        );
        for (authority, expected) in [
            (
                one_not_merged,
                "github reachable: cardinality=one-exact-match pr=19 head=\"pce/example/m2-s1\" base=\"pce/example/milestone-2\" status=not-merged",
            ),
            (
                multiple,
                "github reachable: cardinality=multiple-exact-matches",
            ),
            (
                disagreement,
                "step 1: node=\"m2-s1\" merge-status=inconclusive",
            ),
        ] {
            let state = derive(&[delta(1, "m2-s1")?], &[], &[], &[authority])?;
            assert!(render_human_snapshot(&RunSnapshot::from(&state)).contains(expected));
        }

        for (role, cycle) in [
            ("step-planner", "plan-dispatched"),
            ("step-critic", "critique-dispatched"),
            ("step-executor", "execution-dispatched"),
            ("pr-reviewer", "review-dispatched"),
            ("falsification-critic", "falsification-dispatched"),
        ] {
            let state = derive(&[dispatch(1, "m2-s1", role, "ref")?], &[], &[], &[])?;
            let rendered = render_human_snapshot(&RunSnapshot::from(&state));
            assert!(rendered.contains(&format!("cycle={cycle} cycle-sequence=1")));
            assert!(rendered.contains(&format!("role=\"{role}\" ordinal=1")));
        }
        let state = derive(&[delta(1, "m2-s1")?], &[], &[], &[])?;
        assert!(
            render_human_snapshot(&RunSnapshot::from(&state)).contains("cycle=no-round-dispatch")
        );
        Ok(())
    }

    fn assert_exact_criterion_executions(
        executions: &[CriterionExecutionObservation],
        expected_sequences: &[u64],
    ) {
        assert_eq!(executions.len(), 3);
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.sequence().get())
                .collect::<Vec<_>>(),
            expected_sequences
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.node().as_str())
                .collect::<Vec<_>>(),
            ["m3-s2", "m3-s2", "m3-s2"]
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.criterion().name().as_str())
                .collect::<Vec<_>>(),
            [
                "Runnable criterion",
                "Failing criterion",
                "Install-only criterion"
            ]
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.criterion().input().as_str())
                .collect::<Vec<_>>(),
            [
                "Run the finished command.",
                "Run the broken command.",
                "Install the hook, then attempt the forbidden command."
            ]
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.criterion().observation().as_str())
                .collect::<Vec<_>>(),
            ["It exits 0.", "It exits 0.", "The command is denied."]
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.finished_result().as_str())
                .collect::<Vec<_>>(),
            [
                "main@0123456789abcdef",
                "main@0123456789abcdef",
                "main@0123456789abcdef"
            ]
        );
        assert_eq!(
            executions
                .iter()
                .map(|entry| entry.evidence().as_str())
                .collect::<Vec<_>>(),
            [
                "git rev-parse HEAD\n./finished-command",
                "git rev-parse HEAD\n./broken-command",
                "test -L ~/.claude/hooks/pre-tool-use.sh"
            ]
        );
        assert!(
            matches!(executions[0].outcome(), CriterionExecutionOutcome::Passed { observed_result } if observed_result.as_str() == "The command exited 0.")
        );
        assert!(
            matches!(executions[1].outcome(), CriterionExecutionOutcome::Failed { observed_result } if observed_result.as_str() == "The command exited 7.")
        );
        assert!(
            matches!(executions[2].outcome(), CriterionExecutionOutcome::Unpaid { reason } if reason.as_str() == "The run cannot activate the human-installed hook.")
        );
        assert_eq!(
            executions
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    matches!(entry.outcome(), CriterionExecutionOutcome::Passed { .. })
                        .then_some(index + 1)
                })
                .collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn folds_clean_criterion_executions_at_sequences_one_two_three() -> Result<(), Box<dyn Error>> {
        let lines = [
            r#"{"sequence":1,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}}"#,
            r#"{"sequence":2,"timestamp":"2026-07-27T12:35:04.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"git rev-parse HEAD\n./broken-command"}}"#,
            r#"{"sequence":3,"timestamp":"2026-07-27T12:35:05.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}}"#,
        ];
        let records = lines
            .into_iter()
            .map(parse_event_line)
            .collect::<Result<Vec<_>, _>>()?;
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &VisionSlug::parse("2026-08-03-a-gate-runs-what-was-built")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )?;
        assert_exact_criterion_executions(state.criterion_executions(), &[1, 2, 3]);

        let duplicate_records = [
            parse_event_line(lines[0])?,
            parse_event_line(&lines[0].replacen("\"sequence\":1", "\"sequence\":2", 1))?,
        ];
        let duplicate_state = derive_run_state(
            &duplicate_records,
            &ratified_floor(),
            &VisionSlug::parse("2026-08-03-a-gate-runs-what-was-built")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )?;
        assert_eq!(duplicate_state.criterion_executions().len(), 2);
        Ok(())
    }

    #[test]
    fn unknown_event_between_first_and_second_execution_preserves_exact_fold()
    -> Result<(), Box<dyn Error>> {
        // Sequence 2 belongs to the inserted unknown record, so the latter two typed records are
        // renumbered from the clean fold's [2, 3] to [3, 4].
        let lines = [
            r#"{"sequence":1,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}}"#,
            r#"{"sequence":2,"timestamp":"2026-07-27T12:35:03.500Z","kind":"criterion-execution-v2","node":"m3-s2","payload":{"kept":true}}"#,
            r#"{"sequence":3,"timestamp":"2026-07-27T12:35:04.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"git rev-parse HEAD\n./broken-command"}}"#,
            r#"{"sequence":4,"timestamp":"2026-07-27T12:35:05.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}}"#,
        ];
        let records = lines
            .into_iter()
            .map(parse_event_line)
            .collect::<Result<Vec<_>, _>>()?;
        let state = derive_run_state(
            &records,
            &ratified_floor(),
            &VisionSlug::parse("2026-08-03-a-gate-runs-what-was-built")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )?;

        assert_exact_criterion_executions(state.criterion_executions(), &[1, 3, 4]);
        Ok(())
    }

    #[test]
    fn completion_outcomes_preserve_only_their_own_evidence() -> Result<(), Box<dyn Error>> {
        let observed = [
            parse_event_line(
                r#"{"sequence":1,"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m1-s2","payload":{"role":"step-executor","ref":"abc123","evidence":"fixture"}}"#,
            )?,
            parse_event_line(
                r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m1-s2","payload":{"issuance_sequence":1,"duration_ms":200,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#,
            )?,
        ];
        let observed_state = derive(&observed, &[], &[], &[])?;
        assert!(matches!(
            observed_state.dispatch_lifecycles()[0],
            super::DispatchLifecycleObservation::ObservedChild(_)
        ));
        let reconciled = [
            observed[0].clone(),
            parse_event_line(
                r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m1-s2","payload":{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"not-produced"}}"#,
            )?,
        ];
        let reconciled_state = derive(&reconciled, &[], &[], &[])?;
        let super::DispatchLifecycleObservation::ReconciledDead(value) =
            &reconciled_state.dispatch_lifecycles()[0]
        else {
            panic!("reconciled lifecycle")
        };
        assert_eq!(
            value.outcome(),
            crate::ReconciledDispatchOutcome::ReconciledDead
        );
        assert_eq!(
            value.artifact_production(),
            crate::ArtifactProduction::NotProduced
        );
        Ok(())
    }

    #[test]
    fn dispatch_accounting_and_snapshot_follow_the_unaccounted_ledger() -> Result<(), Box<dyn Error>>
    {
        let issuance = parse_event_line(
            r#"{"sequence":1,"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m1-s2","payload":{"role":"step-executor","ref":"abc123","evidence":"fixture"}}"#,
        )?;
        let open = derive(std::slice::from_ref(&issuance), &[], &[], &[])?;
        let value = serde_json::to_value(RunSnapshot::from(&open))?;
        assert_eq!(
            value["dispatch_accounting"],
            serde_json::json!({"state":"unaccounted","issuance_sequences":[1]})
        );
        assert_eq!(
            value["dispatches"][0],
            serde_json::json!({"sequence":1,"node":"m1-s2","role":"step-executor","ref":"abc123","completion":null})
        );
        let completion = parse_event_line(
            r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m1-s2","payload":{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"not-produced"}}"#,
        )?;
        let closed = derive(&[issuance, completion], &[], &[], &[])?;
        let value = serde_json::to_value(RunSnapshot::from(&closed))?;
        assert_eq!(
            value["dispatch_accounting"],
            serde_json::json!({"state":"all-accounted","issuance_sequences":[]})
        );
        assert_eq!(
            value["dispatches"][0]["completion"]["outcome"],
            "reconciled-dead"
        );
        assert!(snapshot_validator()?.is_valid(&value));
        Ok(())
    }

    fn exceptional_declaration(
        sequence: u64,
        node: &str,
        branch: &str,
        step: u64,
        promotion: u64,
    ) -> Result<EventRecord, Box<dyn Error>> {
        event(
            sequence,
            node,
            KnownPayload::ExceptionalMergeChainDeclared(ExceptionalMergeChainDeclaredPayload {
                integration_branch: DeclaredIntegrationBranch::parse(branch)?,
                step_pull_request_number: PullRequestNumber::parse(step)?,
                promotion_pull_request_number: PullRequestNumber::parse(promotion)?,
                evidence: Evidence::parse("declared evidence")?,
            }),
        )
    }

    fn unreachable_hop() -> Result<ExceptionalMergeChainObservation, Box<dyn Error>> {
        let hop = || -> Result<PullRequestAuthorityObservation, RunStateError> {
            Ok(PullRequestAuthorityObservation::new(
                GitHubAuthorityObservation::Unreachable {
                    failure: AuthorityFailure::parse("gh unavailable")?,
                },
                GitAuthorityObservation::Unreachable {
                    failure: AuthorityFailure::parse("git unavailable")?,
                },
            ))
        };
        Ok(ExceptionalMergeChainObservation::new(hop()?, hop()?))
    }

    #[test]
    fn exceptional_merge_chain_truth_table_is_exhaustive() {
        for step in [
            MergeStatus::Merged,
            MergeStatus::NotMerged,
            MergeStatus::Inconclusive,
        ] {
            for promotion in [
                MergeStatus::Merged,
                MergeStatus::NotMerged,
                MergeStatus::Inconclusive,
            ] {
                let actual = combine_exceptional_merge_statuses(step, promotion);
                let expected = if step == MergeStatus::Merged && promotion == MergeStatus::Merged {
                    MergeStatus::Merged
                } else if step == MergeStatus::Inconclusive
                    || promotion == MergeStatus::Inconclusive
                {
                    MergeStatus::Inconclusive
                } else {
                    MergeStatus::NotMerged
                };
                assert_eq!(actual, expected, "{step:?} + {promotion:?}");
            }
        }
    }

    #[test]
    fn second_declaration_for_the_same_node_is_an_error() -> Result<(), Box<dyn Error>> {
        for second in [
            exceptional_declaration(2, "m1-s3", "branch-a", 179, 180)?,
            exceptional_declaration(2, "m1-s3", "branch-b", 181, 182)?,
        ] {
            let records = [
                exceptional_declaration(1, "m1-s3", "branch-a", 179, 180)?,
                second,
            ];
            let error = derive(&records, &[], &[], &[]).expect_err("duplicate declaration");
            let expected = RunStateError::DuplicateExceptionalMergeChainDeclaration {
                node: NodeId::parse("m1-s3")?,
            };
            assert_eq!(error, expected);
            assert_eq!(
                error.to_string(),
                "duplicate exceptional merge chain declaration for node NodeId(\"m1-s3\")"
            );
        }
        Ok(())
    }

    #[test]
    fn declaration_on_a_non_step_node_is_an_error() -> Result<(), Box<dyn Error>> {
        let records = [exceptional_declaration(1, "m1", "branch-a", 179, 180)?];
        let error = derive(&records, &[], &[], &[]).expect_err("non-step declaration");
        assert_eq!(
            error,
            RunStateError::ExceptionalMergeChainDeclarationOnNonStepNode {
                node: NodeId::parse("m1")?
            }
        );
        assert_eq!(
            error.to_string(),
            "exceptional merge chain declaration requires a canonical step node, got NodeId(\"m1\")"
        );
        Ok(())
    }

    #[test]
    fn declaration_bearing_log_derives_unchanged_without_exceptional_observations()
    -> Result<(), Box<dyn Error>> {
        let declaration = exceptional_declaration(1, "m1-s3", "branch-a", 179, 180)?;
        assert!(matches!(
            declaration.body_ref(),
            EventBodyRef::Known(KnownPayload::ExceptionalMergeChainDeclared(_))
        ));
        let trailing = delta(2, "m1-s3")?;
        let with = derive(&[declaration, trailing.clone()], &[], &[], &[])?;
        let without = derive(&[trailing], &[], &[], &[])?;
        assert_eq!(
            serde_json::to_vec(&RunSnapshot::from(&with))?,
            serde_json::to_vec(&RunSnapshot::from(&without))?
        );
        Ok(())
    }

    #[test]
    fn route_and_observation_must_agree() -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let criteria = ratified_floor();
        let recovery = RecoveryLogPath::new("events.jsonl");
        let node = NodeId::parse("m1-s3")?;
        let other = NodeId::parse("m1-s4")?;
        let declaration = exceptional_declaration(1, "m1-s3", "branch-a", 179, 180)?;
        let derive_exceptional =
            |records: &[EventRecord],
             observations: &[(NodeId, ExceptionalMergeChainObservation)]| {
                derive_run_state_with_exceptional_merge_chains(
                    records,
                    &criteria,
                    &vision,
                    &recovery,
                    &[],
                    &[],
                    &[],
                    observations,
                )
            };
        assert_eq!(
            derive_exceptional(std::slice::from_ref(&declaration), &[]).expect_err("missing"),
            RunStateError::MissingExceptionalMergeChainObservation { node: node.clone() }
        );
        assert_eq!(
            derive_exceptional(&[], &[(node.clone(), unreachable_hop()?)]).expect_err("undeclared"),
            RunStateError::ExceptionalMergeChainObservationWithoutDeclaration {
                node: node.clone()
            }
        );
        assert_eq!(
            derive_exceptional(
                std::slice::from_ref(&declaration),
                &[(other.clone(), unreachable_hop()?)]
            )
            .expect_err("mismatch"),
            RunStateError::ExceptionalMergeChainObservationNodeMismatch {
                expected: node.clone(),
                actual: other
            }
        );
        assert_eq!(
            derive_exceptional(
                std::slice::from_ref(&declaration),
                &[
                    (node.clone(), unreachable_hop()?),
                    (node.clone(), unreachable_hop()?)
                ]
            )
            .expect_err("surplus"),
            RunStateError::SurplusExceptionalMergeChainObservation { node }
        );
        Ok(())
    }

    #[test]
    fn declared_hop_rejects_mismatched_github_and_git_squash_oids() -> Result<(), Box<dyn Error>> {
        let vision = VisionSlug::parse("2026-07-27-example")?;
        let node = StepNode::parse(&NodeId::parse("m1-s3")?)?;
        let chain = ExceptionalMergeChain::new(
            &vision,
            node,
            DeclaredIntegrationBranch::parse("branch-a")?,
            PullRequestNumber::parse(179)?,
            PullRequestNumber::parse(180)?,
        )?;
        let hop = PullRequestAuthorityObservation::new(
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: chain.step_pull_request().clone(),
                    state: ExactPullRequestState::Merged {
                        squash_commit: SquashCommitOid::parse("squash-a")?,
                    },
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable {
                    squash_commit: SquashCommitOid::parse("squash-b")?,
                },
            },
        );
        let promotion = PullRequestAuthorityObservation::new(
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        );
        let observation = ExceptionalMergeChainObservation::new(hop, promotion);
        let (step, _, aggregate) = super::derive_exceptional_merge_status(&chain, &observation);
        assert_eq!(step, MergeStatus::Inconclusive);
        assert_eq!(aggregate, MergeStatus::Inconclusive);
        Ok(())
    }
}
