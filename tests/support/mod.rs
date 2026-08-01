use std::collections::{BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const GIT_SHIM: &str = r#"#!/bin/sh
program=git
root=${PCE_SHIM_ROOT:?PCE_SHIM_ROOT is required}
request="$root/request.$$.bin"
trap 'rm -f "$request"' 0 1 2 3 15
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$request" || exit 126
cat "$request" >> "$root/invocations.bin" || exit 126
for entry in "$root"/responses/*; do
    if cmp -s "$request" "$entry/request.bin"; then
        if ! IFS= read -r exit_code < "$entry/exit_code"; then
            printf 'configured response has no exit code: %s\n' "$entry" >&2
            exit 126
        fi
        cat "$entry/stdout.bin" || exit 126
        cat "$entry/stderr.bin" >&2 || exit 126
        exit "$exit_code"
    fi
done
printf 'unconfigured git invocation' >&2
for argument in "$@"; do
    printf ' <%s>' "$argument" >&2
done
printf '\n' >&2
exit 127
"#;

const GH_SHIM: &str = r#"#!/bin/sh
program=gh
root=${PCE_SHIM_ROOT:?PCE_SHIM_ROOT is required}
request="$root/request.$$.bin"
trap 'rm -f "$request"' 0 1 2 3 15
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$request" || exit 126
cat "$request" >> "$root/invocations.bin" || exit 126
for entry in "$root"/responses/*; do
    if cmp -s "$request" "$entry/request.bin"; then
        if ! IFS= read -r exit_code < "$entry/exit_code"; then
            printf 'configured response has no exit code: %s\n' "$entry" >&2
            exit 126
        fi
        cat "$entry/stdout.bin" || exit 126
        cat "$entry/stderr.bin" >&2 || exit 126
        exit "$exit_code"
    fi
done
printf 'unconfigured gh invocation' >&2
for argument in "$@"; do
    printf ' <%s>' "$argument" >&2
done
printf '\n' >&2
exit 127
"#;

#[allow(dead_code)]
const CODEX_SHIM: &str = r#"#!/bin/sh
program=codex
root=${PCE_CODEX_RECORD_ROOT:?PCE_CODEX_RECORD_ROOT is required}
if [ "${PCE_CODEX_SKIP_RECORDING:-}" = 1 ]; then
    while IFS= read -r line; do printf '%s\n' "$line"; done < "$PCE_CODEX_STDOUT_FILE"
    exit "$PCE_CODEX_EXIT_CODE"
fi
mkdir "$root/invocation" || exit 126
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$root/invocation/request.bin" || exit 126
pwd -P > "$root/invocation/cwd.bin" || exit 126
/usr/bin/env -0 > "$root/invocation/environment.bin" || exit 126
cat > "$root/invocation/stdin.bin" || exit 126
printf '%s\n' "$$" > "$root/invocation/pid" || exit 126
if [ -n "${PCE_CODEX_BLOCK_FILE:-}" ]; then
    while [ ! -e "$PCE_CODEX_BLOCK_FILE" ]; do sleep 0.01; done
fi
if [ -n "${PCE_CODEX_SLEEP_SECONDS:-}" ]; then sleep "$PCE_CODEX_SLEEP_SECONDS"; fi
if [ -n "${PCE_CODEX_OUTPUT_BYTES_FILE:-}" ]; then
    output_path=
    previous=
    for argument in "$@"; do
        if [ "$previous" = -o ]; then output_path=$argument; break; fi
        previous=$argument
    done
    [ -n "$output_path" ] || exit 126
    cat "$PCE_CODEX_OUTPUT_BYTES_FILE" > "$output_path" || exit 126
fi
cat "$PCE_CODEX_STDOUT_FILE" || exit 126
cat "$PCE_CODEX_STDERR_FILE" >&2 || exit 126
if [ -n "${PCE_CODEX_SIGNAL:-}" ]; then kill -"$PCE_CODEX_SIGNAL" "$$"; fi
exit "$PCE_CODEX_EXIT_CODE"
"#;

#[derive(Debug)]
pub struct ScriptedResponse {
    pub program: OsString,
    pub argv: Vec<OsString>,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: OsString,
    pub argv: Vec<OsString>,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct CodexInvocation {
    pub program: OsString,
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: BTreeSet<OsString>,
    pub stdin: Vec<u8>,
}

pub struct CliHarness {
    tempdir: TempDir,
    shim_dir: PathBuf,
    responses_dir: PathBuf,
    invocation_log: PathBuf,
}

impl CliHarness {
    pub fn new() -> io::Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let shim_dir = tempdir.path().join("shims");
        let responses_dir = tempdir.path().join("responses");
        let invocation_log = tempdir.path().join("invocations.bin");
        fs::create_dir(&shim_dir)?;
        fs::create_dir(&responses_dir)?;
        fs::write(&invocation_log, [])?;
        write_shim(&shim_dir.join("git"), GIT_SHIM)?;
        write_shim(&shim_dir.join("gh"), GH_SHIM)?;
        write_shim(&shim_dir.join("codex"), CODEX_SHIM)?;

        Ok(Self {
            tempdir,
            shim_dir,
            responses_dir,
            invocation_log,
        })
    }

    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }

    #[allow(dead_code)]
    pub fn shim_path(&self) -> String {
        format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.shim_dir.display())
    }

    pub fn materialize_responses(&self, responses: &[ScriptedResponse]) -> io::Result<()> {
        let mut requests = HashSet::new();
        for (index, response) in responses.iter().enumerate() {
            let request = encode_request(&response.program, &response.argv);
            if !requests.insert(request.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate scripted response request",
                ));
            }

            let entry = self.responses_dir.join(format!("{index:04}"));
            fs::create_dir(&entry)?;
            fs::write(entry.join("request.bin"), request)?;
            fs::write(entry.join("exit_code"), format!("{}\n", response.exit_code))?;
            fs::write(entry.join("stdout.bin"), &response.stdout)?;
            fs::write(entry.join("stderr.bin"), &response.stderr)?;
        }
        Ok(())
    }

    pub fn run<I, S>(&self, argv: I, stdin: &[u8]) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.shim_dir.display());
        let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(argv)
            .env_clear()
            .env("PATH", path)
            .env("PCE_SHIM_ROOT", self.tempdir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("pce child stdin was not piped"))?;
        child_stdin.write_all(stdin)?;
        drop(child_stdin);
        child.wait_with_output()
    }

    #[allow(dead_code)]
    pub fn run_with_stdin_file<I, S>(
        &self,
        argv: I,
        stdin_path: &Path,
        inherited_marker: (&str, &str),
    ) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let stdin = fs::File::open(stdin_path)?;
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(argv)
            .env_clear()
            .env("PATH", self.shim_path())
            .env("PCE_SHIM_ROOT", self.tempdir.path())
            .env(inherited_marker.0, inherited_marker.1)
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    #[allow(dead_code)]
    pub fn codex_invocations(&self, record_root: &Path) -> io::Result<Vec<CodexInvocation>> {
        let invocation = record_root.join("invocation");
        if !invocation.exists() {
            return Ok(Vec::new());
        }
        let request = fs::read(invocation.join("request.bin"))?;
        let parsed = parse_invocations(&request)?;
        let only = parsed.into_iter().next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing Codex invocation")
        })?;
        let cwd = fs::read(invocation.join("cwd.bin"))?;
        let cwd = PathBuf::from(OsString::from_vec(
            cwd.strip_suffix(b"\n").unwrap_or(&cwd).to_vec(),
        ));
        let environment = parse_nul_set(&fs::read(invocation.join("environment.bin"))?);
        let stdin = fs::read(invocation.join("stdin.bin"))?;
        Ok(vec![CodexInvocation {
            program: only.program,
            argv: only.argv,
            cwd,
            environment,
            stdin,
        }])
    }

    pub fn invocations(&self) -> io::Result<Vec<Invocation>> {
        let bytes = fs::read(&self.invocation_log)?;
        parse_invocations(&bytes)
    }
}

#[allow(dead_code)]
fn parse_nul_set(bytes: &[u8]) -> BTreeSet<OsString> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| OsString::from_vec(field.to_vec()))
        .collect()
}

fn write_shim(path: &Path, source: &str) -> io::Result<()> {
    fs::write(path, source)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

fn encode_request(program: &OsStr, argv: &[OsString]) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_field(&mut encoded, program.as_bytes());
    push_field(&mut encoded, argv.len().to_string().as_bytes());
    for argument in argv {
        push_field(&mut encoded, argument.as_bytes());
    }
    encoded
}

fn push_field(encoded: &mut Vec<u8>, field: &[u8]) {
    encoded.extend_from_slice(field);
    encoded.push(0);
}

fn parse_invocations(bytes: &[u8]) -> io::Result<Vec<Invocation>> {
    let mut fields = bytes.split(|byte| *byte == 0);
    let mut invocations = Vec::new();
    loop {
        let Some(program) = fields.next() else {
            break;
        };
        if program.is_empty() {
            if fields.next().is_none() {
                break;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invocation log contains an empty program",
            ));
        }
        let argc = fields
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing argc"))?;
        let argc = std::str::from_utf8(argc)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .parse::<usize>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let argv = (0..argc)
            .map(|_| {
                fields
                    .next()
                    .map(|field| OsString::from_vec(field.to_vec()))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing invocation argument")
                    })
            })
            .collect::<io::Result<Vec<_>>>()?;
        invocations.push(Invocation {
            program: OsString::from_vec(program.to_vec()),
            argv,
        });
    }
    Ok(invocations)
}

// Every current adapter call has at least one argument. With zero arguments, the
// shim's `printf '%s\0' "$@"` would emit one stray NUL unlike this Rust encoder.
