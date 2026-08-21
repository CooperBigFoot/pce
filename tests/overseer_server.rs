use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use pce_core::{
    EventTimestamp, HoldIdentity, HoldRoute, HoldStore, OVERSEER_MODEL, OVERSEER_REASONING_EFFORT,
    OverseerEvent, OverseerJournal, OverseerLiveness, QueueHoldState, RunRegistration,
    derive_queue_view, render_queue_html,
};
use serde_json::json;
use tempfile::TempDir;

fn at(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 0, second)
            .single()
            .expect("fixture time"),
    )
}

fn seed_human_hold(store: &HoldStore, package: &str, second: u32) -> String {
    let opened = store
        .open(
            HoldIdentity::parse("pce", "v1", package, format!("question-{package}"))
                .expect("identity"),
            json!({"question": format!("question for {package}")}),
            at(second),
        )
        .expect("open hold");
    let key = opened.hold().key().clone();
    store
        .route(&key, HoldRoute::Human, at(second + 1))
        .expect("route human");
    key.as_str().to_owned()
}

#[test]
fn stale_heartbeat_is_reported() {
    let directory = tempfile::tempdir().expect("store root");
    let store = HoldStore::new(directory.path());
    let keys: Vec<_> = (0..3)
        .map(|index| seed_human_hold(&store, &format!("OQ{index}"), index * 2))
        .collect();
    let journal = OverseerJournal::new(directory.path());
    journal
        .append(&OverseerEvent::SessionSpawned {
            timestamp: at(6),
            model: OVERSEER_MODEL.to_owned(),
            reasoning_effort: OVERSEER_REASONING_EFFORT.to_owned(),
        })
        .expect("spawn event");
    journal
        .append(&OverseerEvent::Heartbeat { timestamp: at(7) })
        .expect("heartbeat");

    let now = Utc
        .with_ymd_and_hms(2026, 8, 21, 12, 0, 20)
        .single()
        .expect("now");
    let view = derive_queue_view(&store, &journal, now, Duration::from_secs(5)).expect("view");
    assert_eq!(view.overseer, OverseerLiveness::NotRunning);
    assert_eq!(view.holds.len(), 3);
    assert!(
        view.holds
            .iter()
            .all(|hold| hold.state == QueueHoldState::WaitingForHuman)
    );
    assert!(view.holds.iter().all(|hold| hold.age_seconds > 0));
    let html = render_queue_html(&view);
    assert!(html.contains("overseer is not running"));
    for key in keys {
        assert!(html.contains(&key), "missing hold {key}");
    }
}

#[test]
fn view_lists_a_newly_registered_run() {
    let directory = tempfile::tempdir().expect("store root");
    let vision = directory.path().join("unseen-vision");
    fs::create_dir_all(&vision).expect("vision directory");
    let registration = RunRegistration::parse(
        "new-repository",
        &vision,
        vision.join("graph.json"),
        vision.join("journal.jsonl"),
        Some("new-run".to_owned()),
    )
    .expect("registration");
    let store = HoldStore::new(directory.path().join("holds"));
    store
        .register_run(registration, at(0))
        .expect("register run");
    let journal = OverseerJournal::new(store.root());
    let now = Utc
        .with_ymd_and_hms(2026, 8, 21, 12, 0, 1)
        .single()
        .expect("now");
    let view = derive_queue_view(&store, &journal, now, Duration::from_secs(5)).expect("view");
    assert_eq!(view.runs.len(), 1);
    assert_eq!(view.runs[0].repository(), "new-repository");
    let html = render_queue_html(&view);
    assert!(html.contains("new-repository"));
    assert!(html.contains(&vision.display().to_string()));
}

struct ServerFixture {
    _directory: TempDir,
    root: PathBuf,
    capture: PathBuf,
    child: Child,
    address: String,
}

impl ServerFixture {
    fn start(script_body: &str, cwd: &Path) -> Self {
        let directory = tempfile::tempdir().expect("server directory");
        let root = directory.path().join("holds");
        let capture = directory.path().join("spawn-arguments");
        let script = directory.path().join("session");
        fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CAPTURE\"\n{script_body}\n"),
        )
        .expect("session script");
        let mut permissions = fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("script executable");
        let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args([
                "overseer",
                "serve",
                "--root",
                root.to_str().expect("root UTF-8"),
                "--listen",
                "127.0.0.1:0",
                "--session-program",
                script.to_str().expect("script UTF-8"),
                "--heartbeat-ms",
                "25",
                "--heartbeat-stale-ms",
                "100",
            ])
            .env("CAPTURE", &capture)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start server");
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("server stdout"))
            .read_line(&mut address)
            .expect("read server URL");
        assert!(
            address.starts_with("http://127.0.0.1:"),
            "unexpected URL {address:?}"
        );
        Self {
            _directory: directory,
            root,
            capture,
            child,
            address: address.trim().to_owned(),
        }
    }

    fn wait_for_spawns(&self, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let lines = fs::read_to_string(&self.capture)
                .unwrap_or_default()
                .lines()
                .count();
            if lines >= count {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("did not observe {count} session spawns");
    }

    fn request(&self, request: &[u8]) -> Vec<u8> {
        let address = self
            .address
            .trim_start_matches("http://")
            .trim_end_matches('/');
        let mut stream = TcpStream::connect(address).expect("connect server");
        stream.write_all(request).expect("write request");
        stream
            .shutdown(std::net::Shutdown::Write)
            .expect("finish request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        response
    }
}

impl Drop for ServerFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn session_exit_is_respawned() {
    let cwd = tempfile::tempdir().expect("cwd");
    let server = ServerFixture::start("exit 17", cwd.path());
    server.wait_for_spawns(2);
    let events = OverseerJournal::new(&server.root).read().expect("journal");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, OverseerEvent::SessionExited { code: Some(17), .. }))
    );
    assert!(
        events
            .iter()
            .filter(|event| matches!(event, OverseerEvent::SessionSpawned { .. }))
            .count()
            >= 2
    );
}

#[test]
fn server_mutates_no_repository() {
    let repository = tempfile::tempdir().expect("repository");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(repository.path())
        .status()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repository.path())
        .status()
        .expect("git config");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repository.path())
        .status()
        .expect("git config");
    fs::write(repository.path().join("tracked"), "unchanged\n").expect("tracked file");
    Command::new("git")
        .args(["add", "tracked"])
        .current_dir(repository.path())
        .status()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-qm", "base"])
        .current_dir(repository.path())
        .status()
        .expect("git commit");
    let observe = || {
        let status = Command::new("git")
            .args(["status", "--porcelain=v1", "--untracked-files=all"])
            .current_dir(repository.path())
            .output()
            .expect("git status")
            .stdout;
        let refs = Command::new("git")
            .args(["show-ref", "--head"])
            .current_dir(repository.path())
            .output()
            .expect("git refs")
            .stdout;
        (status, refs)
    };
    let before = observe();
    let server = ServerFixture::start("sleep 2", repository.path());
    let store = HoldStore::new(&server.root);
    let key = seed_human_hold(&store, "OQ4", 0);
    let body = br#"{"by":"human","answer":"continue"}"#;
    let request = format!(
        "POST /api/holds/{key}/answer HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut bytes = request.into_bytes();
    bytes.extend_from_slice(body);
    let response = server.request(&bytes);
    assert!(
        response.starts_with(b"HTTP/1.1 200 OK"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    let hold = store
        .read(&pce_core::HoldKey::parse(key).expect("key"))
        .expect("answered hold");
    assert!(hold.records().iter().any(|record| matches!(record, pce_core::HoldRecord::Answered { by, answer, .. } if by == "human" && answer == "continue")));
    assert_eq!(
        observe(),
        before,
        "server or session mutated repository state or refs"
    );
}

#[test]
fn a_respawned_session_is_the_declared_model() {
    let cwd = tempfile::tempdir().expect("cwd");
    let server = ServerFixture::start("exit 0", cwd.path());
    server.wait_for_spawns(2);
    let invocations = fs::read_to_string(&server.capture).expect("spawn arguments");
    for invocation in invocations.lines().take(2) {
        assert!(
            invocation.contains(&format!("--model {OVERSEER_MODEL}")),
            "{invocation}"
        );
        assert!(
            invocation.contains(&format!("--thinking {OVERSEER_REASONING_EFFORT}")),
            "{invocation}"
        );
    }
    let events = OverseerJournal::new(&server.root).read().expect("journal");
    let spawns: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            OverseerEvent::SessionSpawned {
                model,
                reasoning_effort,
                ..
            } => Some((model, reasoning_effort)),
            _ => None,
        })
        .collect();
    assert!(spawns.len() >= 2);
    assert!(
        spawns
            .iter()
            .all(|(model, effort)| model.as_str() == OVERSEER_MODEL
                && effort.as_str() == OVERSEER_REASONING_EFFORT)
    );
}
