#!/usr/bin/env python3
"""Prepare an upstream merge without modifying the caller's checkout or index.

Both upstream trees are mapped into the fork's crate namespace *before* the
three-way merge. Only a clean, audited tree becomes a candidate commit. The
candidate's real parents record both histories, rather than a synthetic fork
of upstream history. Run this preparation in a read-only-permissions CI job.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
from dataclasses import dataclass
from typing import Iterable


UPSTREAM_REPO = "Pumpkin-MC/Pumpkin"
STATE_PATH = ".github/sync/upstream-state.json"
WIT_PATH = "crates/pumpkin-plugin-wit"
BOT_NAME = "verdantgolem-sync-bot"
BOT_EMAIL = "41898282+github-actions[bot]@users.noreply.github.com"
OWNED_FILES = {
    "README.md", "CONTRIBUTING.md", "AGENTS.md", "CLAUDE.md", "assets/NOTICE.md",
    "Dockerfile", "docker-compose.yml", ".dockerignore", ".gitignore", "rustfmt.toml", "flake.nix", "typos.toml",
    "egg-verdantgolem.json", "docs/upstream-sync.md", "docs/carpet-features.md",
    "docs/technical-survival-testing.md",
}
OWNED_PREFIXES = (".github/", ".devcontainer/", ".comet/", "docs/comet/")
TEXT_SUFFIXES = {".rs", ".toml", ".lock", ".json", ".yml", ".yaml", ".md", ".txt", ".sh", ".nix", ".wit"}
OID = re.compile(r"[0-9a-f]{40}(?:[0-9a-f]{24})?\Z")


class SyncError(RuntimeError):
    pass


@dataclass(frozen=True)
class Entry:
    mode: str
    oid: str


def encode(value: str) -> bytes:
    return value.encode("utf-8", "surrogateescape")


def decode(value: bytes) -> str:
    return value.decode("utf-8", "surrogateescape")


def fork_path(path: str) -> str:
    if path == WIT_PATH or path.startswith(WIT_PATH + "/"):
        return path
    path = re.sub(r"^(crates|tools)/pumpkin(?=/|-|$)", r"\1/verdantgolem", path)
    return "egg-verdantgolem.json" if path == "egg-pumpkin.json" else path


def owned(path: str) -> bool:
    return path in OWNED_FILES or path.startswith(OWNED_PREFIXES)


class Git:
    def __init__(self, root: Path):
        self.root = root.resolve()
        self.blobs: dict[str, bytes] = {}

    def run(self, *args: str, data: bytes | None = None, ok=(0,), env=None):
        result = subprocess.run(
            ["git", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", *args],
            cwd=self.root, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=env,
        )
        if result.returncode not in ok:
            raise SyncError(f"git {' '.join(args)}: {decode(result.stderr).strip()}")
        return result

    def text(self, *args: str) -> str:
        return decode(self.run(*args).stdout).strip()

    def commit_id(self, ref: str) -> str:
        return self.text("rev-parse", "--verify", f"{ref}^{{commit}}")

    def ancestor(self, base: str, head: str) -> bool:
        return self.run("merge-base", "--is-ancestor", base, head, ok=(0, 1)).returncode == 0

    def tree(self, ref: str) -> dict[str, Entry]:
        result = {}
        for record in self.run("ls-tree", "-rz", "--full-tree", ref).stdout.split(b"\0"):
            if record:
                header, path = record.split(b"\t", 1)
                mode, _kind, oid = decode(header).split()
                result[decode(path)] = Entry(mode, oid)
        return result

    def read_blobs(self, ids: Iterable[str]) -> None:
        missing = sorted(set(ids) - self.blobs.keys())
        if not missing:
            return
        raw = self.run("cat-file", "--batch", data=encode("\n".join(missing) + "\n")).stdout
        cursor = 0
        for expected in missing:
            end = raw.index(b"\n", cursor)
            header = decode(raw[cursor:end]).split()
            if len(header) != 3 or header[0] != expected or header[1] != "blob":
                raise SyncError(f"Expected a blob for {expected}")
            length = int(header[2])
            cursor = end + 1
            self.blobs[expected] = raw[cursor:cursor + length]
            cursor += length + 1

    def blob(self, oid: str) -> bytes:
        self.read_blobs([oid])
        return self.blobs[oid]

    def hash_blob(self, data: bytes) -> str:
        oid = self.text_hash(data)
        self.blobs[oid] = data
        return oid

    def text_hash(self, data: bytes) -> str:
        return decode(self.run("hash-object", "-w", "--stdin", data=data).stdout).strip()

    def write_tree(self, entries: dict[str, Entry]) -> str:
        with tempfile.TemporaryDirectory(prefix="vg-sync-index-") as tmp:
            env = dict(os.environ, GIT_INDEX_FILE=str(Path(tmp) / "index"))
            self.run("read-tree", "--empty", env=env)
            records = b"".join(
                encode(f"{entry.mode} {entry.oid}\t{path}") + b"\0"
                for path, entry in sorted(entries.items())
            )
            self.run("update-index", "-z", "--index-info", data=records, env=env)
            return decode(self.run("write-tree", env=env).stdout).strip()

    def commit_tree(self, tree: str, parents: list[str], message: str, timestamp: int) -> str:
        env = dict(os.environ)
        for role in ("AUTHOR", "COMMITTER"):
            env[f"GIT_{role}_NAME"] = BOT_NAME
            env[f"GIT_{role}_EMAIL"] = BOT_EMAIL
            env[f"GIT_{role}_DATE"] = f"@{timestamp} +0000"
        args = ["commit-tree", tree]
        for parent in parents:
            args.extend(["-p", parent])
        return decode(self.run(*args, data=encode(message + "\n"), env=env).stdout).strip()


def text_file(path: str, entry: Entry) -> bool:
    return entry.mode != "160000" and (
        Path(path).suffix.lower() in TEXT_SUFFIXES
        or Path(path).name in {"Dockerfile", "Makefile", ".gitmodules"}
        or entry.mode == "120000"
    )


class Normalizer:
    def __init__(self, git: Git, trees: Iterable[dict[str, Entry]]):
        self.git = git
        self.crates = set()
        for tree in trees:
            for path in tree:
                match = re.match(r"^(?:crates|tools)/(pumpkin-[a-z0-9_-]+)/", path)
                if match and match[1] != "pumpkin-plugin-wit":
                    self.crates.add(match[1])
        names = self.crates | {name.replace("-", "_") for name in self.crates}
        self.tokens = re.compile(r"(?<![\w-])(?:" + "|".join(map(re.escape, sorted(names, key=len, reverse=True))) + r")(?![\w-])") if names else None
        self.cache: dict[tuple[str, str], str] = {}
        self.formatted: dict[str, str] = {}

    def content(self, raw: bytes, path: str) -> bytes:
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            return raw
        if "\0" in text:
            return raw
        if self.tokens:
            text = self.tokens.sub(lambda match: match[0].replace("pumpkin", "verdantgolem", 1), text)
        text = re.sub(r"\b(crates|tools)/pumpkin(?=/|[\"'\s]|$)", r"\1/verdantgolem", text)
        # WIT's pumpkin::plugin namespace is an ABI, not the Rust main crate.
        if "/pumpkin-api-macros/" in path or "/verdantgolem-api-macros/" in path:
            text = re.sub(r"\bpumpkin::", "verdantgolem::", text)
        else:
            text = re.sub(r"\bpumpkin::(?!plugin\b)", "verdantgolem::", text)
        if Path(path).name in {"Cargo.toml", "Cargo.lock"}:
            text = re.sub(r'(?m)^(\s*(?:name|package|default-run)\s*=\s*)["\']pumpkin["\']', r'\1"verdantgolem"', text)
            text = re.sub(r"(?m)^(\s*)pumpkin(\s*=)", r"\1verdantgolem\2", text)
            if Path(path).name == "Cargo.lock":
                text = re.sub(r'"pumpkin(?="| )', '"verdantgolem', text)
        return text.encode("utf-8")

    def tree(self, source: dict[str, Entry]) -> dict[str, Entry]:
        self.git.read_blobs(entry.oid for path, entry in source.items() if text_file(path, entry))
        result = {}
        for path, entry in source.items():
            target = fork_path(path)
            if text_file(path, entry):
                context = (Path(path).name if Path(path).name.startswith("Cargo.") else "macros" if "api-macros/" in path else "text")
                key = (entry.oid, context)
                if key not in self.cache:
                    raw = self.git.blob(entry.oid)
                    normalized = self.content(raw, path)
                    self.cache[key] = entry.oid if normalized == raw else self.git.hash_blob(normalized)
                entry = Entry(entry.mode, self.cache[key])
            if target in result and result[target] != entry:
                raise SyncError(f"Namespace mapping collision at {target!r}")
            result[target] = entry
        return result

    def format_rust(self, entries: dict[str, Entry], config: bytes, output: Path) -> dict[str, Entry]:
        """Make rename-induced line wrapping identical on all three sides.

        Rustfmt never runs build scripts. Numbered temporary files and
        skip_children prevent source module paths from traversing the checkout.
        Shared blobs are formatted once, rather than spawning per source file.
        """
        ids = {entry.oid for path, entry in entries.items()
               if path.endswith(".rs") and entry.mode.startswith("100") and not owned(path)}
        pending = sorted(ids - self.formatted.keys())
        self.git.read_blobs(pending)
        if pending:
            with tempfile.TemporaryDirectory(prefix="vg-sync-fmt-") as tmp:
                directory = Path(tmp)
                config_path = directory / "rustfmt.toml"
                config_path.write_bytes(config)
                files = []
                for index, oid in enumerate(pending):
                    path = directory / f"source_{index}.rs"
                    path.write_bytes(self.git.blob(oid))
                    files.append(path)
                with (output / "rustfmt.log").open("ab") as log:
                    for offset in range(0, len(files), 64):
                        result = subprocess.run(
                            ["rustfmt", "--edition", "2024", "--config-path", str(config_path),
                             "--config", "skip_children=true", *map(str, files[offset:offset + 64])],
                            stdout=log, stderr=subprocess.STDOUT,
                        )
                        if result.returncode:
                            raise SyncError("Rust namespace formatting failed; see rustfmt.log")
                for oid, path in zip(pending, files):
                    raw = path.read_bytes()
                    target = oid if raw == self.git.blob(oid) else self.git.hash_blob(raw)
                    self.formatted[oid] = target
                    self.formatted[target] = target
        return {path: Entry(entry.mode, self.formatted[entry.oid])
                if path.endswith(".rs") and entry.mode.startswith("100") and not owned(path)
                else entry for path, entry in entries.items()}


def audit_tree(entries: dict[str, Entry]) -> None:
    bad = [path for path in entries if fork_path(path) != path]
    if bad:
        raise SyncError("Unmapped upstream paths: " + json.dumps(sorted(bad)))


def parse_merge_output(raw: bytes, conflicted: bool) -> tuple[str, list[str]]:
    fields = raw.split(b"\0")
    tree = decode(fields[0]).strip()
    if not OID.fullmatch(tree):
        raise SyncError("git merge-tree did not return a tree object")
    paths = []
    if conflicted:
        for field in fields[1:]:
            if not field:
                break
            paths.append(decode(field))
        if not paths:
            raise SyncError("Merge failed without a conflict path report")
    return tree, sorted(set(paths))


def refresh_lock(git: Git, commit: str, entries: dict[str, Entry], output: Path) -> dict[str, Entry]:
    if "Cargo.toml" not in entries:
        return entries
    # Never pass a publication token to manifest/credential-provider execution.
    env = {key: value for key, value in os.environ.items() if key in {
        "PATH", "HOME", "USER", "LOGNAME", "TMPDIR", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_HOME",
        "SSL_CERT_FILE", "SSL_CERT_DIR", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY",
        "SYSTEMROOT", "COMSPEC",
    }}
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
    with tempfile.TemporaryDirectory(prefix="vg-sync-lock-") as tmp:
        checkout = Path(tmp) / "checkout"
        git.run("worktree", "add", "--detach", str(checkout), commit)
        try:
            with (output / "cargo-lock.log").open("wb") as log:
                for offline in (True, False):
                    command = ["cargo", "update", "--workspace"] + (["--offline"] if offline else [])
                    result = subprocess.run(command, cwd=checkout, env=env, stdout=log, stderr=subprocess.STDOUT)
                    if result.returncode == 0:
                        break
                else:
                    raise SyncError("Workspace lockfile refresh failed; see cargo-lock.log")
            lock = checkout / "Cargo.lock"
            if not lock.is_file():
                raise SyncError("Cargo did not produce the workspace lockfile")
            entries = dict(entries)
            entries["Cargo.lock"] = Entry("100644", git.hash_blob(lock.read_bytes()))
            return entries
        finally:
            git.run("worktree", "remove", "--force", str(checkout))


def prepare(root: Path, head_ref: str, upstream_ref: str, output: Path, refresh: bool = False, bundle: bool = False, format_rust: bool = False) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    git = Git(root)
    head, upstream = git.commit_id(head_ref), git.commit_id(upstream_ref)
    report = {"schema_version": 1, "repository": UPSTREAM_REPO, "base": head, "upstream": upstream}
    ours = git.tree(head)
    state = ours.get(STATE_PATH)
    if state:
        try:
            previous = json.loads(git.blob(state.oid))
            recorded = previous["upstream_commit"]
            if previous["schema_version"] != 1 or previous["repository"] != UPSTREAM_REPO or not OID.fullmatch(recorded):
                raise ValueError("invalid upstream state")
        except (ValueError, KeyError, TypeError) as error:
            raise SyncError(f"Invalid {STATE_PATH}: {error}") from error
        if not git.ancestor(recorded, head) or not git.ancestor(recorded, upstream):
            raise SyncError("Upstream history/state diverged; inspect the recorded upstream commit before syncing")
    if git.ancestor(upstream, head):
        return dict(report, status="up_to_date", count=0)
    bases = git.text("merge-base", "--all", head, upstream).splitlines()
    if len(bases) != 1:
        raise SyncError("Expected exactly one common upstream merge base")
    base = bases[0]
    report.update(merge_base=base, count=int(git.text("rev-list", "--count", f"{head}..{upstream}")))
    raw_base, raw_upstream = git.tree(base), git.tree(upstream)
    normalizer = Normalizer(git, [raw_base, raw_upstream])
    base_tree, upstream_tree = normalizer.tree(raw_base), normalizer.tree(raw_upstream)

    merge_head = head
    if format_rust:
        config = git.blob(ours["rustfmt.toml"].oid) if "rustfmt.toml" in ours else b""
        base_tree = normalizer.format_rust(base_tree, config, output)
        upstream_tree = normalizer.format_rust(upstream_tree, config, output)
        formatted_ours = normalizer.format_rust(ours, config, output)
        merge_head = git.commit_tree(git.write_tree(formatted_ours), [], f"Formatted fork {head}", 1700000000)

    # Fork-owned files remain unchanged even when Git would merge them cleanly.
    for path in set(base_tree) | set(upstream_tree):
        if owned(path):
            if path in base_tree:
                upstream_tree[path] = base_tree[path]
            else:
                upstream_tree.pop(path, None)
    # These two inputs intentionally follow upstream, then the lock is refreshed
    # for the merged workspace. Setting base=ours makes this a one-sided update.
    for path in ("Cargo.lock", WIT_PATH):
        if path in upstream_tree:
            if path in ours:
                base_tree[path] = ours[path]
            else:
                base_tree.pop(path, None)

    timestamp = max(int(git.text("show", "-s", "--format=%ct", ref)) for ref in (head, upstream)) + 1
    base_commit = git.commit_tree(git.write_tree(base_tree), [], f"Normalized base {base}", timestamp)
    upstream_commit = git.commit_tree(git.write_tree(upstream_tree), [], f"Normalized upstream {upstream}", timestamp)
    merged = git.run("merge-tree", "--write-tree", "--name-only", "-z", "--messages", f"--merge-base={base_commit}", merge_head, upstream_commit, ok=(0, 1))
    tree, conflicts = parse_merge_output(merged.stdout, merged.returncode == 1)
    (output / "merge-details.txt").write_text(decode(merged.stdout).replace("\0", "\n"), errors="backslashreplace")
    if conflicts:
        (output / "merge.patch").write_bytes(git.run("diff", "--binary", head, tree).stdout)
        return dict(report, status="conflicts", conflicts=conflicts, merge_tree=tree)

    entries = git.tree(tree)
    audit_tree(entries)
    state_data = {"schema_version": 1, "repository": UPSTREAM_REPO, "upstream_commit": upstream, "normalization_version": 1}
    entries[STATE_PATH] = Entry("100644", git.hash_blob(encode(json.dumps(state_data, indent=2) + "\n")))
    message = f"chore: sync upstream Pumpkin ({upstream[:12]})"
    candidate = git.commit_tree(git.write_tree(entries), [head, upstream], message, timestamp)
    if refresh:
        entries = refresh_lock(git, candidate, entries, output)
        audit_tree(entries)
        candidate = git.commit_tree(git.write_tree(entries), [head, upstream], message, timestamp)
    if bundle:
        ref = "refs/upstream-sync/candidate"
        # This ref is private to this preparation checkout, never master or the
        # caller's active branch. The publication job verifies its parents/tree.
        git.run("update-ref", ref, candidate)
        git.run("bundle", "create", str((output / "candidate.bundle").resolve()), ref, f"^{head}", f"^{upstream}")
    (output / "changes.txt").write_bytes(git.run("diff", "--stat", head, candidate).stdout)
    return dict(report, status="ready", commit=candidate, tree=git.text("rev-parse", f"{candidate}^{{tree}}"), conflicts=[])


def write_report(output: Path, report: dict) -> None:
    output.mkdir(parents=True, exist_ok=True)
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    lines = ["## Upstream sync preparation", "", f"Status: **{report['status']}**", ""]
    for key in ("base", "upstream", "merge_base", "commit", "count", "error"):
        if key in report:
            lines.append(f"- {key}: `{json.dumps(report[key])}`")
    if report.get("conflicts"):
        lines.extend(["", "These overlapping changes require review; no branch was pushed.", "", "```json", json.dumps(report["conflicts"], indent=2), "```"])
    summary = "\n".join(lines) + "\n"
    (output / "summary.md").write_text(summary)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as file:
            file.write(summary)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as file:
            for key in ("status", "base", "upstream", "commit", "count"):
                file.write(f"{key}={report.get(key, '')}\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--upstream", default="upstream/master")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--refresh-lock", action="store_true")
    parser.add_argument("--format-rust", action="store_true", help="Normalize rename-induced formatting on all three Rust trees")
    parser.add_argument("--bundle", action="store_true")
    args = parser.parse_args()
    try:
        report = prepare(args.root, args.head, args.upstream, args.output, args.refresh_lock, args.bundle, args.format_rust)
    except (SyncError, OSError, ValueError) as error:
        report = {"schema_version": 1, "status": "error", "error": str(error)}
    write_report(args.output, report)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] in {"ready", "up_to_date"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
