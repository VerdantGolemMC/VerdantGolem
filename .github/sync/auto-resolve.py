#!/usr/bin/env python3
"""Automatic conflict resolution for VerdantGolem upstream syncs.

Resolution order per conflicted path (all paths below are our verdantgolem
paths; upstream paths are the pumpkin equivalents):

1. New upstream file (no merge-base copy, nothing on our side either):
   take upstream content with the mechanical rename re-applied.
2. Add/add where our copy is normalize-equal to upstream's: same as (1).
3. Both sides exist with a merge-base copy: run a real three-way
   `git merge-file`. Clean results are written and staged; overlapping
   hunks stay conflicted for the report.
4. Upstream deleted the file and our copy carries only mechanical rename
   differences: delete ours.

Anything left over is written to conflicted-paths.txt for the workflow to
report; nothing half-merged is ever pushed.

Run from the repository root while a merge is in progress.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

CRATE_SNAKE = [
    "api_macros", "codecs", "config", "data", "inventory", "macros", "nbt",
    "plugin_api", "plugin_utils", "protocol", "util", "world", "codegen",
    "fuzzer",
]
CRATE_HYPHEN = [
    "api-macros", "codecs", "config", "data", "inventory", "macros", "nbt",
    "plugin-api", "plugin-utils", "protocol", "util", "world", "codegen",
    "fuzzer",
]

PATH_REPLACEMENTS = [
    ("crates/verdantgolem-plugin-api/", "crates/pumpkin-plugin-api/"),
    ("crates/verdantgolem-plugin-utils/", "crates/pumpkin-plugin-utils/"),
    ("crates/verdantgolem-protocol/", "crates/pumpkin-protocol/"),
    ("crates/verdantgolem-data/", "crates/pumpkin-data/"),
    ("crates/verdantgolem-world/", "crates/pumpkin-world/"),
    ("crates/verdantgolem-util/", "crates/pumpkin-util/"),
    ("crates/verdantgolem-nbt/", "crates/pumpkin-nbt/"),
    ("crates/verdantgolem-config/", "crates/pumpkin-config/"),
    ("crates/verdantgolem-inventory/", "crates/pumpkin-inventory/"),
    ("crates/verdantgolem-macros/", "crates/pumpkin-macros/"),
    ("crates/verdantgolem-codecs/", "crates/pumpkin-codecs/"),
    ("crates/verdantgolem-api-macros/", "crates/pumpkin-api-macros/"),
    ("crates/verdantgolem/", "crates/pumpkin/"),
    ("tools/verdantgolem-", "tools/pumpkin-"),
]


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"git {args} failed: {result.stderr.strip()}")
    return result.stdout


def try_git(*args: str) -> str | None:
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else None


def try_git_bytes(*args: str) -> bytes | None:
    result = subprocess.run(["git", *args], capture_output=True)
    return result.stdout if result.returncode == 0 else None


def decode_blob(raw: bytes | None) -> str | None:
    if raw is None:
        return None
    try:
        return raw.decode()
    except UnicodeDecodeError:
        return None  # binary blob: leave it to manual resolution


def to_upstream_path(path: str) -> str:
    for ours, upstream in PATH_REPLACEMENTS:
        path = path.replace(ours, upstream)
    return path


def apply_rename(content: str) -> str:
    """Re-apply the mechanical rename to upstream content.

    Deliberately narrower than a global replace: WIT contract names and other
    upstream-compatibility identifiers must stay Pumpkin-flavored.
    """
    for crate in CRATE_SNAKE:
        content = re.sub(rf"\bpumpkin_{crate}\b", f"verdantgolem_{crate}", content)
    for crate in CRATE_HYPHEN:
        content = re.sub(rf"\bpumpkin-{crate}\b", f"verdantgolem-{crate}", content)
    # Main crate paths, excluding the WIT namespace `pumpkin::plugin`.
    content = re.sub(r"\bpumpkin::(?!plugin\b)", "verdantgolem::", content)
    # Exception: api-macros templates generate plugin-side crate references
    # that ARE the renamed crate (only the WIT bindings keep pumpkin::plugin).
    if content.startswith("#!")[0] if False else False:
        pass
    content = content.replace("crates/pumpkin/", "crates/verdantgolem/")
    return content


def normalize(content: str) -> str:
    """Collapse mechanical rename differences so both sides compare equal."""
    return content.lower().replace("verdantgolem", "pumpkin")


def three_way_merge(ours: str, base: str, theirs: str) -> str | None:
    """Run git merge-file; return the merged content or None on conflict."""
    with tempfile.TemporaryDirectory() as tmp:
        ours_path = Path(tmp) / "ours"
        base_path = Path(tmp) / "base"
        theirs_path = Path(tmp) / "theirs"
        ours_path.write_text(ours)
        base_path.write_text(base)
        theirs_path.write_text(theirs)
        result = subprocess.run(
            ["git", "merge-file", "-p", "-L", "ours", "-L", "base",
             "-L", "theirs", str(ours_path), str(base_path), str(theirs_path)],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return result.stdout
        return None


def main() -> int:
    base_ref = git("merge-base", "HEAD", "upstream/master").strip()
    conflicted = git("diff", "--name-only", "--diff-filter=U").split()
    if not conflicted:
        print("No conflicted paths.")
        Path("conflicted-paths.txt").write_text("")
        return 0

    resolved: list[str] = []
    unresolved: list[str] = []

    for path in conflicted:
        upstream_path = to_upstream_path(path)
        ours_path = Path(path)
        # During a conflicted merge the working tree holds marker-laden
        # content; stage 2 is the clean "ours" version.
        ours = decode_blob(try_git_bytes("show", f":2:{path}"))
        if ours is None:
            ours = decode_blob(try_git_bytes("show", f"HEAD:{path}"))
        if ours is None and ours_path.exists():
            try:
                ours = ours_path.read_text()
            except UnicodeDecodeError:
                ours = None
        base = decode_blob(try_git_bytes("show", f"{base_ref}:{upstream_path}"))
        # Prefer stage 3 (mapped onto our path by rename detection), then the
        # upstream path itself.
        theirs = decode_blob(try_git_bytes("show", f":3:{path}"))
        if theirs is None:
            theirs = decode_blob(try_git_bytes("show", f"upstream/master:{upstream_path}"))

        merged: str | None = None
        action = ""

        if path == "Cargo.lock":
            # The lock is upstream-shaped; cargo rewrites our package entries
            # during the next build, so upstream's side always wins.
            if theirs is not None:
                ours_path.write_text(theirs)
                git("add", "--", path)
                action = "lockfile tracks upstream"
            else:
                action = ""
        elif ours is None and theirs is not None:
            # New upstream file (possibly a rename detected on our side).
            merged = apply_rename(theirs)
            ours_path.write_text(merged)
            git("add", "--", path)
            action = "new upstream file"
        elif ours is not None and theirs is not None and base is None:
            # Add/add: accept when normalize-equal, otherwise report.
            if normalize(ours) == normalize(theirs):
                ours_path.write_text(apply_rename(theirs))
                git("add", "--", path)
                action = "add/add normalize-equal"
            else:
                action = ""
        elif ours is not None and theirs is not None and base is not None:
            if normalize(ours) == normalize(base):
                # Our side only carries mechanical renames: take theirs.
                merged = apply_rename(theirs)
                action = "ours is mechanical only"
            else:
                merged = three_way_merge(ours, base, theirs)
                action = "three-way merge"
            if merged is not None:
                ours_path.write_text(merged)
                git("add", "--", path)
            else:
                action = ""
        elif ours is not None and theirs is None and base is not None:
            if normalize(ours) == normalize(base):
                git("rm", "-q", "--", path)
                resolved.append(path)
                action = "upstream deleted; ours mechanical"
            else:
                action = ""
        else:
            action = ""

        if action:
            print(f"RESOLVED  {path}  ({action})")
            resolved.append(path)
        else:
            print(f"UNRESOLVED {path}  (upstream: {upstream_path})")
            unresolved.append(path)

    print(f"auto-resolved: {len(resolved)}; unresolved: {len(unresolved)}")
    Path("conflicted-paths.txt").write_text(
        "".join(f"{path}\n" for path in unresolved)
    )
    return 1 if unresolved else 0


if __name__ == "__main__":
    sys.exit(main())
