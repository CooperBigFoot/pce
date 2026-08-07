#[allow(dead_code)]
mod support;

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use support::CliHarness;

const VISION_DIR_NAME: &str = "2026-08-03-criterion-protection-fixture";
const REFUSE_CONSTRUCT: &[u8] = b"REFUSED: criterion protection could not construct the complete proposed vision for verification.\n";
const REFUSE_VERIFIER: &[u8] = b"REFUSED: criterion protection could not obtain an accepting decision from pce criteria check. Ratified criteria may not be removed, reordered, or changed; record additive criteria first with pce log --kind criterion-added.\n";
const REFUSE_BASH: &[u8] = b"REFUSED: Bash may not access vision.md during an active run; use Read for inspection, Edit or Write for a proposed change, and pce log --kind criterion-added for additive criteria.\n";
const RATIFIED: &str = "# Vision: criterion protection fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"}]}\n```\n";
const RATIFIED_ADDED: &str = "# Vision: criterion protection fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"},{\"name\":\"Added criterion\",\"input\":\"Run the added probe.\",\"observation\":\"The added probe exits 0.\"}]}\n```\n";
const TWO_RATIFIED: &str = "# Vision: criterion protection fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"},{\"name\":\"Second ratified criterion\",\"input\":\"Run the second probe.\",\"observation\":\"The second probe exits 0.\"}]}\n```\n";
const DUPLICATE_TEXT: &str = "# Vision: criterion protection fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"},{\"name\":\"Second ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"}]}\n```\n";
const INSTALL_ONLY: &str = "# Vision: criterion protection fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Install-only criterion\",\"input\":\"Install the hook, then attempt the forbidden command.\",\"observation\":\"The command is denied.\"}]}\n```\n";
const CAPTURING_PCE: &str = "#!/bin/sh\nset -eu\n: \"${PCE_CAPTURE_ARGV:?}\"\n: \"${PCE_CAPTURE_STDIN:?}\"\n: \"${PCE_CAPTURE_CWD:?}\"\nprintf '%s\\0' \"$@\" > \"$PCE_CAPTURE_ARGV\"\ncat > \"$PCE_CAPTURE_STDIN\"\npwd -P > \"$PCE_CAPTURE_CWD\"\nexit \"${PCE_FAKE_EXIT:-0}\"\n";

#[test]
fn write_delegates_complete_candidate_with_exact_argv_and_stdin() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let capture = Capture::new(&fixture.root);
    install_script(&fixture.home, CAPTURING_PCE, 0o755);
    let output = run_hook(&fixture, &write_payload(&fixture, RATIFIED), capture.env());
    assert_silent_success(&output);
    capture.assert_call(&fixture, RATIFIED.as_bytes());
}

#[test]
fn edit_reconstructs_candidate_and_refusal_is_fail_closed() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let capture = Capture::new(&fixture.root);
    install_script(&fixture.home, CAPTURING_PCE, 0o755);
    let payload = edit_payload(
        &fixture,
        "The probe exits 0.",
        "The probe may exit 0.",
        Some(json!(false)),
    );
    let output = run_hook(&fixture, &payload, capture.env());
    assert_silent_success(&output);
    capture.assert_call(
        &fixture,
        RATIFIED
            .replace("The probe exits 0.", "The probe may exit 0.")
            .as_bytes(),
    );

    install_real_pce(&fixture.home);
    assert_refusal(run_hook(&fixture, &payload, Vec::new()), REFUSE_VERIFIER);

    fs::write(&fixture.vision, TWO_RATIFIED).expect("write two-criterion vision");
    let removal = edit_payload(
        &fixture,
        "{\"name\":\"Ratified criterion\",\"input\":\"Run the ratified probe.\",\"observation\":\"The probe exits 0.\"},",
        "",
        Some(json!(false)),
    );
    assert_refusal(run_hook(&fixture, &removal, Vec::new()), REFUSE_VERIFIER);
}

#[test]
fn logged_addition_is_the_only_additive_path() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    install_real_pce(&fixture.home);
    let payload = write_payload(&fixture, RATIFIED_ADDED);
    assert_refusal(run_hook(&fixture, &payload, Vec::new()), REFUSE_VERIFIER);

    let addition = br#"{"criterion":{"name":"Added criterion","input":"Run the added probe.","observation":"The added probe exits 0."},"change_of_course":"Reality exposed an uncovered failure."}"#;
    let output = harness
        .run(
            [
                OsStr::new("log"),
                OsStr::new("--file"),
                fixture.log.as_os_str(),
                OsStr::new("--kind"),
                OsStr::new("criterion-added"),
                OsStr::new("--node"),
                OsStr::new("m4-s2"),
            ],
            addition,
        )
        .expect("append criterion-added");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    assert_silent_success(&run_hook(&fixture, &payload, Vec::new()));
    assert_eq!(
        fs::read(&fixture.vision).expect("read unchanged vision"),
        RATIFIED.as_bytes()
    );
}

#[test]
fn verification_route_failure_denies_instead_of_allowing() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let payload = write_payload(&fixture, RATIFIED);
    assert_refusal(run_hook(&fixture, &payload, Vec::new()), REFUSE_VERIFIER);
    for (script, mode) in [
        ("#!/bin/sh\nexit 0\n", 0o644),
        ("#!/bin/sh\nexit 127\n", 0o755),
        ("#!/bin/sh\nexit 1\n", 0o755),
    ] {
        install_script(&fixture.home, script, mode);
        assert_refusal(run_hook(&fixture, &payload, Vec::new()), REFUSE_VERIFIER);
    }
}

#[test]
fn candidate_transport_failures_are_closed() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let capture = Capture::new(&fixture.root);
    install_script(&fixture.home, CAPTURING_PCE, 0o755);
    let valid = write_payload(&fixture, RATIFIED);
    assert_refusal(
        run_hook(
            &fixture,
            &valid,
            vec![(
                "PCE_TEST_PYTHON",
                OsStr::new("/definitely/missing/pce-test-python"),
            )],
        ),
        REFUSE_CONSTRUCT,
    );
    let nonexec = fixture.root.join("non-executable-python");
    write_mode(&nonexec, "#!/bin/sh\nexit 0\n", 0o644);
    assert_refusal(
        run_hook(
            &fixture,
            &valid,
            vec![("PCE_TEST_PYTHON", nonexec.as_os_str())],
        ),
        REFUSE_CONSTRUCT,
    );
    for input in [b"not-json\n".as_slice(), b"[]".as_slice()] {
        assert_refusal(run_hook(&fixture, input, capture.env()), REFUSE_CONSTRUCT);
    }
    fs::write(&fixture.vision, DUPLICATE_TEXT).expect("write duplicate-text vision");
    let cases = [
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Write","tool_input":{}}),
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Edit","tool_input":{"file_path":fixture.vision,"new_string":"replacement","replace_all":false}}),
        edit_payload_value(
            &fixture,
            "Run the ratified probe.",
            "Run the ratified probe twice.",
            None,
        ),
        edit_payload_value(
            &fixture,
            "The probe exits 0.",
            "The probe may exit 0.",
            Some(json!("true")),
        ),
    ];
    for case in cases {
        assert_refusal(
            run_hook(
                &fixture,
                &serde_json::to_vec(&case).expect("serialize payload"),
                capture.env(),
            ),
            REFUSE_CONSTRUCT,
        );
    }
    capture.assert_absent();
}

#[test]
fn replace_all_is_explicit_and_deterministic() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, DUPLICATE_TEXT);
    let capture = Capture::new(&fixture.root);
    install_script(&fixture.home, CAPTURING_PCE, 0o755);
    let accepted = edit_payload(
        &fixture,
        "Run the ratified probe.",
        "Run the ratified probe twice.",
        Some(json!(true)),
    );
    assert_silent_success(&run_hook(&fixture, &accepted, capture.env()));
    let proposal = fs::read_to_string(&capture.stdin).expect("read captured proposal");
    assert_eq!(proposal.matches("Run the ratified probe twice.").count(), 2);
    assert_eq!(proposal.matches("Run the ratified probe.").count(), 0);
    capture.clear();
    let refused = edit_payload(
        &fixture,
        "Run the ratified probe.",
        "Run the ratified probe twice.",
        Some(json!(false)),
    );
    assert_refusal(
        run_hook(&fixture, &refused, capture.env()),
        REFUSE_CONSTRUCT,
    );
    capture.assert_absent();
}

#[test]
fn irrelevant_events_tools_and_paths_are_silent_noops() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let capture = Capture::new(&fixture.root);
    install_script(&fixture.home, CAPTURING_PCE, 0o755);
    let outside = fixture.root.join("vision.md");
    fs::write(&outside, RATIFIED).expect("write outside vision");
    let no_log = fixture
        .root
        .join("planning/2026-08-03-criterion-protection-no-log/vision.md");
    fs::create_dir_all(no_log.parent().expect("no-log parent")).expect("create no-log directory");
    fs::write(&no_log, RATIFIED).expect("write no-log vision");
    let notes = fixture.vision_dir.join("notes.md");
    fs::write(&notes, RATIFIED).expect("write notes");
    let cases = [
        json!({"hook_event_name":"SessionStart","cwd":fixture.root,"tool_name":"Write","tool_input":{"file_path":fixture.vision,"content":RATIFIED}}),
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Read","tool_input":{"file_path":fixture.vision}}),
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Write","tool_input":{"file_path":outside,"content":RATIFIED}}),
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Write","tool_input":{"file_path":no_log,"content":RATIFIED}}),
        json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Write","tool_input":{"file_path":notes,"content":RATIFIED}}),
    ];
    for case in cases {
        assert_silent_success(&run_hook(
            &fixture,
            &serde_json::to_vec(&case).expect("serialize noop payload"),
            capture.env(),
        ));
    }
    capture.assert_absent();
}

#[test]
fn bash_vision_access_is_denied_only_with_an_active_run() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, RATIFIED);
    let dangerous = "python3 -c 'open(\"planning/2026-08-03-criterion-protection-fixture/vision.md\",\"w\").write(\"weakened\")'";
    assert_refusal(
        run_hook(
            &fixture,
            &bash_payload(&fixture.root, dangerous),
            Vec::new(),
        ),
        REFUSE_BASH,
    );
    assert_silent_success(&run_hook(
        &fixture,
        &bash_payload(&fixture.root, "cargo test --workspace"),
        Vec::new(),
    ));
    let no_run = fixture.root.join("no-active-run");
    fs::create_dir(&no_run).expect("create no-run root");
    assert_silent_success(&run_hook(
        &fixture,
        &bash_payload(&no_run, dangerous),
        Vec::new(),
    ));
    assert_eq!(
        fs::read(&fixture.vision).expect("read protected vision"),
        RATIFIED.as_bytes()
    );
}

#[test]
fn activation_boundary_is_recorded_unpaid_not_green() {
    let harness = CliHarness::new().expect("create harness");
    let fixture = Fixture::new(&harness, INSTALL_ONLY);
    let payload = br#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"inspect the human-install boundary"}"#;
    let logged = harness
        .run(
            [
                OsStr::new("log"),
                OsStr::new("--file"),
                fixture.log.as_os_str(),
                OsStr::new("--kind"),
                OsStr::new("criterion-execution"),
                OsStr::new("--node"),
                OsStr::new("m4-s2"),
            ],
            payload,
        )
        .expect("append unpaid execution");
    assert!(logged.status.success());
    assert_eq!(logged.stdout, b"");
    assert_eq!(logged.stderr, b"");
    let output = harness
        .run(
            [
                OsStr::new("completion"),
                OsStr::new("check"),
                OsStr::new("--file"),
                fixture.log.as_os_str(),
                OsStr::new("--vision-dir"),
                fixture.vision_dir.as_os_str(),
                OsStr::new("--finished-result"),
                OsStr::new("main@0123456789abcdef"),
            ],
            b"",
        )
        .expect("check completion");
    let expected = b"{\"decision\":\"complete\",\"finished_result\":\"main@0123456789abcdef\",\"criteria\":[{\"criterion\":{\"name\":\"Install-only criterion\",\"input\":\"Install the hook, then attempt the forbidden command.\",\"observation\":\"The command is denied.\"},\"status\":\"unpaid\",\"reason\":\"The run cannot activate the human-installed hook.\"}]}\n";
    assert!(output.status.success());
    assert_eq!(output.stdout, expected);
    assert_eq!(output.stderr, b"");
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse completion report");
    let criterion = &report["criteria"][0];
    assert_eq!(criterion["status"], "unpaid");
    assert_eq!(
        criterion["reason"],
        "The run cannot activate the human-installed hook."
    );
    assert!(criterion.get("observed_result").is_none());
}

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    vision_dir: PathBuf,
    vision: PathBuf,
    log: PathBuf,
}

impl Fixture {
    fn new(harness: &CliHarness, document: &str) -> Self {
        let root = fs::canonicalize(harness.path())
            .expect("canonicalize harness root")
            .join("scratch-repository");
        let home = harness.path().join("fake-home");
        let vision_dir = root.join("planning").join(VISION_DIR_NAME);
        let vision = vision_dir.join("vision.md");
        let log = vision_dir.join("events.jsonl");
        fs::create_dir_all(home.join(".local/bin")).expect("create fake bin");
        fs::create_dir_all(&vision_dir).expect("create vision directory");
        fs::write(&vision, document).expect("write vision");
        let contract = format!(
            r#"{{"repository":"pce","repo_root":"{}","stated":{{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"}},"observations":{{"format":0,"lint":0,"typecheck":0,"test":0,"build":0}},"workflow_map":{{"ci.yml":"cargo test --workspace","docs.yml":null}},"appendable":{{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]}},"evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"}}"#,
            root.display()
        );
        let output = harness
            .run(
                [
                    OsStr::new("log"),
                    OsStr::new("--file"),
                    log.as_os_str(),
                    OsStr::new("--kind"),
                    OsStr::new("repository-contract"),
                    OsStr::new("--node"),
                    OsStr::new("m4-s2"),
                ],
                contract.as_bytes(),
            )
            .expect("append repository contract");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"");
        assert_eq!(output.stderr, b"");
        Self {
            root,
            home,
            vision_dir,
            vision,
            log,
        }
    }
}

struct Capture {
    argv: PathBuf,
    stdin: PathBuf,
    cwd: PathBuf,
}

impl Capture {
    fn new(root: &Path) -> Self {
        Self {
            argv: root.join("capture.argv"),
            stdin: root.join("capture.stdin"),
            cwd: root.join("capture.cwd"),
        }
    }

    fn env(&self) -> Vec<(&'static str, &OsStr)> {
        vec![
            ("PCE_CAPTURE_ARGV", self.argv.as_os_str()),
            ("PCE_CAPTURE_STDIN", self.stdin.as_os_str()),
            ("PCE_CAPTURE_CWD", self.cwd.as_os_str()),
            ("PCE_FAKE_EXIT", OsStr::new("0")),
        ]
    }

    fn assert_call(&self, fixture: &Fixture, document: &[u8]) {
        let mut expected = b"criteria\0check\0--file\0".to_vec();
        expected.extend_from_slice(fixture.log.as_os_str().as_encoded_bytes());
        expected.extend_from_slice(b"\0--vision-dir\0");
        expected.extend_from_slice(fixture.vision_dir.as_os_str().as_encoded_bytes());
        expected.push(0);
        assert_eq!(fs::read(&self.argv).expect("read captured argv"), expected);
        assert_eq!(
            fs::read(&self.stdin).expect("read captured stdin"),
            document
        );
        assert_eq!(
            fs::read(&self.cwd).expect("read captured cwd"),
            format!("{}\n", fixture.root.display()).as_bytes()
        );
    }

    fn clear(&self) {
        for path in [&self.argv, &self.stdin, &self.cwd] {
            if path.exists() {
                fs::remove_file(path).expect("remove capture");
            }
        }
    }

    fn assert_absent(&self) {
        assert!(!self.argv.exists());
        assert!(!self.stdin.exists());
        assert!(!self.cwd.exists());
    }
}

fn run_hook(fixture: &Fixture, payload: &[u8], environment: Vec<(&str, &OsStr)>) -> Output {
    let mut command =
        Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/pce-protect-criteria.sh"));
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &fixture.home)
        .current_dir(&fixture.root);
    for (name, value) in environment {
        command.env(name, value);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook");
    child
        .stdin
        .take()
        .expect("piped hook stdin")
        .write_all(payload)
        .expect("write hook payload");
    child.wait_with_output().expect("wait for hook")
}

fn write_payload(fixture: &Fixture, content: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Write","tool_input":{"file_path":fixture.vision,"content":content}})).expect("serialize write")
}

fn edit_payload(fixture: &Fixture, old: &str, new: &str, replace_all: Option<Value>) -> Vec<u8> {
    serde_json::to_vec(&edit_payload_value(fixture, old, new, replace_all)).expect("serialize edit")
}

fn edit_payload_value(
    fixture: &Fixture,
    old: &str,
    new: &str,
    replace_all: Option<Value>,
) -> Value {
    let mut input = json!({"file_path":fixture.vision,"old_string":old,"new_string":new});
    if let Some(value) = replace_all {
        input["replace_all"] = value;
    }
    json!({"hook_event_name":"PreToolUse","cwd":fixture.root,"tool_name":"Edit","tool_input":input})
}

fn bash_payload(root: &Path, command: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"hook_event_name":"PreToolUse","cwd":root,"tool_name":"Bash","tool_input":{"command":command}})).expect("serialize Bash")
}

fn install_real_pce(home: &Path) {
    let path = home.join(".local/bin/pce");
    if path.exists() {
        fs::remove_file(&path).expect("remove prior pce");
    }
    symlink(env!("CARGO_BIN_EXE_pce"), path).expect("symlink real pce");
}

fn install_script(home: &Path, script: &str, mode: u32) {
    write_mode(&home.join(".local/bin/pce"), script, mode);
}

fn write_mode(path: &Path, content: &str, mode: u32) {
    if path.exists() {
        fs::remove_file(path).expect("remove previous executable");
    }
    fs::write(path, content).expect("write executable");
    let mut permissions = fs::metadata(path).expect("read mode").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set mode");
}

fn assert_silent_success(output: &Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
}

fn assert_refusal(output: Output, stderr: &[u8]) {
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, stderr);
}
