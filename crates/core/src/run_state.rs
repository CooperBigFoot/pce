//! run_state : Ordered<EventRecord> × VisionSlug × ArtifactDigestObservation* × RepositoryObservation* × StepAuthorityObservation* → DerivedRunState ∪ RunStateError   (pure, deterministic)
//! This module performs no I/O.

use thiserror::Error;
use tracing::instrument;

use crate::event_log::{
    ArtifactPath, DispatchRef, DispatchRole, EscalationKey, EventBodyRef, EventRecord,
    EventTimestamp, KnownPayload, NodeId, RepositoryName, Sequence, Sha256Digest,
};

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

/// A positive milestone number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MilestoneNumber(u64);

impl MilestoneNumber {
    /// Return the milestone number.
    pub const fn get(self) -> u64 {
        self.0
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

/// A derived milestone integration branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationBranch(String);

impl IntegrationBranch {
    /// Return the complete integration branch name.
    pub fn as_str(&self) -> &str {
        &self.0
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
        let base = IntegrationBranch(format!("milestone-{}", node.milestone().get()));
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

/// A positive GitHub pull-request number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            if identity.selector() == subject.selector() {
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
            if identity.selector() == subject.selector() && github_oid == git_oid {
                MergeStatus::Merged
            } else {
                MergeStatus::Inconclusive
            }
        }
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
            "milestone-critic" | "step-critic" | "step-plan-critic" | "pr-reviewer" => {
                Self::CritiqueProducing
            }
            "step-executor" => Self::Execution,
            "repository-analyst" => Self::ExplicitlyNonRoundBearing,
            _ => Self::Unrecognized,
        }
    }

    fn is_round_bearing(self) -> bool {
        matches!(
            self,
            Self::PlanProducing | Self::CritiqueProducing | Self::Execution
        )
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

/// A checked count of dispatches in one exact `(node, role)` series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundCount(u64);

impl RoundCount {
    /// Return the derived count.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One first-seen-ordered round series keyed by exact node and role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundSeries {
    node: NodeId,
    role: DispatchRole,
    classification: DispatchRoleClass,
    count: RoundCount,
}

impl RoundSeries {
    /// Return the exact node key.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the exact role key.
    pub const fn role(&self) -> &DispatchRole {
        &self.role
    }

    /// Return the pinned role classification.
    pub const fn classification(&self) -> DispatchRoleClass {
        self.classification
    }

    /// Return the checked dispatch count.
    pub const fn count(&self) -> RoundCount {
        self.count
    }
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

/// Derived merge result for one supplied log-visible canonical step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepMergeResult {
    node: NodeId,
    subject: MergeSubject,
    status: MergeStatus,
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

/// Pure derived run state with deterministic collection order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedRunState {
    repositories: Vec<RepositoryObservation>,
    steps: Vec<StepMergeResult>,
    dispatches: Vec<DispatchObservation>,
    rounds: Vec<RoundSeries>,
    holds: Vec<HoldObservation>,
    provenance: Vec<ArtifactProvenance>,
    resume: ResumeObservation,
}

impl DerivedRunState {
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

    /// Return first-seen-ordered exact `(node, role)` round series.
    pub fn rounds(&self) -> &[RoundSeries] {
        &self.rounds
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

/// Fold ordered records and typed authority observations into deterministic run state.
///
/// # Errors
///
/// Returns a named [`RunStateError`] for non-increasing records, checked round overflow,
/// close-before-open history, duplicate or missing cross-input identities, malformed or
/// absent authority nodes, or empty raw values passed to typed constructors.
#[instrument(skip(records, vision, artifacts, repositories, authorities))]
pub fn derive_run_state(
    records: &[EventRecord],
    vision: &VisionSlug,
    artifacts: &[CurrentArtifactObservation],
    repositories: &[RepositoryObservation],
    authorities: &[StepAuthorityObservation],
) -> Result<DerivedRunState, RunStateError> {
    validate_repository_inputs(repositories)?;
    validate_artifact_inputs(artifacts)?;
    validate_authority_duplicates(authorities)?;

    let mut visible_nodes = Vec::<VisibleNode>::new();
    let mut dispatches = Vec::<DispatchObservation>::new();
    let mut rounds = Vec::<RoundSeries>::new();
    let mut holds = Vec::<HoldObservation>::new();
    let mut approvals = Vec::<LatestApproval>::new();
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
                if classification.is_round_bearing() {
                    increment_round_series(
                        &mut rounds,
                        record.node(),
                        &payload.role,
                        classification,
                    )?;
                    update_cycle_position(
                        &mut visible_nodes,
                        record.node(),
                        &payload.role,
                        record.sequence(),
                    );
                }
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
            }
            EventBodyRef::Known(
                KnownPayload::Delta(_)
                | KnownPayload::KeyFinding(_)
                | KnownPayload::RepositoryContract(_),
            )
            | EventBodyRef::Unknown { .. } => {}
        }
    }

    let provenance = derive_provenance(&approvals, artifacts)?;
    let steps = derive_step_results(&visible_nodes, vision, authorities)?;
    let resume = derive_resume(&visible_nodes, &steps);

    Ok(DerivedRunState {
        repositories: repositories.to_vec(),
        steps,
        dispatches,
        rounds,
        holds,
        provenance,
        resume,
    })
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
        "pr-reviewer" => CyclePosition::ReviewDispatched { sequence },
        _ => return,
    };
    if let Some(existing) = visible_nodes.iter_mut().find(|item| item.node == *node) {
        existing.cycle_position = position;
    }
}

fn increment_round_series(
    rounds: &mut Vec<RoundSeries>,
    node: &NodeId,
    role: &DispatchRole,
    classification: DispatchRoleClass,
) -> Result<(), RunStateError> {
    if let Some(existing) = rounds
        .iter_mut()
        .find(|item| item.node == *node && item.role == *role)
    {
        existing.count = checked_round_increment(existing.count, node, role)?;
    } else {
        rounds.push(RoundSeries {
            node: node.clone(),
            role: role.clone(),
            classification,
            count: RoundCount(1),
        });
    }
    Ok(())
}

fn checked_round_increment(
    count: RoundCount,
    node: &NodeId,
    role: &DispatchRole,
) -> Result<RoundCount, RunStateError> {
    count
        .0
        .checked_add(1)
        .map(RoundCount)
        .ok_or_else(|| RunStateError::RoundCountOverflow {
            node: node.clone(),
            role: role.clone(),
        })
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
    /// Returned when a vision basename does not begin with ASCII `YYYY-MM-DD-` shape.
    #[error("vision directory basename lacks YYYY-MM-DD- prefix shape: {value}")]
    MalformedVisionBasename { value: String },
    /// Returned when a date-shaped vision basename has no suffix.
    #[error("vision directory basename has an empty slug suffix: {value}")]
    EmptyVisionSlug { value: String },
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
    /// Returned when an exact `(node, role)` dispatch count cannot be incremented.
    #[error("round count overflow for node {node:?} and role {role:?}")]
    RoundCountOverflow { node: NodeId, role: DispatchRole },
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
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::event_log::{
        ArtifactPath, DeltaPayload, DispatchPayload, DispatchRef, DispatchRole,
        EscalationClosePayload, EscalationKey, EscalationOpenPayload, EventRecord, EventTimestamp,
        Evidence, KnownPayload, NodeId, PlanningArtifactApprovedPayload, RepositoryName, Sequence,
        Sha256Digest,
    };
    use crate::run_state::{
        ArtifactProvenanceCondition, AuthorityFailure, BranchState, CurrentArtifactObservation,
        CurrentArtifactState, CyclePosition, DispatchRoleClass, ExactPullRequestIdentity,
        ExactPullRequestState, GitAuthorityObservation, GitHubAuthorityObservation,
        GitHubPullRequestObservation, GitMergeObservation, HoldStatus, MergeStatus, MergeSubject,
        PullRequestNumber, RepositoryBranchName, RepositoryFetchObservation, RepositoryObservation,
        RepositoryObservationFailure, RepositoryObservationRef, ResumeObservation, RoundCount,
        RunStateError, SquashCommitOid, StepAuthorityObservation, StepNode, TagName, TagState,
        TagTarget, VisionSlug, WorktreeIdentity, WorktreeState, checked_round_increment,
        derive_merge_status, derive_run_state,
    };

    fn subject(slug: &str, node: &str) -> Result<MergeSubject, Box<dyn Error>> {
        let slug = VisionSlug::parse(slug)?;
        let node = NodeId::parse(node)?;
        let node = StepNode::parse(&node)?;
        Ok(MergeSubject::derive(&slug, node))
    }

    fn identity(subject: &MergeSubject) -> Result<ExactPullRequestIdentity, Box<dyn Error>> {
        Ok(ExactPullRequestIdentity::from_selector(
            PullRequestNumber::parse(17)?,
            subject.selector(),
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

    fn digest(byte: char) -> Result<Sha256Digest, Box<dyn Error>> {
        Ok(Sha256Digest::parse(&byte.to_string().repeat(64))?)
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
                    identity: identity(&merge_subject)?,
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
        derive_run_state(records, &vision, artifacts, repositories, authorities)
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
        assert_eq!(subject.integration_branch().as_str(), "milestone-2");
        assert_eq!(subject.selector().head(), subject.head());
        assert_eq!(subject.selector().base(), subject.integration_branch());
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
    fn exact_identity_is_constructed_from_subject_selector() -> Result<(), Box<dyn Error>> {
        let subject = subject("2026-07-27-example", "m3-s4")?;
        let identity = identity(&subject)?;

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
        let exact_identity = identity(&subject)?;
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
                identity: identity(&other_subject)?,
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
    fn round_series_join_exact_node_and_role_and_preserve_all_dispatches()
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
        ];

        let state = derive(&records, &[], &[], &[])?;
        assert_eq!(state.dispatches().len(), 8);
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
        assert_eq!(state.rounds().len(), 3);
        assert_eq!(state.rounds()[0].node().as_str(), "m2-s1");
        assert_eq!(state.rounds()[0].role().as_str(), "step-planner");
        assert_eq!(state.rounds()[0].count().get(), 2);
        assert_eq!(state.rounds()[1].role().as_str(), "step-critic");
        assert_eq!(state.rounds()[1].count().get(), 3);
        assert_eq!(state.rounds()[2].node().as_str(), "m2-s2");
        assert_eq!(state.rounds()[2].count().get(), 1);
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
        assert_eq!(state.rounds().len(), 8);
        assert_eq!(state.dispatches().len(), 10);
        Ok(())
    }

    #[test]
    fn checked_round_increment_reports_exact_overflow_key() -> Result<(), Box<dyn Error>> {
        let node = NodeId::parse("m2-s1")?;
        let role = DispatchRole::new("step-planner");
        assert_eq!(
            checked_round_increment(RoundCount(u64::MAX), &node, &role),
            Err(RunStateError::RoundCountOverflow { node, role })
        );
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
            RepositoryBranchName::parse("milestone-2")?,
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
        assert_eq!(step.subject().integration_branch().as_str(), "milestone-2");
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
        let state = derive_run_state(&records, &vision, &[], &[], &authorities)?;
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
            &vision,
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
        }
        Ok(())
    }

    #[test]
    fn sequence_order_is_strict_and_core_never_sorts() -> Result<(), Box<dyn Error>> {
        let records = vec![delta(2, "later")?, delta(1, "earlier")?];
        assert!(matches!(
            derive(&records, &[], &[], &[]),
            Err(RunStateError::NonIncreasingSequence { previous, current })
                if previous.get() == 2 && current.get() == 1
        ));
        let duplicate = vec![delta(1, "a")?, delta(1, "b")?];
        assert!(matches!(
            derive(&duplicate, &[], &[], &[]),
            Err(RunStateError::NonIncreasingSequence { previous, current })
                if previous.get() == 1 && current.get() == 1
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
                identity: identity(&other_subject)?,
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
}
