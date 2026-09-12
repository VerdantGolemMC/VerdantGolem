"""Standard-library unit tests; these never launch a Minecraft server."""

from __future__ import annotations

import argparse
import gzip
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import Mock

import technical_survival_smoke as smoke


class FragmentedSocket:
    def __init__(self, data: bytes):
        self.data = data
        self.sent = []
        self.closed = False

    def recv(self, limit: int) -> bytes:
        take = min(3, limit, len(self.data))
        data, self.data = self.data[:take], self.data[take:]
        return data

    def sendall(self, data: bytes) -> None:
        self.sent.append(data)

    def settimeout(self, timeout: float) -> None:
        if timeout <= 0:
            raise AssertionError("Expected a bounded positive timeout")

    def close(self) -> None:
        self.closed = True


def packet(request: int, kind: int, body: str) -> bytes:
    encoded = body.encode()
    return struct.pack("<iii", len(encoded) + 10, request, kind) + encoded + b"\0\0"


def client(data: bytes) -> smoke.Rcon:
    result = smoke.Rcon.__new__(smoke.Rcon)
    result.socket = FragmentedSocket(data)
    result.password = "unit-test-password"
    result.timeout = 1.0
    result.sequence = 0
    return result


class NbtTests(unittest.TestCase):
    def test_all_tag_types_and_modified_utf8_round_trip(self):
        root = {
            "byte": smoke.Nbt(1, -128),
            "short": smoke.Nbt(2, -1234),
            "int": smoke.Nbt(3, 2147483647),
            "long": smoke.Nbt(4, -9223372036854775808),
            "float": smoke.Nbt(5, 1.25),
            "double": smoke.Nbt(6, -3.125),
            "bytes": smoke.Nbt(7, b"\x00\x7f\xff"),
            "text": smoke.Nbt(8, "A\0中文\U0001f33f"),
            "empty": smoke.Nbt(9, smoke.NbtList(0, [])),
            "list": smoke.Nbt(9, smoke.NbtList(10, [{"item": smoke.Nbt(8, "stone")}])),
            "compound": smoke.Nbt(10, {"nested": smoke.Nbt(3, 42)}),
            "ints": smoke.Nbt(11, [-1, 0, 2147483647]),
            "longs": smoke.Nbt(12, [-1, 0, 9223372036854775807]),
        }
        self.assertEqual(smoke.read_nbt(smoke.write_nbt(root, "fixture")), ("fixture", root))

    def test_truncated_trailing_and_invalid_list_payloads_fail(self):
        encoded = smoke.write_nbt({"value": smoke.Nbt(3, 4)})
        for broken in (encoded[:-1], encoded + b"extra", b"\x01\0\0", b"\x0a\0\0\x09\0\0\0\0\0\0\x01"):
            with self.subTest(broken=broken), self.assertRaises(smoke.SmokeFailure):
                smoke.read_nbt(broken)

    def test_region_replacement_keeps_other_chunks_and_root_name(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "r.0.0.mca"
            path.write_bytes(b"\0" * smoke.SECTOR_SIZE * 2)
            first, second = {"value": smoke.Nbt(3, 1)}, {"value": smoke.Nbt(3, 2)}
            smoke.replace_chunk(path, (0, 0), "first", first)
            smoke.replace_chunk(path, (1, 0), "second", second)
            first["new"] = smoke.Nbt(8, "updated")
            smoke.replace_chunk(path, (0, 0), "first", first)
            self.assertEqual(smoke.read_chunk(path, (0, 0)), ("first", first))
            self.assertEqual(smoke.read_chunk(path, (1, 0)), ("second", second))
            with self.assertRaises(smoke.SmokeFailure):
                smoke.read_chunk(path, (2, 0))

    def test_canonical_and_legacy_dimension_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            world = Path(directory)
            canonical = world / "dimensions" / "minecraft" / "overworld"
            self.assertEqual(smoke.region_path(world, (10, 10)), canonical / "region" / "r.0.0.mca")
            (world / "region").mkdir()
            self.assertEqual(smoke.region_path(world, (-1, -33)), world / "region" / "r.-1.-2.mca")
            canonical.mkdir(parents=True)
            self.assertEqual(smoke.region_path(world, (10, 10)), canonical / "region" / "r.0.0.mca")

    def test_block_entity_identity_and_inventory_counts(self):
        entity = {
            "x": smoke.Nbt(3, 1), "y": smoke.Nbt(3, 200), "z": smoke.Nbt(3, 3),
            "Items": smoke.Nbt(9, smoke.NbtList(10, [
                {"id": smoke.Nbt(8, "minecraft:stone"), "count": smoke.Nbt(3, 40)},
                {"id": smoke.Nbt(8, "minecraft:stone"), "Count": smoke.Nbt(1, 24)},
                {"id": smoke.Nbt(8, "minecraft:dirt"), "count": smoke.Nbt(3, 1)},
            ])),
        }
        root = {"block_entities": smoke.Nbt(9, smoke.NbtList(10, [entity]))}
        self.assertIs(smoke.block_entity(root, (1, 200, 3)), entity)
        self.assertEqual(smoke.inventory_count(entity, "minecraft:stone"), 64)
        self.assertEqual(smoke.inventory_count({}, "minecraft:stone"), 0)
        with self.assertRaises(smoke.SmokeFailure):
            smoke.block_entity(root, (2, 200, 3))

    def test_packed_block_palette_reads_the_actual_placement(self):
        position = smoke.USE_TARGET
        x, y, z = position
        index = ((y & 15) * 16 + (z & 15)) * 16 + (x & 15)
        packed = [0] * 256
        packed[index // 16] = 1 << (index % 16 * 4)
        root = {"sections": smoke.Nbt(9, smoke.NbtList(10, [{
            "Y": smoke.Nbt(1, y // 16),
            "block_states": smoke.Nbt(10, {
                "palette": smoke.Nbt(9, smoke.NbtList(10, [
                    {"Name": smoke.Nbt(8, "minecraft:air")},
                    {"Name": smoke.Nbt(8, "minecraft:stone")},
                ])),
                "data": smoke.Nbt(12, packed),
            }),
        }]))}
        self.assertEqual(smoke.block_name(root, position), "minecraft:stone")
        self.assertEqual(smoke.block_name(root, (x + 1, y, z)), "minecraft:air")

    def test_fixture_is_fresh_flat_world_with_small_spawn_radius(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            smoke.prepare_world(root, smoke.DATA_VERSION)
            _, level = smoke.read_nbt(gzip.decompress((root / "world" / "level.dat").read_bytes()))
            self.assertEqual(smoke.field(smoke.field(level, "Data", 10), "DataVersion", 3), smoke.DATA_VERSION)
            path = root / "world" / "data" / "minecraft" / "world_gen_settings.dat"
            _, settings = smoke.read_nbt(gzip.decompress(path.read_bytes()))
            settings = smoke.field(settings, "data", 10)
            self.assertEqual(smoke.field(settings, "seed", 4), 1)
            dims = smoke.field(settings, "dimensions", 10)
            gen = smoke.field(smoke.field(dims, "minecraft:overworld", 10), "generator", 10)
            self.assertEqual(smoke.field(gen, "type", 8), "minecraft:flat")
            self.assertEqual(json.loads((root / "carpet_rules.json").read_text()), {"spawnChunkRadius": 1})


class RconTests(unittest.TestCase):
    def test_multiline_responses_use_marker_and_survive_fragmentation(self):
        connection = client(
            packet(2, 0, "\x1b[32mfirst\x1b[0m") + packet(2, 0, "second") + packet(3, 2, "")
        )
        self.assertEqual(connection.command("counter"), "first\nsecond")
        self.assertEqual(struct.unpack("<iii", connection.socket.sent[0][:12])[1:], (2, 2))
        self.assertEqual(struct.unpack("<iii", connection.socket.sent[1][:12])[1:], (3, 3))
        self.assertEqual(connection.socket.data, b"")

    def test_wrong_id_and_bad_terminator_fail(self):
        for data in (packet(-1, 2, ""), packet(2, 0, "bad")[:-1] + b"X"):
            with self.subTest(data=data), self.assertRaises(smoke.SmokeFailure):
                client(data).command("counter")

    def test_disconnect_is_accepted_only_for_stop(self):
        with self.assertRaises(EOFError):
            client(b"").command("counter")
        self.assertEqual(client(packet(2, 0, "Stopping the server")).command("stop", stopping=True), "Stopping the server")

    def test_new_session_configuration_has_no_nonloopback_listener(self):
        config = smoke.config_text(40001, 40002, "example-password")
        self.assertNotIn("0.0.0.0", config)
        self.assertNotIn("[::]", config)
        self.assertIn('address = "127.0.0.1:40001"', config)
        self.assertIn('address = "127.0.0.1:40002"', config)
        for section in ("networking.bedrock", "networking.query", "networking.lan_broadcast"):
            self.assertIn(f"[{section}]\nenabled = false", config)


class ProcessAndReportTests(unittest.TestCase):
    def test_block_placement_accepts_actual_cross_edition_success_only(self):
        for output in ("Block placed", "Changed the block at 160, 199, 160", " block placed\n"):
            self.assertTrue(smoke.block_placement_succeeded(output))
        for output in ("That position is not loaded", "Failed: Block placed", "Unknown command", ""):
            self.assertFalse(smoke.block_placement_succeeded(output))

    def test_normal_stop_closes_stdin_before_waiting_for_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            server = smoke.SmokeServer(argparse.Namespace(shutdown_timeout=1), Path(directory))
            child = Mock(stdin=io.BytesIO())
            child.poll.return_value = 0
            server.process = child
            server.command = Mock(return_value="Stopping the server")

            def wait_for_exit(**_kwargs):
                self.assertTrue(child.stdin.closed, "Console input must see EOF before shutdown joins it")
                return 0

            child.wait.side_effect = wait_for_exit
            server.stop()
            server.cleanup()
            self.assertEqual(server.command.call_args_list[0].args[0], "save-all")
            self.assertEqual(server.command.call_args_list[1].args[0], "stop")

    def test_cleanup_only_terminates_the_owned_child(self):
        with tempfile.TemporaryDirectory() as directory:
            server = smoke.SmokeServer(argparse.Namespace(), Path(directory))
            child = Mock()
            child.poll.return_value = None
            child.stdin.closed = False
            child.wait.side_effect = [
                subprocess.TimeoutExpired("owned-server", 10),
                subprocess.TimeoutExpired("owned-server", 5),
                0,
            ]
            server.process = child
            server.cleanup()
            child.stdin.write.assert_called_once_with(b"stop\n")
            child.terminate.assert_called_once_with()
            child.kill.assert_called_once_with()
            self.assertEqual(child.wait.call_count, 3)

    def test_saved_nbt_requires_stopped_child(self):
        with tempfile.TemporaryDirectory() as directory:
            server = smoke.SmokeServer(argparse.Namespace(), Path(directory))
            child = Mock()
            child.poll.return_value = None
            server.process = child
            with self.assertRaises(smoke.SmokeFailure):
                server.saved_chunk()
            child.poll.return_value = 0
            server.cleanup()

    def test_counter_and_player_report_parsing(self):
        server = Mock()
        server.command.return_value = (
            "red: 60 items total; 1.00s game-time (20 ticks @ 20 TPS); 216000.0 items/h\n"
            "stone: 40 items (144000.0 items/h)"
        )
        report = smoke.counter(server, "red")
        self.assertEqual((report["total"], report["ticks"], report["seconds"], report["rate"]),
                         (60, 20, 1.0, 216000.0))
        server.command.return_value = "SmokeBot has entity data: {Other: 3, Pos: [166.5d, 200d, 167.5d]}"
        self.assertEqual(smoke.player_position(server), (166.5, 200.0, 167.5))
        server.command.return_value = "No entity was found"
        with self.assertRaises(smoke.SmokeFailure):
            smoke.player_position(server)


if __name__ == "__main__":
    unittest.main()
