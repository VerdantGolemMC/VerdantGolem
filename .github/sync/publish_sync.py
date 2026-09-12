#!/usr/bin/env python3
"""Publish a prepared bundle, explicitly run pinned CI, then merge via a PR.

Run only from the trusted workflow checkout, never from the candidate tree.
GITHUB_TOKEN-created pushes/PRs do not reliably trigger other workflows, so
workflow_dispatch and correlated run IDs are deliberate requirements here.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import quote, urlencode

from upstream_sync import BOT_EMAIL, Git, OID, STATE_PATH, UPSTREAM_REPO, SyncError, audit_tree, owned


REQUIRED_WORKFLOWS = ("rust.yml", "typos.yml", "nix.yml")
REQUIRED_JOBS = {
    "rust.yml": {
        "Check formatting": 1, "Detect unused dependencies": 1,
        "Run lints (debug)": 1, "Run lints (release)": 1,
        "Build project and test": 3, "Test on Android AArch64": 1,
        "Build project in release": 5,
        "Build project in release for Android (AArch64)": 1,
        "Upstream sync regression tests": 1, "Technical survival smoke tests": 1,
    },
    "typos.yml": {"Spell Check with Typos": 1},
    "nix.yml": {"Validate Flake & Build": 1},
}
START = "<!-- verdantgolem-upstream-sync:start -->"
END = "<!-- verdantgolem-upstream-sync:end -->"


class GitHub:
    def __init__(self, repository: str):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise SyncError("Expected an owner/repository GitHub identifier")
        self.repository = repository

    def api(self, path: str, method="GET", data=None, missing_ok=False):
        endpoint = f"repos/{self.repository}" + (f"/{path}" if path else "")
        command = ["gh", "api", endpoint, "--method", method]
        if data is not None:
            command.extend(["--input", "-"])
        result = subprocess.run(command, input=json.dumps(data) if data is not None else None, capture_output=True, text=True)
        if result.returncode:
            if missing_ok and "HTTP 404" in result.stderr:
                return None
            raise SyncError(f"GitHub {method} {path}: {result.stderr.strip()}")
        return json.loads(result.stdout) if result.stdout.strip() else None

    def ref(self, branch: str, missing_ok=False):
        ref = self.api("git/ref/heads/" + quote(branch, safe="/"), missing_ok=missing_ok)
        return ref["object"]["sha"] if ref else None


def validate_report(report: dict) -> None:
    if report.get("schema_version") != 1 or report.get("status") != "ready" or report.get("repository") != UPSTREAM_REPO:
        raise SyncError("Preparation did not produce a ready upstream candidate")
    for key in ("base", "upstream", "commit", "tree"):
        if not isinstance(report.get(key), str) or not OID.fullmatch(report[key]):
            raise SyncError(f"Invalid candidate {key}")


def candidate_body(report: dict, previous="") -> str:
    block = f"""{START}
Automated upstream synchronization from {UPSTREAM_REPO}.

- Upstream commit: `{report['upstream']}`
- Base commit: `{report['base']}`
- Candidate commit: `{report['commit']}`
- Incoming upstream commits: {report.get('count', '?')}
- Crate paths/imports are normalized before a three-way merge. Fork-owned CI,
  branding and Carpet changes are preserved; unresolved overlaps stop preparation.
- Rust, spelling and Nix workflows are explicitly dispatched for this exact head.
  Failed, cancelled, missing or stale checks prevent automatic merging.

{END}"""
    if START in previous and END in previous:
        before, rest = previous.split(START, 1)
        _, after = rest.split(END, 1)
        return before + block + after
    return (previous.rstrip() + "\n\n" if previous else "") + block


def find_pr(github: GitHub, branch: str, base_branch: str):
    owner = github.repository.split("/")[0]
    prs = github.api("pulls?" + urlencode({"state": "open", "head": f"{owner}:{branch}", "base": base_branch, "per_page": 100}))
    if len(prs) > 1:
        raise SyncError("Multiple open synchronization PRs match this branch")
    return prs[0] if prs else None


def verify_bundle(git: Git, report: dict, bundle: Path) -> None:
    verify_upstream_lineage(git, report["upstream"])
    git.run("bundle", "verify", str(bundle))
    expected = report["commit"] + " refs/upstream-sync/candidate"
    if git.text("bundle", "list-heads", str(bundle)) != expected:
        raise SyncError("Bundle candidate ref does not match the preparation report")
    git.run("fetch", str(bundle), "+refs/upstream-sync/candidate:refs/upstream-sync/imported")
    commit = report["commit"]
    if git.text("show", "-s", "--format=%P", commit).split() != [report["base"], report["upstream"]]:
        raise SyncError("Candidate parents do not match the pinned base/upstream commits")
    if git.text("rev-parse", f"{commit}^{{tree}}") != report["tree"]:
        raise SyncError("Candidate tree does not match the preparation report")
    entries = git.tree(commit)
    audit_tree(entries)
    baseline = git.tree(report["base"])
    for path in set(baseline) | set(entries):
        if path != STATE_PATH and owned(path) and baseline.get(path) != entries.get(path):
            raise SyncError(f"Candidate changed fork-owned file {path!r}")
    state = json.loads(git.blob(entries[STATE_PATH].oid))
    if (state.get("schema_version") != 1 or state.get("normalization_version") != 1
            or state.get("repository") != UPSTREAM_REPO or state.get("upstream_commit") != report["upstream"]):
        raise SyncError("Candidate upstream state does not match its parent")


def verify_upstream_lineage(git: Git, upstream: str) -> None:
    git.run("fetch", "--no-tags", f"https://github.com/{UPSTREAM_REPO}.git", "+master:refs/remotes/upstream/master")
    if not git.ancestor(upstream, "refs/remotes/upstream/master"):
        raise SyncError("The prepared upstream commit is no longer on the configured upstream master history")


def check_existing_branch(git: Git, tip: str | None, report: dict, pr) -> None:
    if not tip or tip == report["commit"] or git.ancestor(tip, report["base"]):
        return
    if pr:
        recorded = re.search(r"Candidate commit: `([0-9a-f]{40,64})`", pr.get("body") or "")
        if not recorded or recorded[1] != tip or pr["head"]["sha"] != tip:
            raise SyncError("The synchronization PR has manual head changes; they will not be overwritten")
        return
    # Recover a run interrupted between pushing and opening its PR. An ordinary
    # manual commit does not have the generated merge's author and two parents.
    parents = git.text("show", "-s", "--format=%P", tip).split()
    author = git.text("show", "-s", "--format=%ae", tip)
    message = git.text("show", "-s", "--format=%s", tip)
    if len(parents) != 2 or author != BOT_EMAIL or not message.startswith("chore: sync upstream Pumpkin ("):
        raise SyncError("The reserved synchronization branch has non-bot changes; choose a different branch or review them")


def dispatch_checks(github: GitHub, branch: str, marker: str) -> None:
    for workflow in REQUIRED_WORKFLOWS:
        github.api(f"actions/workflows/{workflow}/dispatches", method="POST", data={"ref": branch, "inputs": {"sync_run_id": marker}})


def verify_jobs(github: GitHub, runs: dict) -> None:
    for workflow, run in runs.items():
        jobs = github.api(f"actions/runs/{run['id']}/jobs?filter=latest&per_page=100")["jobs"]
        for prefix, minimum in REQUIRED_JOBS[workflow].items():
            matches = [job for job in jobs if job["name"] == prefix or job["name"].startswith(prefix + " (")]
            if len(matches) < minimum:
                raise SyncError(f"{workflow}: required job/matrix {prefix!r} is missing")
            if any(job.get("status") != "completed" or job.get("conclusion") != "success" for job in matches):
                raise SyncError(f"{workflow}: required job {prefix!r} did not run successfully (skips are not success)")


def wait_checks(github: GitHub, branch: str, commit: str, marker: str, timeout: float, progress, clock=time.monotonic, sleep=time.sleep):
    deadline = clock() + timeout
    expected_title = f"Upstream sync {marker}"
    found = {}
    while clock() < deadline:
        for workflow in REQUIRED_WORKFLOWS:
            query = urlencode({"event": "workflow_dispatch", "branch": branch, "head_sha": commit, "per_page": 100})
            response = github.api(f"actions/workflows/{workflow}/runs?{query}")
            matches = [run for run in response["workflow_runs"] if
                       run.get("display_title") == expected_title
                       and run.get("head_sha") == commit
                       and run.get("head_branch") == branch
                       and run.get("event") == "workflow_dispatch"]
            if len(matches) > 1:
                raise SyncError(f"Ambiguous correlated CI runs for {workflow}")
            if not matches:
                continue
            run = matches[0]
            found[workflow] = {key: run.get(key) for key in ("id", "status", "conclusion", "html_url", "head_sha")}
            progress(found)
            if run["status"] == "completed" and run.get("conclusion") != "success":
                raise SyncError(f"{workflow} finished as {run.get('conclusion')}: {run['html_url']}")
        if len(found) == len(REQUIRED_WORKFLOWS) and all(run["status"] == "completed" and run["conclusion"] == "success" for run in found.values()):
            verify_jobs(github, found)
            return found
        sleep(min(30, max(0, deadline - clock())))
    missing = sorted(set(REQUIRED_WORKFLOWS) - set(found))
    raise SyncError(f"Timed out waiting for pinned CI; missing workflows: {missing}; no merge performed")


def merge_after_checks(github: GitHub, report: dict, pr_number: int, base_branch: str, sync_branch: str):
    if github.ref(base_branch) != report["base"]:
        raise SyncError("The base branch advanced during CI; rerun preparation instead of merging an untested base")
    if github.ref(sync_branch) != report["commit"]:
        raise SyncError("The synchronization branch changed during CI; no merge performed")
    pr = github.api(f"pulls/{pr_number}")
    if pr["state"] != "open" or pr["head"]["sha"] != report["commit"] or pr["base"]["sha"] != report["base"]:
        raise SyncError("The PR no longer matches the verified base/head")
    if pr.get("mergeable") is not True or pr.get("mergeable_state") != "clean":
        raise SyncError("GitHub has not marked this PR mergeable; required reviews/rules are not bypassed")
    result = github.api(f"pulls/{pr_number}/merge", method="PUT", data={"sha": report["commit"], "merge_method": "merge"})
    if not result.get("merged"):
        raise SyncError(f"GitHub did not merge the PR: {result.get('message')}")
    return result["sha"]


def publish(root: Path, report: dict, bundle: Path, github: GitHub, branch: str, base_branch: str, marker: str, auto_merge: bool, timeout: float, output: Path):
    validate_report(report)
    if not re.fullmatch(r"[A-Za-z0-9_-]+", marker):
        raise SyncError("Invalid CI correlation ID")
    git = Git(root)
    git.run("check-ref-format", "--branch", branch)
    git.run("check-ref-format", "--branch", base_branch)
    state = {"status": "preflight", "candidate": report["commit"], "base": report["base"], "ci": {}}
    output.mkdir(parents=True, exist_ok=True)

    def save():
        (output / "publication.json").write_text(json.dumps(state, indent=2) + "\n")

    def progress(runs):
        state["ci"] = dict(runs)
        save()

    pr = None
    try:
        save()
        try:
            policy = github.api("actions/permissions/workflow")
        except SyncError as error:
            # This settings endpoint may require administrator-read privileges,
            # which GITHUB_TOKEN deliberately lacks. The PR API still enforces
            # its setting; do not request a privileged PAT just for this read.
            if "HTTP 403" not in str(error):
                raise
            policy = None
        if policy is not None and not policy.get("can_approve_pull_request_reviews"):
            raise SyncError("Enable Settings > Actions > General > Allow GitHub Actions to create and approve pull requests; keep default workflow permissions read-only")
        if auto_merge and not github.api("").get("allow_merge_commit"):
            raise SyncError("Merge commits must be enabled so the upstream parent history is preserved")
        if github.ref(base_branch) != report["base"]:
            raise SyncError("The base branch advanced after preparation; rerun the workflow")
        verify_bundle(git, report, bundle)
        tip = github.ref(branch, missing_ok=True)
        if tip:
            git.run("fetch", "--no-tags", "origin", tip)
        pr = find_pr(github, branch, base_branch)
        check_existing_branch(git, tip, report, pr)
        if tip != report["commit"]:
            git.run("push", f"--force-with-lease=refs/heads/{branch}:{tip or ''}", "origin", f"{report['commit']}:refs/heads/{branch}")
        body = candidate_body(report, (pr or {}).get("body") or "")
        payload = {"title": "chore: sync upstream Pumpkin", "body": body}
        if pr:
            pr = github.api(f"pulls/{pr['number']}", method="PATCH", data=payload)
        else:
            pr = github.api("pulls", method="POST", data=dict(payload, head=branch, base=base_branch))
        state.update(status="checking", pull_request=pr["html_url"], pr_number=pr["number"])
        save()
        dispatch_checks(github, branch, marker)
        state["ci"] = wait_checks(github, branch, report["commit"], marker, timeout, progress)
        if not auto_merge:
            state["status"] = "ready_for_review"
            save()
            return state
        verify_upstream_lineage(git, report["upstream"])
        state["merge_commit"] = merge_after_checks(github, report, pr["number"], base_branch, branch)
        state["status"] = "merged"
        save()
        # GITHUB_TOKEN merge pushes are not relied upon to trigger publication.
        github.api("actions/workflows/rust.yml/dispatches", method="POST", data={
            "ref": base_branch, "inputs": {"sync_run_id": marker + "-publish", "publish_nightly": "true"},
        })
        github.api("actions/workflows/docker.yml/dispatches", method="POST", data={"ref": base_branch})
        state["publication_dispatched"] = True
        save()
        return state
    except (SyncError, OSError, ValueError, KeyError) as error:
        state.update(status="merged_publication_failed" if "merge_commit" in state else "error", error=str(error))
        save()
        if pr:
            try:
                github.api(f"issues/{pr['number']}/comments", method="POST", data={"body": "Upstream synchronization stopped. No branch-protection override was used.\n\n" + str(error)})
            except SyncError:
                pass
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--branch", default="codex/upstream-sync")
    parser.add_argument("--base-branch", default="master")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--auto-merge", choices=("true", "false"), default="true")
    parser.add_argument("--timeout", type=float, default=10800)
    args = parser.parse_args()
    try:
        result = publish(args.root, json.loads(args.report.read_text()), args.bundle, GitHub(args.repository), args.branch, args.base_branch, args.run_id, args.auto_merge == "true", args.timeout, args.output)
        print(json.dumps(result, indent=2))
        return 0
    except (SyncError, OSError, ValueError, KeyError) as error:
        print(f"Upstream synchronization stopped: {error}")
        return 1
    finally:
        report_path = args.output / "publication.json"
        if report_path.exists() and os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
                summary.write("## Upstream synchronization publication\n\n```json\n" + report_path.read_text() + "```\n")


if __name__ == "__main__":
    raise SystemExit(main())
