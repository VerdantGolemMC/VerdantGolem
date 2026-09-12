#!/usr/bin/env python3
"""End-to-end technical-survival checks against a locally built server.

Requires only Python 3.9+ and an existing VerdantGolem executable. Nothing is
downloaded or built. Each invocation owns a fresh temporary superflat world,
loopback-only Java/RCON ports, and a single child server process at a time.
Failed runs retain their directory, complete server logs, RCON transcript and
summary.json. Successful runs remove it unless --keep is supplied.

The current server has no /data get block or /setblock NBT support, and /info
requires a player/command-block sender. Consequently the script uses actual
/setblock and /item commands, then reads saved Anvil NBT *after a normal stop*.
For the optional fake-player check it also prepares one command block's Command
tag while stopped; there is no test-only plugin or production-code bypass.

Example:
    python3 tools/tests/technical_survival_smoke.py --server /absolute/server --keep
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback
import uuid
import zlib
from dataclasses import dataclass
from typing import Any, Callable


HOST = "127.0.0.1"
DATA_VERSION = 4903  # Current server's 26.2 world format, not a protocol ID.
SECTOR_SIZE = 4096
MAX_NBT_BYTES = 64 * 1024 * 1024
CHUNK = (10, 10)  # Deliberately outside the initial spawn-chunk ticket.
BASE = (160, 160)
BOT = "SmokeBot"
DISPENSERS = {
    "stone_target": (162, 201, 162),
    "air_target": (166, 201, 162),
    "disabled_control": (170, 201, 162),
}
HOPPER = (170, 201, 166)
FED_HOPPER = (173, 201, 169)
SOURCE_CHEST = (173, 202, 169)
COMMAND_BLOCK = (162, 201, 170)
USE_TARGET = (166, 200, 169)
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]|\u00a7[0-9a-fk-or]", re.I)


class SmokeFailure(RuntimeError):
    """A failed assertion, unsupported fixture, or bounded operation timeout."""


def require(condition: bool, message: str) -> None:
    # Unlike assert, these checks remain enabled under python -O.
    if not condition:
        raise SmokeFailure(message)


@dataclass
class Nbt:
    kind: int
    value: Any


@dataclass
class NbtList:
    kind: int
    values: list[Any]


SCALAR_FORMATS = {1: ">b", 2: ">h", 3: ">i", 4: ">q", 5: ">f", 6: ">d"}


def nbt_string_bytes(value: str) -> bytes:
    """Java modified UTF-8, including NUL and surrogate-pair encoding."""
    units = value.encode("utf-16-be", "surrogatepass")
    encoded = b"".join(
        chr(int.from_bytes(units[i:i + 2], "big")).encode("utf-8", "surrogatepass")
        for i in range(0, len(units), 2)
    ).replace(b"\0", b"\xc0\x80")
    require(len(encoded) <= 65535, "NBT string exceeds its unsigned-short length")
    return struct.pack(">H", len(encoded)) + encoded


class NbtReader:
    def __init__(self, data: bytes):
        require(len(data) <= MAX_NBT_BYTES, "NBT fixture exceeds the size limit")
        self.stream = io.BytesIO(data)

    def take(self, length: int) -> bytes:
        require(0 <= length <= MAX_NBT_BYTES, "Invalid NBT length")
        data = self.stream.read(length)
        require(len(data) == length, "Truncated NBT fixture")
        return data

    def number(self, fmt: str) -> Any:
        return struct.unpack(fmt, self.take(struct.calcsize(fmt)))[0]

    def string(self) -> str:
        raw = self.take(self.number(">H")).replace(b"\xc0\x80", b"\0")
        decoded = raw.decode("utf-8", "surrogatepass")
        return decoded.encode("utf-16-be", "surrogatepass").decode(
            "utf-16-be", "surrogatepass"
        )

    def payload(self, kind: int, depth: int = 0) -> Any:
        require(depth <= 64, "NBT nesting exceeds the fixture limit")
        if kind in SCALAR_FORMATS:
            return self.number(SCALAR_FORMATS[kind])
        if kind == 7:
            return self.take(self.number(">i"))
        if kind == 8:
            return self.string()
        if kind == 9:
            child_kind, length = self.number(">B"), self.number(">i")
            require(0 <= length <= MAX_NBT_BYTES, "Invalid NBT list length")
            require(child_kind != 0 or length == 0, "Nonempty TAG_End list")
            return NbtList(
                child_kind, [self.payload(child_kind, depth + 1) for _ in range(length)]
            )
        if kind == 10:
            result = {}
            while True:
                child_kind = self.number(">B")
                if child_kind == 0:
                    return result
                name = self.string()
                require(name not in result, f"Duplicate NBT key {name!r}")
                result[name] = Nbt(child_kind, self.payload(child_kind, depth + 1))
        if kind in (11, 12):
            length = self.number(">i")
            fmt, width = (">i", 4) if kind == 11 else (">q", 8)
            data = self.take(length * width)
            return [number[0] for number in struct.iter_unpack(fmt, data)]
        raise SmokeFailure(f"Unsupported NBT tag {kind}")


def read_nbt(data: bytes) -> tuple[str, dict[str, Nbt]]:
    reader = NbtReader(data)
    require(reader.number(">B") == 10, "NBT root must be a compound")
    name = reader.string()
    root = reader.payload(10)
    require(not reader.stream.read(1), "Unexpected trailing NBT bytes")
    return name, root


def nbt_payload(kind: int, value: Any) -> bytes:
    if kind in SCALAR_FORMATS:
        return struct.pack(SCALAR_FORMATS[kind], value)
    if kind == 7:
        return struct.pack(">i", len(value)) + value
    if kind == 8:
        return nbt_string_bytes(value)
    if kind == 9:
        return struct.pack(">Bi", value.kind, len(value.values)) + b"".join(
            nbt_payload(value.kind, item) for item in value.values
        )
    if kind == 10:
        return b"".join(
            bytes([tag.kind]) + nbt_string_bytes(name) + nbt_payload(tag.kind, tag.value)
            for name, tag in value.items()
        ) + b"\0"
    if kind in (11, 12):
        fmt = ">i" if kind == 11 else ">q"
        return struct.pack(">i", len(value)) + b"".join(
            struct.pack(fmt, item) for item in value
        )
    raise SmokeFailure(f"Unsupported NBT tag {kind}")


def write_nbt(root: dict[str, Nbt], name: str = "") -> bytes:
    return b"\x0a" + nbt_string_bytes(name) + nbt_payload(10, root)


def field(compound: dict[str, Nbt], key: str, kind: int) -> Any:
    require(key in compound, f"Missing NBT field {key}")
    tag = compound[key]
    require(tag.kind == kind, f"NBT {key}: expected tag {kind}, received {tag.kind}")
    return tag.value


def region_path(world: Path, chunk: tuple[int, int]) -> Path:
    canonical = world / "dimensions" / "minecraft" / "overworld"
    dimension = canonical if canonical.exists() or not (world / "region").exists() else world
    return dimension / "region" / f"r.{chunk[0] >> 5}.{chunk[1] >> 5}.mca"


def read_chunk(path: Path, chunk: tuple[int, int]) -> tuple[str, dict[str, Nbt]]:
    raw = path.read_bytes()
    require(len(raw) >= SECTOR_SIZE * 2, f"Truncated region: {path}")
    index = ((chunk[0] & 31) + (chunk[1] & 31) * 32) * 4
    location = int.from_bytes(raw[index:index + 4], "big")
    offset, sectors = (location >> 8) * SECTOR_SIZE, location & 255
    require(offset >= SECTOR_SIZE * 2 and sectors > 0, f"Chunk {chunk} was not saved")
    require(offset + sectors * SECTOR_SIZE <= len(raw), "Region sector outside file")
    length = int.from_bytes(raw[offset:offset + 4], "big")
    require(1 <= length <= sectors * SECTOR_SIZE - 4, "Invalid Anvil chunk length")
    compression = raw[offset + 4]
    require(not compression & 128, "External .mcc chunks are not expected in this fixture")
    payload = raw[offset + 5:offset + 4 + length]
    if compression in (1, 2):
        decoder = zlib.decompressobj(31 if compression == 1 else 15)
        payload = decoder.decompress(payload, MAX_NBT_BYTES + 1)
        require(len(payload) <= MAX_NBT_BYTES and decoder.eof, "Oversized/truncated compressed NBT")
    else:
        require(compression == 3, f"Expected ZLib fixture, received compression {compression}")
    return read_nbt(payload)


def replace_chunk(path: Path, chunk: tuple[int, int], name: str, root: dict[str, Nbt]) -> None:
    """Append one replacement chunk, preserving every other region entry."""
    raw = bytearray(path.read_bytes())
    require(len(raw) >= SECTOR_SIZE * 2, "Invalid region header")
    compressed = zlib.compress(write_nbt(root, name), 1)
    record = struct.pack(">I", len(compressed) + 1) + b"\x02" + compressed
    sectors = (len(record) + SECTOR_SIZE - 1) // SECTOR_SIZE
    require(sectors < 256, "Fixture chunk exceeds Anvil's inline size limit")
    offset = (len(raw) + SECTOR_SIZE - 1) // SECTOR_SIZE
    require(offset < (1 << 24), "Fixture region is too large")
    raw.extend(b"\0" * (offset * SECTOR_SIZE - len(raw)))
    raw.extend(record.ljust(sectors * SECTOR_SIZE, b"\0"))
    index = ((chunk[0] & 31) + (chunk[1] & 31) * 32) * 4
    raw[index:index + 4] = ((offset << 8) | sectors).to_bytes(4, "big")
    raw[SECTOR_SIZE + index:SECTOR_SIZE + index + 4] = int(time.time()).to_bytes(4, "big")
    temporary = path.with_suffix(".smoke-new")
    temporary.write_bytes(raw)
    temporary.replace(path)


def block_entity(root: dict[str, Nbt], position: tuple[int, int, int]) -> dict[str, Nbt]:
    entities = field(root, "block_entities", 9)
    require(entities.kind == 10, "Expected compound block-entity list")
    found = [
        entity for entity in entities.values
        if tuple(field(entity, axis, 3) for axis in ("x", "y", "z")) == position
    ]
    require(len(found) == 1, f"Expected exactly one block entity at {position}, found {len(found)}")
    return found[0]


def inventory_count(compound: dict[str, Nbt], item: str, key: str = "Items") -> int:
    if key not in compound:
        return 0
    inventory = field(compound, key, 9)
    require(inventory.kind in (0, 10), "Expected compound inventory list")
    total = 0
    for stack in inventory.values:
        if field(stack, "id", 8) == item:
            tag = stack.get("count", stack.get("Count"))
            require(tag is not None and tag.kind in (1, 3), "Missing inventory item count")
            total += tag.value
    return total


def block_name(root: dict[str, Nbt], position: tuple[int, int, int]) -> str:
    """Read modern Anvil's non-straddling packed block-state palette."""
    x, y, z = position
    for section in field(root, "sections", 9).values:
        if field(section, "Y", 1) != y // 16:
            continue
        states = field(section, "block_states", 10)
        palette = field(states, "palette", 9).values
        require(bool(palette), "Empty block-state palette")
        palette_index = 0
        if len(palette) > 1:
            bits = max(4, (len(palette) - 1).bit_length())
            per_long = 64 // bits
            index = ((y & 15) * 16 + (z & 15)) * 16 + (x & 15)
            packed = field(states, "data", 12)
            palette_index = (packed[index // per_long] >> (index % per_long * bits)) & ((1 << bits) - 1)
        require(palette_index < len(palette), "Invalid block-state palette index")
        return field(palette[palette_index], "Name", 8)
    raise SmokeFailure(f"No saved chunk section for {position}")


def prepare_world(directory: Path, data_version: int) -> None:
    world = directory / "world"
    data = world / "data" / "minecraft"
    data.mkdir(parents=True)
    level = {
        "Data": Nbt(10, {
            "DataVersion": Nbt(3, data_version),
            "version": Nbt(3, 19133),
            "LevelName": Nbt(8, "technical-survival-smoke"),
            "SpawnX": Nbt(3, 0), "SpawnY": Nbt(3, 200), "SpawnZ": Nbt(3, 0),
            "Difficulty": Nbt(1, 0),
            "WorldGenSettings": Nbt(10, {"seed": Nbt(4, 1)}),
        }),
    }
    settings = {
        "DataVersion": Nbt(3, data_version),
        "data": Nbt(10, {
            "seed": Nbt(4, 1),
            "dimensions": Nbt(10, {
                "minecraft:overworld": Nbt(10, {
                    "type": Nbt(8, "minecraft:overworld"),
                    "generator": Nbt(10, {
                        "type": Nbt(8, "minecraft:flat"),
                        "settings": Nbt(10, {
                            "biome": Nbt(8, "minecraft:plains"),
                            "layers": Nbt(9, NbtList(10, [
                                {"block": Nbt(8, "minecraft:bedrock"), "height": Nbt(3, 1)},
                            ])),
                        }),
                    }),
                }),
            }),
        }),
    }
    (world / "level.dat").write_bytes(gzip.compress(write_nbt(level)))
    (data / "world_gen_settings.dat").write_bytes(gzip.compress(write_nbt(settings)))
    (directory / "carpet_rules.json").write_text(
        json.dumps({"spawnChunkRadius": 1}), encoding="utf-8"
    )


def config_text(java_port: int, rcon_port: int, password: str) -> str:
    return f'''seed = "1"
allow_nether = false
allow_end = false
default_level_name = "world"
use_favicon = false
tps = 20.0

[logging]
color = false
threads = false
file = "server.log"

[commands]
use_console = true
use_tty = false
broadcast_console_to_ops = false

[networking.java]
enabled = true
address = "127.0.0.1:{java_port}"
online_mode = false
encryption = false
max_players = 4
view_distance = 2
simulation_distance = 2

[networking.java.authentication]
enabled = false

[networking.bedrock]
enabled = false
online_mode = false

[networking.bedrock.nethernet]
enabled = false
address = "127.0.0.1:0"

[networking.query]
enabled = false
address = "127.0.0.1:0"

[networking.lan_broadcast]
enabled = false

[networking.rcon]
enabled = true
address = "127.0.0.1:{rcon_port}"
password = "{password}"
max_connections = 4

[networking.rcon.logging]
logged_successfully = false
commands = true
quit = false

[world]
autosave_ticks = 0

[world.chunk]
type = "anvil"

[world.chunk.compression]
algorithm = "ZLib"
level = 1
'''


def reserve_ports() -> tuple[list[socket.socket], list[int]]:
    reservations = []
    try:
        for _ in range(2):
            reservation = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            reservation.bind((HOST, 0))
            reservations.append(reservation)
        ports = [sock.getsockname()[1] for sock in reservations]
        require(all(port > 1024 for port in ports), "Expected unprivileged ephemeral ports")
        return reservations, ports
    except BaseException:
        for sock in reservations:
            sock.close()
        raise


class Rcon:
    def __init__(self, port: int, password: str, timeout: float):
        self.socket = socket.create_connection((HOST, port), timeout=timeout)
        self.socket.settimeout(timeout)
        self.timeout = timeout
        self.deadline = time.monotonic() + timeout
        self.password = password
        self.sequence = 0
        try:
            self.send(1, 3, password)
            request, kind, _ = self.receive()
            require(request == 1 and kind == 2, "RCON authentication failed")
        except BaseException:
            self.close()
            raise

    def close(self) -> None:
        self.socket.close()

    def exact(self, length: int) -> bytes:
        chunks = bytearray()
        while len(chunks) < length:
            remaining = self.deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("RCON response exceeded its total deadline")
            self.socket.settimeout(remaining)
            part = self.socket.recv(length - len(chunks))
            if not part:
                raise EOFError("RCON connection closed")
            chunks.extend(part)
        return bytes(chunks)

    def send(self, request: int, kind: int, text: str) -> None:
        body = text.encode("utf-8")
        require(len(body) + 10 <= 1460, "RCON command exceeds the server packet limit")
        self.socket.sendall(struct.pack("<iii", len(body) + 10, request, kind) + body + b"\0\0")

    def receive(self) -> tuple[int, int, str]:
        length = struct.unpack("<i", self.exact(4))[0]
        require(10 <= length <= MAX_NBT_BYTES, "Invalid RCON response length")
        packet = self.exact(length)
        require(packet[-2:] == b"\0\0", "RCON response lacks null terminators")
        request, kind = struct.unpack("<ii", packet[:8])
        return request, kind, packet[8:-2].decode("utf-8")

    def command(self, text: str, stopping: bool = False) -> str:
        self.deadline = time.monotonic() + self.timeout
        self.socket.settimeout(self.timeout)
        self.sequence += 2
        request, marker = self.sequence, self.sequence + 1
        self.send(request, 2, text)
        # The server sends one response per output line, not an end packet.
        # Reauthentication is supported by its RCON state machine and provides
        # an ordered, side-effect-free end marker, avoiding a timing-based drain.
        self.send(marker, 3, self.password)
        output = []
        try:
            while True:
                response_id, kind, body = self.receive()
                if response_id == marker:
                    require(kind == 2, "Unexpected RCON end marker")
                    break
                require(response_id == request and kind == 0, "Unexpected RCON response ID/type")
                output.append(body)
        except (EOFError, ConnectionResetError, BrokenPipeError):
            if not stopping:
                raise
        return ANSI.sub("", "\n".join(output))


class SmokeServer:
    def __init__(self, args: argparse.Namespace, directory: Path):
        self.args, self.directory = args, directory
        self.process = None
        self.rcon = None
        self.log_handle = None
        self.run_number = 0
        self.transcript = (directory / "rcon.jsonl").open("a", encoding="utf-8")
        self.results = []

    def healthy(self) -> None:
        require(self.process is not None, "Server has not started")
        require(self.process.poll() is None, f"Server exited unexpectedly: {self.process.returncode}")

    def start(self) -> None:
        require(self.process is None or self.process.poll() is not None, "Server already running")
        reservations, (java_port, rcon_port) = reserve_ports()
        password = secrets.token_hex(24)
        config = self.directory / "pumpkin.toml"
        config.write_text(config_text(java_port, rcon_port, password), encoding="utf-8")
        config.chmod(0o600)
        self.run_number += 1
        self.log_handle = (self.directory / f"server-{self.run_number}.log").open("wb")
        env = dict(os.environ, RAYON_NUM_THREADS="2", TOKIO_WORKER_THREADS="2", RUST_LOG="info")
        # Release immediately before Popen. A bind race fails this owned child;
        # it never causes a connection to another server (fresh random password).
        for reservation in reservations:
            reservation.close()
        self.process = subprocess.Popen(
            [str(self.args.server)], cwd=str(self.directory), env=env,
            stdin=subprocess.PIPE, stdout=self.log_handle, stderr=subprocess.STDOUT,
        )
        print(f"Starting server #{self.run_number}, PID {self.process.pid}, RCON {HOST}:{rcon_port}", flush=True)
        deadline = time.monotonic() + self.args.startup_timeout
        last_error = "RCON not listening"
        while time.monotonic() < deadline:
            self.healthy()
            try:
                self.rcon = Rcon(rcon_port, password, min(self.args.command_timeout, 1.0))
                self.rcon.timeout = self.args.command_timeout
                self.command("list", contains="players online")
                return
            except (OSError, EOFError) as error:
                last_error = str(error)
                if self.rcon is not None:
                    self.rcon.close()
                    self.rcon = None
                time.sleep(0.1)
        raise SmokeFailure(f"Startup timeout: {last_error}; inspect server-{self.run_number}.log")

    def command(self, text: str, contains: str | None = None, stopping: bool = False) -> str:
        self.healthy()
        require(self.rcon is not None, "RCON not connected")
        started = time.monotonic()
        try:
            output = self.rcon.command(text, stopping=stopping)
        except BaseException as error:
            self.record_command(text, f"{type(error).__name__}: {error}", started)
            raise
        self.record_command(text, output, started)
        if contains is not None:
            require(contains.lower() in output.lower(), f"/{text}: expected {contains!r}, got {output!r}")
        return output

    def record_command(self, command: str, output: str, started: float) -> None:
        self.transcript.write(json.dumps({
            "run": self.run_number, "command": command, "response": output,
            "duration_seconds": time.monotonic() - started,
        }) + "\n")
        self.transcript.flush()

    def passed(self, name: str, **details: Any) -> None:
        self.results.append({"test": name, "status": "passed", **details})
        print(f"PASS {name}", flush=True)

    def until(self, label: str, probe: Callable[[], Any], predicate: Callable[[Any], bool]) -> Any:
        deadline = time.monotonic() + self.args.check_timeout
        last = None
        while time.monotonic() < deadline:
            self.healthy()
            last = probe()
            if predicate(last):
                return last
            time.sleep(0.05)
        raise SmokeFailure(f"Timed out waiting for {label}; last result: {last!r}")

    def stop(self) -> None:
        self.command("save-all", contains="Saving")
        try:
            self.command("stop", stopping=True)
        finally:
            if self.rcon is not None:
                self.rcon.close()
                self.rcon = None
            # setup_stdin_console has a tracked receiver waiting on a blocking
            # reader thread. EOF lets that task finish after an RCON stop.
            if self.process.stdin is not None:
                self.process.stdin.close()
        try:
            code = self.process.wait(timeout=self.args.shutdown_timeout)
        except subprocess.TimeoutExpired as error:
            raise SmokeFailure("Normal server stop timed out") from error
        require(code == 0, f"Server stopped with exit code {code}")
        self.close_handles()

    def close_handles(self) -> None:
        if self.process is not None and self.process.stdin is not None:
            self.process.stdin.close()
        if self.log_handle is not None:
            self.log_handle.close()
            self.log_handle = None

    def cleanup(self) -> None:
        """Only this object's child may be stopped; never signal a PID search."""
        if self.rcon is not None:
            try:
                self.rcon.command("stop", stopping=True)
            except (OSError, EOFError, SmokeFailure):
                pass
            self.rcon.close()
            self.rcon = None
        if self.process is not None and self.process.poll() is None:
            try:
                if self.process.stdin is not None and not self.process.stdin.closed:
                    self.process.stdin.write(b"stop\n")
                    self.process.stdin.flush()
                    self.process.stdin.close()
                self.process.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired):
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=5)
        self.close_handles()
        self.transcript.close()

    def saved_chunk(self) -> tuple[str, dict[str, Nbt]]:
        require(self.process is not None and self.process.poll() is not None,
                "Saved NBT may only be inspected/edited after this server stops")
        return read_chunk(region_path(self.directory / "world", CHUNK), CHUNK)


def block_placement_succeeded(output: str) -> bool:
    # The cross-edition console renderer currently selects Bedrock's success
    # text. Accept Java's equivalent too, but never an error containing it.
    return output.strip().lower().startswith(("block placed", "changed the block"))


def set_block(server: SmokeServer, position: tuple[int, int, int], block: str) -> None:
    output = server.command(f"setblock {' '.join(map(str, position))} minecraft:{block}")
    require(block_placement_succeeded(output), f"Block placement failed: {output!r}")


def set_item(server: SmokeServer, position: tuple[int, int, int], slot: int, item: str, count: int) -> None:
    server.command(
        f"item replace block {' '.join(map(str, position))} container.{slot} with minecraft:{item} {count}",
        contains="Replaced a slot",
    )


def counter(server: SmokeServer, color: str, realtime: bool = False) -> dict[str, Any]:
    output = server.command(f"counter {color}" + (" realtime" if realtime else ""))
    total = re.search(rf"{color}: (\d+) items total", output)
    require(total is not None, f"Malformed /counter response: {output!r}")
    elapsed = re.search(r"([\d.]+)s (game-time|real-time)", output)
    ticks = re.search(r"\((\d+) ticks @ 20 TPS\)", output)
    rate = re.search(r"([\d.]+) items/h", output)
    return {
        "total": int(total[1]), "seconds": float(elapsed[1]) if elapsed else None,
        "ticks": int(ticks[1]) if ticks else None,
        "rate": float(rate[1]) if rate else None, "text": output,
    }


def freeze(server: SmokeServer) -> None:
    server.command("tick freeze", contains="frozen")
    server.command("counter black reset", contains="Reset hopper counter")
    # Observe the game clock settling, rather than assuming freeze took effect
    # on the exact networking thread which handled the RCON request.
    deadline = time.monotonic() + server.args.check_timeout
    last_ticks, stable_since = None, time.monotonic()
    while time.monotonic() < deadline:
        ticks = counter(server, "black")["ticks"]
        if ticks != last_ticks:
            last_ticks, stable_since = ticks, time.monotonic()
        elif time.monotonic() - stable_since >= 0.2:
            return
        time.sleep(0.05)
    raise SmokeFailure("Game clock did not stop after /tick freeze")


def step(server: SmokeServer, ticks: int, sprint: bool = False) -> None:
    before = counter(server, "black")["ticks"]
    require(before is not None, "Step probe channel is not initialized")
    command = "sprint" if sprint else "step"
    output = server.command(f"tick {command} {ticks}t")
    require("unknown" not in output.lower() and "must be frozen" not in output.lower(),
            f"Tick command failed: {output!r}")
    result = server.until(
        f"{ticks} {command} ticks", lambda: counter(server, "black"),
        lambda report: report["ticks"] >= before + ticks,
    )
    require(result["ticks"] == before + ticks, f"/{command} advanced an unexpected number of ticks")


def pulse(server: SmokeServer, position: tuple[int, int, int]) -> None:
    power = (position[0] + 1, position[1], position[2])
    set_block(server, power, "redstone_block")
    step(server, 8)
    set_block(server, power, "air")
    step(server, 2)


def query_forced(server: SmokeServer, expected: bool) -> None:
    output = server.command(f"forceload query {BASE[0]} {BASE[1]}")
    phrase = "is marked for force loading" if expected else "is not marked for force loading"
    require(phrase in output, f"Forceload query did not match {expected}: {output!r}")


def wait_for_fixture_chunk(server: SmokeServer) -> None:
    # This both waits for full chunk generation and proves the restored ticket
    # actually makes a distant chunk available, not merely visible in /query.
    server.until(
        "distant forced chunk loading",
        lambda: server.command(f"setblock {BASE[0]} 199 {BASE[1]} minecraft:stone"),
        block_placement_succeeded,
    )


def initial_world_checks(server: SmokeServer) -> None:
    query_forced(server, False)
    server.command(f"forceload add {BASE[0]} {BASE[1]}", contains="Marked chunk")
    query_forced(server, True)
    wait_for_fixture_chunk(server)
    server.passed("forceload_add_and_load")
    freeze(server)
    server.command("fill 160 199 160 175 199 175 minecraft:stone", contains="filled")
    server.command("carpet rotatorBlock true", contains="Set rule rotatorBlock to true")
    for name, position in DISPENSERS.items():
        set_block(server, position, "dispenser")
        target = (position[0], position[1], position[2] - 1)
        set_block(server, target, "air" if name == "air_target" else "stone")
        set_item(server, position, 0, "cactus", 4)
    pulse(server, DISPENSERS["stone_target"])
    pulse(server, DISPENSERS["stone_target"])
    pulse(server, DISPENSERS["air_target"])
    server.command("carpet rotatorBlock false", contains="Set rule rotatorBlock to false")
    pulse(server, DISPENSERS["disabled_control"])
    server.command("carpet rotatorBlock true", contains="Set rule rotatorBlock to true")

    server.command("carpet hopperCounters true", contains="Set rule hopperCounters to true")
    set_block(server, (FED_HOPPER[0], FED_HOPPER[1] - 1, FED_HOPPER[2]), "blue_wool")
    set_block(server, FED_HOPPER, "hopper")
    set_block(server, SOURCE_CHEST, "chest")
    server.command("counter blue reset", contains="Reset hopper counter")
    # Feed only the chest, never this hopper: this catches an empty counter
    # hopper short-circuiting before its normal suck-in path can run.
    set_item(server, SOURCE_CHEST, 0, "stone", 3)
    require(counter(server, "blue")["total"] == 0, "Frozen chest-fed counter ran early")
    step(server, 40)
    fed = counter(server, "blue")
    require(fed["total"] == 3 and fed["ticks"] == 40,
            f"Empty hopper failed to pull/count three chest items: {fed}")
    server.passed("empty_counter_hopper_pulls_from_chest", counted_items=3)

    # A separate channel verifies atomic multi-slot draining and exact rate
    # arithmetic without tying it to the normal single-item transfer cooldown.
    set_block(server, (HOPPER[0], HOPPER[1] - 1, HOPPER[2]), "red_wool")
    set_block(server, HOPPER, "hopper")
    server.command("counter red reset", contains="Reset hopper counter")
    set_item(server, HOPPER, 0, "stone", 40)
    set_item(server, HOPPER, 1, "dirt", 20)
    before = counter(server, "red")
    require(before["total"] == 0 and before["ticks"] == 0 and before["rate"] is None,
            f"Frozen/reset counter should be empty with a pending rate: {before}")
    step(server, 20)
    game = counter(server, "red")
    require(game["total"] == 60 and game["ticks"] == 20 and game["rate"] == 216000.0,
            f"Hopper batch/game-time rate mismatch: {game}")
    require("stone: 40 items" in game["text"] and "dirt: 20 items" in game["text"],
            "Per-item hopper counts missing")
    real_before = counter(server, "red", realtime=True)
    time.sleep(0.35)
    frozen, real_after = counter(server, "red"), counter(server, "red", realtime=True)
    require(frozen["ticks"] == 20 and frozen["total"] == 60, "Frozen game counter advanced")
    require(real_after["seconds"] >= real_before["seconds"] + 0.25,
            "Real-time counter did not include frozen time")
    require(real_after["rate"] < real_before["rate"], "Real-time rate did not decrease during freeze")
    server.command("counter realtime", contains="real-time, including pauses and lag")
    server.command("tick rate 10", contains="target tick rate to 10")
    step(server, 20)
    slowed = counter(server, "red")
    require(slowed["ticks"] == 40 and slowed["seconds"] == 2.0 and slowed["rate"] == 108000.0,
            f"Tick rate changed the normalized game-time measurement: {slowed}")
    step(server, 4, sprint=True)
    require(counter(server, "red")["ticks"] == 44, "Sprint did not advance four game ticks")
    server.command("tick rate 20", contains="target tick rate to 20")
    server.command("counter red reset", contains="Reset hopper counter")
    step(server, 1)
    cleared = counter(server, "red")
    require(cleared["total"] == 0 and cleared["ticks"] == 1 and cleared["rate"] == 0.0,
            f"Reset did not start an empty timed session: {cleared}")
    server.passed("hopper_counts_rates_freeze_step_sprint_and_reset", counted_items=60)
    if not server.args.skip_fake_player:
        set_block(server, COMMAND_BLOCK, "command_block")


def saved_initial_checks(server: SmokeServer) -> None:
    name, root = server.saved_chunk()
    counts = {}
    for label, position in DISPENSERS.items():
        entity = block_entity(root, position)
        require(field(entity, "id", 8) == "minecraft:dispenser", "Wrong saved dispenser type")
        counts[label] = inventory_count(entity, "minecraft:cactus")
    require(counts == {"stone_target": 4, "air_target": 4, "disabled_control": 3},
            f"Cactus no-consumption/negative-control failure: {counts}")
    server.passed("rotator_noop_targets_keep_cactus_and_disabled_rule_ejects", inventory=counts)
    hopper = block_entity(root, HOPPER)
    require(not hopper.get("Items", Nbt(9, NbtList(0, []))).value.values,
            "Counted hopper still contains items after save")
    for position in (FED_HOPPER, SOURCE_CHEST):
        inventory = block_entity(root, position)
        require(not inventory.get("Items", Nbt(9, NbtList(0, []))).value.values,
                f"Chest-fed counter left uncounted inventory at {position}")
    server.passed("hopper_counted_items_are_removed_from_saved_inventory")
    if not server.args.skip_fake_player:
        command = block_entity(root, COMMAND_BLOCK)
        require(field(command, "id", 8) == "minecraft:command_block", "Wrong saved command block")
        command.update({
            "Command": Nbt(8, f"player {BOT} spawn"), "auto": Nbt(1, 0),
            "powered": Nbt(1, 0), "TrackOutput": Nbt(1, 1), "SuccessCount": Nbt(3, 0),
        })
        replace_chunk(region_path(server.directory / "world", CHUNK), CHUNK, name, root)


def player_position(server: SmokeServer) -> tuple[float, float, float]:
    output = server.command(f"data get entity {BOT}", contains="Pos:")
    match = re.search(r"\bPos:\s*\[([^\]]+)\]", output)
    require(match is not None, f"Player NBT has no position: {output!r}")
    values = tuple(float(item.strip().rstrip("dDfF")) for item in match[1].split(","))
    require(len(values) == 3 and all(math.isfinite(value) for value in values), "Invalid player position")
    return values


def fake_player_checks(server: SmokeServer) -> None:
    server.command("setworldspawn 166 200 170", contains="spawn")
    freeze(server)
    # A full chunk can become readable just before its first world tick
    # migrates saved block entities into the active map.
    step(server, 2)
    pulse(server, COMMAND_BLOCK)
    server.until("command-block fake-player spawn", lambda: server.command("player list"),
                 lambda text: BOT in text)
    step(server, 4)
    server.command(f"player {BOT} look 0 0", contains="Turned fake player")
    before = player_position(server)
    server.command(f"player {BOT} move forward", contains="Moving forward")
    step(server, 5)
    moved = player_position(server)
    require(moved[2] > before[2] + 0.05, f"Forward input did not move the fake player: {before} -> {moved}")
    server.command(f"player {BOT} move stop", contains="Stopped movement")
    step(server, 20)  # Allow normal physical momentum to decay.
    settled = player_position(server)
    step(server, 10)
    stopped = player_position(server)
    require(math.hypot(stopped[0] - settled[0], stopped[2] - settled[2]) < 0.01,
            f"Stopped fake player kept moving: {settled} -> {stopped}")
    server.passed("fake_player_spawn_move_and_stop", before=before, moved=moved, stopped=stopped)

    # Exercise scheduling syntax with an empty hand, then clear it before the
    # single-placement assertion so its result cannot be masked by repetition.
    server.command(f"player {BOT} use continuous", contains="Scheduled use continuously")
    server.command(f"player {BOT} use interval 4", contains="Scheduled use every 4 ticks")
    server.command(f"player {BOT} stop", contains="Stopped actions")
    server.command(f"teleport {BOT} 166.5 200 167.5", contains="Teleported")
    step(server, 2)
    position = player_position(server)
    require(math.dist(position, (166.5, 200.0, 167.5)) < 0.1, f"Headless teleport failed: {position}")
    server.command(f"item replace entity {BOT} weapon.mainhand with minecraft:stone 4", contains="Replaced")
    server.command(f"player {BOT} look 0 45", contains="Turned fake player")
    server.command(f"player {BOT} use once", contains="Scheduled use once")
    step(server, 2)
    server.command(f"player {BOT} stop", contains="Stopped actions")

    # A main-hand block has no air-use action. The fallback must consume the
    # offhand bottle, not the unrelated remaining stone stack in the main hand.
    server.command(f"player {BOT} look 0 -90", contains="Turned fake player")
    server.command(
        f"item replace entity {BOT} weapon.offhand with minecraft:experience_bottle 2",
        contains="Replaced",
    )
    server.command(f"player {BOT} use once", contains="Scheduled use once")
    step(server, 2)
    server.command(f"player {BOT} stop", contains="Stopped actions")
    server.command(f"player {BOT} kill")
    server.until("fake-player removal", lambda: server.command("player list"), lambda text: BOT not in text)
    # Keep the third-start assertion outside spawn loading again.
    server.command("setworldspawn 0 200 0", contains="spawn")


def saved_fake_player_checks(server: SmokeServer) -> None:
    _, root = server.saved_chunk()
    require(block_name(root, USE_TARGET) == "minecraft:stone",
            f"/player use once did not place a block at {USE_TARGET}")
    bot_uuid = uuid.UUID(bytes=hashlib.md5(f"OfflinePlayer:{BOT}".encode()).digest(), version=3)
    player_file = server.directory / "world" / "players" / "data" / f"{bot_uuid}.dat"
    require(player_file.is_file(), f"Fake-player data was not persisted: {player_file}")
    _, player = read_nbt(gzip.decompress(player_file.read_bytes()))
    count = inventory_count(player, "minecraft:stone", key="Inventory")
    require(count == 3, f"One fake-player block use should consume one of four stones, found {count}")
    server.passed("fake_player_use_once_places_one_block_and_consumes_one_item")
    bottles = inventory_count(player, "minecraft:experience_bottle", key="Inventory")
    require(bottles == 1, f"Offhand use must consume one of two bottles, found {bottles}")
    server.passed("fake_player_offhand_use_consumes_only_the_offhand_item")


def run_checks(server: SmokeServer) -> None:
    server.start()
    initial_world_checks(server)
    server.stop()
    saved_initial_checks(server)

    server.start()
    query_forced(server, True)
    wait_for_fixture_chunk(server)
    server.passed("forceload_restored_after_normal_restart")
    if not server.args.skip_fake_player:
        fake_player_checks(server)
    server.command(f"forceload remove {BASE[0]} {BASE[1]}", contains="Unmarked chunk")
    query_forced(server, False)
    server.stop()
    if not server.args.skip_fake_player:
        saved_fake_player_checks(server)

    server.start()
    query_forced(server, False)
    server.passed("forceload_removal_survives_another_restart")
    server.stop()


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--server", type=Path, required=True, help="Existing server executable (resolved before changing cwd)")
    parser.add_argument("--keep", action="store_true", help="Keep the fresh temporary world even after success")
    parser.add_argument("--temp-parent", type=Path, help="Parent for a new temporary directory, never an existing world")
    parser.add_argument("--skip-fake-player", action="store_true", help="Run core persistence/rotator/counter checks only")
    parser.add_argument("--data-version", type=int, default=DATA_VERSION, help="NBT fixture version for this server (default: %(default)s)")
    parser.add_argument("--startup-timeout", type=float, default=180, help="Seconds allowed per server startup")
    parser.add_argument("--shutdown-timeout", type=float, default=120, help="Seconds allowed for a normal save/stop")
    parser.add_argument("--command-timeout", type=float, default=10, help="Seconds allowed per RCON response")
    parser.add_argument("--check-timeout", type=float, default=30, help="Seconds allowed for chunk loading, step, spawn, etc.")
    args = parser.parse_args(argv)
    args.server = args.server.expanduser().resolve()
    if not args.server.is_file() or not os.access(args.server, os.X_OK):
        parser.error(f"--server is not an executable file: {args.server}")
    if not 0 < args.data_version < 2 ** 31:
        parser.error("--data-version must be a positive signed 32-bit integer")
    for option in ("startup_timeout", "shutdown_timeout", "command_timeout", "check_timeout"):
        if not math.isfinite(getattr(args, option)) or getattr(args, option) <= 0:
            parser.error(f"--{option.replace('_', '-')} must be a positive finite number")
    if args.temp_parent is not None:
        args.temp_parent = args.temp_parent.expanduser().resolve()
        if not args.temp_parent.is_dir():
            parser.error("--temp-parent must be an existing directory")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    directory = Path(tempfile.mkdtemp(prefix="verdantgolem-smoke-", dir=args.temp_parent)).resolve()
    marker = secrets.token_hex(16)
    (directory / ".smoke-owner").write_text(marker, encoding="ascii")
    print(f"Smoke fixture: {directory}", flush=True)
    server = SmokeServer(args, directory)
    success, error = False, None
    try:
        prepare_world(directory, args.data_version)
        run_checks(server)
        success = True
    except (Exception, KeyboardInterrupt) as exception:
        error = f"{type(exception).__name__}: {exception}"
        (directory / "failure.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"FAIL {error}", file=sys.stderr, flush=True)
    finally:
        try:
            server.cleanup()
        except Exception as exception:
            success = False
            error = f"{error or ''}; cleanup: {exception}"
            print(f"Cleanup error: {exception}", file=sys.stderr)
        summary = {
            "status": "passed" if success else "failed", "error": error,
            "server": str(args.server), "directory": str(directory),
            "fake_player_checks": not args.skip_fake_player, "tests": server.results,
        }
        (directory / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    if success and not args.keep:
        require((directory / ".smoke-owner").read_text(encoding="ascii") == marker, "Fixture ownership marker changed")
        shutil.rmtree(directory)
        print(f"PASS {len(server.results)} checks; temporary fixture removed", flush=True)
    else:
        print(f"Logs and fixture retained: {directory}", flush=True)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
