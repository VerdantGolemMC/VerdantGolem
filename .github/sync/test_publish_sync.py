from copy import deepcopy
from pathlib import Path
import json
import tempfile
import unittest
from unittest.mock import Mock, patch

from publish_sync import (
    END, REQUIRED_JOBS, REQUIRED_WORKFLOWS, START, SyncError,
    candidate_body, check_existing_branch, dispatch_checks,
    merge_after_checks, publish, validate_report, wait_checks,
    verify_bundle,
    verify_upstream_lineage,
)
from upstream_sync import UPSTREAM_REPO
from upstream_sync import Entry
from test_upstream_sync import Repository


BASE = "a" * 40
UPSTREAM = "b" * 40
CANDIDATE = "c" * 40
TREE = "d" * 40
BRANCH = "codex/upstream-sync"
MARKER = "123-1"
REPORT = {"schema_version": 1, "repository": UPSTREAM_REPO, "status": "ready", "base": BASE,
          "upstream": UPSTREAM, "commit": CANDIDATE, "tree": TREE, "count": 5}


class Clock:
    def __init__(self):
        self.now = 0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class FakeGitHub:
    repository = "fixture/repository"

    def __init__(self):
        self.calls = []
        self.refs = {"master": BASE, BRANCH: CANDIDATE}
        self.policy = {"can_approve_pull_request_reviews": True}
        self.current_pr = None
        self.runs = {}
        self.jobs = {}
        for index, workflow in enumerate(REQUIRED_WORKFLOWS):
            self.runs[workflow] = [{"id": index + 1, "status": "completed", "conclusion": "success",
                "display_title": f"Upstream sync {MARKER}", "head_sha": CANDIDATE,
                "head_branch": BRANCH, "event": "workflow_dispatch", "html_url": f"https://example.invalid/run/{index + 1}"}]
            self.jobs[index + 1] = [
                {"name": prefix + (f" (matrix {i})" if minimum > 1 else ""), "status": "completed", "conclusion": "success"}
                for prefix, minimum in REQUIRED_JOBS[workflow].items() for i in range(minimum)
            ]

    def ref(self, branch, missing_ok=False):
        return self.refs.get(branch)

    def api(self, path, method="GET", data=None, missing_ok=False):
        self.calls.append((path, method, data))
        if path == "actions/permissions/workflow":
            if isinstance(self.policy, Exception):
                raise self.policy
            return self.policy
        if path == "":
            return {"allow_merge_commit": True}
        if path.startswith("actions/workflows/"):
            workflow = path.split("/")[2]
            return {"workflow_runs": deepcopy(self.runs[workflow])} if "/runs?" in path else None
        if path.startswith("actions/runs/"):
            return {"jobs": deepcopy(self.jobs[int(path.split("/")[2])])}
        if path.startswith("pulls?"):
            return [deepcopy(self.current_pr)] if self.current_pr else []
        if path == "pulls" and method == "POST":
            self.current_pr = {"number": 7, "html_url": "https://example.invalid/pull/7", "body": data["body"],
                               "state": "open", "head": {"sha": CANDIDATE}, "base": {"sha": BASE},
                               "mergeable": True, "mergeable_state": "clean"}
            return deepcopy(self.current_pr)
        if path == "pulls/7/merge":
            return {"merged": True, "sha": "e" * 40}
        if path == "pulls/7":
            if method == "PATCH":
                self.current_pr.update(data)
            return deepcopy(self.current_pr)
        if path == "issues/7/comments":
            return {}
        raise AssertionError(f"Unexpected GitHub request: {path} {method}")


class CheckTests(unittest.TestCase):
    def wait(self, github, timeout=60):
        clock = Clock()
        return wait_checks(github, BRANCH, CANDIDATE, MARKER, timeout, lambda _: None, clock, clock.sleep)

    def test_all_workflows_are_explicitly_dispatched_with_one_correlation_id(self):
        github = FakeGitHub()
        dispatch_checks(github, BRANCH, MARKER)
        self.assertEqual(len(github.calls), len(REQUIRED_WORKFLOWS))
        for (_, method, data) in github.calls:
            self.assertEqual(method, "POST")
            self.assertEqual(data, {"ref": BRANCH, "inputs": {"sync_run_id": MARKER}})

    def test_matching_successful_runs_and_complete_job_matrices_pass(self):
        self.assertEqual(set(self.wait(FakeGitHub())), set(REQUIRED_WORKFLOWS))

    def test_no_checks_and_missing_workflow_never_mean_success(self):
        for missing in (REQUIRED_WORKFLOWS, ("nix.yml",)):
            github = FakeGitHub()
            for workflow in missing:
                github.runs[workflow] = []
            with self.assertRaisesRegex(SyncError, "Timed out"):
                self.wait(github)

    def test_stale_sha_marker_event_and_branch_are_ignored(self):
        for key, value in (("head_sha", BASE), ("display_title", "Upstream sync old"),
                           ("event", "pull_request"), ("head_branch", "different")):
            github = FakeGitHub()
            github.runs["rust.yml"][0][key] = value
            with self.assertRaisesRegex(SyncError, "Timed out"):
                self.wait(github)

    def test_failed_cancelled_skipped_and_timed_out_runs_block_merge(self):
        for conclusion in ("failure", "cancelled", "skipped", "timed_out", "action_required"):
            github = FakeGitHub()
            github.runs["rust.yml"][0]["conclusion"] = conclusion
            with self.assertRaisesRegex(SyncError, "finished as"):
                self.wait(github)

    def test_successful_workflow_with_skipped_or_missing_jobs_is_rejected(self):
        for change in ("skip", "missing"):
            github = FakeGitHub()
            if change == "skip":
                github.jobs[1][0]["conclusion"] = "skipped"
            else:
                github.jobs[1] = []
            with self.assertRaises(SyncError):
                self.wait(github)

    def test_incomplete_release_matrix_is_rejected(self):
        github = FakeGitHub()
        github.jobs[1] = [job for job in github.jobs[1] if job["name"] != "Build project in release (matrix 4)"]
        with self.assertRaisesRegex(SyncError, "matrix.*missing"):
            self.wait(github)

    def test_duplicate_correlated_runs_are_rejected(self):
        github = FakeGitHub()
        github.runs["rust.yml"] *= 2
        with self.assertRaisesRegex(SyncError, "Ambiguous"):
            self.wait(github)


class PublicationTests(unittest.TestCase):
    def test_upstream_rewrite_during_ci_prevents_merging_removed_commit(self):
        git = Mock()
        git.ancestor.return_value = False
        with self.assertRaisesRegex(SyncError, "no longer on"):
            verify_upstream_lineage(git, UPSTREAM)
        git.ancestor.assert_called_once_with(UPSTREAM, "refs/remotes/upstream/master")

    def test_report_validation_rejects_conflicts_and_untrusted_identifiers(self):
        validate_report(REPORT)
        for key, value in (("status", "conflicts"), ("commit", "HEAD; echo bad"), ("base", None), ("repository", "other/repo")):
            report = dict(REPORT, **{key: value})
            with self.assertRaises(SyncError):
                validate_report(report)

    def test_generated_pr_body_preserves_manual_notes(self):
        previous = "Maintainer notes\n" + candidate_body(REPORT) + "\nMore notes"
        updated = candidate_body(dict(REPORT, count=9), previous)
        self.assertTrue(updated.startswith("Maintainer notes"))
        self.assertTrue(updated.endswith("More notes"))
        self.assertEqual(updated.count(START), 1)
        self.assertEqual(updated.count(END), 1)
        self.assertIn("Incoming upstream commits: 9", updated)

    def test_manual_changes_to_sync_pr_are_not_overwritten(self):
        git = Mock()
        git.ancestor.return_value = False
        old = "1" * 40
        pr = {"head": {"sha": old}, "body": candidate_body(dict(REPORT, commit="2" * 40))}
        with self.assertRaisesRegex(SyncError, "manual head"):
            check_existing_branch(git, old, REPORT, pr)
        pr["body"] = candidate_body(dict(REPORT, commit=old))
        check_existing_branch(git, old, REPORT, pr)

    def test_unknown_reserved_branch_is_not_force_overwritten(self):
        git = Mock()
        git.ancestor.return_value = False
        git.text.side_effect = [BASE, "human@example.invalid", "manual fix"]
        with self.assertRaisesRegex(SyncError, "non-bot"):
            check_existing_branch(git, "1" * 40, REPORT, None)

    def ready_pr(self, github):
        return github.api("pulls", "POST", {"body": candidate_body(REPORT)})

    def test_merge_uses_head_precondition_and_does_not_override_protection(self):
        github = FakeGitHub()
        self.ready_pr(github)
        self.assertEqual(merge_after_checks(github, REPORT, 7, "master", BRANCH), "e" * 40)
        request = next(call for call in github.calls if call[0] == "pulls/7/merge")
        self.assertEqual(request[2], {"sha": CANDIDATE, "merge_method": "merge"})

    def test_stale_base_head_or_protected_pr_never_merges(self):
        for condition in ("base", "head", "protected"):
            github = FakeGitHub()
            self.ready_pr(github)
            if condition == "base":
                github.refs["master"] = "f" * 40
            elif condition == "head":
                github.refs[BRANCH] = "f" * 40
            else:
                github.current_pr["mergeable_state"] = "blocked"
            with self.assertRaises(SyncError):
                merge_after_checks(github, REPORT, 7, "master", BRANCH)
            self.assertFalse(any(path == "pulls/7/merge" for path, _, _ in github.calls))

    def publish_fixture(self, github, auto_merge=True):
        git = Mock()
        github.refs[BRANCH] = None
        def git_run(*args, **kwargs):
            if args[0] == "push":
                github.refs[BRANCH] = CANDIDATE
            return Mock(stdout=b"")
        git.run.side_effect = git_run
        with tempfile.TemporaryDirectory() as tmp, patch("publish_sync.Git", return_value=git), patch("publish_sync.verify_bundle"), patch("publish_sync.wait_checks", return_value={"checked": True}):
            result = publish(Path(tmp), REPORT, Path(tmp) / "bundle", github, BRANCH, "master", MARKER, auto_merge, 60, Path(tmp) / "out")
            saved = json.loads((Path(tmp) / "out/publication.json").read_text())
        return result, saved, git

    def test_publication_uses_force_with_lease_and_explicit_release_dispatch(self):
        github = FakeGitHub()
        result, saved, git = self.publish_fixture(github)
        self.assertEqual(result["status"], "merged")
        self.assertTrue(saved["publication_dispatched"])
        push = next(call.args for call in git.run.call_args_list if call.args[0] == "push")
        self.assertIn(f"--force-with-lease=refs/heads/{BRANCH}:", push)
        publish_call = next(data for path, method, data in github.calls if path == "actions/workflows/rust.yml/dispatches" and data["ref"] == "master")
        self.assertEqual(publish_call["inputs"]["publish_nightly"], "true")

    def test_review_only_does_not_merge_or_publish_nightly(self):
        github = FakeGitHub()
        result, _, _ = self.publish_fixture(github, auto_merge=False)
        self.assertEqual(result["status"], "ready_for_review")
        self.assertFalse(any(path == "pulls/7/merge" for path, _, _ in github.calls))

    def test_administrator_read_is_not_required_for_github_token(self):
        github = FakeGitHub()
        github.policy = SyncError("GitHub settings HTTP 403: Resource not accessible by integration")
        result, _, _ = self.publish_fixture(github)
        self.assertEqual(result["status"], "merged")

    def test_disabled_pr_permission_stops_before_any_push(self):
        github = FakeGitHub()
        github.policy = {"can_approve_pull_request_reviews": False}
        with self.assertRaisesRegex(SyncError, "Allow GitHub Actions"):
            self.publish_fixture(github)
        self.assertFalse(any(path == "pulls" for path, _, _ in github.calls))


class WorkflowContractTests(unittest.TestCase):
    def test_required_workflows_accept_correlation_input_and_name_runs(self):
        workflows = Path(__file__).resolve().parents[1] / "workflows"
        for name in REQUIRED_WORKFLOWS:
            text = (workflows / name).read_text()
            self.assertIn("sync_run_id:", text)
            self.assertIn("format('Upstream sync {0}', inputs.sync_run_id)", text)
            for job_name in REQUIRED_JOBS[name]:
                self.assertIn("name: " + job_name, text)
        rust = (workflows / "rust.yml").read_text()
        self.assertIn("inputs.publish_nightly", rust)
        self.assertIn("github.ref == 'refs/heads/master'", rust)

    def test_preparation_has_no_publication_token_and_old_blind_merge_is_gone(self):
        workflow = (Path(__file__).resolve().parents[1] / "workflows/upstream-sync.yml").read_text()
        prepare_job = workflow.split("  prepare:", 1)[1].split("  publish:", 1)[0]
        self.assertNotIn("GH_TOKEN:", prepare_job)
        self.assertNotIn("contents: write", prepare_job)
        self.assertIn("persist-credentials: false", prepare_job)
        self.assertNotIn("git push -f", workflow)
        self.assertNotIn("gh pr checks", workflow)


class BundleIntegrityTests(unittest.TestCase):
    def test_write_permission_job_rechecks_fork_owned_content(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Repository(tmp)
            upstream = repo.upstream({"crates/pumpkin/src/new.rs": "new\n"})
            report = repo.prepare(upstream, bundle=True)
            bundle = Path(tmp) / "report/candidate.bundle"
            original_run = repo.git.run
            def offline_fetch(*args, **kwargs):
                if args[:3] == ("fetch", "--no-tags", f"https://github.com/{UPSTREAM_REPO}.git"):
                    original_run("update-ref", "refs/remotes/upstream/master", upstream)
                    return Mock(stdout=b"")
                return original_run(*args, **kwargs)
            with patch.object(repo.git, "run", side_effect=offline_fetch):
                verify_bundle(repo.git, report, bundle)
            entries = repo.git.tree(report["commit"])
            entries[".github/workflows/rust.yml"] = Entry("100644", repo.git.hash_blob(b"tampered CI\n"))
            tree = repo.git.write_tree(entries)
            commit = repo.git.commit_tree(tree, [report["base"], upstream], "forged", 1700000000)
            forged = dict(report, tree=tree, commit=commit)
            repo.git.run("update-ref", "refs/upstream-sync/candidate", commit)
            tampered = Path(tmp) / "tampered.bundle"
            repo.git.run("bundle", "create", str(tampered), "refs/upstream-sync/candidate", f"^{report['base']}", f"^{upstream}")
            with patch.object(repo.git, "run", side_effect=offline_fetch), self.assertRaisesRegex(SyncError, "fork-owned"):
                verify_bundle(repo.git, forged, tampered)


if __name__ == "__main__":
    unittest.main()
