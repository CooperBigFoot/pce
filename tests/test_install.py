from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
INSTALLER = ROOT / "install.sh"
SKILLS = (
    "grill-me",
    "to-vision",
    "implement-vision",
    "chart-program",
    "grill-ticket",
    "land-ticket",
)
ENVIRONMENT_ROOTS = (".claude/skills", ".codex/skills", ".prime/agent/skills")
MATRIX = {
    f"{root}/{name}": ROOT / "skills" / name
    for root in ENVIRONMENT_ROOTS
    for name in SKILLS
}


class InstallTests(unittest.TestCase):
    def run_installer(self, home: Path) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["HOME"] = str(home)
        return subprocess.run(
            [str(INSTALLER)],
            cwd=ROOT,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
        )

    def assert_matrix(self, home: Path) -> None:
        for relative, expected in MATRIX.items():
            link = home / relative
            self.assertTrue(link.is_symlink(), relative)
            self.assertEqual(link.resolve(), expected.resolve())

    def test_installs_exact_matrix_and_safe_rerun(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            unrelated = home / ".claude/skills/unrelated"
            unrelated.parent.mkdir(parents=True)
            unrelated.write_text("keep", encoding="utf-8")

            first = self.run_installer(home)
            second = self.run_installer(home)

            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assert_matrix(home)
            self.assertEqual(unrelated.read_text(encoding="utf-8"), "keep")
            installed = {
                path.relative_to(home).as_posix()
                for path in home.rglob("*")
                if path.is_symlink()
            }
            self.assertEqual(installed, set(MATRIX))

    def test_removes_only_proven_owned_legacy_links(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            owned_skill = home / ".codex/skills/overseer"
            owned_skill.parent.mkdir(parents=True)
            owned_skill.symlink_to(ROOT / "skills/overseer", target_is_directory=True)
            prior_link = home / ".claude/skills/implement-vision"
            prior_link.parent.mkdir(parents=True)
            prior_link.symlink_to(
                ROOT / "skills/implement-vision", target_is_directory=True
            )
            owned_binary = home / ".local/bin/pce"
            owned_binary.parent.mkdir(parents=True)
            owned_binary.symlink_to(ROOT / "target/release/pce")
            foreign_target = home / "foreign-skill"
            foreign_target.mkdir()
            foreign_link = home / ".claude/skills/overseer"
            foreign_link.parent.mkdir(parents=True, exist_ok=True)
            foreign_link.symlink_to(foreign_target, target_is_directory=True)
            copied = home / ".local/bin/pce-rehydrate"
            copied.write_text("copied", encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(os.path.lexists(owned_skill))
            self.assertTrue(prior_link.is_symlink())
            self.assertEqual(prior_link.resolve(), (ROOT / "skills/implement-vision").resolve())
            self.assertFalse(os.path.lexists(owned_binary))
            self.assertTrue(foreign_link.is_symlink())
            self.assertEqual(foreign_link.resolve(), foreign_target.resolve())
            self.assertEqual(copied.read_text(encoding="utf-8"), "copied")
            self.assertIn(str(home.resolve() / ".claude/skills/overseer"), result.stderr)
            self.assertIn(str(copied.resolve()), result.stderr)

    def test_retires_only_owned_grill_with_docs_links(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            owned = home / ".claude/skills/grill-with-docs"
            owned.parent.mkdir(parents=True)
            owned.symlink_to(ROOT / "skills/grill-with-docs", target_is_directory=True)
            foreign_target = home / "foreign-grill-with-docs"
            foreign_target.mkdir()
            foreign = home / ".codex/skills/grill-with-docs"
            foreign.parent.mkdir(parents=True)
            foreign.symlink_to(foreign_target, target_is_directory=True)

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(os.path.lexists(owned))
            self.assertTrue(foreign.is_symlink())
            self.assertEqual(foreign.resolve(), foreign_target.resolve())
            self.assertIn(str(foreign), result.stderr)
            self.assert_matrix(home)

    def test_conflict_refuses_before_creating_any_matrix_link(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            conflict = home / ".codex/skills/to-vision"
            conflict.mkdir(parents=True)
            marker = conflict / "keep"
            marker.write_text("unrelated", encoding="utf-8")

            result = self.run_installer(home)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"ERROR: conflict: {conflict.resolve()} is a directory", result.stderr)
            self.assertIn(f"Move or remove {conflict.resolve()}", result.stderr)
            self.assertEqual(marker.read_text(encoding="utf-8"), "unrelated")
            self.assertFalse(os.path.lexists(home / ".claude/skills/grill-me"))

    def test_cleans_owned_session_start_hook_and_preserves_other_settings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            settings = {
                "theme": "dark",
                "hooks": {
                    "SessionStart": [
                        {
                            "matcher": "resume|compact",
                            "hooks": [
                                {"type": "command", "command": "$HOME/.local/bin/pce-rehydrate"},
                                {"type": "command", "command": "echo keep"},
                            ],
                        },
                        {"matcher": "startup", "hooks": [{"type": "command", "command": "other"}]},
                    ],
                    "PreToolUse": [{"hooks": [{"type": "command", "command": "keep-pre"}]}],
                },
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            actual = json.loads(settings_path.read_text(encoding="utf-8"))
            self.assertEqual(actual["theme"], "dark")
            commands = [
                hook["command"]
                for group in actual["hooks"]["SessionStart"]
                for hook in group.get("hooks", [])
            ]
            self.assertEqual(commands, ["echo keep", "other"])
            self.assertEqual(actual["hooks"]["PreToolUse"], settings["hooks"]["PreToolUse"])

    def test_removes_owned_absolute_hook_with_symlink_spelled_home(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            real_home = base / "real-home"
            real_home.mkdir()
            home = base / "home-alias"
            home.symlink_to(real_home, target_is_directory=True)

            owned_binary = home / ".local/bin/pce-rehydrate"
            owned_binary.parent.mkdir(parents=True)
            owned_binary.symlink_to(ROOT / "target/release/pce-rehydrate")
            owned_command = str(home / ".local/bin/pce-rehydrate")
            ambiguous_command = str(home / ".local/bin/pce")
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            settings = {
                "hooks": {
                    "SessionStart": [
                        {
                            "hooks": [
                                {"type": "command", "command": owned_command},
                                {"type": "command", "command": ambiguous_command},
                                {"type": "command", "command": "echo keep"},
                            ]
                        }
                    ]
                }
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(os.path.lexists(owned_binary))
            actual = json.loads(settings_path.read_text(encoding="utf-8"))
            commands = [
                hook["command"]
                for group in actual["hooks"]["SessionStart"]
                for hook in group.get("hooks", [])
            ]
            self.assertEqual(commands, [ambiguous_command, "echo keep"])
            self.assertIn(f"Removed retired PCE SessionStart hook: {owned_command}", result.stdout)
            self.assertIn(repr(ambiguous_command), result.stderr)

    def test_removes_owned_protection_hook_and_preserves_other_pre_tool_hooks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            owned_binary = home / ".local/bin/pce-protect-criteria"
            owned_binary.parent.mkdir(parents=True)
            owned_binary.symlink_to(ROOT / "hooks/pce-protect-criteria.sh")
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            settings = {
                "theme": "dark",
                "hooks": {
                    "PreToolUse": [
                        {
                            "matcher": "Bash|Edit|Write",
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": "$HOME/.local/bin/pce-protect-criteria",
                                },
                                {"type": "command", "command": "echo keep-pre"},
                            ],
                        }
                    ],
                    "SessionStart": [
                        {"hooks": [{"type": "command", "command": "echo keep-session"}]}
                    ],
                },
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(os.path.lexists(owned_binary))
            actual = json.loads(settings_path.read_text(encoding="utf-8"))
            pre_commands = [
                hook["command"]
                for group in actual["hooks"]["PreToolUse"]
                for hook in group.get("hooks", [])
            ]
            self.assertEqual(pre_commands, ["echo keep-pre"])
            self.assertEqual(actual["hooks"]["SessionStart"], settings["hooks"]["SessionStart"])
            self.assertEqual(actual["theme"], "dark")

    def test_preserves_foreign_protection_binary_and_hook(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            foreign_binary = home / ".local/bin/pce-protect-criteria"
            foreign_binary.parent.mkdir(parents=True)
            foreign_binary.write_text("foreign", encoding="utf-8")
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            settings = {
                "hooks": {
                    "PreToolUse": [
                        {
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": "$HOME/.local/bin/pce-protect-criteria",
                                },
                                {"type": "command", "command": "echo keep"},
                            ]
                        }
                    ]
                }
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(foreign_binary.read_text(encoding="utf-8"), "foreign")
            actual = json.loads(settings_path.read_text(encoding="utf-8"))
            self.assertEqual(actual, settings)
            self.assertIn("preserved ambiguous PreToolUse hook", result.stderr)

    def test_preserves_unrelated_hook_whose_command_is_inside_checkout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            command = str(ROOT / "README.md")
            settings = {
                "hooks": {
                    "SessionStart": [
                        {"hooks": [{"type": "command", "command": command}]}
                    ]
                }
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                json.loads(settings_path.read_text(encoding="utf-8")), settings
            )
            self.assertNotIn("Removed retired PCE SessionStart hook", result.stdout)

    def test_removes_exact_known_retired_hook_source_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            retired = str(ROOT / "hooks/pce-rehydrate.sh")
            keep = str(ROOT / "unrelated-tooling/pce-rehydrate")
            settings = {
                "hooks": {
                    "SessionStart": [
                        {
                            "hooks": [
                                {"type": "command", "command": retired},
                                {"type": "command", "command": keep},
                            ]
                        }
                    ]
                }
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            actual = json.loads(settings_path.read_text(encoding="utf-8"))
            self.assertEqual(
                actual["hooks"]["SessionStart"],
                [{"hooks": [{"type": "command", "command": keep}]}],
            )
            self.assertIn(f"Removed retired PCE SessionStart hook: {retired}", result.stdout)

    def test_preserves_unrelated_retired_basename_hook_inside_checkout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            command = str(ROOT / "unrelated-tooling/pce-rehydrate")
            settings = {
                "hooks": {
                    "SessionStart": [
                        {"hooks": [{"type": "command", "command": command}]}
                    ]
                }
            }
            settings_path.write_text(json.dumps(settings), encoding="utf-8")

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                json.loads(settings_path.read_text(encoding="utf-8")), settings
            )
            self.assertNotIn("Removed retired PCE SessionStart hook", result.stdout)

    def test_malformed_settings_are_reported_and_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            settings_path = home / ".claude/settings.json"
            settings_path.parent.mkdir(parents=True)
            original = b"{not-json\n"
            settings_path.write_bytes(original)

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(settings_path.read_bytes(), original)
            self.assertIn("preserved ambiguous Claude settings", result.stderr)
            self.assert_matrix(home)


if __name__ == "__main__":
    unittest.main()
