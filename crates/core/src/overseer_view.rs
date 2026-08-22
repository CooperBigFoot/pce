//! queue view = render(holds × registrations × registered graphs × latest overseer events × now)
//!
//! The view is derived from durable store records. It never treats a missing or stale heartbeat as
//! an empty queue, and it carries no repository authority.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    EventTimestamp, HoldRoute, HoldState, HoldStore, HoldStoreError, RepairFailureStage,
    RepairRecord, SiftedCard,
};

/// The provider used for every overseer session.
///
/// The overseer wakes all day. Every Claude Code session on one machine shares a single
/// subscription window, and this corpus has recorded that window being exhausted mid-gate. Running
/// the overseer on a separately metered provider leaves that window to the runs.
pub const OVERSEER_PROVIDER: &str = "openai-codex";
/// The model used for every initial and replacement overseer session.
pub const OVERSEER_MODEL: &str = "gpt-5.6-sol";
/// The reasoning effort used for every initial and replacement overseer session.
pub const OVERSEER_REASONING_EFFORT: &str = "high";

/// One durable supervisor observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OverseerEvent {
    Heartbeat {
        timestamp: EventTimestamp,
    },
    SessionSpawned {
        timestamp: EventTimestamp,
        model: String,
        reasoning_effort: String,
    },
    SessionExited {
        timestamp: EventTimestamp,
        code: Option<i32>,
        /// A bounded excerpt of the session's own output, so a session that dies on an unknown
        /// model or flag leaves a visible reason rather than silence.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// The server decided there is a pass to perform. Written before the session is spawned, so a
    /// session that never starts still leaves a record that it was asked to.
    WakeStarted {
        timestamp: EventTimestamp,
        reason: OverseerWakeReason,
    },
    /// The session itself reports that it finished a pass. Written by the session, never by the
    /// server, so it attests to the session rather than to the process that spawned it.
    PassCompleted {
        timestamp: EventTimestamp,
    },
}

/// Why the server decided to wake the overseer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverseerWakeReason {
    /// The server just started.
    Startup,
    /// A hold or registration record changed.
    StoreChanged,
    /// The slow periodic wake, for work no store change announces.
    Periodic,
    /// A previous wake ended without a completed pass.
    Retry,
}

impl OverseerEvent {
    pub const fn timestamp(&self) -> EventTimestamp {
        match self {
            Self::Heartbeat { timestamp }
            | Self::PassCompleted { timestamp }
            | Self::SessionSpawned { timestamp, .. }
            | Self::SessionExited { timestamp, .. }
            | Self::WakeStarted { timestamp, .. } => *timestamp,
        }
    }
}

/// Append-only supervisor journal stored beside, not inside, any repository.
#[derive(Clone, Debug)]
pub struct OverseerJournal {
    path: PathBuf,
}

impl OverseerJournal {
    pub fn new(store_root: impl AsRef<Path>) -> Self {
        Self {
            path: store_root.as_ref().join("overseer").join("events.jsonl"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends and synchronizes one complete event.
    ///
    /// # Errors
    ///
    /// Returns an error if the journal parent cannot be created or the event cannot be serialized,
    /// appended, or synchronized.
    pub fn append(&self, event: &OverseerEvent) -> Result<(), OverseerViewError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| OverseerViewError::MissingParent {
                path: self.path.clone(),
            })?;
        fs::create_dir_all(parent).map_err(|source| OverseerViewError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
        let mut bytes = serde_json::to_vec(event).map_err(OverseerViewError::SerializeEvent)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| OverseerViewError::OpenJournal {
                path: self.path.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .map_err(|source| OverseerViewError::WriteJournal {
                path: self.path.clone(),
                source,
            })?;
        file.sync_all()
            .map_err(|source| OverseerViewError::SyncJournal {
                path: self.path.clone(),
                source,
            })
    }

    /// Reads all complete events in append order.
    ///
    /// # Errors
    ///
    /// Returns an error if the journal cannot be read or contains an incomplete or malformed line.
    pub fn read(&self) -> Result<Vec<OverseerEvent>, OverseerViewError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.path).map_err(|source| OverseerViewError::ReadJournal {
            path: self.path.clone(),
            source,
        })?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(OverseerViewError::IncompleteTail {
                path: self.path.clone(),
            });
        }
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        let mut previous: Option<EventTimestamp> = None;
        for (index, line) in bytes
            .strip_suffix(b"\n")
            .unwrap_or(&bytes)
            .split(|byte| *byte == b'\n')
            .enumerate()
        {
            if line.is_empty() {
                return Err(OverseerViewError::EmptyLine {
                    path: self.path.clone(),
                    line: index + 1,
                });
            }
            let event: OverseerEvent =
                serde_json::from_slice(line).map_err(|source| OverseerViewError::ParseEvent {
                    path: self.path.clone(),
                    line: index + 1,
                    source,
                })?;
            if previous
                .is_some_and(|timestamp| event.timestamp().as_datetime() < timestamp.as_datetime())
            {
                return Err(OverseerViewError::TimestampWentBackwards {
                    path: self.path.clone(),
                    line: index + 1,
                });
            }
            previous = Some(event.timestamp());
            events.push(event);
        }
        Ok(events)
    }
}

/// The liveness conclusion shown to an operator.
///
/// Nothing to do and cannot start are different facts and must be distinguishable at a glance. The
/// heartbeat attests to the server process only; `Working`, `Idle` and `CannotStart` are derived
/// from the server's child-process records and the session's completion record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverseerLiveness {
    /// A wake is in flight: the session was asked to run and has neither completed nor exited.
    Working,
    /// The server is up and the last wake completed its pass. Idle costs nothing.
    Idle,
    /// The server's heartbeat is stale or absent. Nothing is watching the queue.
    NotRunning,
    /// A wake ended without a completed pass. The session could not start or died mid-pass.
    CannotStart,
}

/// The operator-facing state of one hold card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueHoldState {
    WaitingForHuman,
    WithOverseer,
    WithReportingRun,
    Closed,
}

/// One hold card with its stable identity and age.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueueHold {
    pub key: String,
    pub repository: String,
    pub plan_version: String,
    pub package: String,
    pub question_kind: String,
    pub state: QueueHoldState,
    pub age_seconds: u64,
    /// The opening report, always retained. The card never replaces it.
    pub report: serde_json::Value,
    /// The operator-facing translation, when the overseer has sifted this hold.
    pub card: Option<SiftedCard>,
    /// The conversation so far, oldest first, as (who, what).
    pub thread: Vec<(String, String)>,
    /// What this hold is waiting on while it sits with the overseer.
    pub pending: Option<String>,
    /// Whether the sifted card's declared requested act opens an attributed door.
    pub is_door: bool,
}

/// One registered run rendered as a fleet roster entry rather than a path listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QueueRun {
    pub repository: String,
    pub vision_name: String,
    pub plan_version: String,
    pub state: String,
    pub vision_directory: PathBuf,
    pub frozen_graph: PathBuf,
    pub journal: PathBuf,
    pub herdr_session: Option<String>,
}

impl QueueRun {
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

/// Operator-facing lifecycle of one binary repair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum QueueRepairState {
    Dispatching,
    ReadyForAcceptance,
    Accepted,
    WaitingForFleet {
        blocking_runs: Vec<String>,
    },
    Installed,
    Failed {
        stage: RepairFailureStage,
        detail: String,
    },
}

/// One durable binary repair shown independently of decision holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QueueRepair {
    pub id: String,
    pub summary: String,
    pub digest: Option<String>,
    pub state: QueueRepairState,
    pub age_seconds: u64,
    pub touched_paths: Vec<PathBuf>,
}

/// A complete queue snapshot. Holds are never suppressed because the overseer is unavailable.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueueView {
    pub overseer: OverseerLiveness,
    pub holds: Vec<QueueHold>,
    pub runs: Vec<QueueRun>,
    pub repairs: Vec<QueueRepair>,
}

/// Derives a queue snapshot from the stores at a caller-supplied time.
///
/// # Errors
///
/// Returns an error when any durable hold, registration, or overseer record cannot be read.
pub fn derive_queue_view(
    store: &HoldStore,
    journal: &OverseerJournal,
    now: DateTime<Utc>,
    stale_after: Duration,
) -> Result<QueueView, OverseerViewError> {
    let events = journal.read()?;
    let latest_heartbeat = events.iter().rev().find_map(|event| match event {
        OverseerEvent::Heartbeat { timestamp } => Some(timestamp.as_datetime()),
        _ => None,
    });
    let heartbeat_fresh = latest_heartbeat.is_some_and(|timestamp| {
        let elapsed = now.signed_duration_since(*timestamp).num_milliseconds();
        elapsed >= 0 && elapsed <= i64::try_from(stale_after.as_millis()).unwrap_or(i64::MAX)
    });
    // The heartbeat proves only that the server loop is running. Process state comes from the
    // server's spawn and reap records. A session's completion claim cannot make a live child idle.
    let last = |predicate: fn(&OverseerEvent) -> bool| events.iter().rposition(predicate);
    let last_wake = last(|event| matches!(event, OverseerEvent::WakeStarted { .. }));
    let last_spawned = last(|event| matches!(event, OverseerEvent::SessionSpawned { .. }));
    let last_completed = last(|event| matches!(event, OverseerEvent::PassCompleted { .. }));
    let last_exited = last(|event| matches!(event, OverseerEvent::SessionExited { .. }));
    let child_is_alive =
        last_spawned.is_some_and(|spawned| last_exited.is_none_or(|exited| exited < spawned));
    let overseer = if !heartbeat_fresh {
        OverseerLiveness::NotRunning
    } else if child_is_alive {
        OverseerLiveness::Working
    } else {
        match last_wake {
            // A wake that ended without a completed pass is a child that could not run its pass.
            Some(wake)
                if last_completed.is_none_or(|completed| completed < wake)
                    && last_exited.is_some_and(|exited| exited > wake) =>
            {
                OverseerLiveness::CannotStart
            }
            _ => OverseerLiveness::Idle,
        }
    };

    let holds: Vec<QueueHold> = store
        .list()
        .map_err(OverseerViewError::HoldStore)?
        .into_iter()
        .map(|hold| {
            let state = match hold.state() {
                HoldState::Open {
                    route: HoldRoute::Human,
                } => QueueHoldState::WaitingForHuman,
                HoldState::Open {
                    route: HoldRoute::Overseer,
                } => QueueHoldState::WithOverseer,
                HoldState::Open {
                    route: HoldRoute::ReportingRun,
                } => QueueHoldState::WithReportingRun,
                HoldState::Closed => QueueHoldState::Closed,
            };
            let age_seconds = now
                .signed_duration_since(*hold.opened_at().as_datetime())
                .num_seconds()
                .max(0) as u64;
            let card = hold.card();
            let is_door = card
                .as_ref()
                .is_some_and(|sifted| sifted.requested_act.opens_door());
            QueueHold {
                key: hold.key().as_str().to_owned(),
                repository: hold.identity().repository().to_owned(),
                plan_version: hold.identity().plan_version().to_owned(),
                package: hold.identity().package().to_owned(),
                question_kind: hold.identity().question_kind().to_owned(),
                state,
                age_seconds,
                report: hold.report().clone(),
                card,
                thread: hold.thread(),
                pending: hold.pending_with_overseer(),
                is_door,
            }
        })
        .collect();
    let registrations = store
        .run_registrations()
        .map_err(OverseerViewError::HoldStore)?;
    let runs = registrations
        .into_iter()
        .map(|registration| {
            let graph_path = registration.frozen_graph().to_path_buf();
            let bytes =
                fs::read(&graph_path).map_err(|source| OverseerViewError::ReadRunGraph {
                    path: graph_path.clone(),
                    source,
                })?;
            let graph: serde_json::Value = serde_json::from_slice(&bytes).map_err(|source| {
                OverseerViewError::ParseRunGraph {
                    path: graph_path.clone(),
                    source,
                }
            })?;
            let vision_name = graph
                .get("vision")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| OverseerViewError::MissingRunGraphField {
                    path: graph_path.clone(),
                    field: "vision",
                })?
                .to_owned();
            let plan_version = graph
                .get("plan_version")
                .and_then(|value| match value {
                    serde_json::Value::Number(number) => Some(number.to_string()),
                    serde_json::Value::String(text) if !text.trim().is_empty() => {
                        Some(text.clone())
                    }
                    _ => None,
                })
                .ok_or_else(|| OverseerViewError::MissingRunGraphField {
                    path: graph_path.clone(),
                    field: "plan_version",
                })?;
            let state = [
                (QueueHoldState::WaitingForHuman, "waiting for human"),
                (QueueHoldState::WithOverseer, "with overseer"),
                (QueueHoldState::WithReportingRun, "with reporting run"),
            ]
            .into_iter()
            .find_map(|(state, label)| {
                holds
                    .iter()
                    .any(|hold| {
                        hold.repository == registration.repository()
                            && hold.plan_version == plan_version
                            && hold.state == state
                    })
                    .then_some(label)
            })
            .unwrap_or("no open holds")
            .to_owned();
            Ok(QueueRun {
                repository: registration.repository().to_owned(),
                vision_name,
                plan_version,
                state,
                vision_directory: registration.vision_directory().to_path_buf(),
                frozen_graph: graph_path,
                journal: registration.journal().to_path_buf(),
                herdr_session: registration.herdr_session().map(str::to_owned),
            })
        })
        .collect::<Result<Vec<_>, OverseerViewError>>()?;
    let repairs = derive_queue_repairs(
        &store
            .repair_records()
            .map_err(OverseerViewError::HoldStore)?,
        now,
    );
    Ok(QueueView {
        overseer,
        holds,
        runs,
        repairs,
    })
}

fn derive_queue_repairs(records: &[RepairRecord], now: DateTime<Utc>) -> Vec<QueueRepair> {
    let mut repairs: BTreeMap<String, QueueRepair> = BTreeMap::new();
    for record in records {
        match record {
            RepairRecord::DefectBriefDispatched {
                timestamp,
                repair_id,
                summary,
                ..
            } => {
                let age_seconds = now
                    .signed_duration_since(*timestamp.as_datetime())
                    .num_seconds()
                    .max(0) as u64;
                repairs
                    .entry(repair_id.clone())
                    .and_modify(|repair| {
                        repair.summary.clone_from(summary);
                        repair.digest = None;
                        repair.state = QueueRepairState::Dispatching;
                        repair.age_seconds = age_seconds;
                        repair.touched_paths.clear();
                    })
                    .or_insert_with(|| QueueRepair {
                        id: repair_id.clone(),
                        summary: summary.clone(),
                        digest: None,
                        state: QueueRepairState::Dispatching,
                        age_seconds,
                        touched_paths: Vec::new(),
                    });
            }
            RepairRecord::Built {
                repair_id,
                digest,
                touched_paths,
                ..
            } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.digest = Some(digest.clone());
                    repair.touched_paths.clone_from(touched_paths);
                    repair.state = QueueRepairState::ReadyForAcceptance;
                }
            }
            RepairRecord::InstallAccepted { repair_id, .. } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.state = QueueRepairState::Accepted;
                }
            }
            RepairRecord::InstallDeferred {
                repair_id,
                blocking_runs,
                ..
            } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.state = QueueRepairState::WaitingForFleet {
                        blocking_runs: blocking_runs.clone(),
                    };
                }
            }
            RepairRecord::Installed { repair_id, .. } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.state = QueueRepairState::Installed;
                }
            }
            RepairRecord::Failed {
                repair_id,
                stage,
                detail,
                ..
            } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.state = QueueRepairState::Failed {
                        stage: *stage,
                        detail: detail.clone(),
                    };
                }
            }
            RepairRecord::NotificationsCompleted { repair_id, .. } => {
                if let Some(repair) = repairs.get_mut(repair_id) {
                    repair.state = QueueRepairState::Installed;
                }
            }
            RepairRecord::RunNotified { .. } => {}
        }
    }
    repairs.into_values().collect()
}

/// The page's stylesheet.
///
/// The register split is the page's subject, so it is carried by type: monospace for anything the
/// machine owns, serif for the prose the operator reads, sans for chrome. No webfont is linked —
/// the server is loopback and may be offline, and a page whose typography depends on a network
/// fetch loses the split exactly when it is needed.
const QUEUE_STYLE: &str = r#"
*{box-sizing:border-box}
:root{
--ground:#EEF0F3;--surface:#FFF;--surface-2:#F6F7F9;--ink:#171A1F;--ink-2:#4A515C;--ink-3:#7C8592;
--rule:#D9DDE3;--rule-soft:#E7EAEF;--accent:#B26205;--accent-soft:#FBF0DF;
--handled:#0E6E64;--handled-soft:#E2F0EE;--defect:#A33B2A;--defect-soft:#F8E9E5;
--shadow:0 1px 2px rgba(23,26,31,.06),0 8px 24px -12px rgba(23,26,31,.18);
--mono:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;
--sans:system-ui,-apple-system,"Segoe UI",sans-serif;
--serif:ui-serif,Georgia,"Times New Roman",serif;
}
:root:not([data-theme="light"]){@media (prefers-color-scheme:dark){
--ground:#101318;--surface:#171B21;--surface-2:#1D222A;--ink:#E7EAEE;--ink-2:#A9B2BE;--ink-3:#6F7885;
--rule:#2A313A;--rule-soft:#222831;--accent:#E8A33D;--accent-soft:#2E2415;
--handled:#4FB5A6;--handled-soft:#152825;--defect:#D9705C;--defect-soft:#2C1B18;
--shadow:0 1px 2px rgba(0,0,0,.4),0 8px 24px -12px rgba(0,0,0,.6);}}
:root[data-theme="dark"]{
--ground:#101318;--surface:#171B21;--surface-2:#1D222A;--ink:#E7EAEE;--ink-2:#A9B2BE;--ink-3:#6F7885;
--rule:#2A313A;--rule-soft:#222831;--accent:#E8A33D;--accent-soft:#2E2415;
--handled:#4FB5A6;--handled-soft:#152825;--defect:#D9705C;--defect-soft:#2C1B18;
--shadow:0 1px 2px rgba(0,0,0,.4),0 8px 24px -12px rgba(0,0,0,.6);}
body{margin:0;background:var(--ground);color:var(--ink);font-family:var(--sans);font-size:15px;line-height:1.5;-webkit-font-smoothing:antialiased}
.shell{max-width:1240px;margin:0 auto;padding:20px 20px 64px;display:flex;flex-direction:column;gap:14px}
.bar{display:flex;align-items:baseline;flex-wrap:wrap;gap:10px 18px;padding-bottom:12px;border-bottom:1px solid var(--rule)}
.mark{font-family:var(--mono);font-weight:600;font-size:13px;letter-spacing:.1em;text-transform:uppercase}
.mark span{color:var(--accent)}
.count{font-family:var(--mono);font-size:13px;color:var(--ink-2);font-variant-numeric:tabular-nums}
.count b{color:var(--accent);font-weight:600}
.bar-spacer{flex:1 1 auto}
.pulse{display:flex;align-items:center;gap:7px;font-family:var(--mono);font-size:11.5px;letter-spacing:.04em;
border-radius:999px;padding:3px 11px 3px 9px;font-variant-numeric:tabular-nums}
.pulse-dot{width:6px;height:6px;border-radius:50%;background:currentColor;flex:none}
.live-working{color:var(--handled);background:var(--handled-soft);border:1px solid color-mix(in srgb,var(--handled) 30%,transparent)}
.live-working .pulse-dot{animation:beat 2.4s ease-in-out infinite}
.live-idle{color:var(--ink-3);background:var(--surface-2);border:1px solid var(--rule)}
.live-cannot{color:var(--defect);background:var(--defect-soft);border:1px solid color-mix(in srgb,var(--defect) 40%,transparent)}
.live-cannot .pulse-dot{animation:beat 1s steps(2,end) infinite}
.live-down{color:var(--defect);background:var(--defect-soft);border:1px dashed color-mix(in srgb,var(--defect) 55%,transparent);font-weight:600}
@keyframes beat{0%,100%{opacity:1}50%{opacity:.25}}
.console{display:grid;grid-template-columns:330px minmax(0,1fr);gap:14px;align-items:start}
@media (max-width:880px){.console{grid-template-columns:1fr}}
.rail{background:var(--surface);border:1px solid var(--rule);border-radius:3px;box-shadow:var(--shadow);overflow:hidden}
.rail-head{padding:10px 14px;border-bottom:1px solid var(--rule);background:var(--surface-2);font-family:var(--mono);
font-size:11.5px;letter-spacing:.09em;text-transform:uppercase;color:var(--ink-3);display:flex;justify-content:space-between}
.rail-list{display:flex;flex-direction:column}
.pick{position:absolute;width:1px;height:1px;opacity:0;pointer-events:none;margin:0}
.item{border-bottom:1px solid var(--rule-soft);border-left:3px solid transparent;padding:12px 14px 12px 13px;
display:flex;flex-direction:column;gap:6px;text-decoration:none;color:inherit;cursor:pointer}
.item:hover{background:var(--surface-2)}
.item.is-selected{background:var(--surface-2);border-left-color:var(--accent)}
.item:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
.item-top{display:flex;align-items:center;gap:8px;min-width:0;max-width:100%}
.repo{font-family:var(--mono);font-weight:600;font-size:13.5px;color:var(--ink);min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.age{margin-left:auto;font-family:var(--mono);font-size:11.5px;color:var(--ink-3);font-variant-numeric:tabular-nums}
.item-line{font-family:var(--serif);font-size:14px;line-height:1.4;color:var(--ink-2);text-wrap:pretty;overflow-wrap:anywhere;max-width:100%}
.state{font-family:var(--mono);font-size:10.5px;letter-spacing:.05em;color:var(--ink-3);display:flex;align-items:center;gap:5px}
.state::before{content:"";width:6px;height:6px;border-radius:50%;background:currentColor;flex:none}
.state-waiting-for-human{color:var(--accent)}
.state-with-overseer{color:var(--handled)}
.state-with-reporting-run{color:var(--ink-3)}
.state-closed{color:var(--ink-3)}
.tag{font-family:var(--mono);font-size:10.5px;font-weight:500;letter-spacing:.06em;text-transform:uppercase;
padding:2px 6px;border-radius:2px;white-space:nowrap;background:var(--surface-2);color:var(--ink-2);max-width:100%;overflow:hidden;text-overflow:ellipsis}
.tag-door{background:var(--accent-soft);color:var(--accent)}
.deck{min-width:0}
.card{background:var(--surface);border:1px solid var(--rule);border-radius:3px;box-shadow:var(--shadow);
padding:26px 30px 24px;display:none;flex-direction:column;gap:20px}
@media (max-width:560px){.card{padding:20px 18px}}
.card-meta{display:flex;align-items:center;gap:10px;flex-wrap:wrap;font-family:var(--mono);font-size:12px;color:var(--ink-3)}
.card h1{margin:0;font-family:var(--sans);font-size:21px;font-weight:600;line-height:1.25;letter-spacing:-.01em;text-wrap:balance}
.block{display:flex;flex-direction:column;gap:7px}
.block h3{margin:0;font-family:var(--mono);font-size:11px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}
.markdown{font-family:var(--serif);font-size:16.5px;line-height:1.55;max-width:66ch;text-wrap:pretty;min-width:0}
.markdown p{margin:0 0 .65em}.markdown p:last-child{margin-bottom:0}
.markdown ul{margin:.35em 0;padding-left:1.35em}.markdown li{margin:.15em 0}
.markdown code{font-family:var(--mono);font-size:.86em;background:var(--surface-2);border:1px solid var(--rule-soft);border-radius:2px;padding:.08em .3em;overflow-wrap:anywhere}
.markdown pre{margin:.5em 0;padding:11px 13px;background:var(--surface-2);border:1px solid var(--rule-soft);border-radius:3px;overflow:auto;max-width:100%}
.markdown pre code{padding:0;border:0;background:none;white-space:pre}
.options{margin:0;padding:0;list-style:none;display:flex;flex-direction:column;gap:2px;counter-reset:opt}
.options li{display:grid;grid-template-columns:26px minmax(0,1fr);gap:4px;padding:9px 10px;border-radius:3px;
font-family:var(--serif);font-size:16px;line-height:1.5;max-width:68ch}
.options li::before{counter-increment:opt;content:counter(opt);font-family:var(--mono);font-size:12.5px;font-weight:600;color:var(--ink-3);padding-top:3px}
.options li.rec{background:var(--accent-soft)}
.options li.rec::before{color:var(--accent)}
.options .why{grid-column:2;color:var(--ink-2);font-size:15px}
.rec-flag{font-family:var(--mono);font-size:10.5px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--accent);margin-left:8px;white-space:nowrap}
.consequence{border-left:3px solid var(--accent);background:var(--accent-soft);padding:12px 16px;border-radius:0 3px 3px 0;
font-family:var(--serif);font-size:16px;line-height:1.5;max-width:66ch}
.fact{display:flex;gap:10px;align-items:baseline;font-family:var(--sans);font-size:13.5px;color:var(--ink-2)}
.fact .label{font-family:var(--mono);font-size:10.5px;font-weight:600;letter-spacing:.07em;text-transform:uppercase;color:var(--handled);white-space:nowrap}
.pending{border:1px dashed color-mix(in srgb,var(--handled) 40%,transparent);background:var(--handled-soft);border-radius:3px;
padding:12px 16px;font-size:14px;color:var(--ink-2);display:flex;gap:10px;align-items:baseline}
.pending .label{font-family:var(--mono);font-size:10.5px;font-weight:600;letter-spacing:.07em;text-transform:uppercase;color:var(--handled);white-space:nowrap}
.thread{display:flex;flex-direction:column;gap:10px;border-top:1px solid var(--rule-soft);padding-top:14px}
.turn{display:flex;gap:10px;align-items:flex-start}
.who{font-family:var(--mono);font-size:10.5px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3);
padding-top:4px;width:70px;flex:none;text-align:right;overflow-wrap:anywhere}
.turn .markdown{font-size:15.5px;line-height:1.5;max-width:62ch;color:var(--ink-2)}
.answer{border-top:1px solid var(--rule);padding-top:18px;display:flex;flex-direction:column;gap:10px}
.answer label{font-family:var(--mono);font-size:11px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}
.answer input,.answer textarea,.feedback textarea{width:100%;padding:11px 13px;border:1px solid var(--rule);border-radius:3px;
background:var(--surface-2);color:var(--ink);font-family:var(--serif);font-size:16px;line-height:1.5}
.answer textarea,.feedback textarea{min-height:76px;resize:vertical}
.answer input{font-family:var(--mono);font-size:14px;max-width:280px}
.answer :focus-visible,.feedback :focus-visible{outline:2px solid var(--accent);outline-offset:-1px;background:var(--surface)}
.answer-row{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.btn{font-family:var(--sans);font-size:14px;font-weight:500;padding:8px 16px;border-radius:3px;border:1px solid transparent;cursor:pointer}
.btn-primary{background:var(--accent);color:#FFF9EF}
.btn:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.hint{font-family:var(--sans);font-size:13px;color:var(--ink-3);max-width:60ch}
.raw{border-top:1px solid var(--rule-soft);padding-top:12px}
.raw summary{cursor:pointer;font-family:var(--mono);font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:var(--ink-3)}
.raw pre{margin:10px 0 0;padding:12px 14px;background:var(--surface-2);border:1px solid var(--rule-soft);border-radius:3px;
font-family:var(--mono);font-size:12.5px;line-height:1.5;color:var(--ink-2);white-space:pre-wrap;overflow-wrap:anywhere;max-height:340px;overflow:auto}
.feedback,.drawer{background:var(--surface);border:1px solid var(--rule);border-radius:3px}
.feedback{padding:14px 16px 16px;display:flex;flex-direction:column;gap:9px}
.feedback h2{margin:0;font-family:var(--mono);font-size:11px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}
.feedback-row{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.drawer{overflow:hidden}
.drawer summary{cursor:pointer;padding:11px 14px;display:flex;align-items:center;gap:9px;font-family:var(--mono);
font-size:11.5px;letter-spacing:.08em;text-transform:uppercase;color:var(--ink-3)}
.drawer summary b{color:var(--handled);font-weight:600}
.drawer summary:hover{background:var(--surface-2)}
.drawer-body{padding:4px 14px 14px 30px;border-top:1px solid var(--rule-soft)}
.drawer-body ul{margin:12px 0 0;padding:0;list-style:none;display:flex;flex-direction:column;gap:8px;
font-family:var(--serif);font-size:15px;line-height:1.45;color:var(--ink-2);max-width:78ch}
.drawer-body code{font-family:var(--mono);font-size:12.5px;color:var(--ink)}
.drawer-note{margin:10px 0 0;font-size:12.5px;color:var(--ink-3)}
.empty{padding:22px;font-family:var(--serif);font-size:16px;color:var(--ink-2)}
.updates{display:flex;flex-direction:column;gap:8px}.update{background:var(--surface);border:1px solid var(--rule);
border-left:4px solid var(--accent);border-radius:8px;padding:12px 14px;box-shadow:var(--shadow)}
.update h2{margin:0 0 5px;font-family:var(--serif);font-size:18px}.update p{margin:3px 0;color:var(--ink-2)}
.update code{font-family:var(--mono);font-size:11.5px}.update-waiting{border-left-color:var(--defect)}
.foot{font-family:var(--mono);font-size:11.5px;color:var(--ink-3);text-align:center;padding-top:4px}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
"#;

/// Renders the queue as a self-contained HTML page.
///
/// The page is a worklist, not a feed: a rail of every hold and exactly one open card. Selection is
/// `:target`, so the operator's core surface needs no script.
pub fn render_queue_html(view: &QueueView) -> String {
    let (status, live_class) = match view.overseer {
        OverseerLiveness::Working => ("overseer is working", "live-working"),
        OverseerLiveness::Idle => ("overseer is idle", "live-idle"),
        OverseerLiveness::NotRunning => ("overseer is not running", "live-down"),
        OverseerLiveness::CannotStart => ("overseer cannot start", "live-cannot"),
    };
    let waiting = view
        .holds
        .iter()
        .filter(|hold| hold.state == QueueHoldState::WaitingForHuman)
        .count();
    let with_overseer = view
        .holds
        .iter()
        .filter(|hold| hold.state == QueueHoldState::WithOverseer)
        .count();
    let settled: Vec<&QueueHold> = view
        .holds
        .iter()
        .filter(|hold| hold.state == QueueHoldState::Closed)
        .collect();
    // The queue is what is still open. Closed holds live in the drawer.
    let queue: Vec<&QueueHold> = view
        .holds
        .iter()
        .filter(|hold| hold.state != QueueHoldState::Closed)
        .collect();

    let mut html = String::with_capacity(16_384);
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    html.push_str("<title>PCE hold queue</title><style>");
    html.push_str(QUEUE_STYLE);
    // Selection is a radio, not a link. A fragment link would make the browser scroll the open
    // card into view on every click, yanking the operator back up the page; a checked radio
    // changes nothing about scroll position. Each hold gets the rules that open its card, mark its
    // rail entry, and show keyboard focus.
    for hold in &queue {
        let key = escape_html(&hold.key);
        html.push_str(&format!(
            ".console:has(#pick-{key}:checked) #hold-{key}{{display:flex}}\
.console:has(#pick-{key}:checked) label[for=\"pick-{key}\"]\
{{background:var(--surface-2);border-left-color:var(--accent)}}\
.console:has(#pick-{key}:focus-visible) label[for=\"pick-{key}\"]\
{{outline:2px solid var(--accent);outline-offset:-2px}}"
        ));
    }
    html.push_str("</style></head><body><div class=\"shell\">");

    let pending_installs = view
        .repairs
        .iter()
        .filter(|repair| {
            matches!(
                repair.state,
                QueueRepairState::Accepted
                    | QueueRepairState::WaitingForFleet { .. }
                    | QueueRepairState::Failed {
                        stage: RepairFailureStage::Install | RepairFailureStage::Notify,
                        ..
                    }
            )
        })
        .count();
    html.push_str(&format!(
        r#"<header class="bar"><div class="mark">pce <span>overseer</span></div>
<div class="count"><b>{waiting}</b> waiting for you &middot; {with_overseer} with the overseer &middot; {} settled without you &middot; {} defect briefs dispatched &middot; {pending_installs} pending installs</div>
<div class="bar-spacer"></div>
<div class="pulse {live_class}" id="overseer-status"><span class="pulse-dot"></span>{status}</div></header>"#,
        settled.len(),
        view.repairs.len(),
    ));

    if !view.repairs.is_empty() {
        html.push_str(r#"<section class="updates" aria-label="Binary updates">"#);
        for repair in &view.repairs {
            html.push_str(&render_repair(repair));
        }
        html.push_str("</section>");
    }

    html.push_str(r#"<div class="console">"#);
    // The radios live before both columns so `:has()` on `.console` can see them.
    for (index, hold) in queue.iter().enumerate() {
        html.push_str(&format!(
            "<input class=\"pick\" type=\"radio\" name=\"open-hold\" id=\"pick-{key}\"{checked}>",
            key = escape_html(&hold.key),
            checked = if index == 0 { " checked" } else { "" },
        ));
    }
    html.push_str("<nav class=\"rail\" aria-label=\"Question queue\">");
    html.push_str("<div class=\"rail-head\"><span>Queue</span><span>oldest first</span></div><div class=\"rail-list\">");
    if queue.is_empty() {
        html.push_str(
            "<p class=\"empty\">Nothing is waiting. The overseer settled everything it could.</p>",
        );
    }
    for hold in &queue {
        let state = state_slug(hold.state);
        let line = hold.card.as_ref().map_or_else(
            || format!("Package {} · plan {}", hold.package, hold.plan_version),
            |card| card.title.clone(),
        );
        let requested_act = hold
            .card
            .as_ref()
            .and_then(|card| requested_act_label(card.requested_act));
        let act_tag = requested_act.map_or_else(String::new, |act| {
            format!("<span class=\"tag tag-door\">{}</span>", escape_html(act))
        });
        html.push_str(&format!(
            "<label class=\"item\" for=\"pick-{key}\" data-hold-key=\"{key}\" data-state=\"{state}\">\
<span class=\"item-top\"><span class=\"repo\">{repo}</span>{act_tag}<span class=\"item-update\"></span><span class=\"age\">{age}</span></span>\
<span class=\"item-line\">{line}</span>\
<span class=\"state state-{state}\">{state_label}</span></label>",
            key = escape_html(&hold.key),
            state = state,
            repo = escape_html(&hold.repository),
            act_tag = act_tag,
            age = escape_html(&format_age(hold.age_seconds)),
            line = escape_html(&line),
            state_label = state_label(hold.state),
        ));
    }

    html.push_str("</div></nav><main class=\"deck\">");

    if queue.is_empty() {
        html.push_str(
            "<article class=\"card\"><p class=\"empty\">No hold is waiting for you.</p></article>",
        );
    }
    for hold in &queue {
        html.push_str(&render_card(hold));
    }
    html.push_str("</main></div>");

    html.push_str(
        "<section class=\"feedback\"><h2>Feedback</h2>\
<form method=\"post\" action=\"/feedback\">\
<textarea name=\"text\" required placeholder=\"Dump it raw. Half a sentence is fine. It waits here until you sit down with it.\"></textarea>\
<div class=\"feedback-row\"><button class=\"btn btn-primary\" type=\"submit\">File it</button>\
<span class=\"hint\">Collected, not acted on. Nothing reads this until you run a session over it on purpose; the overseer never sees it and this page never touches a repository.</span>\
</div></form></section>",
    );

    html.push_str(&format!(
        "<details class=\"drawer\"><summary>Settled without you &nbsp;<b>{}</b></summary><div class=\"drawer-body\"><ul>",
        settled.len()
    ));
    for hold in &settled {
        html.push_str(&format!(
            "<li data-hold-key=\"{key}\" data-state=\"closed\"><code>{repo}</code> &mdash; {line}</li>",
            key = escape_html(&hold.key),
            repo = escape_html(&hold.repository),
            line = escape_html(
                &hold
                    .card
                    .as_ref()
                    .map_or_else(
                        || format!("Package {} · plan {}", hold.package, hold.plan_version),
                        |card| card.title.clone(),
                    )
            ),
        ));
    }
    html.push_str("</ul><p class=\"drawer-note\">Nothing here needed you.</p></div></details>");

    html.push_str(&format!(
        "<details class=\"drawer\"><summary>Registered runs &nbsp;<b>{}</b></summary><div class=\"drawer-body\"><ul>",
        view.runs.len()
    ));
    for run in &view.runs {
        html.push_str(&format!(
            r#"<li data-repository="{repo}"><code>{repo}</code> &mdash; {vision} &middot; plan {plan} &middot; {state}
<details class="run-paths"><summary>Paths and session</summary><code>vision: {vision_path}</code><br>
<code>graph: {graph}</code><br><code>journal: {journal}</code><br><code>herdr session: {session}</code></details></li>"#,
            repo = escape_html(run.repository()),
            vision = escape_html(&run.vision_name),
            plan = escape_html(&run.plan_version),
            state = escape_html(&run.state),
            vision_path = escape_html(&run.vision_directory.display().to_string()),
            graph = escape_html(&run.frozen_graph.display().to_string()),
            journal = escape_html(&run.journal.display().to_string()),
            session = escape_html(run.herdr_session.as_deref().unwrap_or("none")),
        ));
    }
    html.push_str("</ul><p class=\"drawer-note\">A run joins the fleet by existing. Nobody registers one by hand.</p></div></details>");

    html.push_str("<p class=\"foot\">One append-only file per question, keyed by repository, plan version, package and kind.</p>");
    html.push_str(
        r#"</div><script>
const refreshIntervalMs = 15000;
setInterval(() => {
  const fields = [...document.querySelectorAll('textarea, input:not([type="radio"]), select')];
  const active = document.activeElement;
  const fieldFocused = active && active.matches('input, textarea, select');
  const allEmpty = fields.every(field => field.value.length === 0);
  if (!fieldFocused && allEmpty) window.location.reload();
}, refreshIntervalMs);
</script></body></html>"#,
    );
    html
}

fn render_repair(repair: &QueueRepair) -> String {
    let (label, class, detail) = match &repair.state {
        QueueRepairState::Dispatching => (
            "repair in progress",
            "",
            "Worker and checks are still running.".to_owned(),
        ),
        QueueRepairState::ReadyForAcceptance => (
            "update ready",
            "",
            "Accept once. The overseer installs it when the fleet is quiet.".to_owned(),
        ),
        QueueRepairState::Accepted => (
            "accepted; waiting for install pass",
            "",
            "The next safe pass installs this artifact.".to_owned(),
        ),
        QueueRepairState::WaitingForFleet { blocking_runs } => (
            "accepted; fleet is not quiet",
            " update-waiting",
            format!("Blocking runs: {}", blocking_runs.join(", ")),
        ),
        QueueRepairState::Installed => (
            "installed",
            "",
            "Every registered run received the installed digest.".to_owned(),
        ),
        QueueRepairState::Failed { stage, detail } => (
            "repair failed",
            " update-waiting",
            format!("{stage:?}: {detail}"),
        ),
    };
    let digest = repair.digest.as_deref().unwrap_or("not built");
    let accept = if matches!(repair.state, QueueRepairState::ReadyForAcceptance) {
        format!(
            r#"<form method="post" action="/repairs/{}/accept"><button class="btn btn-primary" type="submit">Accept update</button></form>"#,
            escape_html(&repair.id),
        )
    } else {
        String::new()
    };
    format!(
        r#"<article class="update{class}" data-repair-id="{id}" data-state="{state}"><h2>{summary}</h2><p><strong>{label}</strong> &middot; {age}</p><p>{detail}</p><p><code>sha256 {digest}</code></p>{accept}</article>"#,
        id = escape_html(&repair.id),
        state = match repair.state {
            QueueRepairState::Dispatching => "dispatching",
            QueueRepairState::ReadyForAcceptance => "ready-for-acceptance",
            QueueRepairState::Accepted => "accepted",
            QueueRepairState::WaitingForFleet { .. } => "waiting-for-fleet",
            QueueRepairState::Installed => "installed",
            QueueRepairState::Failed { .. } => "failed",
        },
        summary = escape_html(&repair.summary),
        age = escape_html(&format_age(repair.age_seconds)),
        detail = escape_html(&detail),
        digest = escape_html(digest),
    )
}

fn render_card(hold: &QueueHold) -> String {
    let mut card = String::with_capacity(2048);
    card.push_str(&format!(
        "<article class=\"card\" id=\"hold-{key}\" data-hold-key=\"{key}\" data-state=\"{state}\">",
        key = escape_html(&hold.key),
        state = state_slug(hold.state),
    ));
    card.push_str(&format!(
        "<div class=\"card-head\"><div class=\"card-meta\"><span class=\"repo\">{repo}</span>\
<span>package {package}</span><span>plan {plan}</span><span>{kind}</span><span>{age}</span></div>",
        repo = escape_html(&hold.repository),
        package = escape_html(&hold.package),
        plan = escape_html(&hold.plan_version),
        kind = escape_html(&hold.question_kind),
        age = escape_html(&format_age(hold.age_seconds)),
    ));

    match hold.card.as_ref() {
        Some(sifted) => {
            card.push_str(&format!("<h1>{}</h1></div>", escape_html(&sifted.title)));
            for block in &sifted.blocks {
                card.push_str(&format!(
                    "<section class=\"block\"><h3>{}</h3><div class=\"markdown\">{}</div></section>",
                    escape_html(&block.heading),
                    render_markdown(&block.body)
                ));
            }
            for fact in &sifted.overseer_facts {
                // Whose claim this is must be readable at a glance: the overseer's, not the run's.
                card.push_str(&format!(
                    "<p class=\"fact\"><span class=\"label\">Overseer fact</span><span>{}</span></p>",
                    escape_html(fact)
                ));
            }
            card.push_str(
                "<section class=\"block\"><h3>The run's options</h3><ol class=\"options\">",
            );
            for option in &sifted.options {
                card.push_str(&format!(
                    "<li class=\"{class}\"><span>{text}{flag}</span>{why}</li>",
                    class = if option.recommended_by_run { "rec" } else { "" },
                    text = escape_html(&option.option),
                    flag = if option.recommended_by_run {
                        "<span class=\"rec-flag\">the run's recommendation</span>"
                    } else {
                        ""
                    },
                    why = option
                        .note
                        .as_ref()
                        .map_or_else(String::new, |note| format!(
                            "<span class=\"why\">{}</span>",
                            escape_html(note)
                        )),
                ));
            }
            card.push_str("</ol></section>");
            if let Some(consequence) = &sifted.consequence {
                card.push_str(&format!(
                    "<p class=\"consequence\">{}</p>",
                    escape_html(consequence)
                ));
            }
            card.push_str(&format!(
                "<p class=\"fact\"><span class=\"label\">Sifted by</span><span>{}</span></p>",
                escape_html(&sifted.by)
            ));
        }
        None => {
            card.push_str("<h1>Not yet sifted</h1></div>");
            card.push_str(
                "<section class=\"block\"><h3>What this is</h3>\
<p>The overseer has not translated this hold yet. Its reporting run's own words are below, \
unchanged.</p></section>",
            );
        }
    }

    if !hold.thread.is_empty() {
        card.push_str("<section class=\"thread\">");
        for (who, what) in &hold.thread {
            card.push_str(&format!(
                "<div class=\"turn\"><span class=\"who\">{}</span><div class=\"markdown\">{}</div></div>",
                escape_html(who),
                render_markdown(what)
            ));
        }
        card.push_str("</section>");
    }

    if hold.state == QueueHoldState::WithOverseer {
        card.push_str(&format!(
            "<p class=\"pending\"><span class=\"label\">With the overseer</span><span>{}</span></p>",
            escape_html(
                hold.pending
                    .as_deref()
                    .unwrap_or("It is working on this and will come back to you here.")
            )
        ));
    }
    if hold.state != QueueHoldState::Closed {
        card.push_str(&format!(
            "<form class=\"answer\" method=\"post\" action=\"/holds/{key}/answer\">\
<label for=\"answer-{key}\">Your answer</label>\
<textarea id=\"answer-{key}\" name=\"answer\" required placeholder=\"One sentence is enough. A question back is also an answer.\"></textarea>\
<div class=\"answer-row\"><button class=\"btn btn-primary\" type=\"submit\">Record it</button>\
<span class=\"hint\">This does not close the hold. Your words are recorded as yours and the overseer answers next. \
Only the overseer writes the closing record.</span></div></form>",
            key = escape_html(&hold.key)
        ));
    }

    // The card never replaces the report. The original text stays one click away.
    card.push_str(&format!(
        "<details class=\"raw\"><summary>The run's own report</summary><pre>{}</pre></details>",
        escape_html(&hold.report.to_string())
    ));
    card.push_str("</article>");
    card
}

const fn requested_act_label(act: crate::RequestedAct) -> Option<&'static str> {
    match act {
        crate::RequestedAct::CriterionRevision => Some("criterion revision"),
        crate::RequestedAct::WorkerEnvironmentExtension => Some("environment extension"),
        crate::RequestedAct::BaseCurrencyAcceptance => Some("risk acceptance"),
        crate::RequestedAct::ParkOverrule => Some("park overrule"),
        crate::RequestedAct::Publication => Some("publication"),
        crate::RequestedAct::Spend => Some("spend"),
        crate::RequestedAct::NonDoor => None,
    }
}

const fn state_slug(state: QueueHoldState) -> &'static str {
    match state {
        QueueHoldState::WaitingForHuman => "waiting-for-human",
        QueueHoldState::WithOverseer => "with-overseer",
        QueueHoldState::WithReportingRun => "with-reporting-run",
        QueueHoldState::Closed => "closed",
    }
}

const fn state_label(state: QueueHoldState) -> &'static str {
    match state {
        QueueHoldState::WaitingForHuman => "waiting for you",
        QueueHoldState::WithOverseer => "with the overseer",
        QueueHoldState::WithReportingRun => "back with its run",
        QueueHoldState::Closed => "closed",
    }
}

fn format_age(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
        _ => format!("{}d", seconds / 86_400),
    }
}

fn render_markdown(value: &str) -> String {
    let lines: Vec<&str> = value.lines().collect();
    let mut html = String::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            index += 1;
        } else if let Some(language) = line.trim().strip_prefix("```") {
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() && !lines[index].trim().starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            let language: String = language
                .chars()
                .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
                .collect();
            let class = if language.is_empty() {
                String::new()
            } else {
                format!(" class=\"language-{}\"", escape_html(&language))
            };
            html.push_str(&format!(
                "<pre><code{class}>{}</code></pre>",
                escape_html(&code.join("\n"))
            ));
        } else if line.trim_start().starts_with("- ") {
            html.push_str("<ul>");
            while index < lines.len() {
                let Some(item) = lines[index].trim_start().strip_prefix("- ") else {
                    break;
                };
                html.push_str(&format!("<li>{}</li>", render_inline_markdown(item)));
                index += 1;
            }
            html.push_str("</ul>");
        } else {
            let mut paragraph = vec![line.trim()];
            index += 1;
            while index < lines.len()
                && !lines[index].trim().is_empty()
                && !lines[index].trim().starts_with("```")
                && !lines[index].trim_start().starts_with("- ")
            {
                paragraph.push(lines[index].trim());
                index += 1;
            }
            html.push_str(&format!(
                "<p>{}</p>",
                render_inline_markdown(&paragraph.join(" "))
            ));
        }
    }
    html
}

fn render_inline_markdown(value: &str) -> String {
    let mut html = String::new();
    let mut rest = value;
    while !rest.is_empty() {
        if let Some(code) = rest.strip_prefix('`')
            && let Some(end) = code.find('`')
        {
            html.push_str(&format!("<code>{}</code>", escape_html(&code[..end])));
            rest = &code[end + 1..];
        } else if let Some(strong) = rest.strip_prefix("**")
            && let Some(end) = strong.find("**")
        {
            html.push_str(&format!(
                "<strong>{}</strong>",
                render_inline_markdown(&strong[..end])
            ));
            rest = &strong[end + 2..];
        } else {
            let next = rest
                .find('`')
                .unwrap_or(rest.len())
                .min(rest.find("**").unwrap_or(rest.len()));
            if next == 0 {
                let mut characters = rest.chars();
                if let Some(character) = characters.next() {
                    html.push_str(&escape_html(&character.to_string()));
                    rest = characters.as_str();
                }
            } else {
                html.push_str(&escape_html(&rest[..next]));
                rest = &rest[next..];
            }
        }
    }
    html
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A queue view or supervisor journal operation failed.
#[derive(Debug, Error)]
pub enum OverseerViewError {
    /// A registered run's frozen graph cannot be read.
    #[error("failed to read registered run graph `{path}`: {source}")]
    ReadRunGraph {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A registered run's frozen graph is not JSON.
    #[error("failed to parse registered run graph `{path}`: {source}")]
    ParseRunGraph {
        path: PathBuf,
        source: serde_json::Error,
    },
    /// A registered run's graph omits a roster field.
    #[error("registered run graph `{path}` has no valid `{field}`")]
    MissingRunGraphField { path: PathBuf, field: &'static str },
    /// The store reader rejected a durable record.
    #[error("hold store read failed: {0}")]
    HoldStore(#[source] HoldStoreError),
    /// The journal path has no parent directory.
    #[error("overseer journal path has no parent: `{path}`")]
    MissingParent { path: PathBuf },
    /// The journal directory cannot be created.
    #[error("failed to create overseer journal directory `{path}`: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The journal cannot be opened.
    #[error("failed to open overseer journal `{path}`: {source}")]
    OpenJournal {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The journal cannot be read.
    #[error("failed to read overseer journal `{path}`: {source}")]
    ReadJournal {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A journal event cannot be serialized.
    #[error("failed to serialize overseer event: {0}")]
    SerializeEvent(#[source] serde_json::Error),
    /// A journal event cannot be appended.
    #[error("failed to append overseer journal `{path}`: {source}")]
    WriteJournal {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A journal append cannot be synchronized.
    #[error("failed to synchronize overseer journal `{path}`: {source}")]
    SyncJournal {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The journal ends without a newline, so its last append is not authoritative.
    #[error("overseer journal `{path}` has an incomplete final line")]
    IncompleteTail { path: PathBuf },
    /// The journal contains an empty interior record.
    #[error("overseer journal `{path}` line {line} is empty")]
    EmptyLine { path: PathBuf, line: usize },
    /// Journal time moved backwards.
    #[error("overseer journal `{path}` line {line} has an earlier timestamp than its predecessor")]
    TimestampWentBackwards { path: PathBuf, line: usize },
    /// A retained journal line is malformed.
    #[error("failed to parse overseer journal `{path}` line {line}: {source}")]
    ParseEvent {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
}
