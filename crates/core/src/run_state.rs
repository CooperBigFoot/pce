//! merge_status : MergeSubject × GitHubAuthorityObservation × GitAuthorityObservation → MergeStatus   (pure, deterministic)
//! This module performs no I/O.

use thiserror::Error;
use tracing::instrument;

use crate::event_log::NodeId;

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

/// Errors parsing raw values at the run-state domain boundary.
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
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::event_log::NodeId;
    use crate::run_state::{
        AuthorityFailure, ExactPullRequestIdentity, ExactPullRequestState, GitAuthorityObservation,
        GitHubAuthorityObservation, GitHubPullRequestObservation, GitMergeObservation, MergeStatus,
        MergeSubject, PullRequestNumber, SquashCommitOid, StepNode, VisionSlug,
        derive_merge_status,
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
}
