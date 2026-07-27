use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Error, Result, anyhow, bail};
use pce_core::{
    AppendError, CreationDate, EventLogTail, EventLogTailLine, NodeId, UnparsedPayload, VisionName,
    WriteKind, append_event, create_vision,
};

const USAGE: &str = concat!(
    "usage: pce vision new \"<name>\"\n",
    "       pce log --file <LOG_PATH> --kind <KIND> --node <NODE>"
);

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let mut input = std::io::stdin().lock();
    run(std::env::args().skip(1), &mut input)
}

fn run(args: impl Iterator<Item = String>, input: &mut dyn Read) -> Result<()> {
    let args: Vec<String> = args.collect();
    match args.as_slice() {
        [verb, action, raw_name] if verb == "vision" && action == "new" => run_vision_new(raw_name),
        [
            verb,
            file_flag,
            raw_path,
            kind_flag,
            raw_kind,
            node_flag,
            raw_node,
        ] if verb == "log"
            && file_flag == "--file"
            && kind_flag == "--kind"
            && node_flag == "--node" =>
        {
            run_log(raw_path, raw_kind, raw_node, input)
        }
        _ => bail!(USAGE),
    }
}

fn run_vision_new(raw_name: &str) -> Result<()> {
    let name = VisionName::parse(raw_name).context("failed to parse vision name")?;
    let new_vision = create_vision(&name, Path::new("planning"), CreationDate::today())
        .context("failed to create vision")?;
    println!("{}", new_vision.dir());

    Ok(())
}

fn run_log(raw_path: &str, raw_kind: &str, raw_node: &str, input: &mut dyn Read) -> Result<()> {
    let path = PathBuf::from(raw_path);
    let kind = WriteKind::parse(raw_kind).context("failed to parse event kind")?;
    let node = NodeId::parse(raw_node).context("failed to parse event node")?;

    let mut payload = String::new();
    input
        .read_to_string(&mut payload)
        .context("failed to read event payload from stdin to EOF")?;

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open or create event log {}", path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", path.display()))?;

    let operation = append_locked(&mut file, payload, kind, node, &path);
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", path.display()));

    match (operation, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(unlock_error)) => Err(unlock_error),
        (Err(primary), Err(unlock_error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {unlock_error:#}"
        ))),
    }
}

fn append_locked(
    file: &mut File,
    payload: String,
    kind: WriteKind,
    node: NodeId,
    path: &Path,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to seek event log {} for tail read", path.display()))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .with_context(|| format!("failed to read event-log tail from {}", path.display()))?;
    let content = String::from_utf8(content)
        .with_context(|| format!("event-log tail in {} is not valid UTF-8", path.display()))?;
    let tail = match content.lines().last() {
        Some(line) => EventLogTail::Present(EventLogTailLine::new(line)),
        None if content.is_empty() => EventLogTail::Empty,
        None => EventLogTail::Present(EventLogTailLine::new("")),
    };

    append_event::<std::io::Error, _>(
        kind,
        UnparsedPayload::new(payload),
        tail,
        node,
        SystemTime::now(),
        |bytes| {
            file.seek(SeekFrom::End(0))?;
            file.write_all(bytes)
        },
    )
    .map_err(classify_append_error)?;

    file.sync_all()
        .with_context(|| format!("failed to sync event log {}", path.display()))?;
    Ok(())
}

fn classify_append_error(error: AppendError<std::io::Error>) -> Error {
    let context = match &error {
        AppendError::MalformedSubmittedPayload { .. } => {
            "failed to parse submitted event payload JSON"
        }
        AppendError::InvalidSubmittedPayload { .. } => "failed to validate submitted event payload",
        AppendError::InvalidTail { .. } => "failed to validate current event-log tail",
        AppendError::SequenceOverflow { .. } => {
            "failed to derive successor from current event-log tail"
        }
        AppendError::SerializationFailed { .. } => "failed to serialize validated event record",
        AppendError::CapabilityFailed { .. } => "failed to append event record",
    };
    anyhow!(error).context(context)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Read};
    use std::path::{Path, PathBuf};

    use pce_core::{KnownPayload, ReadKind, ReadPayload, WriteKind, parse_event_line};
    use tempfile::tempdir;

    use crate::run;

    const DELTA_PAYLOAD: &str = r#"{"message":"append one validated event"}"#;
    const UNKNOWN_TAIL: &str = r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"future-kind","node":"m1-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#;
    const MULTILINE_PAYLOAD: &str = "{\n  \"finding\":\"the measured fact\",\n  \"evidence\":\"git rev-parse HEAD\\ncargo test --workspace\"\n}";

    fn log_args(path: &Path, kind: &str) -> Vec<String> {
        vec![
            "log".to_owned(),
            "--file".to_owned(),
            path.to_string_lossy().into_owned(),
            "--kind".to_owned(),
            kind.to_owned(),
            "--node".to_owned(),
            "m1-s3".to_owned(),
        ]
    }

    fn invoke(path: &Path, kind: &str, payload: &str) -> anyhow::Result<()> {
        let mut input = Cursor::new(payload.as_bytes());
        run(log_args(path, kind).into_iter(), &mut input)
    }

    fn bytes_if_present(path: &Path) -> Vec<u8> {
        if path.exists() {
            fs::read(path).expect("log should be readable")
        } else {
            Vec::new()
        }
    }

    #[test]
    fn consecutive_writes_append_exact_five_key_envelopes() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("events.jsonl");

        invoke(&path, "delta", DELTA_PAYLOAD).expect("first append should succeed");
        invoke(&path, "delta", DELTA_PAYLOAD).expect("second append should succeed");

        let content = fs::read_to_string(&path).expect("log should be UTF-8");
        assert!(content.ends_with('\n'));
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for (line, sequence) in lines.iter().zip([1, 2]) {
            assert!(line.starts_with(&format!(r#"{{"sequence":{sequence},"timestamp":""#)));
            assert!(line.ends_with(
                r#"","kind":"delta","node":"m1-s3","payload":{"message":"append one validated event"}}"#
            ));
            parse_event_line(line).expect("written line should parse");
        }
    }

    #[test]
    fn accepts_all_seven_registered_payload_schemas() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            (
                "dispatch",
                r#"{"role":"step-executor","ref":"ca9788ded3daec9b9e9fd7679caa24e7c64a8193","evidence":"git rev-parse HEAD"}"#,
                WriteKind::Dispatch,
            ),
            (
                "delta",
                r#"{"message":"Require exact UTC timestamp spelling in the event envelope."}"#,
                WriteKind::Delta,
            ),
            (
                "escalation-open",
                r#"{"key":"timestamp-precision","question":"Which RFC 3339 sub-second precision is canonical?"}"#,
                WriteKind::EscalationOpen,
            ),
            (
                "escalation-close",
                r#"{"key":"timestamp-precision","resolution":"Use milliseconds and a Z suffix."}"#,
                WriteKind::EscalationClose,
            ),
            (
                "key-finding",
                r##"{"finding":"The repository has exactly five tests at the ground-truth ref.","evidence":"git grep -n '#[test]' ca9788ded3daec9b9e9fd7679caa24e7c64a8193 -- crates/core/src/vision.rs"}"##,
                WriteKind::KeyFinding,
            ),
            (
                "repository-contract",
                r#"{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}"#,
                WriteKind::RepositoryContract,
            ),
            (
                "planning-artifact-approved",
                r#"{"path":"planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","evidence":"shasum -a 256 planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json"}"#,
                WriteKind::PlanningArtifactApproved,
            ),
        ];

        for (index, (kind, payload, expected_kind)) in fixtures.into_iter().enumerate() {
            let path = directory.path().join(format!("event-{index}.jsonl"));
            invoke(&path, kind, payload).expect("registered payload should append");
            let content = fs::read_to_string(path).expect("log should be readable");
            let record = parse_event_line(content.trim_end_matches('\n'))
                .expect("registered output should parse");
            assert_eq!(record.kind(), ReadKind::Known(expected_kind));
        }
    }

    #[test]
    fn rejected_kinds_and_payloads_append_no_bytes() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            ("key-finding", r#"{"finding":"no evidence supplied"}"#),
            (
                "delta",
                r#"{"message":"wrong evidence","evidence":"must be absent"}"#,
            ),
            ("future-kind", DELTA_PAYLOAD),
            ("delta", "{"),
            ("delta", "{}"),
        ];

        for (index, (kind, payload)) in fixtures.into_iter().enumerate() {
            let path = directory.path().join(format!("rejected-{index}.jsonl"));
            assert!(invoke(&path, kind, payload).is_err());
            assert_eq!(bytes_if_present(&path), Vec::<u8>::new());
        }
    }

    #[test]
    fn malformed_tails_are_preserved_byte_for_byte() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            "{",
            r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"delta","node":"m1-s1"}"#,
        ];

        for (index, tail) in fixtures.into_iter().enumerate() {
            let path = directory
                .path()
                .join(format!("malformed-tail-{index}.jsonl"));
            fs::write(&path, tail).expect("tail fixture should seed");
            let before = fs::read(&path).expect("seed should read");
            assert!(invoke(&path, "delta", DELTA_PAYLOAD).is_err());
            assert_eq!(fs::read(&path).expect("log should read"), before);
        }
    }

    #[test]
    fn unknown_kind_tail_remains_readable_and_yields_successor_42() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("unknown-tail.jsonl");
        let seeded = format!("{UNKNOWN_TAIL}\n");
        fs::write(&path, seeded.as_bytes()).expect("unknown tail should seed");

        invoke(&path, "delta", DELTA_PAYLOAD).expect("successor should append");

        let content = fs::read_to_string(&path).expect("log should read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(format!("{}\n", lines[0]).as_bytes(), seeded.as_bytes());
        let original = parse_event_line(lines[0]).expect("unknown record should remain readable");
        assert_eq!(original.kind(), ReadKind::Unknown("future-kind".to_owned()));
        let successor = parse_event_line(lines[1]).expect("successor should parse");
        assert_eq!(successor.sequence().get(), 42);
        assert_eq!(successor.kind(), ReadKind::Known(WriteKind::Delta));
    }

    #[test]
    fn multiline_stdin_compacts_and_round_trips_logical_newline() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("multiline.jsonl");

        invoke(&path, "key-finding", MULTILINE_PAYLOAD).expect("multiline payload should append");

        let content = fs::read_to_string(&path).expect("log should read");
        assert!(content.ends_with('\n'));
        assert_eq!(content.lines().count(), 1);
        let record = parse_event_line(content.trim_end_matches('\n')).expect("record should parse");
        let ReadPayload::Known(KnownPayload::KeyFinding(payload)) = record.payload() else {
            panic!("record should contain a key-finding payload");
        };
        assert_eq!(
            payload.evidence.as_str(),
            "git rev-parse HEAD\ncargo test --workspace"
        );
    }

    #[test]
    fn vision_routing_does_not_read_injected_input() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                panic!("vision command must not read stdin")
            }
        }

        let args = vec!["vision".to_owned(), "new".to_owned(), String::new()];
        let error = run(args.into_iter(), &mut FailingReader)
            .expect_err("empty vision name should fail before filesystem work");
        assert!(error.to_string().contains("failed to parse vision name"));
    }

    #[test]
    fn log_parser_requires_literal_flag_order_and_no_payload_argument() {
        let directory = tempdir().expect("temporary directory should create");
        let path: PathBuf = directory.path().join("events.jsonl");
        let mut reordered = log_args(&path, "delta");
        reordered.swap(1, 3);
        let mut input = Cursor::new(DELTA_PAYLOAD.as_bytes());
        assert!(run(reordered.into_iter(), &mut input).is_err());

        let mut extra = log_args(&path, "delta");
        extra.push(DELTA_PAYLOAD.to_owned());
        let mut input = Cursor::new(Vec::<u8>::new());
        assert!(run(extra.into_iter(), &mut input).is_err());
        assert!(!path.exists());
    }
}
