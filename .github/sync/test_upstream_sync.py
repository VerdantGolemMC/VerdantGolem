from pathlib import Path
import json
import os
import tempfile
import unittest
from unittest.mock import patch
import subprocess

from upstream_sync import Entry, Git, Normalizer, STATE_PATH, SyncError, WIT_PATH, audit_tree, fork_path, prepare


class Repository:
    def __init__(self, directory):
        self.root = Path(directory) / "repo"
        self.root.mkdir()
        self.git = Git(self.root)
        self.git.run("init", "-b", "upstream")
        self.git.run("config", "user.name", "Fixture")
        self.git.run("config", "user.email", "fixture@example.invalid")
        self.write("crates/pumpkin/src/lib.rs", "one\ntwo\nthree\nfour\nfive\nsix\n")
        self.write("crates/pumpkin-data/src/lib.rs", "pub struct Data;\n")
        self.write("README.md", "upstream branding\n")
        self.write(".github/workflows/rust.yml", "upstream CI\n")
        self.base = self.commit("base")
        tree = self.git.tree(self.base)
        normalized = Normalizer(self.git, [tree]).tree(tree)
        self.git.run("switch", "-c", "fork")
        self.git.run("read-tree", "--reset", "-u", self.git.write_tree(normalized))
        self.write("README.md", "fork branding\n")
        self.write(".github/workflows/rust.yml", "fork CI\n")
        self.ours = self.commit("rename fork")

    def write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content if isinstance(content, bytes) else content.encode())

    def commit(self, message):
        self.git.run("add", "-A")
        self.git.run("commit", "--allow-empty", "-m", message)
        return self.git.commit_id("HEAD")

    def upstream(self, changes, message="upstream update"):
        self.git.run("switch", "upstream")
        for path, content in changes.items():
            if content is None:
                (self.root / path).unlink()
            else:
                self.write(path, content)
        result = self.commit(message)
        self.git.run("switch", "fork")
        return result

    def local(self, changes):
        for path, content in changes.items():
            self.write(path, content)
        self.ours = self.commit("local hooks")

    def prepare(self, upstream, name="report", head=None, bundle=False):
        return prepare(self.root, head or self.ours, upstream, self.root.parent / name, bundle=bundle)

    def content(self, commit, path):
        entry = self.git.tree(commit)[path]
        return self.git.blob(entry.oid)


class NamespaceTests(unittest.TestCase):
    def test_generic_new_crates_and_wit_contract(self):
        self.assertEqual(fork_path("crates/pumpkin-command/src/lib.rs"), "crates/verdantgolem-command/src/lib.rs")
        self.assertEqual(fork_path("crates/pumpkin-gametest/Cargo.toml"), "crates/verdantgolem-gametest/Cargo.toml")
        self.assertEqual(fork_path("tools/pumpkin-codegen/src/main.rs"), "tools/verdantgolem-codegen/src/main.rs")
        self.assertEqual(fork_path(WIT_PATH), WIT_PATH)
        self.assertEqual(fork_path(WIT_PATH + "/world.wit"), WIT_PATH + "/world.wit")
        self.assertEqual(fork_path("egg-pumpkin.json"), "egg-verdantgolem.json")

    def test_path_audit_checks_names_not_name_status_columns(self):
        for path in ("crates/pumpkin/src/new.rs", "crates/pumpkin-command/Cargo.toml", "tools/pumpkin-new/src/lib.rs"):
            with self.assertRaises(SyncError):
                audit_tree({path: Entry("100644", "0" * 40)})
        audit_tree({WIT_PATH: Entry("160000", "0" * 40)})

    def test_exact_tokens_preserve_case_abi_and_minecraft_identifiers(self):
        normalizer = Normalizer(None, [{"crates/pumpkin-plugin-api/Cargo.toml": None, "crates/pumpkin-data/Cargo.toml": None}])
        text = b'use pumpkin_data::X; use pumpkin::server::S; use pumpkin::plugin::W; #[pumpkin_block] const PUMPKIN_API_VERSION: &str = "minecraft:pumpkin"; // CaseSensitive\n'
        expected = b'use verdantgolem_data::X; use verdantgolem::server::S; use pumpkin::plugin::W; #[pumpkin_block] const PUMPKIN_API_VERSION: &str = "minecraft:pumpkin"; // CaseSensitive\n'
        self.assertEqual(normalizer.content(text, "crates/pumpkin/src/lib.rs"), expected)
        self.assertEqual(normalizer.content(b"pumpkin::plugin::X", "crates/pumpkin-api-macros/src/lib.rs"), b"verdantgolem::plugin::X")
        self.assertEqual(normalizer.content(b"\xffpumpkin_data", "x.rs"), b"\xffpumpkin_data")
        self.assertEqual(normalizer.content(b"\0pumpkin_data", "x.rs"), b"\0pumpkin_data")

    def test_cargo_package_keys_members_and_lock_names(self):
        normalizer = Normalizer(None, [{"crates/pumpkin-command/Cargo.toml": None}])
        manifest = b'[package]\nname = "pumpkin"\ndefault-run = "pumpkin"\n[dependencies]\npumpkin = { path = "crates/pumpkin" }\npumpkin-command = {path="crates/pumpkin-command"}\n'
        output = normalizer.content(manifest, "Cargo.toml")
        self.assertNotIn(b'"pumpkin"', output)
        self.assertIn(b'verdantgolem = { path = "crates/verdantgolem" }', output)
        self.assertIn(b'verdantgolem-command', output)
        lock = b'name = "pumpkin"\ndependencies = ["pumpkin-command", "pumpkin 0.1.0"]\n'
        self.assertNotIn(b'"pumpkin', normalizer.content(lock, "Cargo.lock"))


class MergeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="vg-sync-test-")
        self.addCleanup(self.tmp.cleanup)
        self.repo = Repository(self.tmp.name)

    def test_nonoverlapping_hooks_survive_and_real_history_is_recorded(self):
        self.repo.local({"crates/verdantgolem/src/lib.rs": "one\ntwo\nthree\nfour\nCARPET\nsix\n"})
        upstream = self.repo.upstream({"crates/pumpkin/src/lib.rs": "one\nUPDATED\nthree\nfour\nfive\nsix\n"})
        report = self.repo.prepare(upstream)
        self.assertEqual(report["status"], "ready")
        content = self.repo.content(report["commit"], "crates/verdantgolem/src/lib.rs")
        self.assertIn(b"UPDATED", content)
        self.assertIn(b"CARPET", content)
        parents = self.repo.git.text("show", "-s", "--format=%P", report["commit"]).split()
        self.assertEqual(parents, [self.repo.ours, upstream])
        self.assertTrue(self.repo.git.ancestor(upstream, report["commit"]))

    def test_new_files_and_new_crates_are_normalized_without_conflicts(self):
        upstream = self.repo.upstream({
            "crates/pumpkin-command/Cargo.toml": '[package]\nname="pumpkin-command"\n',
            "crates/pumpkin-command/src/lib.rs": "use pumpkin_data::Data;\n",
            "crates/pumpkin/src/new file.rs": "use pumpkin_command::Command;\n",
        })
        report = self.repo.prepare(upstream)
        self.assertEqual(report["status"], "ready")
        entries = self.repo.git.tree(report["commit"])
        audit_tree(entries)
        self.assertIn("crates/verdantgolem-command/Cargo.toml", entries)
        self.assertEqual(self.repo.content(report["commit"], "crates/verdantgolem/src/new file.rs"), b"use verdantgolem_command::Command;\n")

    def test_true_and_case_only_conflicts_never_create_candidate(self):
        self.repo.local({"crates/verdantgolem/src/lib.rs": "one\nTWO\nthree\nfour\nfive\nsix\n"})
        upstream = self.repo.upstream({"crates/pumpkin/src/lib.rs": "one\nupstream\nthree\nfour\nfive\nsix\n"})
        report = self.repo.prepare(upstream, bundle=True)
        self.assertEqual(report["status"], "conflicts")
        self.assertIn("crates/verdantgolem/src/lib.rs", report["conflicts"])
        self.assertNotIn("commit", report)
        self.assertFalse((Path(self.tmp.name) / "report/candidate.bundle").exists())

    def test_fork_owned_files_survive_clean_changes_and_new_workflows(self):
        upstream = self.repo.upstream({
            "README.md": "new upstream branding\n",
            ".github/workflows/rust.yml": "new upstream CI\n",
            ".github/workflows/extra.yml": "upstream-only automation\n",
        })
        report = self.repo.prepare(upstream)
        self.assertEqual(self.repo.content(report["commit"], "README.md"), b"fork branding\n")
        self.assertEqual(self.repo.content(report["commit"], ".github/workflows/rust.yml"), b"fork CI\n")
        self.assertNotIn(".github/workflows/extra.yml", self.repo.git.tree(report["commit"]))
        self.assertIn(STATE_PATH, self.repo.git.tree(report["commit"]))

    def test_deletion_does_not_resurrect_old_namespace(self):
        upstream = self.repo.upstream({"crates/pumpkin-data/src/lib.rs": None})
        report = self.repo.prepare(upstream)
        entries = self.repo.git.tree(report["commit"])
        self.assertNotIn("crates/verdantgolem-data/src/lib.rs", entries)
        audit_tree(entries)

    def test_deletion_of_locally_changed_file_requires_review(self):
        self.repo.local({"crates/verdantgolem-data/src/lib.rs": "pub struct CarpetData;\n"})
        upstream = self.repo.upstream({"crates/pumpkin-data/src/lib.rs": None})
        self.assertEqual(self.repo.prepare(upstream)["status"], "conflicts")

    def test_upstream_rename_keeps_local_hook(self):
        self.repo.local({"crates/verdantgolem/src/lib.rs": "one\ntwo\nthree\nfour\nCARPET\nsix\n"})
        upstream = self.repo.upstream({
            "crates/pumpkin/src/lib.rs": None,
            "crates/pumpkin/src/renamed.rs": "one\ntwo\nthree\nfour\nfive\nsix\n",
        })
        report = self.repo.prepare(upstream)
        self.assertEqual(report["status"], "ready")
        self.assertIn(b"CARPET", self.repo.content(report["commit"], "crates/verdantgolem/src/renamed.rs"))

    def test_binary_mode_and_gitlink_are_preserved(self):
        upstream = self.repo.upstream({"assets/binary.bin": b"\0\xffpumpkin_data", "tools/pumpkin-codegen/run.sh": "#!/bin/sh\necho pumpkin_data\n"})
        entries = self.repo.git.tree(upstream)
        entries["tools/pumpkin-codegen/run.sh"] = Entry("100755", entries["tools/pumpkin-codegen/run.sh"].oid)
        entries[WIT_PATH] = Entry("160000", "a" * 40)
        upstream = self.repo.git.commit_tree(self.repo.git.write_tree(entries), [upstream], "gitlink", 1700000000)
        report = self.repo.prepare(upstream)
        candidate = self.repo.git.tree(report["commit"])
        self.assertEqual(self.repo.content(report["commit"], "assets/binary.bin"), b"\0\xffpumpkin_data")
        self.assertEqual(candidate["tools/verdantgolem-codegen/run.sh"].mode, "100755")
        self.assertEqual(candidate[WIT_PATH], Entry("160000", "a" * 40))

    def test_mapping_collision_is_explicit(self):
        upstream = self.repo.upstream({"crates/pumpkin/src/collision.rs": "up\n", "crates/verdantgolem/src/collision.rs": "other\n"})
        with self.assertRaisesRegex(SyncError, "collision"):
            self.repo.prepare(upstream)

    def test_repeat_run_is_deterministic_and_next_sync_advances_base(self):
        upstream = self.repo.upstream({"crates/pumpkin/src/next.rs": "first\n"})
        first = self.repo.prepare(upstream)
        second = self.repo.prepare(upstream, "report2")
        self.assertEqual(first["commit"], second["commit"])
        self.assertEqual(self.repo.prepare(upstream, "report3", first["commit"])["status"], "up_to_date")
        later = self.repo.upstream({"crates/pumpkin/src/next.rs": "second\n"})
        result = self.repo.prepare(later, "report4", first["commit"])
        self.assertEqual(result["status"], "ready")
        self.assertEqual(result["merge_base"], upstream)
        self.assertEqual(json.loads(self.repo.content(result["commit"], STATE_PATH))["upstream_commit"], later)

    def test_dirty_worktree_and_index_are_untouched(self):
        upstream = self.repo.upstream({"crates/pumpkin/src/new.rs": "new\n"})
        self.repo.write("README.md", "unstaged user changes\n")
        self.repo.write("user context.txt", "leave this alone\n")
        self.repo.write("staged.txt", "staged user change\n")
        self.repo.git.run("add", "staged.txt")
        before = [self.repo.git.run(*args).stdout for args in [("status", "--porcelain=v1", "-z"), ("diff",), ("diff", "--cached")]]
        self.repo.prepare(upstream)
        after = [self.repo.git.run(*args).stdout for args in [("status", "--porcelain=v1", "-z"), ("diff",), ("diff", "--cached")]]
        self.assertEqual(before, after)
        self.assertFalse((self.repo.root / ".git/MERGE_HEAD").exists())

    def test_upstream_rewrite_or_invalid_state_is_rejected(self):
        upstream = self.repo.upstream({"crates/pumpkin/src/new.rs": "new\n"})
        first = self.repo.prepare(upstream)
        with self.assertRaisesRegex(SyncError, "diverged"):
            self.repo.prepare(self.repo.base, "rewritten", first["commit"])
        self.repo.local({STATE_PATH: "not JSON\n"})
        with self.assertRaisesRegex(SyncError, "Invalid"):
            self.repo.prepare(upstream, "invalid")

    def test_bundle_has_one_candidate_and_expected_prerequisites(self):
        upstream = self.repo.upstream({"crates/pumpkin/src/new.rs": "new\n"})
        result = self.repo.prepare(upstream, bundle=True)
        bundle = Path(self.tmp.name) / "report/candidate.bundle"
        self.repo.git.run("bundle", "verify", str(bundle))
        self.assertEqual(self.repo.git.text("bundle", "list-heads", str(bundle)), result["commit"] + " refs/upstream-sync/candidate")

    def test_rustfmt_normalizes_rename_wrapping_before_three_way_merge(self):
        git = self.repo.git
        entries = git.tree(self.repo.base)
        source = b'use pumpkin_data::{VeryLongUpstreamTypeNameOne, VeryLongUpstreamTypeNameTwo};\npub fn value() -> i32 { 1 }\n'
        entries["crates/pumpkin/src/lib.rs"] = Entry("100644", git.hash_blob(source))
        config = b"max_width = 60\n"
        entries["rustfmt.toml"] = Entry("100644", git.hash_blob(config))
        base = git.commit_tree(git.write_tree(entries), [], "rust base", 1700000000)
        normalizer = Normalizer(git, [entries])
        output = Path(self.tmp.name) / "format-helper"
        output.mkdir()
        ours = normalizer.format_rust(normalizer.tree(entries), config, output)
        path = "crates/verdantgolem/src/lib.rs"
        ours[path] = Entry("100644", git.hash_blob(git.blob(ours[path].oid) + b"\npub const CARPET: bool = true;\n"))
        head = git.commit_tree(git.write_tree(ours), [base], "formatted fork", 1700000001)
        entries["crates/pumpkin/src/lib.rs"] = Entry("100644", git.hash_blob(source.replace(b"{ 1 }", b"{ 2 }")))
        upstream = git.commit_tree(git.write_tree(entries), [base], "upstream", 1700000002)
        report = prepare(self.repo.root, head, upstream, Path(self.tmp.name) / "formatted", format_rust=True)
        self.assertEqual(report["status"], "ready")
        result = self.repo.content(report["commit"], path)
        self.assertIn(b"CARPET: bool = true", result)
        self.assertIn(b"    2\n", result)
        self.assertIn(b"verdantgolem_data", result)

    def test_lock_refresh_preserves_checkout_and_does_not_receive_tokens(self):
        upstream = self.repo.upstream({"Cargo.toml": '[workspace]\nmembers=[]\n', "Cargo.lock": "# upstream lock\n"})
        original_run = subprocess.run
        cargo_calls = []
        def run(command, **kwargs):
            if command[0] != "cargo":
                return original_run(command, **kwargs)
            cargo_calls.append((command, kwargs["env"]))
            if "--offline" in command:
                return subprocess.CompletedProcess(command, 1)
            (Path(kwargs["cwd"]) / "Cargo.lock").write_text("# refreshed fork workspace\n")
            return subprocess.CompletedProcess(command, 0)
        before = self.repo.git.run("status", "--porcelain=v1", "-z").stdout
        with patch.dict(os.environ, {"GH_TOKEN": "publication-secret", "ACTIONS_RUNTIME_TOKEN": "artifact-secret"}), patch("upstream_sync.subprocess.run", side_effect=run):
            report = prepare(self.repo.root, self.repo.ours, upstream, Path(self.tmp.name) / "lock", refresh=True)
        self.assertEqual(report["status"], "ready")
        self.assertEqual(len(cargo_calls), 2)
        for _, env in cargo_calls:
            self.assertNotIn("GH_TOKEN", env)
            self.assertNotIn("ACTIONS_RUNTIME_TOKEN", env)
        self.assertEqual(self.repo.content(report["commit"], "Cargo.lock"), b"# refreshed fork workspace\n")
        self.assertEqual(before, self.repo.git.run("status", "--porcelain=v1", "-z").stdout)
        self.assertNotIn("vg-sync-lock", self.repo.git.text("worktree", "list", "--porcelain"))

    def test_real_cargo_refresh_adds_fork_only_workspace_packages(self):
        git = self.repo.git
        entries = git.tree(self.repo.base)
        def put(tree, path, text):
            tree[path] = Entry("100644", git.hash_blob(text.encode()))
        put(entries, "Cargo.toml", '[workspace]\nresolver="3"\nmembers=["crates/pumpkin"]\n')
        put(entries, "crates/pumpkin/Cargo.toml", '[package]\nname="pumpkin"\nversion="0.1.0"\nedition="2024"\n')
        put(entries, "Cargo.lock", 'version=4\n[[package]]\nname="pumpkin"\nversion="0.1.0"\n')
        base = git.commit_tree(git.write_tree(entries), [], "cargo base", 1700000000)
        ours = Normalizer(git, [entries]).tree(entries)
        put(ours, "Cargo.toml", '[workspace]\nresolver="3"\nmembers=["crates/verdantgolem", "crates/verdantgolem-extra"]\n')
        put(ours, "crates/verdantgolem-extra/Cargo.toml", '[package]\nname="verdantgolem-extra"\nversion="0.1.0"\nedition="2024"\n')
        put(ours, "crates/verdantgolem-extra/src/lib.rs", "pub struct Extra;\n")
        head = git.commit_tree(git.write_tree(ours), [base], "fork package", 1700000001)
        put(entries, "crates/pumpkin/src/added.rs", "pub struct Added;\n")
        upstream = git.commit_tree(git.write_tree(entries), [base], "upstream source", 1700000002)
        report = prepare(self.repo.root, head, upstream, Path(self.tmp.name) / "real-cargo", refresh=True)
        self.assertEqual(report["status"], "ready")
        lock = self.repo.content(report["commit"], "Cargo.lock")
        self.assertIn(b'name = "verdantgolem-extra"', lock)
        self.assertIn(b'name = "verdantgolem"', lock)
        self.assertNotIn(b'name = "pumpkin"', lock)


if __name__ == "__main__":
    unittest.main()
