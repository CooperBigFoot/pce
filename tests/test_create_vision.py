from __future__ import annotations

import datetime as dt
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "skills/to-vision/scripts/create_vision.py"
SPEC = importlib.util.spec_from_file_location("create_vision", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CreateVisionTests(unittest.TestCase):
    def test_slugify_normalizes_human_readable_names(self) -> None:
        cases = {
            " Auth Refactor ": "auth-refactor",
            "Café + API v2": "cafe-api-v2",
            "many___separators": "many-separators",
        }
        for name, expected in cases.items():
            with self.subTest(name=name):
                self.assertEqual(MODULE.slugify(name), expected)

    def test_slugify_rejects_names_without_ascii_letters_or_numbers(self) -> None:
        with self.assertRaisesRegex(ValueError, "letter or number"):
            MODULE.slugify("--- 💧 ---")

    def test_create_is_idempotent_and_never_overwrites(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            day = dt.date(2026, 8, 31)
            relative = MODULE.create_vision(root, "Skill Only PCE", day)
            destination = root / relative
            self.assertEqual(relative, Path("planning/visions/2026-08-31-skill-only-pce.md"))
            self.assertEqual(destination.read_bytes(), b"")
            destination.write_text("owned content\n", encoding="utf-8")

            repeated = MODULE.create_vision(root, "Skill Only PCE", day)

            self.assertEqual(repeated, relative)
            self.assertEqual(destination.read_text(encoding="utf-8"), "owned content\n")

    def test_existing_symlink_is_refused_without_changing_its_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            day = dt.date(2026, 8, 31)
            destination = root / "planning/visions/2026-08-31-linked.md"
            destination.parent.mkdir(parents=True)
            external = root / "external.md"
            external.write_text("external content\n", encoding="utf-8")
            destination.symlink_to(external)

            with self.assertRaisesRegex(ValueError, "not a regular file"):
                MODULE.create_vision(root, "Linked", day)

            self.assertTrue(destination.is_symlink())
            self.assertEqual(external.read_text(encoding="utf-8"), "external content\n")

    def test_symlinked_vision_directory_is_refused_without_external_creation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            root.mkdir()
            planning = root / "planning"
            planning.mkdir()
            external = Path(temporary) / "external"
            external.mkdir()
            (planning / "visions").symlink_to(external, target_is_directory=True)

            with self.assertRaisesRegex(ValueError, "not a regular directory"):
                MODULE.create_vision(root, "Escaped", dt.date(2026, 8, 31))

            self.assertEqual(list(external.iterdir()), [])

    def test_non_directory_path_component_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "planning").write_text("unrelated", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "not a regular directory"):
                MODULE.create_vision(root, "Blocked", dt.date(2026, 8, 31))

            self.assertEqual((root / "planning").read_text(encoding="utf-8"), "unrelated")

    def test_cli_creates_under_working_repository_and_prints_only_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "CLI Vision"],
                cwd=temporary,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            output = result.stdout.strip()
            self.assertRegex(output, r"^planning/visions/\d{4}-\d{2}-\d{2}-cli-vision\.md$")
            self.assertTrue((Path(temporary) / output).is_file())


if __name__ == "__main__":
    unittest.main()
