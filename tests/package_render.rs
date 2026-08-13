use std::fs;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn package_render_writes_standalone_deterministic_html_and_allows_no_journal() {
    let temp = tempdir().expect("tempdir");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, br#"{"vision":"cli-render","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"proof","input":"tree","observation":"passes","command":"true"}],"depends_on":[]}]}"#).expect("graph");
    let first = temp.path().join("first.html");
    let second = temp.path().join("second.html");

    for output in [&first, &second] {
        let result = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(["package", "render", "--graph"])
            .arg(&graph)
            .arg("--output")
            .arg(output)
            .output()
            .expect("invoke pce");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let first_bytes = fs::read(&first).expect("first output");
    assert_eq!(first_bytes, fs::read(&second).expect("second output"));
    let html = String::from_utf8(first_bytes).expect("utf8 html");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("data-package=\"A\""));
    for forbidden in ["http:", "https:", "file:", "@import", "<script", " src=\""] {
        assert!(
            !html.contains(forbidden),
            "external/runtime reference: {forbidden}"
        );
    }
    assert!(!html.contains("://"));
    assert!(html.contains(":root{--ground:#F1F4F2"));
    assert!(html.contains(":root:not([data-theme=\"light\"])"));
    assert!(html.contains(":root[data-theme=\"dark\"]"));
    assert!(html.contains("body{background:var(--ground)"));
    assert!(html.contains(".graph-scroll{overflow-x:auto"));
}

#[test]
fn package_render_does_not_silently_accept_a_misspelled_journal_path() {
    let temp = tempdir().expect("tempdir");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, br#"{"vision":"cli-render","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"proof","input":"tree","observation":"passes","command":"true"}],"depends_on":[] }]}"#).expect("graph");
    let result = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "render", "--graph"])
        .arg(&graph)
        .arg("--journal")
        .arg(temp.path().join("missing.jsonl"))
        .arg("--output")
        .arg(temp.path().join("output.html"))
        .output()
        .expect("invoke pce");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("failed to read render journal"));
}
