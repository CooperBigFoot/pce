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

    def test_retired_protocol_is_not_an_active_dependency(self) -> None:
        for relative in ("land-ticket", "grill-ticket",
                         "implement-vision/effort-implementation.md"):
            self.assertFalse((SKILLS / relative).exists())
        retired = re.compile(r"pce:(?:delivery|landed|program|effort|ticket)|Vision: pending|"
                             r"Depends on:|land-ticket|grill-ticket|effort-validation\.md|"
                             r"effort-implementation\.md|\bFrontier\b|landed predicate")
        for path in SKILLS.rglob("*.md"):
            self.assertIsNone(retired.search(path.read_text()), str(path))

    def test_discovery_composes_the_existing_interview(self) -> None:
        text = (SKILLS / "chart-program/SKILL.md").read_text()
        self.assertIn("../grill-me/SKILL.md", text)
        self.assertNotIn("❓ **Q1", text)
        interview = (SKILLS / "grill-me/SKILL.md").read_text()
        self.assertIn("❓ **Q1", interview)
        self.assertIn("confirm or correct", interview)

    def test_publication_and_completion_remain_distinct(self) -> None:
        publication = (SKILLS / "to-vision/publication.md").read_text()
        implementation = (SKILLS / "implement-vision/SKILL.md").read_text()
        # Focused safety anchors, not a required body template or prose inventory.
        self.assertIn("Publication must leave the issue open", publication)
        self.assertIn("exact prospective merge or squash", publication)
        self.assertIn("exact authored content", publication)
        self.assertIn("stop before planning", implementation)
        self.assertIn("fresh reviewer who did not implement it", implementation)
        self.assertIn("full outcome is delivered and verified", implementation)
        self.assertIn("close the issue the vision explicitly designates", implementation)
        chart = (SKILLS / "chart-program/SKILL.md").read_text()
        self.assertIn("explicitly requests `to-vision`", chart)
        self.assertIn("Do not automatically close the Program", implementation)

    def test_discussions_share_one_issue_mechanism(self) -> None:
        # Saved grills, charted work, and experiment branches all use the label.
        for name in ("save-discussion", "chart-program"):
            self.assertIn("`discussion` label", (SKILLS / name / "SKILL.md").read_text())
        self.assertIn("grill-me <issue>", (SKILLS / "grill-me/SKILL.md").read_text())

    def test_experiment_evidence_returns_without_closing_the_parent(self) -> None:
        for path in ("to-vision/SKILL.md", "implement-vision/SKILL.md"):
            self.assertIn("originating discussion", (SKILLS / path).read_text())
        implementation = (SKILLS / "implement-vision/SKILL.md").read_text()
        self.assertIn("unfinished reconciliation", implementation)
        self.assertIn("not a closing relationship", implementation)


if __name__ == "__main__":
    unittest.main()
