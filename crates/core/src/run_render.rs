//! rendered_run : WorkPackageGraph × DriverEvent* → HTML
//!
//! The renderer is pure and deterministic: graph and journal values are its only inputs.

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{
    CommandExitStatus, CriterionExecution, CriterionOrigin, DependencyKind, DriverEvent,
    DriverPackageState, FindingReplayDecision, PackageDriverError, WorkPackageGraph,
    derive_driver_snapshot,
};

/// A failure to derive the journal view required by the page.
#[derive(Debug, Error)]
pub enum RunRenderError {
    /// The journal contradicts the frozen graph or itself.
    #[error("cannot render contradictory driver history: {source}")]
    InvalidJournal { source: PackageDriverError },
}

#[derive(Clone, Copy)]
struct Point {
    x: usize,
    y: usize,
}

fn escaped(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
    output
}

fn shortened(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_owned()
    } else {
        let mut result = value
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        result.push('…');
        result
    }
}

fn status_text(status: &CommandExitStatus) -> String {
    match status {
        CommandExitStatus::Exited { code } => format!("exit {code}"),
        CommandExitStatus::Signaled { signal } => format!("signal {signal}"),
    }
}

fn state_identity(
    state: &DriverPackageState,
    ready: &HashSet<&str>,
    package: &str,
) -> (&'static str, &'static str, &'static str) {
    match state {
        DriverPackageState::Pending if ready.contains(package) => ("ready", "◇", "Ready"),
        DriverPackageState::Pending => ("not-ready", "○", "Not yet ready"),
        DriverPackageState::Running { .. } => ("running", "▶", "Running"),
        DriverPackageState::Judging { .. } => ("running", "▶", "Running · judging"),
        DriverPackageState::Complete => ("complete", "✓", "Complete"),
        DriverPackageState::Failed { .. } | DriverPackageState::CompositionFailed { .. } => {
            ("failed", "✕", "Failed")
        }
        DriverPackageState::Parked { .. } => ("parked", "Ⅱ", "Parked"),
        // Not a judgement: the criteria never ran, so this is neither passing nor failing.
        DriverPackageState::EnvironmentPreparationFailed { .. } => {
            ("environment", "⚑", "Environment not prepared")
        }
        DriverPackageState::EnvironmentBlocked { .. } => {
            ("environment", "⚑", "Worker environment unavailable")
        }
    }
}

fn topological_order(graph: &WorkPackageGraph) -> Vec<usize> {
    let index = graph
        .packages()
        .iter()
        .enumerate()
        .map(|(position, package)| (package.id().as_str(), position))
        .collect::<HashMap<_, _>>();
    let mut indegree = graph
        .packages()
        .iter()
        .map(|package| package.depends_on().len())
        .collect::<Vec<_>>();
    let mut dependents = vec![Vec::new(); graph.packages().len()];
    for (dependent, package) in graph.packages().iter().enumerate() {
        for edge in package.depends_on() {
            dependents[index[edge.id().as_str()]].push(dependent);
        }
    }
    let mut order = Vec::with_capacity(graph.packages().len());
    let mut emitted = vec![false; graph.packages().len()];
    while order.len() < graph.packages().len() {
        let next = (0..graph.packages().len())
            .find(|position| !emitted[*position] && indegree[*position] == 0)
            .unwrap_or_else(|| unreachable!("validated work-package graph is acyclic"));
        emitted[next] = true;
        order.push(next);
        for dependent in &dependents[next] {
            indegree[*dependent] -= 1;
        }
    }
    order
}

fn critical_path(graph: &WorkPackageGraph) -> (HashSet<usize>, HashSet<(usize, usize)>) {
    let index = graph
        .packages()
        .iter()
        .enumerate()
        .map(|(position, package)| (package.id().as_str(), position))
        .collect::<HashMap<_, _>>();
    let order = topological_order(graph);
    let mut forward = vec![1_usize; graph.packages().len()];
    for position in &order {
        for dependency in graph.packages()[*position]
            .depends_on()
            .iter()
            .filter(|edge| edge.kind().is_binding())
        {
            forward[*position] =
                forward[*position].max(forward[index[dependency.id().as_str()]] + 1);
        }
    }
    let mut reverse = vec![1_usize; graph.packages().len()];
    for position in order.iter().rev() {
        for (dependent, package) in graph.packages().iter().enumerate() {
            if package.depends_on().iter().any(|edge| {
                edge.kind().is_binding()
                    && edge.id().as_str() == graph.packages()[*position].id().as_str()
            }) {
                reverse[*position] = reverse[*position].max(reverse[dependent] + 1);
            }
        }
    }
    let longest = forward.iter().copied().max().unwrap_or(1);
    let nodes = (0..graph.packages().len())
        .filter(|position| forward[*position] + reverse[*position] - 1 == longest)
        .collect::<HashSet<_>>();
    let mut edges = HashSet::new();
    for (dependent, package) in graph.packages().iter().enumerate() {
        for edge in package
            .depends_on()
            .iter()
            .filter(|edge| edge.kind().is_binding())
        {
            let prerequisite = index[edge.id().as_str()];
            if forward[prerequisite] + reverse[dependent] == longest {
                edges.insert((prerequisite, dependent));
            }
        }
    }
    (nodes, edges)
}

fn dependency_depths(graph: &WorkPackageGraph) -> Vec<usize> {
    let index = graph
        .packages()
        .iter()
        .enumerate()
        .map(|(position, package)| (package.id().as_str(), position))
        .collect::<HashMap<_, _>>();
    let mut depths = vec![0_usize; graph.packages().len()];
    for position in topological_order(graph) {
        depths[position] = graph.packages()[position]
            .depends_on()
            .iter()
            .map(|edge| depths[index[edge.id().as_str()]] + 1)
            .max()
            .unwrap_or(0);
    }
    depths
}

fn render_execution(execution: &CriterionExecution) -> String {
    let status = status_text(execution.exit_status());
    let status_class = if execution.exit_status().is_success() {
        "pass"
    } else {
        "fail"
    };
    format!(
        "<div class=\"execution {status_class}\"><div class=\"execution-head\"><span>{status}</span><code>{command}</code></div><p class=\"cwd\">Working directory: <code>{cwd}</code></p><div class=\"streams\"><div><h5>stdout</h5><pre>{stdout}</pre></div><div><h5>stderr</h5><pre>{stderr}</pre></div></div></div>",
        command = escaped(execution.command()),
        cwd = escaped(execution.working_directory()),
        stdout = escaped(execution.stdout()),
        stderr = escaped(execution.stderr()),
    )
}

const STYLE: &str = r##"
:root{--ground:#F1F4F2;--surface:#FFFFFF;--ink:#16201C;--muted:#5B6A63;--rule:#D6DDD8;--accent:#1F6F5C;--accent-soft:#E2EFEA;--warn:#A8442A;--warn-soft:#F7E8E2;--serif:Georgia,"Iowan Old Style","Source Serif 4","Times New Roman",serif;--sans:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;--mono:ui-monospace,SFMono-Regular,"SF Mono",Menlo,Consolas,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--ground:#111512;--surface:#181E1B;--ink:#E3EAE5;--muted:#93A29A;--rule:#2A332E;--accent:#55B79C;--accent-soft:#16302A;--warn:#DE8365;--warn-soft:#33201A}}
:root[data-theme="dark"]{--ground:#111512;--surface:#181E1B;--ink:#E3EAE5;--muted:#93A29A;--rule:#2A332E;--accent:#55B79C;--accent-soft:#16302A;--warn:#DE8365;--warn-soft:#33201A}
*{box-sizing:border-box}body{background:var(--ground);color:var(--ink);font-family:var(--sans);font-size:16px;line-height:1.55;margin:0;padding:0 24px 100px;-webkit-font-smoothing:antialiased}.wrap{max-width:1120px;margin:0 auto}.mast{padding:64px 0 36px;border-bottom:1px solid var(--rule);margin-bottom:44px}.eyebrow,.sub{font-family:var(--mono);font-size:11.5px;letter-spacing:.13em;text-transform:uppercase;color:var(--muted)}h1,h2{font-family:var(--serif);font-weight:400;letter-spacing:-.02em}h1{font-size:clamp(2.4rem,6vw,3.5rem);line-height:1.06;margin:12px 0 18px}h2{font-size:1.8rem;margin:0 0 5px}.thesis{font-family:var(--serif);font-size:1.2rem;color:var(--muted);margin:0}.graph-scroll{overflow-x:auto;background:var(--surface);border:1px solid var(--rule);border-radius:4px;padding:22px;margin:24px 0 18px}.graph-scroll svg{display:block;max-width:none;height:auto}.legend{display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin:18px 0 52px}.legend div{border:1px solid var(--rule);background:var(--surface);padding:12px 15px;border-radius:3px}.legend b{font-family:var(--mono);font-size:12px}.legend p{font-size:13px;color:var(--muted);margin:4px 0 0}.edge-buildability{stroke:var(--accent);stroke-width:2}.edge-safety{stroke:var(--warn);stroke-width:3}.edge-risk-ordering{stroke:var(--muted);stroke-width:2;stroke-dasharray:8 6}.critical-track{stroke:var(--muted);stroke-width:8;stroke-dasharray:1 5;opacity:.45}.edge-critical{}.edge-label{font:600 11px var(--mono);fill:var(--ink)}.node rect{fill:var(--surface);stroke:var(--muted);stroke-width:1.5}.node-critical-true>rect:first-of-type{stroke:var(--accent);stroke-width:4}.node .id{font:700 14px var(--mono);fill:var(--accent)}.node .title{font:15px var(--sans);fill:var(--ink)}.node .badge{font:600 11px var(--mono);fill:var(--ink)}.node-state-complete>rect:first-of-type{stroke:var(--accent);stroke-width:5}.node-state-failed>rect:first-of-type{stroke:var(--warn);stroke-width:5;stroke-dasharray:10 4}.node-state-parked>rect:first-of-type{stroke:var(--muted);stroke-width:5;stroke-dasharray:3 3}.node-state-running>rect:first-of-type{stroke:var(--accent);stroke-width:3;stroke-dasharray:12 4}.node-state-ready>rect:first-of-type{stroke:var(--accent);stroke-width:2}.node-state-not-ready>rect:first-of-type{stroke:var(--muted);stroke-dasharray:2 5}.node-state-environment>rect:first-of-type{stroke:var(--muted);stroke-width:5;stroke-dasharray:14 3 2 3}svg [stroke="#A8442A"]{stroke:var(--warn)}svg [fill="#A8442A"]{fill:var(--warn)}svg [stroke="#1F6F5C"]{stroke:var(--accent)}svg [fill="#1F6F5C"]{fill:var(--accent)}svg [stroke="#5B6A63"]{stroke:var(--muted)}svg [fill="#5B6A63"]{fill:var(--muted)}svg [fill="#16201C"]{fill:var(--ink)}svg [fill="#FFFFFF"]{fill:var(--surface)}.packages{display:grid;gap:18px}.package{background:var(--surface);border:1px solid var(--rule);border-left:4px solid var(--muted);border-radius:4px;padding:22px 24px}.package.failed{border-left-color:var(--warn)}.package.complete,.package.running,.package.ready{border-left-color:var(--accent)}.package-head{display:flex;justify-content:space-between;gap:20px;align-items:start}.package h3{margin:0;font-family:var(--serif);font-size:1.35rem;font-weight:400}.package-id{font-family:var(--mono);font-size:12px;color:var(--accent)}.state-badge{font-family:var(--mono);font-size:12px;border:2px solid currentColor;border-radius:2px;padding:3px 8px;white-space:nowrap}.state-badge.failed{border-style:dashed;color:var(--warn)}.state-badge.parked{border-style:dotted}.state-badge.environment{border-style:double}.reason{background:var(--warn-soft);border-left:3px solid var(--warn);padding:9px 12px}.criterion,.finding{border-top:1px solid var(--rule);padding-top:16px;margin-top:18px}.criterion h4,.finding h4{margin:0 0 6px;font-size:15px}.criterion-meta,.origin,.cwd,.refs{font-size:13px;color:var(--muted);margin:4px 0}.origin.amendment{color:var(--warn);font-family:var(--mono)}code,pre{font-family:var(--mono)}code{background:var(--accent-soft);padding:1px 5px;border-radius:3px}.execution{border:1px solid var(--rule);border-left:3px solid var(--accent);padding:12px;margin-top:12px}.execution.fail{border-left-color:var(--warn);background:var(--warn-soft)}.execution-head{display:flex;gap:14px;align-items:baseline}.execution-head span{font:700 12px var(--mono)}.streams{display:grid;grid-template-columns:1fr 1fr;gap:10px}.streams h5{font:11px var(--mono);text-transform:uppercase;color:var(--muted);margin:4px 0}.streams pre{background:var(--ground);border:1px solid var(--rule);padding:10px;white-space:pre-wrap;overflow-wrap:anywhere;margin:0;min-height:42px}.finding.accepted{border-left:3px solid var(--accent);padding-left:14px}.finding.rejected{border-left:3px dashed var(--warn);padding-left:14px}.decision{font:700 12px var(--mono);text-transform:uppercase}.finding-executions{display:grid;grid-template-columns:1fr 1fr;gap:12px}details summary{cursor:pointer;font-weight:600;margin-top:8px}footer{color:var(--muted);font:12px var(--mono);border-top:1px solid var(--rule);margin-top:48px;padding-top:16px}@media(max-width:760px){.legend,.streams,.finding-executions{grid-template-columns:1fr}.package-head{display:block}.state-badge{display:inline-block;margin-top:8px}}
"##;

/// Render a frozen graph and any durable driver history as one standalone HTML document.
///
/// An empty event slice intentionally renders the plan without lifecycle state. This represents an
/// unstarted run without a separate rendering path.
///
/// # Errors
///
/// Returns [`RunRenderError`] when non-empty journal history cannot be folded against the graph.
pub fn render_package_run(
    graph: &WorkPackageGraph,
    events: &[DriverEvent],
) -> Result<String, RunRenderError> {
    let snapshot = derive_driver_snapshot(graph, events, false)
        .map_err(|source| RunRenderError::InvalidJournal { source })?;
    let states = snapshot
        .packages()
        .iter()
        .map(|(package, state)| (package.as_str(), state))
        .collect::<HashMap<_, _>>();
    let ready = snapshot
        .ready()
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let (critical_nodes, critical_edges) = critical_path(graph);
    let depths = dependency_depths(graph);
    let max_depth = depths.iter().copied().max().unwrap_or(0);
    let mut layer_counts = vec![0_usize; max_depth + 1];
    let mut points = Vec::with_capacity(graph.packages().len());
    for depth in &depths {
        let row = layer_counts[*depth];
        layer_counts[*depth] += 1;
        points.push(Point {
            x: 40 + depth * 290,
            y: 30 + row * 120,
        });
    }
    let width = if graph.packages().len() == 1 {
        300
    } else {
        80 + (max_depth + 1) * 290
    };
    let height = if graph.packages().len() == 1 {
        150
    } else {
        60 + layer_counts.iter().copied().max().unwrap_or(1) * 120
    };
    let index = graph
        .packages()
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id().as_str(), i))
        .collect::<HashMap<_, _>>();

    let mut svg = format!(
        "<svg viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" role=\"img\" aria-label=\"Work-package dependency graph\"><defs><marker id=\"arrow-build\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"#1F6F5C\"/></marker><marker id=\"arrow-warn\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"#A8442A\"/></marker><marker id=\"arrow-muted\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"#5B6A63\"/></marker></defs>"
    );
    for (to, package) in graph.packages().iter().enumerate() {
        for edge in package.depends_on() {
            let from = index[edge.id().as_str()];
            let start = points[from];
            let end = points[to];
            let (kind, marker, label) = match edge.kind() {
                DependencyKind::Buildability => ("buildability", "arrow-build", "B"),
                DependencyKind::Safety => ("safety", "arrow-warn", "S"),
                DependencyKind::RiskOrdering => ("risk-ordering", "arrow-muted", "R · choice"),
            };
            let critical = critical_edges.contains(&(from, to));
            let near = edge.kind() == DependencyKind::RiskOrdering
                && (critical_nodes.contains(&from) || critical_nodes.contains(&to));
            svg.push_str(&format!("<g data-edge-kind=\"{kind}\" data-critical=\"{critical}\" data-near-critical=\"{near}\"><title>{reason}</title>{critical_track}<line class=\"edge-{kind}{critical_class}\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" marker-end=\"url(#{marker})\"/><text class=\"edge-label\" x=\"{lx}\" y=\"{ly}\">{label_display}</text></g>", label_display=if critical { format!("{label} · CP") } else { label.to_owned() }, critical_track=if critical { format!("<line class=\"critical-track\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>", start.x+220, start.y+45, end.x-5, end.y+45) } else { String::new() }, critical_class=if critical { " edge-critical" } else { "" }, reason=escaped(edge.reason()), x1=start.x+220, y1=start.y+45, x2=end.x-5, y2=end.y+45, lx=(start.x+220+end.x)/2, ly=(start.y+end.y)/2+38));
        }
    }
    for (position, package) in graph.packages().iter().enumerate() {
        let point = points[position];
        let critical = critical_nodes.contains(&position);
        let (state_class, state_markup, state_data) = if events.is_empty() {
            ("", String::new(), String::new())
        } else {
            let (identity, symbol, label) =
                state_identity(states[package.id().as_str()], &ready, package.id().as_str());
            (
                identity,
                format!(
                    "<text class=\"badge\" x=\"{}\" y=\"{}\">{} {}</text>",
                    point.x + 14,
                    point.y + 72,
                    symbol,
                    label
                ),
                format!(" data-state=\"{identity}\""),
            )
        };
        svg.push_str(&format!("<a href=\"#package-{position}\"><g class=\"node node-critical-{critical} node-state-{state_class}\" data-critical=\"{critical}\" data-package=\"{id}\"{state_data}><title>{full_title}</title><rect x=\"{x}\" y=\"{y}\" width=\"220\" height=\"90\" rx=\"3\"/><text class=\"id\" x=\"{tx}\" y=\"{iy}\">{id}</text><text class=\"title\" x=\"{tx}\" y=\"{ty}\">{title}</text>{state_markup}</g></a>", id=escaped(package.id().as_str()), title=escaped(&shortened(package.title(), 27)), full_title=escaped(package.title()), x=point.x, y=point.y, tx=point.x+14, iy=point.y+25, ty=point.y+49));
    }
    svg.push_str("</svg>");

    let mut details = String::new();
    for (position, package) in graph.packages().iter().enumerate() {
        let state = states[package.id().as_str()];
        let (identity, symbol, label) = state_identity(state, &ready, package.id().as_str());
        let state_badge = if events.is_empty() {
            String::new()
        } else {
            format!(
                "<span class=\"state-badge {identity}\" data-state=\"{identity}\">{symbol} {label}</span>"
            )
        };
        let reason = if events.is_empty() {
            String::new()
        } else {
            match state {
                DriverPackageState::Failed { reason } | DriverPackageState::Parked { reason } => {
                    format!("<p class=\"reason\">{}</p>", escaped(reason))
                }
                _ => String::new(),
            }
        };
        details.push_str(&format!("<article class=\"package {identity}\" id=\"package-{position}\" data-package=\"{id}\"><div class=\"package-head\"><div><span class=\"package-id\">{id}</span><h3>{title}</h3></div>{state_badge}</div>{reason}<p class=\"criterion-meta\">Repositories: {repositories}</p>", id=escaped(package.id().as_str()), title=escaped(package.title()), repositories=package.repositories().iter().map(|value| escaped(value)).collect::<Vec<_>>().join(", ")));

        let amendments = snapshot
            .amendments()
            .iter()
            .filter(|(id, _)| id == package.id().as_str())
            .map(|(_, criterion)| criterion);
        for (name, command, origin, input, observation) in package
            .criteria()
            .iter()
            .map(|criterion| {
                (
                    criterion.name(),
                    criterion.command(),
                    CriterionOrigin::Authored,
                    Some(criterion.input()),
                    Some(criterion.observation()),
                )
            })
            .chain(amendments.map(|criterion| {
                (
                    criterion.name.as_str(),
                    criterion.command.as_str(),
                    criterion.origin.clone(),
                    None,
                    None,
                )
            }))
        {
            let origin_markup = match &origin {
                CriterionOrigin::Authored => {
                    "<p class=\"origin\">Authored criterion</p>".to_owned()
                }
                CriterionOrigin::Amendment { gate, finding } => format!(
                    "<p class=\"origin amendment\">Amendment from gate {}, finding {finding}</p>",
                    escaped(gate)
                ),
            };
            let description = match (input, observation) {
                (Some(input), Some(observation)) => format!(
                    "<p class=\"criterion-meta\">Input: {} · Expected: {}</p>",
                    escaped(input),
                    escaped(observation)
                ),
                _ => String::new(),
            };
            let last_execution = events.iter().rev().find_map(|event| match event {
                DriverEvent::CriterionExecuted {
                    package: event_package,
                    name: event_name,
                    origin: event_origin,
                    execution,
                } if event_package == package.id().as_str()
                    && event_name == name
                    && event_origin == &origin =>
                {
                    Some(execution)
                }
                _ => None,
            });
            let execution = last_execution
                .map(render_execution)
                .unwrap_or_else(|| "<p class=\"criterion-meta\">Not executed</p>".to_owned());
            details.push_str(&format!("<section class=\"criterion\"><h4>{}</h4>{origin_markup}{description}<p><code>{}</code></p>{execution}</section>", escaped(name), escaped(command)));
        }

        for event in events {
            if let DriverEvent::FindingReplayed {
                package: event_package,
                gate,
                finding,
                command,
                repository_refs,
                witness,
                repair,
                decision,
            } = event
                && event_package == package.id().as_str()
            {
                let (decision_name, decision_label) = match decision {
                    FindingReplayDecision::Accepted => {
                        ("accepted", "Accepted · credited as amendment".to_owned())
                    }
                    FindingReplayDecision::Rejected { reason } => (
                        "rejected",
                        format!(
                            "Rejected · {}",
                            match reason {
                                crate::FindingRejectionReason::WitnessPassed => "witness-passed",
                                crate::FindingRejectionReason::RepairFailed => "repair-failed",
                                crate::FindingRejectionReason::StructurallyMalformed => {
                                    "structurally-malformed"
                                }
                            }
                        ),
                    ),
                };
                let refs = repository_refs
                    .iter()
                    .map(|item| {
                        format!(
                            "{}: witness {} → repair {}",
                            escaped(&item.repository),
                            escaped(&item.witness_ref),
                            escaped(&item.repair_ref)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                details.push_str(&format!("<section class=\"finding {decision_name}\" data-finding-decision=\"{decision_name}\"><h4>Gate {gate}, finding {finding}</h4><p class=\"decision\">{decision_label}</p><p><code>{command}</code></p><p class=\"refs\">{refs}</p><details><summary>Replay evidence</summary><div class=\"finding-executions\"><div><h5>Witness</h5>{witness}</div><div><h5>Repair</h5>{repair}</div></div></details></section>", gate=escaped(gate), command=escaped(command), witness=render_execution(witness), repair=render_execution(repair)));
            }
            if let DriverEvent::FindingRejected {
                package: event_package,
                gate,
                finding,
                command,
                reason,
                detail,
            } = event
                && event_package == package.id().as_str()
            {
                let reason = match reason {
                    crate::FindingRejectionReason::StructurallyMalformed => {
                        "structurally-malformed"
                    }
                    crate::FindingRejectionReason::WitnessPassed => "witness-passed",
                    crate::FindingRejectionReason::RepairFailed => "repair-failed",
                };
                details.push_str(&format!(r#"<section class="finding rejected" data-finding-decision="rejected"><h4>Gate {gate}, finding {finding}</h4><p class="decision">Rejected · {reason}</p><p><code>{command}</code></p><p class="validation-detail">{detail}</p></section>"#, gate=escaped(gate), command=escaped(command), detail=escaped(detail)));
            }
            if let DriverEvent::GateFinished {
                package: event_package,
                gate,
            } = event
                && event_package == package.id().as_str()
                && !events.iter().any(|candidate| match candidate {
                    DriverEvent::FindingReplayed {
                        package,
                        gate: candidate_gate,
                        ..
                    }
                    | DriverEvent::FindingRejected {
                        package,
                        gate: candidate_gate,
                        ..
                    } => package == event_package && candidate_gate == gate,
                    _ => false,
                })
            {
                details.push_str(&format!(r#"<section class="gate-summary no-findings"><h4>Gate {}</h4><p>No findings reported</p></section>"#, escaped(gate)));
            }
        }
        details.push_str("</article>");
    }

    let run_copy = if events.is_empty() {
        "Plan structure · run not started"
    } else {
        "Run state reconstructed from the driver journal"
    };
    Ok(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{vision} · work-package run</title><style>{STYLE}</style></head><body><div class=\"wrap\"><header class=\"mast\"><p class=\"eyebrow\">Plan v{version} · authored at {authored}</p><h1>{vision}</h1><p class=\"thesis\">{run_copy}. The highlighted binding chain is the computed critical path.</p></header><main><section><h2>The run, rendered</h2><p class=\"sub\">Dependency depth flows left to right</p><div class=\"graph-scroll\">{svg}</div><div class=\"legend\"><div><b>━━ B · Buildability</b><p>Solid code fact. Binding.</p></div><div><b>━━━━ S · Safety</b><p>Heavy irreversible-act guard. Binding.</p></div><div><b>┄┄ R · Risk ordering</b><p>Overridable choice, not a fact.</p></div></div></section><section><h2>Packages and evidence</h2><p class=\"sub\">Last criterion executions and every gate finding</p><div class=\"packages\">{details}</div></section></main><footer>Deterministic rendering · no clock or external assets</footer></div></body></html>",
        vision = escaped(graph.vision()),
        version = graph.plan_version(),
        authored = escaped(graph.authored_at_ref())
    ))
}
