from __future__ import annotations

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "skills"


class SkillContractTests(unittest.TestCase):
    def read_skill(self, name: str) -> str:
        return (SKILLS / name / "SKILL.md").read_text(encoding="utf-8")

    def test_tracked_surface_has_exactly_six_skills(self) -> None:
        expected = {
            "grill-me",
            "to-vision",
            "implement-vision",
            "chart-program",
            "grill-ticket",
            "land-ticket",
        }
        actual = {path.name for path in SKILLS.iterdir() if path.is_dir()}
        self.assertEqual(actual, expected)
        for name in expected:
            text = self.read_skill(name)
            self.assertRegex(text, rf"\A---\nname: {re.escape(name)}\n")

    def test_program_skills_compose_canonical_grill_without_copying_its_loop(self) -> None:
        canonical = self.read_skill("grill-me")
        self.assertIn("❓ **Q1 - <question title>**", canonical)
        for name in ("chart-program", "grill-ticket", "land-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                self.assertIn("canonical", text.lower())
                self.assertIn("`grill-me`", text)
                self.assertNotIn("❓ **Q1 - <question title>**", text)
                self.assertNotIn("grill-with-docs` skill", text)

    def test_program_linkage_contracts_are_cold_session_reconstructable(self) -> None:
        chart = self.read_skill("chart-program")
        grill = self.read_skill("grill-ticket")
        implement = self.read_skill("implement-vision")
        land = self.read_skill("land-ticket")
        for token in (
            "<!-- pce:program -->",
            "<!-- pce:effort -->",
            "Program:",
            "Depends on:",
            "Vision: pending",
        ):
            self.assertIn(token, chart)
        for token in ("Program:", "Effort:", "Vision:"):
            self.assertIn(token, grill)
        self.assertIn("commit only the confirmed vision change", grill)
        self.assertIn("pinned to the pushed commit", grill)
        self.assertIn("never link an uncommitted or unpushed file", grill)
        self.assertIn("<!-- pce:delivery -->", implement)
        self.assertIn("Leave the Effort open", implement)
        self.assertIn("<!-- pce:delivery -->", land)
        self.assertIn("recomput", land.lower())

    def test_chart_resurvey_rejects_ambiguous_state_before_mutation(self) -> None:
        chart = self.read_skill("chart-program")
        gate = chart.index("## Pre-mutation state gate")
        mutation = chart.index("Create labels if absent")
        self.assertLess(gate, mutation)
        pre_mutation = chart[gate:mutation]
        for phrase in (
            "exactly one Map membership",
            "delivery and landing marker uniqueness",
            "`landed` predicate",
            "Stop without mutation",
        ):
            self.assertIn(phrase, pre_mutation)

    def test_landed_predicate_is_consistent_and_rejects_closed_only(self) -> None:
        required = (
            "an Effort is `landed` only when all four facts are verified",
            "exactly one authoritative `<!-- pce:delivery -->`",
            "exactly one `<!-- pce:landed -->`",
            "Closed alone never means landed",
            "Cancelled, malformed, prematurely closed, duplicate-record, and "
            "conflicting-record Efforts fail this predicate",
        )
        for name in ("chart-program", "implement-vision", "land-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                for phrase in required:
                    self.assertIn(phrase, text)

    def test_effort_dependency_and_map_ambiguity_stops_mutation(self) -> None:
        grill = self.read_skill("grill-ticket")
        implement = self.read_skill("implement-vision")
        land = self.read_skill("land-ticket")
        self.assertIn("exactly once as an open member", grill)
        self.assertIn("Do not claim or mechanically repair", grill)
        for text in (implement, land):
            self.assertIn("exactly one unambiguous `Depends on:`", text)
            self.assertIn("same Program", text)
            self.assertIn("acyclic", text)
            self.assertIn("duplicates", text)
            self.assertIn("foreign-Program", text)
            self.assertIn("cycles", text)

    def test_delivery_records_are_unique_and_reruns_update_in_place(self) -> None:
        implement = self.read_skill("implement-vision")
        land = self.read_skill("land-ticket")
        for text in (implement, land):
            self.assertIn("sole authoritative", text)
            self.assertIn("exactly one", text)
            self.assertIn("update that same comment in place", text)
            self.assertIn("stop", text.lower())
        self.assertIn("more than one", implement)
        self.assertIn("multiple markers", land)
        self.assertIn("never append a competing record", implement)
        self.assertIn("duplicate landing markers", land)

    def test_vision_provenance_requires_unique_cross_artifact_match(self) -> None:
        for name in ("grill-ticket", "implement-vision", "land-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                self.assertIn("exactly one canonical `Program:` line", text)
                self.assertIn("exactly one canonical `Effort:` line", text)
                self.assertIn("match", text.lower())
                self.assertIn("duplicate", text.lower())
                self.assertIn("stop", text.lower())

    def test_repository_guidance_describes_both_workflows_and_identity_rules(self) -> None:
        claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        for name in (
            "grill-me",
            "to-vision",
            "chart-program",
            "grill-ticket",
            "land-ticket",
        ):
            self.assertIn(f"`{name}`", claude)
        self.assertIn("grill-me → to-vision → implement-vision", claude)
        self.assertIn(
            "chart-program → grill-ticket → implement-vision → land-ticket", claude
        )
        self.assertIn("root Prime Agent", claude)
        self.assertIn("accepts either a large idea", readme)
        self.assertIn("require an explicit Effort identity", readme)
        self.assertIn("No command infers a repository-wide singleton", readme)

    def test_to_vision_derives_name_only_when_no_explicit_name_exists(self) -> None:
        text = self.read_skill("to-vision")
        self.assertIn("When `$ARGUMENTS` contains", text)
        self.assertIn("derive a concise, descriptive name", text)
        self.assertIn("Ask for a name only when", text)
        self.assertIn("Never silently replace prior content", text)

    def test_program_discovery_rejects_retired_document_workflow(self) -> None:
        for name in ("chart-program", "grill-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                self.assertIn("Do not invoke `grill-with-docs`", text)
                self.assertIn("`CONTEXT.md`", text)
                self.assertIn("ADRs", text)
                self.assertIn("domain-modeling", text)


if __name__ == "__main__":
    unittest.main()
