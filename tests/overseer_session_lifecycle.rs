//! The overseer's session lifecycle: it starts, it performs one pass, it exits, and a session that
//! cannot start says so.
//!
//! The criterion these complement asserts the spawn matches the declared constant, which it does by
//! construction and can never detect a model the spawned program rejects. These check the thing
//! that criterion cannot: that a spawned session actually starts, and that a session which does not
//! is visible rather than silent.

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use pce_core::{
    HoldStore, OverseerEvent, OverseerJournal, OverseerLiveness, derive_queue_view,
    render_queue_html,
};
use tempfile::TempDir;

struct Server {
    _directory: TempDir,
    root: PathBuf,
    capture: PathBuf,
    child: Child,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    /// Starts a server whose session program is a shell script with the supplied body. `$CAPTURE`
    /// receives one line per invocation and `$PCE` is this build of the binary.
    fn start(script_body: &str, extra: &[&str]) -> Self {
        Self::start_impl(script_body, None, extra)
    }

    /// Starts the server with a chosen working directory, which is also the session's.
    fn start_in(script_body: &str, cwd: &Path, extra: &[&str]) -> Self {
        Self::start_impl(script_body, Some(cwd), extra)
    }

    fn start_impl(script_body: &str, cwd: Option<&Path>, extra: &[&str]) -> Self {
        let directory = tempfile::tempdir().expect("server directory");
        let root = directory.path().join("holds");
        let capture = directory.path().join("spawn-arguments");
        let script = directory.path().join("session");
        fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CAPTURE\"\n{script_body}\n"),
        )
        .expect("session script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("executable");

        let mut arguments = vec![
            "overseer".to_owned(),
            "serve".to_owned(),
            "--root".to_owned(),
            root.to_str().expect("root UTF-8").to_owned(),
            "--listen".to_owned(),
            "127.0.0.1:0".to_owned(),
            "--session-program".to_owned(),
            script.to_str().expect("script UTF-8").to_owned(),
            "--heartbeat-ms".to_owned(),
            "25".to_owned(),
            "--heartbeat-stale-ms".to_owned(),
            "2000".to_owned(),
            // Long enough that no periodic wake fires inside a test.
            "--wake-ms".to_owned(),
            "600000".to_owned(),
        ];
        arguments.extend(extra.iter().map(|value| (*value).to_owned()));

        let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(&arguments)
            .env("CAPTURE", &capture)
            .env("PCE", env!("CARGO_BIN_EXE_pce"))
            .current_dir(cwd.unwrap_or_else(|| directory.path()))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start server");
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("stdout"))
            .read_line(&mut address)
            .expect("read URL");
        assert!(address.starts_with("http://127.0.0.1:"), "{address:?}");
        Self {
            _directory: directory,
            root,
            capture,
            child,
        }
    }

    fn spawns(&self) -> usize {
        fs::read_to_string(&self.capture)
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn events(&self) -> Vec<OverseerEvent> {
        OverseerJournal::new(&self.root).read().unwrap_or_default()
    }

    fn wait_for<T>(&self, what: &str, mut ready: impl FnMut(&Self) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(value) = ready(self) {
                return value;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for {what}");
    }

    fn liveness(&self) -> OverseerLiveness {
        let store = HoldStore::new(&self.root);
        let journal = OverseerJournal::new(&self.root);
        derive_queue_view(&store, &journal, chrono::Utc::now(), Duration::from_secs(2))
            .expect("view")
            .overseer
    }
}

/// The falsifier the self-referential model criterion cannot be: a session that actually runs to a
/// completed pass leaves the overseer idle, spawned exactly once, with no respawn storm.
#[test]
fn a_session_that_starts_completes_one_pass_and_the_server_goes_idle() {
    let server = Server::start(
        "\"$PCE\" overseer pass-complete --root \"$PCE_HOLD_STORE_ROOT\"\nexit 0",
        &[],
    );
    server.wait_for("a completed pass", |server| {
        server
            .events()
            .iter()
            .any(|event| matches!(event, OverseerEvent::PassCompleted { .. }))
            .then_some(())
    });
    server.wait_for("the session to exit", |server| {
        server
            .events()
            .iter()
            .any(|event| matches!(event, OverseerEvent::SessionExited { .. }))
            .then_some(())
    });

    assert_eq!(
        server.liveness(),
        OverseerLiveness::Idle,
        "a completed pass must leave the overseer idle, not working and not dead"
    );

    // The old loop respawned on every exit, forever. One pass must stay one pass.
    let after_completion = server.spawns();
    thread::sleep(Duration::from_millis(600));
    assert_eq!(
        server.spawns(),
        after_completion,
        "a completed pass must not be followed by further wakes"
    );
    assert_eq!(
        after_completion, 1,
        "startup should wake exactly one session"
    );
}

/// A session that cannot start is replaced, but a bounded number of times, and the operator can see
/// that it could not start rather than reading a heartbeat that proves only the server is alive.
#[test]
fn a_session_that_never_completes_is_bounded_and_reported_as_cannot_start() {
    let server = Server::start("exit 9", &[]);
    server.wait_for("cannot-start", |server| {
        matches!(server.liveness(), OverseerLiveness::CannotStart).then_some(())
    });

    // Let any remaining backoff elapse, then require the retries to have stopped.
    thread::sleep(Duration::from_secs(1));
    let settled = server.spawns();
    thread::sleep(Duration::from_millis(700));
    assert_eq!(
        server.spawns(),
        settled,
        "an unstartable session must stop being respawned, not spin"
    );
    assert!(
        settled <= 3,
        "expected at most 3 bounded attempts, observed {settled}"
    );

    let html = render_queue_html(
        &derive_queue_view(
            &HoldStore::new(&server.root),
            &OverseerJournal::new(&server.root),
            chrono::Utc::now(),
            Duration::from_secs(2),
        )
        .expect("view"),
    );
    assert!(
        html.contains("overseer cannot start"),
        "the page must distinguish cannot-start from idle and from not-running"
    );
}

/// The bound must hold on an active fleet, not only on a quiet store.
///
/// A store change is new work, never evidence that the session can start. When it cleared the
/// failure budget, a permanently broken session reset its own counter on every scan and respawned
/// forever: measured at 46 spawns in six seconds of churn, which is the hot loop this design
/// replaced, re-entered through a different door.
#[test]
fn a_broken_session_stays_bounded_while_the_store_churns() {
    let server = Server::start("exit 9", &[]);
    let store = HoldStore::new(&server.root);

    // An active fleet: holds keep arriving throughout.
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut index = 0_u32;
    while Instant::now() < deadline {
        let _ = store.open(
            pce_core::HoldIdentity::parse("pce", "v2", format!("P{index}"), "churn")
                .expect("identity"),
            serde_json::json!({ "question": index }),
            pce_core::EventTimestamp::new(chrono::Utc::now()),
        );
        index += 1;
        thread::sleep(Duration::from_millis(200));
    }
    assert!(index > 5, "the fixture must actually churn the store");

    let spawns = server.spawns();
    assert!(
        spawns <= 3,
        "a broken session must stay bounded while the store changes; observed {spawns} spawns"
    );
    assert_eq!(
        server.liveness(),
        OverseerLiveness::CannotStart,
        "the operator must still see that the session cannot start"
    );
}

/// With no skill configured, the spawn still loads the skill this repository ships. The installer
/// omission that made `/overseer` uninvokable must not be able to break the overseer's own startup.
#[test]
fn the_spawn_defaults_to_the_shipped_skill_without_being_told() {
    let repository = tempfile::tempdir().expect("repository");
    let skill = repository.path().join("skills").join("overseer");
    fs::create_dir_all(&skill).expect("skill directory");
    fs::write(skill.join("SKILL.md"), "# overseer\n").expect("skill manifest");

    // No --session-skill, and the server's working directory is the repository.
    let server = Server::start_in("exit 0", repository.path(), &[]);
    let invocation = server.wait_for("an invocation", |server| {
        fs::read_to_string(&server.capture)
            .ok()
            .and_then(|text| text.lines().next().map(str::to_owned))
    });
    // macOS resolves /var through a symlink, so compare the canonical path the child received.
    let expected = skill.canonicalize().expect("canonical skill path");
    assert!(
        invocation.contains(&format!("--skill {}", expected.display())),
        "the spawn must find the shipped skill by path, got {invocation}"
    );
}

/// A session that dies on an unknown model or flag must leave its reason in the journal. Both
/// streams were `Stdio::null()`, which is why a wrong model name shipped behind a passing criterion.
#[test]
fn a_failing_session_records_its_own_output() {
    let server = Server::start("echo 'unknown model: nope' >&2\nexit 2", &[]);
    let detail = server.wait_for("a recorded failure detail", |server| {
        server.events().iter().find_map(|event| match event {
            OverseerEvent::SessionExited {
                code: Some(2),
                detail: Some(detail),
                ..
            } => Some(detail.clone()),
            _ => None,
        })
    });
    assert!(
        detail.contains("unknown model: nope"),
        "the session's own reason must survive into the journal, got {detail:?}"
    );
}

/// Every wake is recorded before the spawn, so a wake with no completion is a visible fact rather
/// than an absence.
#[test]
fn a_wake_is_recorded_before_the_session_is_spawned() {
    let server = Server::start("exit 0", &[]);
    server.wait_for("a wake and a spawn", |server| {
        let events = server.events();
        let wake = events
            .iter()
            .position(|event| matches!(event, OverseerEvent::WakeStarted { .. }))?;
        let spawn = events
            .iter()
            .position(|event| matches!(event, OverseerEvent::SessionSpawned { .. }))?;
        (wake < spawn).then_some(())
    });
}

/// The spawn carries a provider, so a bare model id cannot silently match nothing, and it points at
/// the skill directly rather than depending on the installer having linked it.
#[test]
fn the_spawn_names_a_provider_and_loads_the_skill_without_the_installer() {
    let skill = tempfile::tempdir().expect("skill directory");
    fs::write(skill.path().join("SKILL.md"), "# overseer\n").expect("skill manifest");
    let server = Server::start(
        "exit 0",
        &[
            "--session-skill",
            skill.path().to_str().expect("skill UTF-8"),
        ],
    );
    let invocation = server.wait_for("an invocation", |server| {
        fs::read_to_string(&server.capture)
            .ok()
            .and_then(|text| text.lines().next().map(str::to_owned))
    });
    assert!(
        invocation.contains(&format!("--provider {}", pce_core::OVERSEER_PROVIDER)),
        "{invocation}"
    );
    assert!(
        invocation.contains(&format!(
            "--skill {}",
            skill.path().to_str().expect("skill UTF-8")
        )),
        "{invocation}"
    );
    assert!(invocation.contains("--print"), "{invocation}");
}

/// The declared provider and model must be one the session program actually offers.
///
/// This is the check the criterion cannot be. Asserting that the spawn contains `--model
/// {OVERSEER_MODEL}` compares the constant with itself and stays green for any string; a bare
/// `claude-fable-5` matched nothing and shipped anyway. This asks the program.
#[test]
fn the_declared_provider_and_model_are_offered_by_the_session_program() {
    let Ok(output) = Command::new("prime-agent").args(["model", "list"]).output() else {
        eprintln!("skipped: prime-agent is not installed on this machine");
        return;
    };
    assert!(
        output.status.success(),
        "prime-agent model list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // `prime-agent model list` writes its table to stderr, so reading stdout alone finds nothing
    // and would make this check vacuously green.
    let listing = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let offered = listing.lines().any(|line| {
        let mut columns = line.split_whitespace();
        columns.next() == Some(pce_core::OVERSEER_PROVIDER)
            && columns.next() == Some(pce_core::OVERSEER_MODEL)
    });
    assert!(
        offered,
        "`{} {}` is not offered by prime-agent; the spawned session would reject it",
        pce_core::OVERSEER_PROVIDER,
        pce_core::OVERSEER_MODEL
    );
}

/// The installer must link every skill the repository ships. A hardcoded list shipped `overseer`
/// and `to-graph` invisible while reporting success.
#[test]
fn the_installer_links_every_skill_the_repository_ships() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected: Vec<String> = fs::read_dir(repository.join("skills"))
        .expect("skills directory")
        .flatten()
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        expected.iter().any(|name| name == "overseer"),
        "fixture expects the overseer skill to exist"
    );

    let home = tempfile::tempdir().expect("home");
    let output = Command::new("bash")
        .arg(repository.join("install.sh"))
        .env("HOME", home.path())
        .current_dir(repository)
        .output()
        .expect("run install.sh");
    assert!(
        output.status.success(),
        "install failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for name in expected {
        let link = home.path().join(".claude/skills").join(&name);
        assert!(
            link.is_symlink() && link.exists(),
            "install.sh did not link the `{name}` skill"
        );
    }
}
