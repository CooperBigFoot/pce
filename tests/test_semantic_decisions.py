from __future__ import annotations

import importlib.util
import io
from http.client import IncompleteRead
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
from contextlib import contextmanager
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "skills/implement-vision/scripts/semantic_decisions.py"
SPEC = importlib.util.spec_from_file_location("semantic_decisions", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def excerpt(text="Public example", sharing="public"):
    return {"text": text, "sharing": sharing, "reviewed": True}


def item(kind="context", ref="e1"):
    return {"id": ref, "kind": kind, "subject": excerpt("Preserve user state"),
            "candidate": excerpt("Existing source and observed test output")}


class Response(io.BytesIO):
    pass


class Transport:
    """Fixture at HTTP opener boundary; exercise real request and answer processing."""
    def __init__(self, choice=None, confidence=0.95, error=None, mutate=None):
        self.calls = []
        self.choice = choice
        self.confidence = confidence
        self.error = error
        self.mutate = mutate

    def open(self, request, timeout):
        self.calls.append((request, timeout))
        if self.error:
            raise self.error
        payload = json.loads(request.data)
        answers = {}
        for key, question in payload["questions"].items():
            options = list(question["criteria"])
            selected = self.choice if self.choice in options else options[0]
            probabilities = {option: (1.0 if option == selected else 0.0)
                             for option in options}
            answers[key] = {"type": "choice", "choice": selected,
                            "probabilities": probabilities, "confidence": self.confidence}
        result = {"model": "jev-1.13.0", "answers": answers,
                  "usage": {"input_tokens": 10, "output_tokens": 10}}
        if self.mutate:
            self.mutate(result)
        return Response(json.dumps(result).encode())


class SemanticDecisionTests(unittest.TestCase):
    def evaluate(self, items, transport=None, key="fixture-credential"):
        transport = transport or Transport()
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": key}, clear=True), \
                patch.object(MODULE, "build_opener", return_value=transport):
            result = MODULE.evaluate({"items": items})
        return result, transport

    def test_batches_all_decisions_through_actual_request_path(self):
        items = [item(kind, "e" + str(i)) for i, kind in enumerate(
            ("context", "fidelity", "scope", "review", "evidence"))]
        result, transport = self.evaluate(items)
        self.assertEqual(len(transport.calls), 1)
        request, timeout = transport.calls[0]
        self.assertEqual(request.full_url, "https://api.typesafe.ai/v1/systemone")
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(request.get_header("Authorization"), "Bearer fixture-credential")
        self.assertLessEqual(timeout, 10)
        payload = json.loads(request.data)
        self.assertEqual(payload["model"], "jev-1.13.0")
        self.assertEqual(len(payload["state"]), 5)
        self.assertNotIn("sharing", json.dumps(payload))
        self.assertNotIn("reviewed", json.dumps(payload))
        self.assertEqual(result["status"], "advisory")
        self.assertEqual(result["model"], "jev-1.13.0")
        self.assertEqual(len(result["decisions"]), 5)
        self.assertNotIn("Public example", json.dumps(result))
        self.assertNotIn("Existing source", json.dumps(result))
        self.assertNotIn("fixture-credential", json.dumps(result))
        for decision in result["decisions"]:
            self.assertEqual(decision["action"], "inspect")

    def test_unpermitted_and_unreviewed_content_never_enters_transport(self):
        for field in ("subject", "candidate"):
            for change in ({"sharing": "private"}, {"sharing": "unknown"},
                           {"sharing": "permitted"}, {"reviewed": False}):
                entry = item()
                entry[field].update(change)
                result, transport = self.evaluate([entry])
                self.assertFalse(transport.calls)
                self.assertEqual(result["decisions"][0]["action"], "reasoning-agent")
                self.assertEqual(result["status"], "fallback")

    def test_explicit_permission_requires_reference_and_is_not_sent(self):
        entry = item()
        entry["candidate"].update(sharing="permitted", permission="approved-project-policy")
        result, transport = self.evaluate([entry])
        self.assertEqual(result["status"], "advisory")
        self.assertNotIn("approved-project-policy", transport.calls[0][0].data.decode())

    def test_mixed_batch_keeps_denied_content_local_and_all_candidates_available(self):
        denied = item(ref="e2")
        denied["candidate"] = excerpt("DO NOT TRANSMIT", "private")
        result, transport = self.evaluate([item(), denied])
        self.assertNotIn("DO NOT TRANSMIT", transport.calls[0][0].data.decode())
        self.assertEqual([d["id"] for d in result["decisions"]], ["e1", "e2"])
        self.assertEqual(result["decisions"][1]["action"], "reasoning-agent")

    def test_missing_credentials_no_network_and_safe_fallback(self):
        result, transport = self.evaluate([item()], key="")
        self.assertFalse(transport.calls)
        self.assertEqual(result["reason"], "credentials-unavailable")
        self.assertEqual(result["decisions"][0]["action"], "reasoning-agent")

    def test_known_secret_patterns_and_actual_key_block_network(self):
        for value in ("TYPESAFE_API_KEY=example", "Authorization: Bearer abc",
                      "-----BEGIN PRIVATE KEY-----", "fixture-credential"):
            entry = item()
            entry["candidate"]["text"] = value
            result, transport = self.evaluate([entry])
            self.assertFalse(transport.calls)
            self.assertNotIn(value, json.dumps(result))

    def test_uncertainty_and_no_match_return_to_reasoning_agent(self):
        for fixture in (Transport(confidence=0.2), Transport(choice="insufficient")):
            result, _ = self.evaluate([item()], fixture)
            self.assertEqual(result["decisions"][0]["action"], "reasoning-agent")
            self.assertTrue(result["decisions"][0]["answers"])

    def test_malformed_responses_are_safely_rejected(self):
        mutations = [
            lambda r: r.update(model="unexpected sensitive server text"),
            lambda r: r.update(answers={}),
            lambda r: r["answers"].update(extra={}),
            lambda r: next(iter(r["answers"].values())).update(choice="invented"),
            lambda r: next(iter(r["answers"].values())).update(type="noul"),
            lambda r: next(iter(r["answers"].values())).update(confidence=float("nan")),
            lambda r: next(iter(r["answers"].values())).update(confidence=True),
            lambda r: next(iter(r["answers"].values())).update(probabilities={}),
        ]
        for mutate in mutations:
            result, transport = self.evaluate([item()], Transport(mutate=mutate))
            self.assertEqual(result["status"], "fallback")
            self.assertEqual(len(transport.calls), 1)
            self.assertEqual(result["decisions"][0]["action"], "reasoning-agent")

    def test_service_failures_never_retry_or_echo_error(self):
        for error in (IncompleteRead(b"private", 10), URLError("private server content"), TimeoutError("private"),
                      HTTPError("https://api.typesafe.ai", 401, "private", {}, None),
                      HTTPError("https://api.typesafe.ai", 429, "private", {}, None),
                      HTTPError("https://api.typesafe.ai", 529, "private", {}, None)):
            result, transport = self.evaluate([item()], Transport(error=error))
            self.assertEqual(result["status"], "fallback")
            self.assertNotIn("private", json.dumps(result))
            self.assertEqual(len(transport.calls), 1)

    def test_negative_scores_cannot_remove_context_or_waive_gates(self):
        for kind, choice in (("context", "unrelated"), ("review", "absent"),
                             ("evidence", "direct"), ("fidelity", "preserved")):
            result, _ = self.evaluate([item(kind)], Transport(choice=choice))
            self.assertEqual(len(result["decisions"]), 1)
            self.assertEqual(result["decisions"][0]["action"], "inspect")
            self.assertEqual(result["authority"], "advisory-only")
            self.assertTrue(result["retain_all_candidates"])
            self.assertEqual(result["required_checks"], MODULE.REQUIRED_CHECKS)

    def test_invalid_inputs_fail_closed_without_echo(self):
        entries = [[], [item(), item()], [dict(item(), kind="approve-merge")],
                   [dict(item(), id="private filename / secret")]]
        for entries_case in entries:
            result, transport = self.evaluate(entries_case)
            self.assertFalse(transport.calls)
            self.assertEqual(result["status"], "fallback")
            self.assertNotIn("private filename", json.dumps(result))

    def test_http_error_stream_is_closed_on_fallback(self):
        body = io.BytesIO(b"private server text")
        error = HTTPError("https://api.typesafe.ai", 401, "private", {}, body)
        result, _ = self.evaluate([item()], Transport(error=error))
        self.assertEqual(result["status"], "fallback")
        self.assertTrue(body.closed)

    def test_cli_uses_stdin_not_dotenv_and_emits_safe_json(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, ".env").write_text("TYPESAFE_API_KEY=must-not-read")
            environment = os.environ.copy()
            environment.pop("TYPESAFE_API_KEY", None)
            result = subprocess.run([sys.executable, str(SCRIPT)], cwd=directory,
                                    input=json.dumps({"items": [item()]}), text=True,
                                    capture_output=True, env=environment)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(json.loads(result.stdout)["reason"], "credentials-unavailable")
            self.assertEqual(result.stderr, "")
            self.assertNotIn("must-not-read", result.stdout)

@contextmanager
def http_fixture(mode="useful"):
    """Loopback contract server, never a paid or external inference request."""
    calls = []

    class ContractEndpoint(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            payload = self.rfile.read(int(self.headers["Content-Length"]))
            calls.append((self.path, self.headers.get("Authorization"), payload))
            if mode == "redirect":
                self.send_response(307)
                self.send_header("Location", "/credential-sink")
                self.end_headers()
                return
            if mode == "failure":
                self.send_response(503)
                self.end_headers()
                self.wfile.write(b"private error detail")
                return
            if mode == "incomplete":
                self.send_response(200)
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                self.wfile.write(b"20\r\nshort")
                return
            if mode == "malformed":
                data = b"not JSON with private content"
            elif mode == "oversized":
                data = b"x" * (MODULE.MAX_BYTES + 1)
            else:
                request = MODULE.Request("https://fixture.invalid", data=payload)
                data = Transport().open(request, 8).read()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    server = ThreadingHTTPServer(("127.0.0.1", 0), ContractEndpoint)
    thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
    thread.start()
    try:
        yield "http://127.0.0.1:" + str(server.server_port) + "/v1/systemone", calls
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


class SemanticWorkflowTests(unittest.TestCase):
    # Stages are instructions, not a second workflow runtime. Execute their real
    # question builders and HTTP code with contract fixtures, then verify wiring.
    STAGES = {
        "to-vision": ("context", "fidelity", "scope"),
        "grill-ticket": ("context", "fidelity", "scope"),
        "chart-program": ("context", "fidelity", "scope"),
        "implement-vision": ("context", "review", "evidence"),
        "land-ticket": ("evidence",),
    }

    def test_all_automatic_checkpoints_execute_real_http_contract(self):
        with http_fixture() as (url, calls), \
                patch.object(MODULE, "ENDPOINT", url), \
                patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-only"}, clear=True):
            for skill, kinds in self.STAGES.items():
                text = (ROOT / "skills" / skill / "SKILL.md").read_text()
                for kind in kinds:
                    self.assertIn("`" + kind + "`", text)
                batch = {"items": [item(kind, "e" + str(i)) for i, kind in enumerate(kinds)]}
                result = MODULE.evaluate(batch)
                self.assertEqual(result["status"], "advisory")
                self.assertEqual(result["authority"], "advisory-only")
                self.assertEqual([d["kind"] for d in result["decisions"]], list(kinds))
            self.assertEqual(len(calls), len(self.STAGES))
            for path, authorization, raw in calls:
                self.assertEqual(path, "/v1/systemone")
                self.assertEqual(authorization, "Bearer contract-only")
                request = json.loads(raw)
                for question in request["questions"].values():
                    self.assertEqual(question["type"], "choice")
                    self.assertIn("insufficient", question["criteria"])
                    self.assertIn(".subject`", question["instructions"])
                    self.assertIn(".candidate`", question["instructions"])

    def test_real_http_failures_and_redirects_do_not_retry_or_leak(self):
        for mode in ("redirect", "failure", "malformed", "oversized", "incomplete"):
            with self.subTest(mode=mode), http_fixture(mode) as (url, calls), \
                    patch.object(MODULE, "ENDPOINT", url), \
                    patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-only"}, clear=True):
                result = MODULE.evaluate({"items": [item()]})
                self.assertEqual(result["status"], "fallback")
                self.assertEqual(len(calls), 1)
                self.assertEqual(calls[0][0], "/v1/systemone")
                self.assertNotIn("private", json.dumps(result))
                self.assertNotIn("contract-only", json.dumps(result))
                self.assertEqual(result["decisions"][0]["action"], "reasoning-agent")

    def test_privacy_and_missing_access_never_reach_real_http_server(self):
        with http_fixture() as (url, calls), patch.object(MODULE, "ENDPOINT", url):
            with patch.dict(os.environ, {}, clear=True):
                result = MODULE.evaluate({"items": [item()]})
                self.assertEqual(result["reason"], "credentials-unavailable")
            entry = item()
            entry["subject"]["sharing"] = "private"
            with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-only"}, clear=True):
                result = MODULE.evaluate({"items": [entry]})
                self.assertEqual(result["reason"], "content-not-permitted")
            self.assertEqual(calls, [])

    def test_installed_paths_work_for_all_environments_without_installer_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            environment = os.environ.copy()
            environment["HOME"] = str(home)
            environment.pop("TYPESAFE_API_KEY", None)
            unrelated = home / ".prime/agent/skills/other"
            unrelated.mkdir(parents=True)
            marker = unrelated / "keep"
            marker.write_text("preserve")
            installed = subprocess.run([str(ROOT / "install.sh")], env=environment,
                                       text=True, capture_output=True)
            self.assertEqual(installed.returncode, 0, installed.stderr)
            for environment_path in (".claude/skills", ".codex/skills", ".prime/agent/skills"):
                for skill, kinds in self.STAGES.items():
                    # Follow precisely the shared guide: resolve skill first.
                    installed_skill = (home / environment_path / skill).resolve()
                    shared = installed_skill.parent / "implement-vision"
                    self.assertTrue((shared / "semantic-decisions.md").is_file())
                    helper = shared / "scripts/semantic_decisions.py"
                    self.assertEqual(helper, SCRIPT)
                    result = subprocess.run([sys.executable, str(helper)], env=environment,
                        input=json.dumps({"items": [item(kinds[0])]}), text=True,
                        capture_output=True, cwd=home)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout)["reason"], "credentials-unavailable")
            self.assertEqual(marker.read_text(), "preserve")

    def test_stage_placement_keeps_existing_workflow_gates(self):
        author = (ROOT / "skills/to-vision/SKILL.md").read_text()
        implement = (ROOT / "skills/implement-vision/SKILL.md").read_text()
        land = (ROOT / "skills/land-ticket/SKILL.md").read_text()
        self.assertLess(author.index("## Automatic semantic checks"), author.index("## Draft-only output"))
        self.assertLess(author.index("## Automatic semantic checks"), author.index("## Publish and verify"))
        self.assertLess(implement.index("## Validate the resolved input"),
                        implement.index("## Automatic semantic decisions"))
        self.assertLess(implement.index("## Automatic semantic decisions"), implement.index("## Plan the outcome"))
        self.assertLess(land.index("## Automatic evidence matching"), land.index("## Recovery hierarchy"))
        guide = (ROOT / "skills/implement-vision/semantic-decisions.md").read_text()
        for phrase in ("full-vision/full-diff", "once per root invocation", "complete vision",
                       "negative signal never", "Git/GitHub/visions", "actual resulting"):
            self.assertIn(phrase, guide)
        grill = (ROOT / "skills/grill-me/SKILL.md").read_text()
        self.assertNotIn("Jev", grill)


if __name__ == "__main__":
    unittest.main()
