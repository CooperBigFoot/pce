#[allow(dead_code)]
mod support;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use support::{CliHarness, CodexInvocation};

const INHERITED_MARKER: (&str, &str) = ("PCE_INHERITED_ONLY", "must-not-reach-codex");
const CHILD_MARKER: (&str, &str) = ("PCE_CHILD_MARKER", "explicit-child-value");

struct DispatchCase<'a> {
    name: &'a str,
    structured: bool,
    plan: Option<&'a [u8]>,
    caller_tail: &'a str,
}

#[test]
fn dispatches_unstructured_null_with_explicit_process_configuration() {
    assert_dispatch(DispatchCase {
        name: "unstructured-null",
        structured: false,
        plan: None,
        caller_tail: "NULL_PROMPT_MARKER",
    });
}

#[test]
fn dispatches_structured_null_with_explicit_process_configuration() {
    assert_dispatch(DispatchCase {
        name: "structured-null",
        structured: true,
        plan: None,
        caller_tail: "STRUCTURED_NULL_PROMPT_MARKER",
    });
}

#[test]
fn dispatches_unstructured_plan_with_exact_plan_stdin() {
    assert_dispatch(DispatchCase {
        name: "unstructured-plan",
        structured: false,
        plan: Some(b"plan\0bytes\xffwithout-newline"),
        caller_tail: "-",
    });
}

#[test]
fn dispatches_structured_plan_with_exact_plan_stdin() {
    assert_dispatch(DispatchCase {
        name: "structured-plan",
        structured: true,
        plan: Some(b"structured\x80plan\0bytes"),
        caller_tail: "-",
    });
}

fn assert_dispatch(case: DispatchCase<'_>) {
    let harness = CliHarness::new().expect("create CLI harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize child cwd");
    let record_root = harness.path().join(format!("{}-records", case.name));
    fs::create_dir(&record_root).expect("create record root");
    let stdout_path = harness.path().join(format!("{}.stdout", case.name));
    let stderr_path = harness.path().join(format!("{}.stderr", case.name));
    let expected_stdout = format!("{} stdout\0bytes", case.name).into_bytes();
    let expected_stderr = format!("{} stderr\0bytes", case.name).into_bytes();
    fs::write(&stdout_path, &expected_stdout).expect("write fixture stdout");
    fs::write(&stderr_path, &expected_stderr).expect("write fixture stderr");
    let sentinel_path = harness.path().join(format!("{}.parent-stdin", case.name));
    let sentinel = format!("unique-parent-sentinel-{}", case.name).into_bytes();
    fs::write(&sentinel_path, &sentinel).expect("write parent stdin sentinel");

    let child_environment =
        child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let schema_path = cwd.join("schema.json");
    let output_path = cwd.join("output.json");
    let plan_path = case.plan.map(|bytes| {
        let path = harness.path().join(format!("{}.plan", case.name));
        fs::write(&path, bytes).expect("write plan fixture");
        path
    });
    let argv = dispatch_argv(
        &cwd,
        &child_environment,
        case.structured.then_some((&schema_path, &output_path)),
        plan_path.as_deref(),
        case.caller_tail,
    );
    let output = harness
        .run_with_stdin_file(&argv, &sentinel_path, INHERITED_MARKER)
        .expect("run dispatch");

    let invocations = harness
        .codex_invocations(&record_root)
        .expect("read Codex record");
    assert_eq!(invocations.len(), 1);
    let invocation = &invocations[0];
    assert_invocation(
        invocation,
        &cwd,
        &child_environment,
        case.structured.then_some((&schema_path, &output_path)),
        case.caller_tail,
        case.plan.unwrap_or_default(),
    );
    assert_ne!(invocation.stdin, sentinel);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.ends_with(&expected_stderr));
}

fn child_environment(
    harness: &CliHarness,
    record_root: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
    exit_code: i32,
) -> Vec<(String, String)> {
    vec![
        ("PATH".to_owned(), harness.shim_path()),
        (
            "PCE_CODEX_RECORD_ROOT".to_owned(),
            record_root.display().to_string(),
        ),
        (
            "PCE_CODEX_STDOUT_FILE".to_owned(),
            stdout_path.display().to_string(),
        ),
        (
            "PCE_CODEX_STDERR_FILE".to_owned(),
            stderr_path.display().to_string(),
        ),
        ("PCE_CODEX_EXIT_CODE".to_owned(), exit_code.to_string()),
        (CHILD_MARKER.0.to_owned(), CHILD_MARKER.1.to_owned()),
    ]
}

fn dispatch_argv(
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    plan_path: Option<&Path>,
    caller_tail: &str,
) -> Vec<String> {
    let mut argv = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ];
    for (name, value) in environment {
        argv.extend(["--env".to_owned(), format!("{name}={value}")]);
    }
    if let Some((schema, output)) = structured {
        argv.extend([
            "--output-schema".to_owned(),
            schema.display().to_string(),
            "-o".to_owned(),
            output.display().to_string(),
        ]);
    }
    if let Some(path) = plan_path {
        argv.extend(["--plan-file".to_owned(), path.display().to_string()]);
    }
    argv.extend(["--".to_owned(), caller_tail.to_owned()]);
    argv
}

fn assert_invocation(
    invocation: &CodexInvocation,
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    caller_tail: &str,
    expected_stdin: &[u8],
) {
    let mut expected_argv = vec![
        OsString::from("exec"),
        OsString::from("-C"),
        cwd.as_os_str().to_owned(),
        OsString::from("--sandbox"),
        OsString::from("workspace-write"),
    ];
    if let Some((schema, output)) = structured {
        expected_argv.extend([
            OsString::from("--output-schema"),
            schema.as_os_str().to_owned(),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ]);
    }
    expected_argv.push(OsString::from(caller_tail));
    assert_eq!(invocation.argv, expected_argv);
    assert_eq!(invocation.cwd, cwd);
    let mut expected_environment: BTreeSet<OsString> = environment
        .iter()
        .map(|(name, value)| OsString::from(format!("{name}={value}")))
        .collect();
    expected_environment.insert(OsString::from(format!("PWD={}", cwd.display())));
    expected_environment.insert(OsString::from("SHLVL=1"));
    expected_environment.insert(OsString::from("_=/usr/bin/env"));
    assert_eq!(invocation.environment, expected_environment);
    assert!(!invocation.environment.contains(&OsString::from(format!(
        "{}={}",
        INHERITED_MARKER.0, INHERITED_MARKER.1
    ))));
    assert_eq!(invocation.stdin, expected_stdin);
}

#[test]
fn propagates_nonzero_codex_status() {
    let (harness, cwd, record_root, stdin_path, environment) = failure_fixture("nonzero", 37);
    let argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run nonzero dispatch");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exited with status"));
    assert_eq!(
        harness
            .codex_invocations(&record_root)
            .expect("read record")
            .len(),
        1
    );
}

#[test]
fn reports_codex_spawn_failure() {
    let (harness, cwd, record_root, stdin_path, mut environment) = failure_fixture("spawn", 0);
    environment
        .iter_mut()
        .find(|entry| entry.0 == "PATH")
        .expect("PATH entry")
        .1 = "/usr/bin:/bin".to_owned();
    let argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run missing Codex dispatch");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to spawn `codex`"));
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("read records")
            .is_empty()
    );
}

fn failure_fixture(
    name: &str,
    exit_code: i32,
) -> (CliHarness, PathBuf, PathBuf, PathBuf, Vec<(String, String)>) {
    let harness = CliHarness::new().expect("create harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join(format!("{name}-records"));
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join(format!("{name}.stdout"));
    let stderr_path = harness.path().join(format!("{name}.stderr"));
    let stdin_path = harness.path().join(format!("{name}.stdin"));
    fs::write(&stdout_path, []).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    fs::write(&stdin_path, b"parent sentinel").expect("write stdin");
    let environment = child_environment(
        &harness,
        &record_root,
        &stdout_path,
        &stderr_path,
        exit_code,
    );
    (harness, cwd, record_root, stdin_path, environment)
}

#[test]
fn rejects_invalid_dispatch_inputs_before_invocation() {
    let harness = CliHarness::new().expect("create harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let stdin_path = harness.path().join("parser.stdin");
    fs::write(&stdin_path, b"sentinel").expect("write stdin");
    let absolute = cwd.display().to_string();
    let cases = [
        (
            "relative-cwd",
            "relative",
            "workspace-write",
            vec![],
            "working directory must be absolute",
        ),
        (
            "relative-schema",
            &absolute,
            "workspace-write",
            vec!["--output-schema", "relative", "-o", &absolute],
            "schema path must be absolute",
        ),
        (
            "relative-output",
            &absolute,
            "workspace-write",
            vec!["--output-schema", &absolute, "-o", "relative"],
            "output path must be absolute",
        ),
        (
            "unpaired-schema",
            &absolute,
            "workspace-write",
            vec!["--output-schema", &absolute],
            "expected `-o`",
        ),
        (
            "malformed-env",
            &absolute,
            "workspace-write",
            vec!["--env", "malformed"],
            "environment entry must contain `=`",
        ),
        (
            "duplicate-env",
            &absolute,
            "workspace-write",
            vec!["--env", "A=1", "--env", "A=2"],
            "duplicate environment name `A`",
        ),
        (
            "unknown-sandbox",
            &absolute,
            "unknown",
            vec![],
            "unsupported sandbox `unknown`",
        ),
        (
            "empty-env-name",
            &absolute,
            "workspace-write",
            vec!["--env", "=value"],
            "environment name must not be empty",
        ),
    ];
    for (name, raw_cwd, sandbox, extra_options, expected_stderr) in cases {
        let record_root = harness.path().join(format!("{name}-records"));
        fs::create_dir(&record_root).expect("create parser record root");
        let stdout_path = harness.path().join(format!("{name}.stdout"));
        let stderr_path = harness.path().join(format!("{name}.stderr"));
        fs::write(&stdout_path, []).expect("write parser fixture stdout");
        fs::write(&stderr_path, []).expect("write parser fixture stderr");
        let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
        let mut argv = vec![
            "dispatch".to_owned(),
            "codex".to_owned(),
            "--cwd".to_owned(),
            raw_cwd.to_owned(),
            "--sandbox".to_owned(),
            sandbox.to_owned(),
        ];
        for (name, value) in &environment {
            argv.extend(["--env".to_owned(), format!("{name}={value}")]);
        }
        argv.extend(extra_options.into_iter().map(str::to_owned));
        argv.push("--".to_owned());
        let output = harness
            .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
            .expect("run parser case");
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
            "{name} stderr did not contain {expected_stderr:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            harness
                .codex_invocations(&record_root)
                .expect("read parser records")
                .is_empty(),
            "{name} invoked Codex"
        );
    }
    let missing_plan = harness.path().join("missing.plan");
    let record_root = harness.path().join("missing-plan-records");
    fs::create_dir(&record_root).expect("create missing-plan record root");
    let stdout_path = harness.path().join("missing-plan.stdout");
    let stderr_path = harness.path().join("missing-plan.stderr");
    fs::write(&stdout_path, []).expect("write missing-plan fixture stdout");
    fs::write(&stderr_path, []).expect("write missing-plan fixture stderr");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let argv = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        absolute,
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ]
    .into_iter()
    .chain(
        environment
            .iter()
            .flat_map(|(name, value)| ["--env".to_owned(), format!("{name}={value}")]),
    )
    .chain([
        "--plan-file".to_owned(),
        missing_plan.display().to_string(),
        "--".to_owned(),
        "-".to_owned(),
    ])
    .collect::<Vec<_>>();
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run unreadable plan");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(missing_plan.to_string_lossy().as_ref())
    );
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("read missing-plan records")
            .is_empty()
    );
}
