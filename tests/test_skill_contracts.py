from __future__ import annotations

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "skills"


class SkillContractTests(unittest.TestCase):
    def read_skill(self, name: str) -> str:
        return (SKILLS / name / "SKILL.md").read_text(encoding="utf-8")

    def test_active_surface_has_no_hosted_semantic_integration(self) -> None:
        paths = list(SKILLS.rglob("*.md")) + list(SKILLS.rglob("*.py"))
        paths += [ROOT / name for name in ("README.md", "CLAUDE.md", "install.sh")]
        for path in paths:
            with self.subTest(path=path.relative_to(ROOT)):
                text = path.read_text(encoding="utf-8")
                self.assertIsNone(re.search(
                    r"(?i)\bjev\b|typesafe|semantic[_-]decisions|automatic semantic|fallback",
                    text,
                ), f"Active hosted integration remains in {path.relative_to(ROOT)}")
        for relative in (
            "skills/implement-vision/scripts/semantic_decisions.py",
            "skills/implement-vision/semantic-decisions.md",
            "tests/test_semantic_decisions.py",
        ):
            self.assertFalse((ROOT / relative).exists(), relative)

    def test_native_authoring_checks_preserve_fidelity_and_scope(self) -> None:
        author = self.read_skill("to-vision")
        section = author.split("## Authoring checks", 1)[-1].split("## Draft-only output", 1)[0]
        for phrase in (
            "Investigate source evidence", "each confirmed decision",
            "unapproved outcomes or constraints", "ordinary technical elaboration",
            "uncertain or conflicting", "including draft-only",
            "before publication review",
        ):
            self.assertTrue(phrase in section, phrase)
        for caller in ("chart-program", "grill-ticket"):
            self.assertIn("`to-vision` authoring checks", self.read_skill(caller))

    def test_shared_publication_has_one_owner_and_no_caller_tail(self) -> None:
        author = self.read_skill("to-vision")
        self.assertIn("sole execution owner", author)
        for name in ("grill-ticket", "chart-program"):
            text = self.read_skill(name)
            self.assertTrue("Consume its verified result" in text, name)
            self.assertTrue("do not repeat publication, linkage, or final verification" in text, name)
            for repeated_action in (
                "Replace `Vision: pending`", "Before any vision-link mutation",
                "Fetch the intended target branch after publication",
                "update the ticket to its new commit-pinned URL",
            ):
                self.assertNotIn(repeated_action, text)
        self.assertIn("fresh state validation", author)
        self.assertIn("after an interview or review interval", author)
        self.assertIn("reusing the path, branch, PR, and single link", author)

    def test_same_invocation_evidence_reuse_requires_freshness(self) -> None:
        text = self.read_skill("implement-vision")
        for phrase in (
            "one evidence-gathering pass", "within this invocation",
            "Do not repeat reads solely", "not a persistent cache",
            "after relevant state changes", "external activity or waits",
            "before consequential mutations", "for final verification",
            "earlier valid snapshot is not proof of current state",
        ):
            self.assertTrue(phrase in text, phrase)
        self.assertIn("Only after input validation succeeds, classify prior work", text)
        self.assertIn("Every fresh invocation must reconstruct from Git and GitHub", text)
        for classification in ("merged", "open", "abandoned", "incomplete", "remaining"):
            self.assertIn(f"**{classification}**", text)
        self.assertIn("Reconstruct prior work before planning", text)

    def test_native_evidence_checks_and_independent_review_remain_explicit(self) -> None:
        implement = self.read_skill("implement-vision")
        land = self.read_skill("land-ticket")
        for text in (implement, land):
            for phrase in (
                "Match each requirement", "tests, source, and observed results",
                "Investigate missing, indirect, unsupported, uncertain, or contradictory evidence",
                "run required validation", "inspect source and target effects",
            ):
                self.assertTrue(phrase in text, phrase)
        self.assertIn("full vision", implement)
        self.assertIn("complete diff", implement)
        self.assertIn("fresh reviewer that did not implement it", implement)
        self.assertIn("Before delegation", implement)
        self.assertIn("uncertain or conflicting sources", implement)

    def test_chart_contract_describes_agreed_work_without_uncertainty_inventory(self) -> None:
        text = self.read_skill("chart-program")
        active = text.split("## Legacy Program content", 1)[0]
        self.assertNotRegex(active, r"(?i)\bfog\b")
        proposal = active.split("## Approval gate", 1)[1].split("## Publish deterministically", 1)[0]
        for field in ("Destination", "Effort tickets", "Dependencies", "Exclusions", "Re-survey changes"):
            self.assertIn(f"**{field}:**", proposal)
        for phrase in (
            "Do not invent work to populate a template",
            "Concrete risks belong with the relevant issue or vision",
            "grill only additions, changed boundaries, and conflicts",
            "open Efforts, Frontier, landed one-line outcomes, and exclusions",
        ):
            self.assertIn(phrase, active)
        self.assertNotRegex(active, r"(?i)\*\*(?:risks|uncertainty|backlog):\*\*")

    def test_landing_verifies_delivery_without_scope_expansion(self) -> None:
        text = self.read_skill("land-ticket")
        active = text.split("## Legacy Program content", 1)[0]
        self.assertNotRegex(active, r"(?i)\bfog\b")
        self.assertNotIn("`grill-me`", text)
        for removed in (
            "newly visible territory", "intent-level mutation proposal",
            "new approved Efforts", "evolve Fog", "evolve the Map",
        ):
            self.assertNotIn(removed, text)
        for phrase in (
            "Do not discover future work, interview about future work, or propose new Efforts",
            "explicitly requested `chart-program` re-survey",
            "Investigate and report concrete delivery problems",
            "recompute Frontier using the `landed` predicate",
            "Perform read-back verification",
        ):
            self.assertIn(phrase, text)
        closure = active.split("When no open Efforts remain", 1)[1]
        self.assertIn("verified delivery", closure)
        self.assertIn("Ask once for authority to close the Program Map", closure)
        self.assertIn("only after that explicit confirmation", closure)
        self.assertIn("leave it open", closure)
        for phrase in (
            "implementation is incomplete and the vision remains valid",
            "recommend resuming `implement-vision` with the same vision",
            "flawed outcome, missing requirement, or obsolete assumption",
            "recommend rerunning `grill-ticket` on this same ticket and vision",
            "If evidence is ambiguous or any delivery blocker fails",
            "do not close the Effort",
        ):
            self.assertIn(phrase, text)

    def test_legacy_program_content_is_not_a_gate_or_migration_authority(self) -> None:
        for name in ("chart-program", "land-ticket"):
            with self.subTest(name=name):
                text = self.read_skill(name)
                self.assertIn("## Legacy Program content", text)
                legacy = text.split("## Legacy Program content", 1)[1]
                for phrase in (
                    "Legacy Fog text is not a prerequisite",
                    "proposing Program completion",
                    "Preserve unrelated issue content",
                    "do not silently delete it or convert it into tickets",
                    "Do not bulk-edit existing Programs",
                ):
                    self.assertIn(phrase, legacy)

    def test_guidance_describes_delivery_only_landing(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
        self.assertNotRegex(readme, r"(?i)\bfog\b")
        for phrase in (
            "recompute Frontier", "no open Efforts", "explicit human approval",
            "explicitly requested `chart-program` re-survey",
            "ordinary GitHub issues", "does not grant blanket authority to expand scope",
        ):
            self.assertIn(phrase, readme)
        self.assertNotIn("Program evolution", claude)
        self.assertIn("delivery verification", claude)

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
        for name in ("chart-program", "grill-ticket"):
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
        # The shared procedure owns these safeguards, not a second caller tail.
        author = self.read_skill("to-vision")
        self.assertIn("`to-vision` publication contract once", grill)
        self.assertIn("commit only the confirmed vision change", author)
        self.assertIn("verified commit-pinned GitHub blob URL", author)
        self.assertIn("Never create competing links, link unpushed content", author)
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

    def test_implementation_refuses_unpublished_standalone_without_bootstrap(self) -> None:
        text = self.read_skill("implement-vision")
        gate = text.split("### Target-branch durability gate", 1)[1].split(
            "### Effort provenance", 1
        )[0]
        self.assertNotIn("local standalone bootstrap", gate)
        self.assertIn("Do not publish drafts", gate)
        self.assertIn("`to-vision`", gate)
        self.assertIn("stop before planning", gate)
        self.assertIn("preserve and report", gate)
        self.assertIn("Never manufacture Program or Effort provenance", gate)

    def test_authoring_owns_ordered_reviewed_publication(self) -> None:
        # These tests inspect the actual instructions, not simulated GitHub behavior.
        text = self.read_skill("to-vision")
        self.assertIn("Invoking `to-vision` authorizes publication", text)
        self.assertNotIn("`implement-vision` must publish", text)
        publication = text.split("## Publish and verify", 1)[1].split("\n1. ", 1)[1]
        ordered = (
            "Inspect contribution rules", "commit only the confirmed vision change",
            "verify the remote commit", "Open or reuse a documentation PR",
            "independent review", "Before issue mutation", "Before merge",
            "Merge the documentation PR", "Fetch the intended target branch",
        )
        positions = [publication.index(phrase) for phrase in ordered]
        self.assertEqual(positions, sorted(positions))
        for phrase in (
            "ignored planning paths", "required checks", "required approvals",
            "exact authored content", "regular", "nonblocking progress updates",
            "file, branch, commit, PR", "not implementation-ready",
            "publication PR", "verified Git refs", "Do not start implementation",
        ):
            self.assertIn(phrase, text)

    def test_draft_only_has_no_publication_and_preserves_same_path(self) -> None:
        text = self.read_skill("to-vision")
        draft = text.split("## Draft-only output", 1)[1].split("## Publish and verify", 1)[0]
        for phrase in (
            "explicitly requests draft-only", "no commit, push, PR, issue mutation, or merge",
            "not yet verified on the target branch", "not implementation-ready",
            "same path",
        ):
            self.assertIn(phrase, draft)
        self.assertIn("reuse that same regular", text)

    def test_effort_publication_composes_validation_and_safe_linkage(self) -> None:
        author = self.read_skill("to-vision")
        grill = self.read_skill("grill-ticket")
        for phrase in (
            "Before issue mutation", "Validate and claim", "repository identity",
            "exactly one Markdown link", "commit-pinned GitHub blob URL",
            "Reload the issue", "both directions", "declaration uniqueness",
            "matching provenance", "issue remains open",
        ):
            self.assertIn(phrase, author)
        self.assertIn("Load and follow", grill)
        self.assertIn("`to-vision` publication contract", grill)
        self.assertIn("publication PR", grill)
        self.assertIn("issue remains open", grill)

    def test_effort_draft_only_precedes_claim_and_keeps_validation_read_only(self) -> None:
        # Follow the composed entry path: grill-ticket claims before authoring.
        text = self.read_skill("grill-ticket")
        validation = text.split("## Validate and claim", 1)[1].split(
            "## Discover through", 1
        )[0]
        detection = validation.index("Detect an explicit draft-only request before any mutation")
        assignment = validation.index("If unassigned, assign")
        self.assertLess(detection, assignment)
        self.assertIn("For draft-only, validate read-only and do not assign", validation)
        self.assertIn("For publication mode only", validation)
        author = self.read_skill("to-vision")
        self.assertIn("state validation, not its assignment action", author)

    def test_publication_checks_prospective_merge_payload_before_merge(self) -> None:
        text = self.read_skill("to-vision")
        guard = text.split("Before merge", 1)[1].split("Merge the documentation PR", 1)[0]
        self.assertIn("prospective exact merge or squash title and body", guard)
        self.assertIn("before submitting the merge", guard)
        self.assertIn("pass that inspected payload explicitly", guard)

    def test_publication_guards_cover_negated_closing_reference_failure(self) -> None:
        text = self.read_skill("to-vision")
        guard = text.split("Before merge", 1)[1].split("Merge the documentation PR", 1)[0]
        for phrase in (
            "PR descriptions", "commit messages", "including negated phrases",
            "existing PR text", "explicit closing relationships",
            "Related Effort: #225", "close", "closes", "closed", "fix", "fixes",
            "fixed", "resolve", "resolves", "resolved",
        ):
            self.assertIn(phrase, guard)
        for phrase in (
            "Unexpected closure is a publication error", "not evidence of delivery",
            "Do not blindly reopen", "restored and verified through permitted action",
            "`land-ticket`", "never mark an Effort delivered or landed",
        ):
            self.assertIn(phrase, text)
        self.assertIn("closing-reference safeguards", self.read_skill("grill-ticket"))

    def test_guidance_removes_deferred_publication_and_documents_draft_only(self) -> None:
        for filename in ("README.md", "CLAUDE.md"):
            with self.subTest(filename=filename):
                text = (ROOT / filename).read_text(encoding="utf-8")
                self.assertIn("draft-only", text)
                self.assertIn("documentation PR", text)
                self.assertNotIn("first run it publishes", text)
                self.assertNotIn("first standalone run can publish", text)
        chart = self.read_skill("chart-program")
        self.assertIn("`to-vision` publication contract", chart)

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
        policy = text.split("## Worktree and preservation policy", 1)[1].split("## Execute", 1)[0]
        for phrase in (
            "every checkout any agent in the invocation creates",
            "implementation, independent review, audit, red or failing reproduction",
            "side-by-side comparison",
            "git worktrees, clones, and plain copies",
            "below `<repository>/.worktrees/`",
            "Reviewers and other delegated agents must receive this policy",
            "initial canonical clone",
            "when no local checkout exists",
            "exempt from managed placement and disposable-checkout cleanup",
            "retained role",
            "Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence",
            "logs, receipts, diffs, and patches",
            "never copy or retain a build directory to prove a result",
            "source and history, not build output",
            "final target-branch audit, enumerate every checkout the invocation created, in every location",
            "status, branch reachability, upstream state, target merge evidence, and ownership",
            "another run or a human created",
            "plain copies without Git metadata",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, policy)

    def test_land_ticket_requires_target_vision_and_worktree_closure_gate(self) -> None:
        text = self.read_skill("land-ticket")
        gate = text.index("## Target-copy and worktree closure gate")
        self.assertIn("## Land and update the Map", text)
        land = text.index("## Land and update the Map")
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
        for phrase in (
            "every checkout under `<repository>/.worktrees/` that relates to the Effort",
            "git worktrees, clones, and plain copies",
            "review, audit, reproduction, and comparison",
            "only to checkouts this invocation created",
            "another run or a human created",
            "Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence",
            "logs, receipts, diffs, and patches",
            "never a reason to preserve a checkout",
            "status, branch reachability, upstream state, target merge evidence, and ownership",
            "plain copies without Git metadata",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, section)

    def test_repository_guidance_covers_all_created_checkouts(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        paragraph = next(p for p in readme.split("\n\n") if "PCE-created checkouts" in p)
        for phrase in (
            "Every checkout PCE creates, for any purpose",
            "`<repository>/.worktrees/`",
            "git worktrees, clones, and plain copies",
            "review, audit, reproduction, and comparison",
            "initial canonical clone",
            "when no local checkout exists",
            "Build output is never evidence",
            "logs, receipts, diffs, and patches",
            "every checkout the invocation created",
            "another run or a human",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, paragraph)

    def test_authoring_workflows_report_publication_readiness_precisely(self) -> None:
        expectations = {
            "grill-me": ("confirmation is not publication", "not ready for `implement-vision`"),
            "to-vision": ("publication PR", "verified Git refs", "Do not start implementation automatically"),
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
