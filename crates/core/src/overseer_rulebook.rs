//! rulebook : corpus rulings × named defect classes → ordered routing rules
//!
//! The starting rulebook preserves why a report belongs on a route. The discriminating fact is
//! part of every rule so a shared symptom cannot make two different causes equivalent.

use serde::{Deserialize, Serialize};

/// Where a starting routing rule came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingRuleOrigin {
    RecordedRuling,
    NamedDefectClass,
}

/// The authority that can settle a report matching a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingRuleClass {
    HumanRuling,
    RunLocalDecision,
    FleetFact,
    BinaryDefect,
}

/// What the overseer does after a rule matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingRuleAction {
    RouteToHuman,
    ReturnToRun,
    SupplyFleetFact,
    DispatchBinaryBrief,
}

/// One immutable starting rule and the fact that distinguishes its cause.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingRule {
    id: String,
    origin: RoutingRuleOrigin,
    classification: RoutingRuleClass,
    action: RoutingRuleAction,
    discriminating_fact: String,
    instruction: String,
}

impl RoutingRule {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn origin(&self) -> RoutingRuleOrigin {
        self.origin
    }
    pub const fn classification(&self) -> RoutingRuleClass {
        self.classification
    }
    pub const fn action(&self) -> RoutingRuleAction {
        self.action
    }
    pub fn discriminating_fact(&self) -> &str {
        &self.discriminating_fact
    }
    pub fn instruction(&self) -> &str {
        &self.instruction
    }
}

struct SeedRule {
    id: &'static str,
    origin: RoutingRuleOrigin,
    classification: RoutingRuleClass,
    action: RoutingRuleAction,
    fact: &'static str,
    instruction: &'static str,
}

const RECORDED_RULINGS: &[SeedRule] = &[
    SeedRule {
        id: "ruling-recovery-spend",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::HumanRuling,
        action: RoutingRuleAction::RouteToHuman,
        fact: "The requested recovery would exceed the configured spend limit.",
        instruction: "Ask the human to authorise or refuse the additional spend.",
    },
    SeedRule {
        id: "ruling-frozen-criterion-revision",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::HumanRuling,
        action: RoutingRuleAction::RouteToHuman,
        fact: "The proposed change alters or removes an already frozen acceptance criterion.",
        instruction: "Route the exact predecessor, successor, and rationale to the human.",
    },
    SeedRule {
        id: "ruling-fleet-install-window",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::HumanRuling,
        action: RoutingRuleAction::RouteToHuman,
        fact: "Installing the requested binary changes the execution floor of other live runs.",
        instruction: "Ask the human whether the fleet-wide update is wanted; installation still waits for a quiet fleet.",
    },
    SeedRule {
        id: "ruling-herdr-session-scope",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::HumanRuling,
        action: RoutingRuleAction::RouteToHuman,
        fact: "The choice between the shared and dedicated Herdr session changes isolation across the fleet.",
        instruction: "Ask the human to choose the fleet session scope once.",
    },
    SeedRule {
        id: "routing-dirty-source-committed-refs",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "The source worktree is dirty, but every driver input is pinned to a committed ref.",
        instruction: "Record the warning and continue from the committed refs.",
    },
    SeedRule {
        id: "routing-identical-graph-draft",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "graph.json is byte-identical to the approved frozen graph.",
        instruction: "Use the frozen graph; there is no divergent draft to rule on.",
    },
    SeedRule {
        id: "routing-mechanical-freeze",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "The proposed freeze preserves whole-package identity and carries no attributed human record.",
        instruction: "Use the mechanical freeze path without asking for semantic approval.",
    },
    SeedRule {
        id: "routing-base-currency-origin-head-missing",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "Base currency is unavailable specifically because origin HEAD is missing or unreachable.",
        instruction: "Restore or configure origin HEAD; do not treat this as an accepted historical-ref risk.",
    },
    SeedRule {
        id: "routing-base-currency-historical-ref",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "Base currency is unavailable specifically because the freeze targets an intentional offline or historical ref.",
        instruction: "Require the existing exact attributed acceptance path rather than inventing a new ruling.",
    },
    SeedRule {
        id: "routing-gate-finding-replay",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "The finding fails on its witness ref and passes on its repair ref.",
        instruction: "Accept the finding mechanically from paired replay evidence.",
    },
    SeedRule {
        id: "routing-environment-retry",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "Environment preparation failed while its configured retry budget remains.",
        instruction: "Use the bounded environment recovery path.",
    },
    SeedRule {
        id: "routing-stale-process-evidence",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "The recorded process identity no longer matches the live process start identity.",
        instruction: "Reconcile the dead dispatch; do not infer liveness from a PID alone.",
    },
    SeedRule {
        id: "routing-optionless-choice",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::RunLocalDecision,
        action: RoutingRuleAction::ReturnToRun,
        fact: "The report asks which way to proceed but supplies no named options and consequences.",
        instruction: "Return the report and require explicit options before human routing.",
    },
    SeedRule {
        id: "fact-another-run-push",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::FleetFact,
        action: RoutingRuleAction::SupplyFleetFact,
        fact: "The missing evidence is whether another run pushed the relevant ref.",
        instruction: "Read the fleet registration and remote evidence, then return the observed fact to the run.",
    },
    SeedRule {
        id: "fact-another-run-launch-spelling",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::FleetFact,
        action: RoutingRuleAction::SupplyFleetFact,
        fact: "The missing evidence is the exact launch spelling recorded by another run.",
        instruction: "Read that run's registration and supply the retained launch spelling.",
    },
    SeedRule {
        id: "fact-installed-binary-surface",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::FleetFact,
        action: RoutingRuleAction::SupplyFleetFact,
        fact: "The answer turns on the installed binary's verbs rather than the source checkout's surface.",
        instruction: "Probe the installed binary and return its observed command surface.",
    },
    SeedRule {
        id: "fact-external-daemon-journal",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::FleetFact,
        action: RoutingRuleAction::SupplyFleetFact,
        fact: "The required evidence is in a daemon journal outside the reporting workflow.",
        instruction: "Read the named daemon journal and return the relevant retained fact.",
    },
    SeedRule {
        id: "defect-absent-binary-verb",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The run is blocked because the installed binary does not expose the required verb.",
        instruction: "Dispatch one binary repair brief and record it; do not ask the human to choose.",
    },
    SeedRule {
        id: "defect-binary-refuses-valid-operation",
        origin: RoutingRuleOrigin::RecordedRuling,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The run supplied the documented valid inputs and the binary still refused the operation.",
        instruction: "Dispatch one reproducible binary repair brief and record it.",
    },
];

const NAMED_DEFECT_CLASSES: &[SeedRule] = &[
    SeedRule {
        id: "defect-class-rubber-stamp",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "A check reports success without testing the claim it names.",
        instruction: "Brief a falsification test that plants the named violation.",
    },
    SeedRule {
        id: "defect-class-inert-rule",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "Deleting or disabling the rule leaves its enforcing check green.",
        instruction: "Brief a repair that proves the rule is load-bearing.",
    },
    SeedRule {
        id: "defect-class-unreachable-success",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The workflow's declared success outcome cannot be produced by any valid input.",
        instruction: "Brief a reachable success witness and preserve invalid-input refusal.",
    },
    SeedRule {
        id: "defect-class-wrong-baseline",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The check compares against a baseline known to encode the wrong magnitude or subject.",
        instruction: "Brief a corrected independent baseline and paired witness.",
    },
    SeedRule {
        id: "defect-class-vacuity",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The asserted invariant is true only because the exercised subject set is empty or bypassed.",
        instruction: "Brief a non-empty witness and prove the subject is exercised.",
    },
    SeedRule {
        id: "defect-class-silent-test-omission",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "The test command exits successfully while silently omitting declared tests.",
        instruction: "Brief exhaustive test accounting that fails on any omission.",
    },
    SeedRule {
        id: "defect-class-stale-documentation-truth",
        origin: RoutingRuleOrigin::NamedDefectClass,
        classification: RoutingRuleClass::BinaryDefect,
        action: RoutingRuleAction::DispatchBinaryBrief,
        fact: "Operational documentation still states a one-time or release fact contradicted by retained execution evidence.",
        instruction: "Brief a documentation repair tied to the authoritative evidence.",
    },
];

/// Returns the immutable corpus-derived rules in stable installation order.
pub fn initial_routing_rules() -> Vec<RoutingRule> {
    RECORDED_RULINGS
        .iter()
        .chain(NAMED_DEFECT_CLASSES)
        .map(|seed| RoutingRule {
            id: seed.id.to_owned(),
            origin: seed.origin,
            classification: seed.classification,
            action: seed.action,
            discriminating_fact: seed.fact.to_owned(),
            instruction: seed.instruction.to_owned(),
        })
        .collect()
}
