#[allow(dead_code)]
mod support;

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, FileTimes};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use support::{CliHarness, Invocation, ScriptedResponse};

const DIGEST: &str = r#"{"deltas":{"elisions":[],"entries":[]},"facts":{"elisions":[],"entries":[{"evidence":"rehydration fixture repository contract","kind":"repository-contract","node":"m5-s1","sequence":1}]},"open_holds":{"elisions":[],"entries":[]},"rounds":{"elisions":[],"entries":[]}}"#;
const ENVELOPE: &str = r#"{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"{\"deltas\":{\"elisions\":[],\"entries\":[]},\"facts\":{\"elisions\":[],\"entries\":[{\"evidence\":\"rehydration fixture repository contract\",\"kind\":\"repository-contract\",\"node\":\"m5-s1\",\"sequence\":1}]},\"open_holds\":{\"elisions\":[],\"entries\":[]},\"rounds\":{\"elisions\":[],\"entries\":[]}}"}}"#;

#[test]
fn resume_injects_digest_from_unique_most_recent_log() {
    let harness = CliHarness::new().expect("create CLI harness");
    let fixture = real_status_fixture(&harness, "resume-rehydrate", "pce/resume-rehydrate/m5-s1");
    let older_dir = fixture.root.join("planning/2026-07-28-zz-resume-older");
    let older_log = older_dir.join("events.jsonl");
    fs::create_dir_all(&older_dir).expect("create older candidate directory");
    fs::write(
        &older_log,
        b"{\"sequence\":1,\"timestamp\":\"2026-07-28T09:59:59.999Z\",\"kind\":\"delta\",\"node\":\"m4-s1\",\"payload\":{\"message\":\"older candidate\"}}\n\n",
    )
    .expect("write older candidate");
    File::options()
        .write(true)
        .open(&fixture.log)
        .expect("write-open selected log")
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
        .expect("set selected log time");
    File::options()
        .write(true)
        .open(&older_log)
        .expect("write-open older log")
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(2)))
        .expect("set older log time");

    let payload = payload(&fixture.root, "SessionStart", "resume");
    let output = run_hook(&harness, &fixture.bin_dir, &payload, &[]);
    assert_injection(&output);
    assert_eq!(
        harness.invocations().expect("parse hook invocations"),
        expected_invocations(&fixture.root, "pce/resume-rehydrate/m5-s1")
    );
}

#[test]
fn compact_uses_project_dir_fallback_and_injects_digest() {
    let harness = CliHarness::new().expect("create CLI harness");
    let fixture = real_status_fixture(&harness, "compact-rehydrate", "pce/compact-rehydrate/m5-s1");
    fs::write(
        &fixture.log,
        contract_record(&fixture.root, "2026-07-28T11:00:00.000Z"),
    )
    .expect("write compact candidate");

    let output = run_hook(
        &harness,
        &fixture.bin_dir,
        br#"{"hook_event_name":"SessionStart","source":"compact"}"#,
        &[("CLAUDE_PROJECT_DIR", fixture.root.as_os_str())],
    );
    assert_injection(&output);
    assert_eq!(
        harness.invocations().expect("parse hook invocations"),
        expected_invocations(&fixture.root, "pce/compact-rehydrate/m5-s1")
    );
}

#[test]
fn nonmatching_event_and_sources_are_silent_noops() {
    let cases = [
        ("SessionStart", "startup"),
        ("SessionStart", "clear"),
        ("SessionStart", "fork"),
        ("SessionStart", "unknown"),
        ("UserPromptSubmit", "resume"),
    ];
    for (event, source) in cases {
        let harness = CliHarness::new().expect("create CLI harness");
        let fixture =
            real_status_fixture(&harness, "resume-rehydrate", "pce/resume-rehydrate/m5-s1");
        let output = run_hook(
            &harness,
            &fixture.bin_dir,
            &payload(&fixture.root, event, source),
            &[],
        );
        assert_silent_success(&output);
        assert_eq!(
            harness.invocations().expect("parse hook invocations"),
            Vec::<Invocation>::new()
        );
    }
}

#[test]
fn absent_candidate_tie_and_root_discovery_failure_are_silent_noops() {
    let harness = CliHarness::new().expect("create CLI harness");
    let bin_dir = executable_dir(&harness, false);
    let empty_root = harness.path().join("empty-root");
    fs::create_dir(&empty_root).expect("create empty root");
    let output = run_hook(
        &harness,
        &bin_dir,
        &payload(&empty_root, "SessionStart", "resume"),
        &[],
    );
    assert_silent_success(&output);

    let tie_root = harness.path().join("tie-root");
    let a = tie_root.join("planning/2026-07-28-tie-a/events.jsonl");
    let b = tie_root.join("planning/2026-07-28-tie-b/events.jsonl");
    fs::create_dir_all(a.parent().expect("tie a parent")).expect("create tie a directory");
    fs::create_dir_all(b.parent().expect("tie b parent")).expect("create tie b directory");
    fs::write(&a, contract_record(&tie_root, "2026-07-28T12:00:00.000Z")).expect("write tie a");
    fs::write(&b, contract_record(&tie_root, "2026-07-28T12:00:00.000Z")).expect("write tie b");
    let bin_dir = executable_dir(&harness, true);
    harness
        .materialize_responses(&status_responses(&tie_root, "pce/tie/m5-s1"))
        .expect("materialize tie status responses");
    let output = run_hook(
        &harness,
        &bin_dir,
        &payload(&tie_root, "SessionStart", "resume"),
        &[],
    );
    assert_silent_success(&output);

    let output = run_hook(
        &harness,
        &bin_dir,
        br#"{"hook_event_name":"SessionStart","source":"resume"}"#,
        &[],
    );
    assert_silent_success(&output);
    assert_eq!(
        harness.invocations().expect("parse hook invocations"),
        Vec::<Invocation>::new()
    );
}

#[test]
fn one_malformed_candidate_abandons_entire_scan() {
    let harness = CliHarness::new().expect("create CLI harness");
    let fixture = selected_fixture(&harness, "valid", "2026-07-28T13:00:00.000Z");
    let malformed = fixture.root.join("planning/malformed/events.jsonl");
    fs::create_dir_all(malformed.parent().expect("malformed parent"))
        .expect("create malformed directory");
    fs::write(malformed, b"{\"sequence\":1").expect("write malformed candidate");
    let bin_dir = executable_dir(&harness, true);
    harness
        .materialize_responses(&status_responses(&fixture.root, "pce/valid/m5-s1"))
        .expect("materialize valid status responses");

    let output = run_hook(
        &harness,
        &bin_dir,
        &payload(&fixture.root, "SessionStart", "resume"),
        &[],
    );
    assert_silent_success(&output);
    assert_eq!(
        harness.invocations().expect("parse hook invocations"),
        Vec::<Invocation>::new()
    );
}

#[test]
fn invalid_stdin_and_missing_python_are_silent_noops() {
    let harness = CliHarness::new().expect("create CLI harness");
    let fixture = real_status_fixture(&harness, "resume-rehydrate", "pce/resume-rehydrate/m5-s1");
    let valid = payload(&fixture.root, "SessionStart", "resume");

    assert_silent_success(&run_hook(&harness, &fixture.bin_dir, b"{", &[]));
    assert_silent_success(&run_hook(
        &harness,
        &fixture.bin_dir,
        &valid,
        &[(
            "PCE_TEST_PYTHON",
            OsStr::new("/definitely/missing/pce-test-python"),
        )],
    ));
    let not_executable = harness.path().join("python-not-executable");
    fs::write(&not_executable, b"not executable\n").expect("write non-executable Python");
    let mut permissions = fs::metadata(&not_executable)
        .expect("read non-executable Python metadata")
        .permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&not_executable, permissions).expect("set non-executable Python mode");
    assert_silent_success(&run_hook(
        &harness,
        &fixture.bin_dir,
        &valid,
        &[("PCE_TEST_PYTHON", not_executable.as_os_str())],
    ));
    assert_eq!(
        harness.invocations().expect("parse hook invocations"),
        Vec::<Invocation>::new()
    );
}

#[test]
fn missing_and_failing_pce_are_silent_noops() {
    let harness = CliHarness::new().expect("create CLI harness");
    let fixture = selected_fixture(&harness, "selected", "2026-07-28T14:00:00.000Z");
    let payload = payload(&fixture.root, "SessionStart", "resume");
    assert_silent_success(&run_hook_without_bin(&harness, &payload));

    let argv_file = harness.path().join("failing-pce.argv");
    let failing_bin = harness.path().join("failing-bin");
    fs::create_dir(&failing_bin).expect("create failing pce directory");
    write_executable(
        &failing_bin.join("pce"),
        "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$PCE_TEST_ARGV_FILE\"\nexit 42\n",
    );
    assert_silent_success(&run_hook(
        &harness,
        &failing_bin,
        &payload,
        &[("PCE_TEST_ARGV_FILE", argv_file.as_os_str())],
    ));
    assert_status_argv(&argv_file, &fixture.log, &fixture.vision_dir);
}

#[test]
fn malformed_status_and_missing_digest_are_silent_noops_and_preserve_exact_argv() {
    let cases = [
        (
            "malformed",
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$PCE_TEST_ARGV_FILE\"\nprintf '%s' '{'\nexit 0\n",
        ),
        (
            "missing-digest",
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$PCE_TEST_ARGV_FILE\"\nprintf '%s' '{\"schema_id\":\"pce.run-snapshot\",\"schema_version\":1}'\nexit 0\n",
        ),
    ];
    for (name, script) in cases {
        let harness = CliHarness::new().expect("create CLI harness");
        let fixture = selected_fixture(&harness, "selected", "2026-07-28T15:00:00.000Z");
        let bin_dir = executable_dir(&harness, false);
        write_executable(&bin_dir.join("pce"), script);
        let argv_file = harness.path().join(format!("{name}.argv"));
        let output = run_hook(
            &harness,
            &bin_dir,
            &payload(&fixture.root, "SessionStart", "resume"),
            &[("PCE_TEST_ARGV_FILE", argv_file.as_os_str())],
        );
        assert_silent_success(&output);
        assert_status_argv(&argv_file, &fixture.log, &fixture.vision_dir);
    }
}

#[test]
fn merge_hook_settings_is_isolated_structural_and_idempotent() {
    const MANAGED_COMMAND: &str = "$HOME/.local/bin/pce-rehydrate";

    let harness = CliHarness::new().expect("create CLI harness");
    let fake_home = harness.path().join("fake-home");
    let fake_settings = fake_home.join(".claude/settings.json");
    fs::create_dir_all(fake_settings.parent().expect("fake settings parent"))
        .expect("create fake settings parent");
    let sentinel = b"{\"sentinel\":\"must remain unchanged\"}\n";
    fs::write(&fake_settings, sentinel).expect("write fake-home sentinel");

    let settings_path = harness.path().join("explicit-target/settings.json");
    fs::create_dir_all(settings_path.parent().expect("explicit settings parent"))
        .expect("create explicit settings parent");
    let unrelated_session_group = json!({
        "matcher": "startup",
        "hooks": [{"type": "command", "command": "unrelated-session-command"}],
        "preserved": true
    });
    let unrelated_event = json!([{
        "matcher": "anything",
        "hooks": [{"type": "command", "command": "unrelated-event-command"}]
    }]);
    let seeded = json!({
        "unrelated": {"nested": ["value", 7]},
        "hooks": {
            "CustomEvent": unrelated_event,
            "SessionStart": [
                unrelated_session_group,
                {
                    "matcher": "resume",
                    "hooks": [
                        {"type": "command", "command": MANAGED_COMMAND},
                        {"type": "command", "command": MANAGED_COMMAND},
                        {"type": "command", "command": "preserve-neighbor"}
                    ]
                }
            ],
            "PostCompact": [{
                "matcher": "compact",
                "hooks": [
                    {"type": "command", "command": MANAGED_COMMAND},
                    {"type": "command", "command": "preserve-post-compact"}
                ]
            }]
        }
    });
    fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&seeded).expect("serialize seeded settings"),
    )
    .expect("write seeded settings");

    let first = run_settings_merge(&fake_home, &settings_path, None);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let after_first = fs::read(&settings_path).expect("read first merge result");
    let first_value: Value =
        serde_json::from_slice(&after_first).expect("parse first merge result");
    assert_eq!(first_value["unrelated"], json!({"nested": ["value", 7]}));
    assert_eq!(first_value["hooks"]["CustomEvent"], unrelated_event);
    assert_eq!(
        first_value["hooks"]["SessionStart"][0],
        unrelated_session_group
    );
    assert_eq!(
        first_value["hooks"]["SessionStart"][1],
        json!({
            "matcher": "resume",
            "hooks": [{"type": "command", "command": "preserve-neighbor"}]
        })
    );
    assert_eq!(
        first_value["hooks"]["PostCompact"],
        json!([{
            "matcher": "compact",
            "hooks": [{"type": "command", "command": "preserve-post-compact"}]
        }])
    );
    assert_managed_settings_shape(&first_value, MANAGED_COMMAND);
    assert_fake_home_untouched(&fake_home, sentinel);

    let second = run_settings_merge(&fake_home, &settings_path, None);
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let after_second = fs::read(&settings_path).expect("read second merge result");
    assert_eq!(after_second, after_first);
    let second_value: Value =
        serde_json::from_slice(&after_second).expect("parse second merge result");
    assert_managed_settings_shape(&second_value, MANAGED_COMMAND);
    assert_fake_home_untouched(&fake_home, sentinel);

    let before_missing_python = after_second;
    let missing = run_settings_merge(
        &fake_home,
        &settings_path,
        Some(OsStr::new("/definitely/missing/pce-test-python")),
    );
    assert!(!missing.status.success());
    assert_eq!(missing.stdout, b"");
    assert_eq!(
        missing.stderr,
        b"ERROR: Python 3 interpreter is missing or not executable: /definitely/missing/pce-test-python\n"
    );
    assert_eq!(
        fs::read(&settings_path).expect("read target after missing Python"),
        before_missing_python
    );
    assert_fake_home_untouched(&fake_home, sentinel);
}

struct Fixture {
    root: PathBuf,
    vision_dir: PathBuf,
    log: PathBuf,
}

struct RealStatusFixture {
    root: PathBuf,
    log: PathBuf,
    bin_dir: PathBuf,
}

fn run_settings_merge(fake_home: &Path, settings_path: &Path, python: Option<&OsStr>) -> Output {
    let mut command = Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh"));
    command
        .arg("--merge-hook-settings")
        .arg(settings_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", fake_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(interpreter) = python {
        command.env("PCE_TEST_PYTHON", interpreter);
    }
    command.output().expect("run isolated settings merge")
}

fn assert_managed_settings_shape(settings: &Value, managed_command: &str) {
    let session_start = settings["hooks"]["SessionStart"]
        .as_array()
        .expect("SessionStart array");
    let managed_count = session_start
        .iter()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|hook| hook.get("command").and_then(Value::as_str) == Some(managed_command))
        .count();
    assert_eq!(managed_count, 1);

    let canonical_group = json!({
        "matcher": "resume|compact",
        "hooks": [{"type": "command", "command": managed_command}]
    });
    assert_eq!(
        session_start
            .iter()
            .filter(|group| **group == canonical_group)
            .count(),
        1
    );

    let post_compact_count = settings["hooks"]["PostCompact"]
        .as_array()
        .expect("PostCompact array")
        .iter()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|hook| hook.get("command").and_then(Value::as_str) == Some(managed_command))
        .count();
    assert_eq!(post_compact_count, 0);
}

fn assert_fake_home_untouched(fake_home: &Path, sentinel: &[u8]) {
    let entries = fs::read_dir(fake_home)
        .expect("read fake home")
        .map(|entry| entry.expect("read fake-home entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![OsString::from(".claude")]);
    let claude_dir = fake_home.join(".claude");
    let claude_entries = fs::read_dir(&claude_dir)
        .expect("read fake .claude directory")
        .map(|entry| entry.expect("read fake .claude entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(claude_entries, vec![OsString::from("settings.json")]);
    assert_eq!(
        fs::read(claude_dir.join("settings.json")).expect("read fake-home sentinel"),
        sentinel
    );
}

fn selected_fixture(harness: &CliHarness, name: &str, timestamp: &str) -> Fixture {
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning").join(format!("2026-07-28-{name}"));
    let log = vision_dir.join("events.jsonl");
    fs::create_dir_all(&vision_dir).expect("create selected vision directory");
    fs::write(&log, contract_record(&root, timestamp)).expect("write selected candidate");
    Fixture {
        root,
        vision_dir,
        log,
    }
}

fn real_status_fixture(harness: &CliHarness, name: &str, head: &str) -> RealStatusFixture {
    let fixture = selected_fixture(harness, name, "2026-07-28T10:00:00.000Z");
    let bin_dir = executable_dir(harness, true);
    harness
        .materialize_responses(&status_responses(&fixture.root, head))
        .expect("materialize hook status responses");
    RealStatusFixture {
        root: fixture.root,
        log: fixture.log,
        bin_dir,
    }
}

fn executable_dir(harness: &CliHarness, link_real_pce: bool) -> PathBuf {
    let bin_dir = harness.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create executable directory");
    if link_real_pce {
        symlink(env!("CARGO_BIN_EXE_pce"), bin_dir.join("pce")).expect("symlink pce executable");
    }
    bin_dir
}

fn run_hook(
    harness: &CliHarness,
    bin_dir: &Path,
    stdin: &[u8],
    environment: &[(&str, &OsStr)],
) -> Output {
    run_hook_with_path(harness, Some(bin_dir), stdin, environment)
}

fn run_hook_without_bin(harness: &CliHarness, stdin: &[u8]) -> Output {
    run_hook_with_path(harness, None, stdin, &[])
}

fn run_hook_with_path(
    harness: &CliHarness,
    bin_dir: Option<&Path>,
    stdin: &[u8],
    environment: &[(&str, &OsStr)],
) -> Output {
    let home = harness.path().join("home");
    fs::create_dir_all(&home).expect("create fake home");
    let shim_dir = harness.path().join("shims");
    let path = bin_dir.map_or_else(
        || format!("{}:/usr/bin:/bin", shim_dir.display()),
        |directory| {
            format!(
                "{}:{}:/usr/bin:/bin",
                directory.display(),
                shim_dir.display()
            )
        },
    );
    let cwd = harness.path().join("cwd");
    fs::create_dir_all(&cwd).expect("create hook working directory");
    let mut command =
        Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/pce-rehydrate.sh"));
    command
        .current_dir(cwd)
        .env_clear()
        .env("PATH", path)
        .env("PCE_SHIM_ROOT", harness.path())
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in environment {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn rehydration hook");
    child
        .stdin
        .take()
        .expect("hook stdin is piped")
        .write_all(stdin)
        .expect("write hook stdin");
    child.wait_with_output().expect("wait for rehydration hook")
}

fn contract_record(root: &Path, timestamp: &str) -> Vec<u8> {
    let root = serde_json::to_string(root.to_str().expect("UTF-8 root"))
        .expect("JSON-escape repository root");
    format!(
        "{{\"sequence\":1,\"timestamp\":\"{timestamp}\",\"kind\":\"repository-contract\",\"node\":\"m5-s1\",\"payload\":{{\"repository\":\"pce\",\"repo_root\":{root},\"stated\":{{\"format\":\"cargo fmt --check\",\"lint\":\"cargo clippy --workspace --all-targets\",\"typecheck\":\"cargo check --workspace --all-targets\",\"test\":\"cargo test --workspace\",\"build\":\"cargo build --release\",\"version_policy\":\"NONE\",\"branch_convention\":\"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>\",\"pull_request_convention\":\"step head targets the matching milestone integration branch\"}},\"observations\":{{\"format\":0,\"lint\":0,\"typecheck\":0,\"test\":0,\"build\":0}},\"workflow_map\":{{}},\"appendable\":{{\"environment_hazards\":[],\"gate_orderings\":[],\"lockfile_rules\":[]}},\"evidence\":\"rehydration fixture repository contract\"}}}}\n"
    )
    .into_bytes()
}

fn payload(root: &Path, event: &str, source: &str) -> Vec<u8> {
    let root = serde_json::to_string(root.to_str().expect("UTF-8 root"))
        .expect("JSON-escape payload root");
    format!("{{\"hook_event_name\":\"{event}\",\"source\":\"{source}\",\"cwd\":{root}}}")
        .into_bytes()
}

fn status_responses(root: &Path, head: &str) -> Vec<ScriptedResponse> {
    let root_text = root.to_str().expect("UTF-8 root");
    let vision = head
        .strip_suffix("/m5-s1")
        .expect("step head fixture should end in its node");
    let integration = format!("{vision}/milestone-5");
    vec![
        response(
            "git",
            argv(["-C", root_text, "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/rehydration.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                &format!("refs/heads/{integration}"),
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-rehydration-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{integration}"),
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv(["-C", root_text, "worktree", "list", "--porcelain"]),
            0,
            format!(
                "worktree {}/worktrees/m5-s1\nHEAD fixture-worktree-oid\nbranch refs/heads/{head}\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            1,
            b"",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                head,
                "--base",
                &integration,
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            format!(
                "[{{\"number\":501,\"headRefName\":\"{head}\",\"baseRefName\":\"{integration}\",\"state\":\"OPEN\",\"mergeCommit\":null}}]\n"
            )
            .as_bytes(),
        ),
    ]
}

fn expected_invocations(root: &Path, head: &str) -> Vec<Invocation> {
    let root_arg = root.as_os_str().to_owned();
    let vision = head
        .strip_suffix("/m5-s1")
        .expect("step head fixture should end in its node");
    let integration = format!("{vision}/milestone-5");
    vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                format!("refs/heads/{integration}").into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                format!("refs/heads/{integration}").into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg,
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                head,
                "--base",
                &integration,
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
    ]
}

fn response(program: &str, argv: Vec<OsString>, exit_code: i32, stdout: &[u8]) -> ScriptedResponse {
    ScriptedResponse {
        program: program.into(),
        argv,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

fn invocation(program: &str, argv: Vec<OsString>) -> Invocation {
    Invocation {
        program: program.into(),
        argv,
    }
}

fn argv<const N: usize>(arguments: [&str; N]) -> Vec<OsString> {
    arguments.into_iter().map(OsString::from).collect()
}

fn assert_injection(output: &Output) {
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, ENVELOPE.as_bytes());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("parse hook envelope");
    assert_eq!(
        envelope,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": DIGEST,
            }
        })
    );
    assert_eq!(
        envelope
            .as_object()
            .expect("envelope object")
            .keys()
            .collect::<Vec<_>>(),
        vec!["hookSpecificOutput"]
    );
    let hook_output = envelope["hookSpecificOutput"]
        .as_object()
        .expect("hookSpecificOutput object");
    assert_eq!(hook_output.len(), 2);
    assert!(hook_output.contains_key("hookEventName"));
    assert!(hook_output.contains_key("additionalContext"));
    let context = hook_output["additionalContext"]
        .as_str()
        .expect("additionalContext string");
    assert_eq!(
        serde_json::from_str::<Value>(context).expect("parse recovery digest"),
        serde_json::from_str::<Value>(DIGEST).expect("parse expected recovery digest")
    );
}

fn assert_silent_success(output: &Output) {
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
}

fn write_executable(path: &Path, source: &str) {
    fs::write(path, source).expect("write fake pce");
    let mut permissions = fs::metadata(path)
        .expect("read fake pce metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set fake pce executable mode");
}

fn assert_status_argv(path: &Path, log: &Path, vision_dir: &Path) {
    let bytes = fs::read(path).expect("read fake pce argv");
    let actual = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| OsString::from(String::from_utf8_lossy(field).into_owned()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            "status".into(),
            "--file".into(),
            log.as_os_str().to_owned(),
            "--vision-dir".into(),
            vision_dir.as_os_str().to_owned(),
        ]
    );
}
