from __future__ import annotations

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "skills"


class SkillContractTests(unittest.TestCase):
    def test_distribution_and_entrypoints(self) -> None:
        expected = {
            "chart-program", "save-discussion", "grill-me", "to-vision",
            "implement-vision", "github-writing", "test-first-development",
        }
        self.assertEqual({p.name for p in SKILLS.iterdir() if p.is_dir()}, expected)
        for name in expected:
            text = (SKILLS / name / "SKILL.md").read_text()
            self.assertRegex(text, rf"\A---\nname: {re.escape(name)}\n")

    def test_documentation_references_resolve(self) -> None:
        paths = list(SKILLS.rglob("*.md")) + [ROOT / "README.md", ROOT / "AGENTS.md"]
        for path in paths:
            for target in re.findall(r"\[[^]]+\]\(([^)]+)\)", path.read_text()):
                if "://" in target or target.startswith("#"):
                    continue
                with self.subTest(path=path, target=target):
                    self.assertTrue((path.parent / target.split("#")[0]).is_file())



if __name__ == "__main__":
    unittest.main()
