from __future__ import annotations

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
        for kind in ("file", "directory", "foreign-link", "dangling-link"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                home = Path(temporary)
                conflict = home / ".codex/skills/to-vision"
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
                result = self.run_installer(home)
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
