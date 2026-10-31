from __future__ import annotations

import os
from itertools import product
import re
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
    "save-discussion",
    "github-writing",
    "test-first-development",
)
ENVIRONMENT_ROOTS = (".claude/skills", ".codex/skills", ".prime/agent/skills", ".gemini/skills")
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

    def seed_user_state(self, home: Path) -> None:
        for directory in (".prime/agent", ".gemini"):
            agents = home / directory / "AGENTS.md"
            if not os.path.lexists(agents):
                agents.parent.mkdir(parents=True, exist_ok=True)
                agents.write_bytes(b"user rules\r\n")
        link = home / ".claude/skills/save-discussion"
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(ROOT / "skills/save-discussion")
        retired = home / ".gemini/skills/grill-ticket"
        retired.parent.mkdir(parents=True, exist_ok=True)
        retired.symlink_to(ROOT / "skills/grill-ticket")

    def snapshot(self, home: Path) -> dict:
        return {
            str(path.relative_to(home)): (
                path.lstat().st_ino, path.lstat().st_mode,
                os.readlink(path) if path.is_symlink() else
                path.read_bytes() if path.is_file() else None,
            )
            for path in home.rglob("*")
        }

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

    def test_supporting_links_resolve_from_each_installed_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            result = self.run_installer(home)
            self.assertEqual(result.returncode, 0, result.stderr)
            for environment_root in ENVIRONMENT_ROOTS:
                for name in SKILLS:
                    installed = home / environment_root / name
                    for source in (ROOT / "skills" / name).glob("*.md"):
                        path = installed / source.name
                        self.assertEqual(path.read_bytes(), source.read_bytes())
                        targets = re.findall(r"\[[^]]+\]\(([^)]+\.md(?:#[^)]+)?)\)", path.read_text())
                        for target in targets:
                            relative = target.split("#", 1)[0]
                            linked = path.parent / relative
                            self.assertTrue(linked.is_file(), (path, target))
                            self.assertEqual(linked.resolve(), (source.parent / relative).resolve())

    def test_preserves_independently_installed_skills_and_user_configuration(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            installed = []
            for environment_root in ENVIRONMENT_ROOTS:
                skill = home / environment_root / "typesafe-ai"
                skill.mkdir(parents=True)
                marker = skill / "SKILL.md"
                marker.write_text("independently installed", encoding="utf-8")
                installed.append(marker)
            configuration = home / ".env"
            configuration.write_text("UNRELATED_SETTING=fixture\n", encoding="utf-8")
            original = configuration.read_bytes()

            result = self.run_installer(home)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assert_matrix(home)
            for marker in installed:
                self.assertEqual(marker.read_text(encoding="utf-8"), "independently installed")
            self.assertEqual(configuration.read_bytes(), original)

    def test_preserves_files_links_and_settings_outside_install_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            links = [home / ".codex/skills/overseer", home / ".local/bin/pce"]
            for link in links:
                link.parent.mkdir(parents=True, exist_ok=True)
                link.symlink_to(ROOT / "absent-source")
            settings = home / ".claude/settings.json"
            settings.parent.mkdir(parents=True)
            for content in (b'{"hooks":{"SessionStart":[{"hooks":[{"command":"$HOME/.local/bin/pce-rehydrate"}]}]}}',
                            b"{not-json\n"):
                with self.subTest(content=content):
                    settings.write_bytes(content)
                    result = self.run_installer(home)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(settings.read_bytes(), content)
                    for link in links:
                        self.assertTrue(link.is_symlink())
                        self.assertEqual(os.readlink(link), str(ROOT / "absent-source"))
                    self.assert_matrix(home)

    def test_conflicts_are_preserved_before_any_link_is_created(self) -> None:
        for environment_root, kind in product(
            ENVIRONMENT_ROOTS, ("file", "directory", "foreign-link", "dangling-link")
        ):
            with self.subTest(root=environment_root, kind=kind), tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                conflict = home / environment_root / "to-vision"
                conflict.parent.mkdir(parents=True)
                target = home / "foreign"
                if kind == "directory":
                    conflict.mkdir()
                    (conflict / "keep").write_text("keep")
                elif kind == "file":
                    conflict.write_text("keep")
                else:
                    if kind == "foreign-link":
                        target.mkdir()
                    conflict.symlink_to(target)
                self.seed_user_state(home)
                before = self.snapshot(home)
                result = self.run_installer(home)
                self.assertEqual(self.snapshot(home), before)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("ERROR: conflict:", result.stderr)
                self.assertIn("Move or remove", result.stderr)
                self.assertFalse(os.path.lexists(home / ".claude/skills/grill-me"))
                if kind == "directory":
                    self.assertEqual((conflict / "keep").read_text(), "keep")
                elif kind == "file":
                    self.assertEqual(conflict.read_text(), "keep")
                else:
                    self.assertEqual(os.readlink(conflict), str(target))

    def test_creates_global_task_pointers_once(self) -> None:
        for guidance_root in (".prime/agent", ".gemini"):
            with tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                result = self.run_installer(home)
                self.assertEqual(result.returncode, 0, result.stderr)
                agents = home / guidance_root / "AGENTS.md"
                self.assertTrue(agents.is_file())
                content = agents.read_bytes()
                text = content.decode()
                self.assertEqual(text.count("<!-- pce:guidance:start -->"), 1)
                self.assertEqual(text.count("<!-- pce:guidance:end -->"), 1)
                for trigger in ("Before drafting or revising a GitHub issue or PR body",
                                "Before implementing or reviewing code or tests"):
                    self.assertIn(trigger, text)
                for name in ("github-writing", "test-first-development"):
                    self.assertIn(f"~/{guidance_root}/skills/{name}/SKILL.md", text)
                    self.assertTrue((home / guidance_root / "skills" / name / "SKILL.md").is_file())
                self.assertNotIn("compaction", text)
                self.assertEqual(self.run_installer(home).returncode, 0)
                self.assertEqual(agents.read_bytes(), content)

    def test_updates_only_owned_section_and_preserves_other_user_state(self) -> None:
        for guidance_root in (".prime/agent", ".gemini"):
            with tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                agents = home / guidance_root / "AGENTS.md"
                agents.parent.mkdir(parents=True)
                prefix = b"# Personal rules\r\nKeep my exact bytes.\r\n"
                suffix = b"\n# More rules\nKeep these too."
                stale = b"<!-- pce:guidance:start -->\nOld PCE pointers\n<!-- pce:guidance:end -->"
                agents.write_bytes(prefix + stale + suffix)
                agents.chmod(0o640)
                user_files = [home / ".claude/CLAUDE.md", home / ".codex/AGENTS.md",
                              home / ".prime/agent/settings.json", home / ".prime/agent/extensions/custom.ts"]
                for path in user_files:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(b"unrelated user state\r\n")
                result = self.run_installer(home)
                self.assertEqual(result.returncode, 0, result.stderr)
                updated = agents.read_bytes()
                self.assertTrue(updated.startswith(prefix))
                self.assertTrue(updated.endswith(suffix))
                self.assertNotIn(b"Old PCE pointers", updated)
                self.assertIn(b"skills/github-writing/SKILL.md", updated)
                self.assertEqual(agents.stat().st_mode & 0o777, 0o640)
                self.assertEqual(self.run_installer(home).returncode, 0)
                self.assertEqual(agents.read_bytes(), updated)
                for path in user_files:
                    self.assertEqual(path.read_bytes(), b"unrelated user state\r\n")

    def test_appends_to_unowned_content_without_rewriting_it(self) -> None:
        for guidance_root in (".prime/agent", ".gemini"):
            with tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                agents = home / guidance_root / "AGENTS.md"
                agents.parent.mkdir(parents=True)
                original = b"# My instructions\r\nNo final newline"
                agents.write_bytes(original)
                agents.chmod(0o600)
                result = self.run_installer(home)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(agents.read_bytes().startswith(original + b"\n\n<!-- pce:guidance:start -->"))
                self.assertEqual(agents.stat().st_mode & 0o777, 0o600)
                updated = agents.read_bytes()
                self.assertEqual(self.run_installer(home).returncode, 0)
                self.assertEqual(agents.read_bytes(), updated)

    def test_refuses_ambiguous_global_ownership_before_mutation(self) -> None:
        for guidance_root in (".prime/agent", ".gemini"):
            start = b"<!-- pce:guidance:start -->"
            end = b"<!-- pce:guidance:end -->"
            for content in (start, end, end + b"\n" + start,
                            start + b"\n" + end + b"\n" + start + b"\n" + end,
                            b"user text " + start + b"\n" + end,
                            start + b"\n" + end + b" user text",
                            b"<!-- pce:guidance:unknown -->"):
                with self.subTest(content=content), tempfile.TemporaryDirectory() as temporary:
                    home = Path(temporary)
                    agents = home / guidance_root / "AGENTS.md"
                    agents.parent.mkdir(parents=True)
                    agents.write_bytes(content)
                    retired = home / ".prime/agent/skills/land-ticket"
                    retired.parent.mkdir(parents=True)
                    retired.symlink_to(ROOT / "skills/land-ticket")
                    self.seed_user_state(home)
                    before = self.snapshot(home)
                    result = self.run_installer(home)
                    self.assertEqual(self.snapshot(home), before)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("ERROR: conflict:", result.stderr)
                    self.assertTrue(retired.is_symlink())
                    self.assertEqual(agents.read_bytes(), content)
                    self.assertFalse(os.path.lexists(home / ".claude/skills/grill-me"))

    def test_refuses_indirect_or_nonregular_global_instructions(self) -> None:
        for guidance_root in (".prime/agent", ".gemini"):
            for kind in ("symlink", "dangling-link", "directory", "hardlink"):
                with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                    home = Path(temporary)
                    agents = home / guidance_root / "AGENTS.md"
                    agents.parent.mkdir(parents=True)
                    target = home / "personal.md"
                    target.write_bytes(b"personal rules")
                    if kind == "directory":
                        agents.mkdir()
                    elif kind == "hardlink":
                        os.link(target, agents)
                    else:
                        agents.symlink_to(target if kind == "symlink" else home / "missing")
                    self.seed_user_state(home)
                    before = self.snapshot(home)
                    result = self.run_installer(home)
                    self.assertEqual(self.snapshot(home), before)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("ERROR: conflict:", result.stderr)
                    self.assertEqual(target.read_bytes(), b"personal rules")
                    self.assertFalse(os.path.lexists(home / ".claude/skills/grill-me"))
                    if kind in ("symlink", "dangling-link"):
                        self.assertTrue(agents.is_symlink())
                    elif kind == "directory":
                        self.assertTrue(agents.is_dir())
                    else:
                        self.assertEqual(agents.stat().st_ino, target.stat().st_ino)

    def test_does_not_shadow_alternate_global_instructions(self) -> None:
        for directory, name in (
            (".prime/agent", "AGENTS.MD"), (".prime/agent", "CLAUDE.md"),
            (".prime/agent", "CLAUDE.MD"), (".gemini", "AGENTS.MD"),
        ):
            with self.subTest(directory=directory, name=name), tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                alternative = home / directory / name
                alternative.parent.mkdir(parents=True)
                alternative.write_bytes(b"user global instructions")
                result = self.run_installer(home)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("ERROR: conflict:", result.stderr)
                self.assertEqual(alternative.read_bytes(), b"user global instructions")
                self.assertFalse(os.path.lexists(home / ".claude/skills/grill-me"))

    def test_gemini_global_rules_are_cumulative(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            gemini = home / ".gemini/GEMINI.md"
            gemini.parent.mkdir(parents=True)
            gemini.write_bytes(b"personal Gemini rules\r\n")
            result = self.run_installer(home)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(gemini.read_bytes(), b"personal Gemini rules\r\n")
            prime = home / ".prime/agent/AGENTS.md"
            agents = home / ".gemini/AGENTS.md"
            self.assertFalse(agents.is_symlink())
            self.assertFalse(prime.is_symlink())
            self.assertNotEqual(agents.stat().st_ino, prime.stat().st_ino)
            self.assertNotIn(b"~/.prime/", agents.read_bytes())
            self.assertNotIn(b"~/.gemini/", prime.read_bytes())
            self.assertFalse((home / ".gemini/antigravity-cli").exists())
            self.assertFalse((home / ".agents").exists())

    def test_removes_only_checkout_owned_retired_skill_links(self) -> None:
        for name, kind, environment_root in product(
            ("land-ticket", "grill-ticket"),
            ("owned", "relative-owned", "foreign", "directory", "file"), ENVIRONMENT_ROOTS
        ):
            with self.subTest(name=name, kind=kind, root=environment_root), \
                    tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                retired = home / environment_root / name
                retired.parent.mkdir(parents=True)
                target = ROOT / "skills" / name if "owned" in kind else home / "foreign"
                if kind == "directory":
                    retired.mkdir()
                    (retired / "SKILL.md").write_text("user skill")
                elif kind == "file":
                    retired.write_text("user skill")
                else:
                    retired.symlink_to(os.path.relpath(target, retired.parent.resolve())
                                       if kind == "relative-owned" else target)
                result = self.run_installer(home)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assert_matrix(home)
                if "owned" in kind:
                    self.assertFalse(os.path.lexists(retired))
                elif kind == "directory":
                    self.assertEqual((retired / "SKILL.md").read_text(), "user skill")
                elif kind == "file":
                    self.assertEqual(retired.read_text(), "user skill")
                else:
                    self.assertEqual(os.readlink(retired), str(target))

    def test_replaces_checkout_owned_link(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            link = home / ".claude/skills/to-vision"
            link.parent.mkdir(parents=True)
            link.symlink_to(ROOT / "absent-source")
            result = self.run_installer(home)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assert_matrix(home)

    def test_supports_symlink_spelled_home(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            home = base / "real-home"
            home.mkdir()
            alias = base / "home-alias"
            alias.symlink_to(home, target_is_directory=True)
            result = self.run_installer(alias)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assert_matrix(home)


if __name__ == "__main__":
    unittest.main()
