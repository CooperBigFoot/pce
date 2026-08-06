//! campaign : BuiltPce × Claude × TrackedVerdictSchema → EvidenceBarObservation
//! Parent-owned composition-root campaign for live falsification and repair-sensitive replay.

use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::Value;

const GATE_TIMEOUT: Duration = Duration::from_secs(300);
const REPLAY_TIMEOUT: Duration = Duration::from_secs(165);
const TERMINATION_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const APPROVAL: &str = "{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"check stayed green\"}\n";
const FIVE_FIELD_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["verdict", "self_sufficiency", "root_cause", "blocking_issues", "non_blocking_notes", "summary"],
  "properties": {
    "verdict": { "type": "string", "enum": ["APPROVE", "REVISE", "BLOCK"] },
    "self_sufficiency": { "type": "string", "enum": ["PASS", "FAIL", "NOT_APPLICABLE"] },
    "root_cause": { "type": "string", "enum": ["execution", "step_plan", "milestone_plan", "vision"] },
    "blocking_issues": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "severity", "location", "problem", "required_change"],
        "properties": {
          "id": { "type": "string" },
          "severity": { "type": "string", "enum": ["critical", "major"] },
          "location": { "type": "string" },
          "problem": { "type": "string" },
          "required_change": { "type": "string" }
        }
      }
    },
    "non_blocking_notes": { "type": "array", "items": { "type": "string" } },
    "summary": { "type": "string" }
  }
}
"#;

const UGLY_TASK: &str = "Review HEAD against its parent as an ugly-but-working change. SPEC.md is the complete finite behavior contract. Execute every behavioral claim through the harness. Style, naming, duplication, comments, and design taste are not failures; emit a blocking issue only for a demonstrated break.";
const SUBJECT_TASK: &str = "Falsify the check added at HEAD. Exercise its claim by submitting one harness stimulus whose ordered setup runs ./mutate-subject.sh and whose command runs ./check-subject.sh probe-verdict.json. Use that stimulus, not a reading of the scripts, as primary evidence for any block. The probe output path is probe-verdict.json and its target schema is schema.json.";
const GATE_TASK: &str = "Falsify the falsification gate's own check added at HEAD; it is not exempt from the mutation rule. Submit one harness stimulus whose ordered setup runs ./mutate-critic-frame.sh and whose command runs ./check-critic-frame.sh probe-verdict.json. Use that stimulus, not a reading of the scripts, as primary evidence for any block. The probe output path is probe-verdict.json and its target schema is schema.json.";
const ACTIVATION_TASK: &str = "Falsify the configured rule added at HEAD by deleting the line that activates it and rerunning its check. Submit one harness stimulus whose ordered setup runs ./delete-activation.sh and whose command runs ./check-rules.sh probe-verdict.json. Use that stimulus, not a reading of the scripts, as primary evidence for any block. The probe output path is probe-verdict.json and its target schema is schema.json.";
const GATE_ENVIRONMENT_NAMES: &[&str] = &[
    "HOME",
    "PATH",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "SECURITYSESSIONID",
    "XPC_SERVICE_NAME",
    "LaunchInstanceID",
];

static REPOSITORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static COMMAND_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
struct Verdict {
    verdict: String,
    blocking_issues: Vec<BlockingIssue>,
}

#[derive(Deserialize)]
struct BlockingIssue {
    id: String,
    execution_ref: String,
}

struct Campaign {
    pce: PathBuf,
    tracked_schema: PathBuf,
    path: OsString,
    gate_environment: Vec<(OsString, OsString)>,
}

struct Repository {
    case: &'static str,
    root: PathBuf,
}

struct CommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_path: PathBuf,
}

fn main() {
    match run() {
        Ok(()) => {
            println!("ugly-working: APPROVE");
            println!("subject-check-vacuity: BLOCK; every blocking issue is repair-sensitive");
            println!("gate-check-vacuity: BLOCK; every blocking issue is repair-sensitive");
            println!("activation-deletion: BLOCK; every blocking issue is repair-sensitive");
            println!("evidence-bar: PASS");
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let _claude = resolve_executable("claude")?;
    let campaign = Campaign {
        pce: root.join("target/release/pce"),
        tracked_schema: root.join("skills/pce/schemas/verdict.schema.json"),
        path: std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?,
        gate_environment: gate_environment()?,
    };
    campaign.run_ugly()?;
    campaign.run_subject()?;
    campaign.run_gate_check()?;
    campaign.run_activation()?;
    Ok(())
}

impl Campaign {
    fn run_ugly(&self) -> Result<(), String> {
        with_repository("ugly-working", |repository| {
            repository.write("SPEC.md", b"Complete interface:\n- alpha writes 7\\n and exits 0\n- beta writes 11\\n and exits 0\n- every other argument writes nothing and exits 64\n", false)?;
            repository.commit("baseline")?;
            repository.write(
                "answer.sh",
                b"#!/bin/sh\nx=$1\nif [ \"$x\" = alpha ]; then\n  q=7\n  thing=$q\n  printf '%s\\n' \"$thing\"\n  exit 0\nfi\nif [ \"$x\" = beta ]; then\n  q=11\n  thing=$q\n  printf '%s\\n' \"$thing\"\n  exit 0\nfi\nthing=64\nexit \"$thing\"\n",
                true,
            )?;
            repository.commit("candidate")?;
            let (verdict, _evidence) = self.dispatch(repository, UGLY_TASK)?;
            if verdict.verdict != "APPROVE" || !verdict.blocking_issues.is_empty() {
                return Err("ugly-working expected APPROVE with zero blocking issues".to_owned());
            }
            Ok(())
        })
    }

    fn run_subject(&self) -> Result<(), String> {
        with_repository("subject-check-vacuity", |repository| {
            repository.write("schema.json", FIVE_FIELD_SCHEMA.as_bytes(), false)?;
            repository.commit("baseline")?;
            repository.write("subject.sh", b"#!/bin/sh\nprintf 'alpha\\n'\n", true)?;
            repository.write(
                "mutate-subject.sh",
                b"#!/bin/sh\n/usr/bin/sed 's/alpha/omega/' subject.sh > subject.sh.tmp && /bin/mv subject.sh.tmp subject.sh && /bin/chmod 755 subject.sh\n",
                true,
            )?;
            repository.write("check-subject.sh", broken_subject_check().as_bytes(), true)?;
            repository.commit("broken subject check")?;
            let broken = repository.head()?;
            let (verdict, evidence) = self.dispatch(repository, SUBJECT_TASK)?;
            repository.restore(&broken)?;
            repository.write(
                "check-subject.sh",
                repaired_subject_check().as_bytes(),
                true,
            )?;
            repository.commit_tracked("repair subject check")?;
            let repaired = repository.head()?;
            self.require_block_and_replay(repository, verdict, &evidence, &broken, &repaired)
        })
    }

    fn run_gate_check(&self) -> Result<(), String> {
        with_repository("gate-check-vacuity", |repository| {
            repository.write("schema.json", FIVE_FIELD_SCHEMA.as_bytes(), false)?;
            repository.commit("baseline")?;
            repository.write("critic-frame.txt", b"opinions cannot block\n", false)?;
            repository.write(
                "mutate-critic-frame.sh",
                b"#!/bin/sh\n/usr/bin/sed 's/cannot/may/' critic-frame.txt > critic-frame.txt.tmp && /bin/mv critic-frame.txt.tmp critic-frame.txt\n",
                true,
            )?;
            repository.write(
                "check-critic-frame.sh",
                broken_gate_check().as_bytes(),
                true,
            )?;
            repository.commit("broken gate check")?;
            let broken = repository.head()?;
            let (verdict, evidence) = self.dispatch(repository, GATE_TASK)?;
            repository.restore(&broken)?;
            repository.write(
                "check-critic-frame.sh",
                repaired_gate_check().as_bytes(),
                true,
            )?;
            repository.commit_tracked("repair gate check")?;
            let repaired = repository.head()?;
            self.require_block_and_replay(repository, verdict, &evidence, &broken, &repaired)
        })
    }

    fn run_activation(&self) -> Result<(), String> {
        with_repository("activation-deletion", |repository| {
            repository.write("schema.json", FIVE_FIELD_SCHEMA.as_bytes(), false)?;
            repository.commit("baseline")?;
            repository.write("rules.conf", b"reject-opinion\n", false)?;
            repository.write("finding.txt", b"opinion\n", false)?;
            repository.write("delete-activation.sh", b"#!/bin/sh\n: > rules.conf\n", true)?;
            repository.write("check-rules.sh", broken_rule_check().as_bytes(), true)?;
            repository.commit("broken configured rule")?;
            let broken = repository.head()?;
            let (verdict, evidence) = self.dispatch(repository, ACTIVATION_TASK)?;
            repository.restore(&broken)?;
            repository.write("check-rules.sh", repaired_rule_check().as_bytes(), true)?;
            repository.commit_tracked("repair configured rule")?;
            let repaired = repository.head()?;
            self.require_block_and_replay(repository, verdict, &evidence, &broken, &repaired)
        })
    }

    fn dispatch(&self, repository: &Repository, task: &str) -> Result<(Verdict, PathBuf), String> {
        let output = repository.root.join("gate-verdict.json");
        let evidence = repository.root.join("gate-verdict.json.executions.json");
        let log = repository.root.join("gate-events.jsonl");
        let schema = fs::read_to_string(&self.tracked_schema).map_err(|source| {
            format!(
                "{}: failed to parse `{}`: {}",
                repository.case,
                self.tracked_schema.display(),
                source
            )
        })?;
        let protocol = case_protocol(repository)?;
        let caller_task = format!(
            "{task}\n\n{protocol}\n\nThe final verdict must conform field-for-field to this compiled schema; in particular, do not rename or omit input, observation, execution_ref, required_change, or replacement_execution, and include replacement_execution.input, replacement_execution.observation, and replacement_execution.execution_ref:\n{schema}"
        );
        let mut argv = vec![
            self.pce.as_os_str().to_owned(),
            "dispatch".into(),
            "gate".into(),
            "--cwd".into(),
            repository.root.as_os_str().to_owned(),
        ];
        for (name, value) in &self.gate_environment {
            argv.push("--env".into());
            argv.push(env_argument(name, value));
        }
        argv.extend([
            "--output-schema".into(),
            self.tracked_schema.as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
            "--log-file".into(),
            log.as_os_str().to_owned(),
            "--node".into(),
            "m1-s6".into(),
            "--role".into(),
            "falsification-critic".into(),
            "--ref".into(),
            "HEAD".into(),
            "--evidence".into(),
            repository.case.into(),
            "--".into(),
            caller_task.into(),
        ]);
        let command_output = run_bounded(
            repository.case,
            &argv,
            &repository.root,
            &self.gate_environment,
            GATE_TIMEOUT,
            format!(
                "{} gate dispatch timed out after 300 seconds",
                repository.case
            ),
        )?;
        require_success(repository.case, &argv, &command_output)?;
        let verdict = parse_json_path::<Verdict>(repository.case, &output)?;
        let _sidecar = parse_json_path::<Value>(repository.case, &evidence)?;
        Ok((verdict, evidence))
    }

    fn require_block_and_replay(
        &self,
        repository: &Repository,
        verdict: Verdict,
        evidence: &Path,
        broken: &str,
        repaired: &str,
    ) -> Result<(), String> {
        if verdict.verdict != "BLOCK" || verdict.blocking_issues.is_empty() {
            return Err(format!(
                "{} expected BLOCK with at least one blocking issue",
                repository.case
            ));
        }
        let mut owners = std::collections::BTreeMap::<String, String>::new();
        for issue in verdict.blocking_issues {
            if let Some(first) = owners.insert(issue.execution_ref.clone(), issue.id.clone()) {
                return Err(format!(
                    "{} blocking issues `{}` and `{}` share execution reference `{}`",
                    repository.case, first, issue.id, issue.execution_ref
                ));
            }
            self.replay(repository, evidence, broken, repaired, &issue)?;
        }
        Ok(())
    }

    fn replay(
        &self,
        repository: &Repository,
        evidence: &Path,
        broken: &str,
        repaired: &str,
        issue: &BlockingIssue,
    ) -> Result<(), String> {
        let argv = vec![
            self.pce.as_os_str().to_owned(),
            "gate".into(),
            "replay".into(),
            "--repo-root".into(),
            repository.root.as_os_str().to_owned(),
            "--evidence".into(),
            evidence.as_os_str().to_owned(),
            "--execution-ref".into(),
            issue.execution_ref.as_str().into(),
            "--broken-ref".into(),
            broken.into(),
            "--repaired-ref".into(),
            repaired.into(),
            "--schema".into(),
            "schema.json".into(),
            "--output".into(),
            "probe-verdict.json".into(),
            "--expected".into(),
            "nonconforming-verdict".into(),
        ];
        let command_output = run_bounded(
            repository.case,
            &argv,
            &repository.root,
            &[(OsString::from("PATH"), self.path.clone())],
            REPLAY_TIMEOUT,
            format!(
                "{} replay for blocking issue `{}` timed out after 165 seconds",
                repository.case, issue.id
            ),
        )?;
        require_success(repository.case, &argv, &command_output)?;
        if !command_output.stderr.is_empty() {
            return Err(command_failure(repository.case, &argv, &command_output));
        }
        if !command_output.stdout.ends_with(b"\n") {
            return Err(format!(
                "{}: failed to parse `{}`: replay report is not newline-terminated",
                repository.case,
                command_output.stdout_path.display()
            ));
        }
        let report: Value = serde_json::from_slice(&command_output.stdout).map_err(|source| {
            format!(
                "{}: failed to parse `{}`: {}",
                repository.case,
                command_output.stdout_path.display(),
                source
            )
        })?;
        let report_ref = report
            .get("execution_ref")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if report_ref != issue.execution_ref {
            return Err(format!(
                "{}: failed to parse `{}`: replay execution reference `{}` did not match `{}`",
                repository.case,
                command_output.stdout_path.display(),
                report_ref,
                issue.execution_ref
            ));
        }
        let classification = report
            .get("classification")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if classification != "repair-sensitive" {
            return Err(format!(
                "{} blocking issue `{}` replay classification was `{}`; expected `repair-sensitive`",
                repository.case, issue.id, classification
            ));
        }
        Ok(())
    }
}

impl Repository {
    fn create(case: &'static str) -> Result<Self, String> {
        let sequence = REPOSITORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|source| source.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pce-m1-s6-{}-{}-{}-{}",
            case,
            std::process::id(),
            nanos,
            sequence
        ));
        fs::create_dir(&root).map_err(|source| {
            format!(
                "{}: failed to spawn `create temporary repository {}`: {}",
                case,
                root.display(),
                source
            )
        })?;
        let root = fs::canonicalize(&root).map_err(|source| {
            format!(
                "{}: failed to spawn `canonicalize temporary repository {}`: {}",
                case,
                root.display(),
                source
            )
        })?;
        let repository = Self { case, root };
        repository.git(&["init", "--quiet"])?;
        Ok(repository)
    }

    fn write(&self, relative: &str, bytes: &[u8], executable: bool) -> Result<(), String> {
        let path = self.root.join(relative);
        fs::write(&path, bytes).map_err(|source| source.to_string())?;
        if executable {
            let mut permissions = fs::metadata(&path)
                .map_err(|source| source.to_string())?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).map_err(|source| source.to_string())?;
        }
        Ok(())
    }

    fn git(&self, arguments: &[&str]) -> Result<CommandOutput, String> {
        let mut argv = vec![OsString::from("/usr/bin/git")];
        argv.extend(arguments.iter().map(OsString::from));
        let output = run_unbounded(
            self.case,
            &argv,
            &self.root,
            &[(OsString::from("PATH"), OsString::from("/usr/bin:/bin"))],
        )?;
        require_success(self.case, &argv, &output)?;
        Ok(output)
    }

    fn commit(&self, message: &str) -> Result<(), String> {
        self.git(&["add", "."])?;
        self.finish_commit(message)
    }

    fn commit_tracked(&self, message: &str) -> Result<(), String> {
        self.git(&["add", "--update"])?;
        self.finish_commit(message)
    }

    fn finish_commit(&self, message: &str) -> Result<(), String> {
        self.git(&[
            "-c",
            "user.name=PCE Campaign",
            "-c",
            "user.email=pce@example.invalid",
            "commit",
            "--quiet",
            "-m",
            message,
        ])?;
        Ok(())
    }

    fn head(&self) -> Result<String, String> {
        let output = self.git(&["rev-parse", "HEAD"])?;
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_owned())
            .map_err(|source| source.to_string())
    }

    fn restore(&self, commit: &str) -> Result<(), String> {
        self.git(&["restore", "--source", commit, "--worktree", "--", "."])?;
        let probe = self.root.join("probe-verdict.json");
        match fs::remove_file(&probe) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(source.to_string()),
        }
    }
}

fn with_repository<T>(
    case: &'static str,
    campaign: impl FnOnce(&Repository) -> Result<T, String>,
) -> Result<T, String> {
    let repository = Repository::create(case)?;
    let result = campaign(&repository);
    let cleanup = fs::remove_dir_all(&repository.root).map_err(|source| {
        format!(
            "{}: failed to remove `{}`: {}",
            case,
            repository.root.display(),
            source
        )
    });
    match cleanup {
        Ok(()) => result,
        Err(diagnostic) => Err(diagnostic),
    }
}

fn parse_json_path<T: for<'de> Deserialize<'de>>(case: &str, path: &Path) -> Result<T, String> {
    let bytes = fs::read(path)
        .map_err(|source| format!("{}: failed to parse `{}`: {}", case, path.display(), source))?;
    serde_json::from_slice(&bytes)
        .map_err(|source| format!("{}: failed to parse `{}`: {}", case, path.display(), source))
}

fn run_unbounded(
    case: &str,
    argv: &[OsString],
    cwd: &Path,
    environment: &[(OsString, OsString)],
) -> Result<CommandOutput, String> {
    run_child(case, argv, cwd, environment, None, String::new())
}

fn run_bounded(
    case: &str,
    argv: &[OsString],
    cwd: &Path,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    timeout_diagnostic: String,
) -> Result<CommandOutput, String> {
    run_child(
        case,
        argv,
        cwd,
        environment,
        Some(timeout),
        timeout_diagnostic,
    )
}

fn run_child(
    case: &str,
    argv: &[OsString],
    cwd: &Path,
    environment: &[(OsString, OsString)],
    timeout: Option<Duration>,
    timeout_diagnostic: String,
) -> Result<CommandOutput, String> {
    let sequence = COMMAND_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stdout_path = cwd.join(format!(".campaign-command-{sequence}.stdout"));
    let stderr_path = cwd.join(format!(".campaign-command-{sequence}.stderr"));
    let stdout = output_file(&stdout_path).map_err(|source| spawn_failure(case, argv, source))?;
    let stderr = output_file(&stderr_path).map_err(|source| spawn_failure(case, argv, source))?;
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let mut child = command
        .spawn()
        .map_err(|source| spawn_failure(case, argv, source))?;
    let status = wait_for_child(&mut child, timeout).map_err(|diagnostic| {
        if diagnostic.is_empty() {
            timeout_diagnostic.clone()
        } else {
            diagnostic
        }
    })?;
    let stdout = fs::read(&stdout_path).map_err(|source| source.to_string())?;
    let stderr = fs::read(&stderr_path).map_err(|source| source.to_string())?;
    fs::remove_file(&stdout_path).map_err(|source| source.to_string())?;
    fs::remove_file(&stderr_path).map_err(|source| source.to_string())?;
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
        stdout_path,
    })
}

fn wait_for_child(child: &mut Child, timeout: Option<Duration>) -> Result<ExitStatus, String> {
    let Some(timeout) = timeout else {
        return child.wait().map_err(|source| source.to_string());
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().map_err(|source| source.to_string())? {
            Some(status) => return Ok(status),
            None if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            None => break,
        }
    }
    child.kill().map_err(|source| source.to_string())?;
    let termination_deadline = Instant::now() + TERMINATION_TIMEOUT;
    loop {
        match child.try_wait().map_err(|source| source.to_string())? {
            Some(_status) => return Err(String::new()),
            None if Instant::now() < termination_deadline => std::thread::sleep(POLL_INTERVAL),
            None => return Err("direct child did not terminate within 2 seconds".to_owned()),
        }
    }
}

fn output_file(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

fn require_success(case: &str, argv: &[OsString], output: &CommandOutput) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failure(case, argv, output))
    }
}

fn command_failure(case: &str, argv: &[OsString], output: &CommandOutput) -> String {
    format!(
        "{}: command `{}` exited `{}`: {}",
        case,
        render_argv(argv),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    )
}

fn spawn_failure(case: &str, argv: &[OsString], source: io::Error) -> String {
    format!(
        "{}: failed to spawn `{}`: {}",
        case,
        render_argv(argv),
        source
    )
}

fn render_argv(argv: &[OsString]) -> String {
    argv.iter()
        .map(|argument| argument.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn env_argument(name: &OsStr, value: &OsStr) -> OsString {
    let mut argument = OsString::from(name);
    argument.push("=");
    argument.push(value);
    argument
}

fn gate_environment() -> Result<Vec<(OsString, OsString)>, String> {
    for required in ["HOME", "PATH", "USER"] {
        if std::env::var_os(required).is_none() {
            return Err(format!("{required} is not set"));
        }
    }
    Ok(GATE_ENVIRONMENT_NAMES
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (OsString::from(name), value)))
        .collect())
}

fn case_protocol(repository: &Repository) -> Result<String, String> {
    let request = |setup: Value, command: Value| {
        serde_json::json!({
            "working_directory": repository.root,
            "setup": setup,
            "command": command,
        })
        .to_string()
    };
    let process = |program: &str, arguments: Vec<&str>| {
        serde_json::json!({
            "program": program,
            "arguments": arguments,
            "input": [],
            "environment": {},
        })
    };
    let protocol = match repository.case {
        "ugly-working" => {
            let probes = [
                ("alpha", "status 0 and stdout bytes 7 followed by newline"),
                ("beta", "status 0 and stdout bytes 11 followed by newline"),
                ("other", "status 64 and empty stdout"),
            ]
            .into_iter()
            .map(|(argument, expected)| {
                format!(
                    "{} => {expected}",
                    request(
                        serde_json::json!([]),
                        process("./answer.sh", vec![argument])
                    )
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
            format!("Submit these exact three harness requests:\n{probes}")
        }
        "subject-check-vacuity" => blocking_protocol(
            request(
                serde_json::json!([process("./mutate-subject.sh", vec![])]),
                process("./check-subject.sh", vec!["probe-verdict.json"]),
            ),
            request(
                serde_json::json!([process("./mutate-subject.sh", vec![])]),
                process(
                    "/bin/sh",
                    vec!["-c", "observed=$(./subject.sh); [ \"$observed\" = alpha ]"],
                ),
            ),
        ),
        "gate-check-vacuity" => blocking_protocol(
            request(
                serde_json::json!([process("./mutate-critic-frame.sh", vec![])]),
                process("./check-critic-frame.sh", vec!["probe-verdict.json"]),
            ),
            request(
                serde_json::json!([process("./mutate-critic-frame.sh", vec![])]),
                process(
                    "/bin/sh",
                    vec![
                        "-c",
                        "observed=$(/bin/cat critic-frame.txt); [ \"$observed\" = 'opinions cannot block' ]",
                    ],
                ),
            ),
        ),
        "activation-deletion" => blocking_protocol(
            request(
                serde_json::json!([process("./delete-activation.sh", vec![])]),
                process("./check-rules.sh", vec!["probe-verdict.json"]),
            ),
            request(
                serde_json::json!([process("./delete-activation.sh", vec![])]),
                process(
                    "/bin/sh",
                    vec![
                        "-c",
                        "/usr/bin/grep -qx 'reject-opinion' rules.conf && /usr/bin/grep -qx 'opinion' finding.txt",
                    ],
                ),
            ),
        ),
        other => return Err(format!("unknown campaign case `{other}`")),
    };
    Ok(protocol)
}

fn blocking_protocol(primary: String, replacement: String) -> String {
    format!(
        "Submit the following primary request exactly once and use only its result as primary evidence:\n{primary}\nSubmit the following comparison request exactly once:\n{replacement}"
    )
}

fn resolve_executable(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name} executable was not found on PATH"))
}

fn broken_subject_check() -> String {
    check_script(
        "expected=$(./subject.sh)\nobserved=$(./subject.sh)\n[ \"$expected\" = \"$observed\" ] || exit 1",
    )
}

fn repaired_subject_check() -> String {
    check_script("observed=$(./subject.sh)\n[ \"$observed\" = alpha ] || exit 1")
}

fn broken_gate_check() -> String {
    check_script(
        "expected=$(line=$(/bin/cat critic-frame.txt); printf '%s' \"$line\")\nobserved=$(/bin/cat critic-frame.txt)\n[ \"$expected\" = \"$observed\" ] || exit 1",
    )
}

fn repaired_gate_check() -> String {
    check_script(
        "observed=$(/bin/cat critic-frame.txt)\n[ \"$observed\" = 'opinions cannot block' ] || exit 1",
    )
}

fn broken_rule_check() -> String {
    check_script(
        "if /usr/bin/grep -qx 'reject-opinion' rules.conf; then\n  /usr/bin/grep -qx 'opinion' finding.txt || exit 1\nfi",
    )
}

fn repaired_rule_check() -> String {
    check_script(
        "/usr/bin/grep -qx 'reject-opinion' rules.conf || exit 1\n/usr/bin/grep -qx 'opinion' finding.txt || exit 1",
    )
}

fn check_script(body: &str) -> String {
    format!(
        "#!/bin/sh\n/bin/rm -f \"$1\"\n{body}\nprintf '%s\\n' '{}' > \"$1\"\n",
        APPROVAL.trim_end()
    )
}
