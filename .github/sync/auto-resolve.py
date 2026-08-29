#!/usr/bin/env python3
"""Automatic conflict resolution for VerdantGolem upstream syncs.

For every remaining conflicted path, decide whether OUR divergence from the
merge base is purely mechanical (brand/crate renames). If it is, take the
upstream side and re-apply the mechanical rename. Otherwise leave the path
for human resolution.

Run from the repository root while a merge is in progress.
"""

from __future__ import annotations

import re
import subprocess
import sys
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


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"git {args} failed: {result.stderr.strip()}")
    return result.stdout


def try_git(*args: str) -> str | None:
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else None


def to_upstream_path(path: str) -> str:
    replacements = [
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
    for ours, upstream in replacements:
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
    # Path references into the renamed main crate directory.
    content = content.replace("crates/pumpkin/", "crates/verdantgolem/")
    return content


def normalize(content: str) -> str:
    """Collapse mechanical rename differences so both sides compare equal."""
    return content.lower().replace("verdantgolem", "pumpkin")


def is_mechanical(base: str, ours: str) -> bool:
    return normalize(base) == normalize(ours)


def main() -> int:
    base_ref = git("merge-base", "HEAD", "upstream/master").strip()
    conflicted = git("diff", "--name-only", "--diff-filter=U").split()
    if not conflicted:
        print("No conflicted paths.")
        return 0

    resolved: list[str] = []
    unresolved: list[str] = []

    for path in conflicted:
        upstream_path = to_upstream_path(path)
        try:
            base = git("show", f"{base_ref}:{upstream_path}")
        except RuntimeError:
            base = None
        try:
            theirs = git("show", f"upstream/master:{upstream_path}")
        except RuntimeError:
            theirs = None

        ours_path = Path(path)
        ours = ours_path.read_text() if ours_path.exists() else ""

        if theirs is not None and base is not None and is_mechanical(base, ours):
            ours_path.write_text(apply_rename(theirs))
            git("add", "--", path)
            resolved.append(path)
        elif theirs is None and base is not None and is_mechanical(base, ours):
            # Upstream deleted the file and our side only carries renames.
            git("rm", "-q", "--", path)
            resolved.append(path)
        else:
            unresolved.append(path)
            print(f"UNRESOLVED {path} (upstream: {upstream_path})")

    print(f"auto-resolved: {len(resolved)}; unresolved: {len(unresolved)}")
    Path("conflicted-paths.txt").write_text(
        "".join(f"{path}\n" for path in unresolved)
    )
    return 1 if unresolved else 0


if __name__ == "__main__":
    sys.exit(main())
