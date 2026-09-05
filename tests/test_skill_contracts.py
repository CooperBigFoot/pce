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
        self.assertIn("root implementing agent", claude)
        self.assertIn("accepts either a large idea", readme)
        self.assertIn("require an explicit Effort identity", readme)
        self.assertIn("No command infers a repository-wide singleton", readme)

    def test_repository_guidance_documents_all_six_skills_in_every_environment(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
        rows = {
            environment: next(
                line
                for line in readme.splitlines()
                if line.startswith(f"| {environment} (`{directory}`) |")
            )
            for environment, directory in (
                ("Claude Code", "~/.claude/skills"),
                ("Codex", "~/.codex/skills"),
                ("Prime Agent", "~/.prime/agent/skills"),
            )
        }
        for environment, row in rows.items():
            for name in (
                "grill-me",
                "to-vision",
                "implement-vision",
                "chart-program",
                "grill-ticket",
                "land-ticket",
            ):
                with self.subTest(environment=environment, name=name):
                    self.assertIn(f"`{name}`", row)
        for text in (readme, claude):
            self.assertRegex(text, r"All six (PCE )?skills are available in every supported environment")
            self.assertIn("spawn subagents", text)
            self.assertIn("Git and GitHub", text)
        self.assertNotIn("five", claude)
        self.assertNotIn("root Prime Agent", readme)

    def test_skill_text_is_harness_agnostic(self) -> None:
        for path in sorted(SKILLS.glob("*/SKILL.md")):
            text = path.read_text(encoding="utf-8")
            with self.subTest(skill=path.parent.name):
                for phrase in (
                    "Prime Agent",
                    "Claude Code",
                    "Codex",
                    "harness",
                    "persistent goal",
                    "heartbeat",
                    "wake-up",
                    "if your",
                ):
                    self.assertNotIn(phrase, text)
        implement = self.read_skill("implement-vision")
        self.assertNotRegex(implement, r"\bgoals?\b")
        for phrase in (
            "spawning subagents",
            "Work within the active turn",
            "resuming as each delegated subagent completes",
            "stop cleanly, explain exactly what is needed, and end the turn",
            "re-invokes this skill",
            "Nothing outside the vision, Git, and GitHub evidence is the durable truth",
            "environment's own planning and delegation facilities",
        ):
            self.assertIn(phrase, implement)

    def test_to_vision_derives_name_only_when_no_explicit_name_exists(self) -> None:
        text = self.read_skill("to-vision")
        self.assertIn("When `$ARGUMENTS` contains", text)
        self.assertIn("derive a concise, descriptive name", text)
        self.assertIn("Ask for a name only when", text)
        self.assertIn("Never silently replace prior content", text)

    def test_program_discovery_uses_only_canonical_records_and_interview(self) -> None:
        for name in ("chart-program", "grill-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                self.assertIn("only discovery records", text)
                self.assertIn(
                    "canonical `grill-me` interview is the only discovery mechanism",
                    text,
                )
                for retired_term in (
                    "grill-with-docs",
                    "CONTEXT.md",
                    "ADR",
                    "domain-modeling",
                ):
                    self.assertNotIn(retired_term, text)

    def test_implement_vision_has_unified_recoverable_inputs(self) -> None:
        text = self.read_skill("implement-vision")
        for phrase in (
            "Effort issue number in the current repository",
            "canonical Effort issue URL",
            "repository-relative `planning/visions/` path",
        ):
            self.assertIn(phrase, text)
        self.assertIn("derive the repository from the URL", text)
        self.assertIn("do not use the caller's current repository", text)
        self.assertIn("may name a repository different from the caller's current checkout", text)
        self.assertIn("rather than rejecting it as cross-repository", text)
        self.assertNotIn("Reject missing, extra, ambiguous, shorthand, cross-repository", text)
        self.assertIn("normalized canonical remote matches that repository", text)
        self.assertIn("Never read, branch, create a worktree, or implement URL-derived work", text)
        self.assertIn("A path input is standalone only when it has no `Effort:` provenance", text)
        self.assertIn("derive the canonical repository and Effort identity", text)
        self.assertIn("promote the path to that Effort identity", text)
        self.assertIn("never continue as standalone", text)
        self.assertNotIn("require that Effort's number or canonical URL", text)

    def test_implement_vision_validates_before_planning(self) -> None:
        text = self.read_skill("implement-vision")
        validation = text.index("## Validate the resolved input")
        planning = text.index("## Plan the outcome")
        self.assertLess(validation, planning)
        gate = text[validation:planning]
        for phrase in (
            "before any planning, delegation, branch creation, or substantive implementation",
            "missing, duplicated, malformed, foreign, ambiguous, or conflicting",
            "durable linkage",
            "Do not plan, delegate, or create any work",
            "end the turn normally without a continuation loop",
            "### Target-branch durability gate",
        ):
            self.assertIn(phrase, gate)
        plan = text[planning:text.index("## Reconstruct every run")]
        for phrase in (
            "Investigate before asking",
            "Ask the human only about missing intent, priorities, outcome-level trade-offs, credentials, legal or organizational authority, or permission for an exceptional irreversible external act",
            "Choose one PR or several coherent vertical slices",
        ):
            self.assertIn(phrase, plan)

    def test_implement_vision_keeps_delivery_identity_out_of_product_architecture(self) -> None:
        text = self.read_skill("implement-vision")
        execute = text.index("## Execute")
        review = text.index("## Review and land")
        self.assertLess(execute, review)
        implementation_contract = text[execute:review]
        for phrase in (
            "Every implementation owner",
            "before designing or editing production code",
            "read the complete vision",
            "inspect the repository's existing architecture and vocabulary",
            "established repository and domain vocabulary",
            "stable responsibility",
            "vague generic names",
            "Delivery identity must not determine production modules, packages, types, functions, commands, routes, services, runtime schemas, user-facing configuration keys, public APIs, or other maintained product architecture",
            "Removing a ticket number is not enough",
            "Do not introduce or expand ticket-shaped product architecture.",
            "directly modifies or depends on",
            "report unrelated occurrences",
            "Preserve compatibility for existing public interfaces",
            "explicit migration",
            "delivery metadata and traceability evidence",
            "issues, vision provenance",
            "PR descriptions",
            "delivery records",
            "branch and worktree names",
            "commit messages",
            "historical evidence",
            "where traceability requires it",
            "test names, fixtures, examples, or study-specific data configuration",
            "maintained artifacts should prefer behavioral or domain names",
            "explicit metadata or a comment rather than making it the artifact's organizing name",
            "repository-wide naming linter",
            "universal naming convention",
            "perform unrelated cleanup",
            "Do not rename PCE's Program or Effort workflow artifacts",
        ):
            self.assertIn(phrase, implementation_contract)

        review_contract = text[review:]
        for phrase in (
            "complete repository and vision",
            "newly introduced or expanded ticket-derived production identifiers",
            "inherited ticket-shaped architecture that the implementation extends",
            "Effort-shaped abstractions after cosmetic renames",
            "vague generic APIs",
            "compatibility breaks to an existing public interface without an explicit migration",
        ):
            self.assertIn(phrase, review_contract)

    def test_vision_target_copy_gate_is_strict_for_efforts_and_standalone(self) -> None:
        text = self.read_skill("implement-vision")
        for phrase in (
            "regular file",
            "tracked on the intended target branch",
            "fetch the intended target branch",
            "read the vision from that fetched ref",
            "exact content",
        ):
            self.assertIn(phrase, text)
        self.assertIn("An Effort-derived vision must already", text)
        self.assertIn("stop before planning or substantive implementation", text)
        self.assertIn("Do not bootstrap publication for an Effort", text)

    def test_standalone_publication_distinguishes_bootstrap_from_resume_gap(self) -> None:
        text = self.read_skill("implement-vision")
        self.assertIn("local standalone bootstrap", text.lower())
        self.assertIn("new local vision created by `to-vision`", text)
        self.assertIn("publish it through the repository's normal branch and review process", text)
        self.assertIn("verify the resulting target-branch copy", text)
        self.assertIn("unresolved publication gap", text)
        self.assertIn("preserve and report", text)
        self.assertIn("do not begin substantive implementation", text)
        self.assertIn("When repository policy uses a PR", text)
        self.assertIn("when policy permits another reviewed publication path", text)
        self.assertNotIn("verify the pushed commit, PR, merge", text)

    def test_reruns_use_five_way_cold_reconstruction_and_complete_noop(self) -> None:
        text = self.read_skill("implement-vision")
        for classification in ("merged", "open", "abandoned", "incomplete", "remaining"):
            self.assertRegex(text, rf"\b{classification}\b")
        for evidence in (
            "target-branch commits",
            "branches",
            "validation evidence",
            "structured local worktrees",
        ):
            self.assertIn(evidence, text)
        self.assertIn("without the prior agent session", text)
        self.assertIn("already complete", text)
        self.assertIn("no-op", text)
        self.assertIn("do not create a branch, commit, worktree, PR, or delivery record", text)

    def test_vision_worktrees_are_canonical_and_cleanup_is_evidence_safe(self) -> None:
        text = self.read_skill("implement-vision")
        self.assertIn("<repository>/.worktrees/visions/effort-<number>-<slug>/", text)
        self.assertIn("standalone", text)
        self.assertIn("`/private/tmp`", text)
        self.assertIn("arbitrary sibling directories", text)
        for evidence in ("uncommitted", "unpushed", "unmerged", "uncertain ownership"):
            self.assertIn(evidence, text)
        self.assertIn("preserve", text.lower())
        self.assertIn("remove", text.lower())
        self.assertIn("verified merged", text)

    def test_land_ticket_requires_target_vision_and_worktree_closure_gate(self) -> None:
        text = self.read_skill("land-ticket")
        gate = text.index("## Target-copy and worktree closure gate")
        land = text.index("## Land and evolve the Map")
        self.assertLess(gate, land)
        section = text[gate:land]
        for phrase in (
            "fetch the intended target branch",
            "regular `planning/visions/` file",
            "exact matching provenance",
            "canonical Effort worktrees",
            "safely removable",
            "preserve and report",
            "Do not close the Effort",
        ):
            self.assertIn(phrase, section)

    def test_authoring_workflows_report_publication_readiness_precisely(self) -> None:
        expectations = {
            "grill-me": ("confirmation is not publication", "not ready for `implement-vision`"),
            "to-vision": ("local draft", "not yet verified on the target branch"),
            "chart-program": ("publish and verify the merged target-branch copy", "exact `implement-vision` handoff"),
            "grill-ticket": ("merged into the intended target branch", "ready for `implement-vision <Effort number or canonical URL>`"),
        }
        for name, phrases in expectations.items():
            with self.subTest(name=name):
                text = self.read_skill(name)
                for phrase in phrases:
                    self.assertIn(phrase, text)

    def test_repository_guidance_documents_recovery_and_durable_handoffs(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
        for text in (readme, claude):
            self.assertIn("implement-vision <Effort number or canonical URL>", text)
            self.assertIn("planning/visions/<vision>.md", text)
            self.assertIn("Effort-derived path", text)
            self.assertIn(".worktrees/visions/", text)
            self.assertIn("target branch", text)
            self.assertIn("resume", text.lower())


if __name__ == "__main__":
    unittest.main()
