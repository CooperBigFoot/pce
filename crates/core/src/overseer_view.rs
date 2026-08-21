//! queue view = render(holds × registrations × latest overseer events × now)
//!
//! The view is derived from durable store records. It never treats a missing or stale heartbeat as
//! an empty queue, and it carries no repository authority.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EventTimestamp, HoldRoute, HoldState, HoldStore, HoldStoreError, RunRegistration};

/// The model used for every initial and replacement overseer session.
pub const OVERSEER_MODEL: &str = "claude-fable-5";
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
    },
}

impl OverseerEvent {
    pub const fn timestamp(&self) -> EventTimestamp {
        match self {
            Self::Heartbeat { timestamp }
            | Self::SessionSpawned { timestamp, .. }
            | Self::SessionExited { timestamp, .. } => *timestamp,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverseerLiveness {
    Running,
    NotRunning,
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
    pub report: serde_json::Value,
}

/// A complete queue snapshot. Holds are never suppressed because the overseer is unavailable.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueueView {
    pub overseer: OverseerLiveness,
    pub holds: Vec<QueueHold>,
    pub runs: Vec<RunRegistration>,
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
    let latest_session = events.iter().rev().find(|event| {
        matches!(
            event,
            OverseerEvent::SessionSpawned { .. } | OverseerEvent::SessionExited { .. }
        )
    });
    let heartbeat_fresh = latest_heartbeat.is_some_and(|timestamp| {
        let elapsed = now.signed_duration_since(*timestamp).num_milliseconds();
        elapsed >= 0 && elapsed <= i64::try_from(stale_after.as_millis()).unwrap_or(i64::MAX)
    });
    let overseer = if heartbeat_fresh
        && matches!(latest_session, Some(OverseerEvent::SessionSpawned { .. }))
    {
        OverseerLiveness::Running
    } else {
        OverseerLiveness::NotRunning
    };

    let holds = store
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
            QueueHold {
                key: hold.key().as_str().to_owned(),
                repository: hold.identity().repository().to_owned(),
                plan_version: hold.identity().plan_version().to_owned(),
                package: hold.identity().package().to_owned(),
                question_kind: hold.identity().question_kind().to_owned(),
                state,
                age_seconds,
                report: hold.report().clone(),
            }
        })
        .collect();
    let runs = store
        .run_registrations()
        .map_err(OverseerViewError::HoldStore)?;
    Ok(QueueView {
        overseer,
        holds,
        runs,
    })
}

/// Renders the queue as a self-contained HTML page.
pub fn render_queue_html(view: &QueueView) -> String {
    let status = match view.overseer {
        OverseerLiveness::Running => "overseer is running",
        OverseerLiveness::NotRunning => "overseer is not running",
    };
    let mut html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>PCE hold queue</title></head><body><h1>Hold queue</h1><p id=\"overseer-status\">{status}</p><section id=\"holds\">"
    );
    for hold in &view.holds {
        let state = match hold.state {
            QueueHoldState::WaitingForHuman => "waiting-for-human",
            QueueHoldState::WithOverseer => "with-overseer",
            QueueHoldState::WithReportingRun => "with-reporting-run",
            QueueHoldState::Closed => "closed",
        };
        html.push_str(&format!("<article data-hold-key=\"{}\" data-state=\"{}\"><h2>{} · {}</h2><p>{}</p><p>age: {} seconds</p></article>",
            escape_html(&hold.key), state, escape_html(&hold.repository), escape_html(&hold.package), escape_html(&hold.question_kind), hold.age_seconds));
        html.push_str(&format!(
            "<pre class=\"report\">{}</pre><form method=\"post\" action=\"/api/holds/{}/answer\"><input name=\"by\" required><textarea name=\"answer\" required></textarea><button>Answer</button></form>",
            escape_html(&hold.report.to_string()), escape_html(&hold.key)
        ));
    }
    html.push_str("</section><section id=\"runs\"><h2>Runs</h2>");
    for run in &view.runs {
        html.push_str(&format!(
            "<article data-repository=\"{}\"><h3>{}</h3><p>{}</p><p>{}</p></article>",
            escape_html(run.repository()),
            escape_html(run.repository()),
            escape_html(&run.vision_directory().display().to_string()),
            escape_html(&format!(
                "graph: {}; journal: {}; herdr session: {}",
                run.frozen_graph().display(),
                run.journal().display(),
                run.herdr_session().unwrap_or("none")
            ))
        ));
    }
    html.push_str("</section></body></html>");
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
