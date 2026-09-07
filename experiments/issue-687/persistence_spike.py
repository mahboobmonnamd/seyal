#!/usr/bin/env python3
"""Reproducible, research-only persistence comparison for Seyal Issue #687.

This is deliberately outside the production workspace/runtime crates.  It uses
stdlib SQLite and files to compare recovery semantics and rough costs for the
four models named by Issue #687.  The numbers are diagnostic, not product
budgets and not terminal-runtime measurements.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import tempfile
import threading
import time
from pathlib import Path
from typing import Any


MODEL_NAMES = ("sqlite", "journal", "snapshot", "hybrid")


def now_ns() -> int:
    return time.perf_counter_ns()


def fsync_file(path: Path) -> None:
    with path.open("rb") as handle:
        os.fsync(handle.fileno())


def atomic_write(path: Path, data: bytes) -> None:
    temporary = path.with_name(path.name + ".tmp")
    with temporary.open("wb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    directory = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def stable_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def clone(value: Any) -> Any:
    return json.loads(json.dumps(value))


def canonical_state(value: dict[str, Any]) -> dict[str, Any]:
    state = clone(value)
    for workspace in state.get("workspaces", {}).values():
        workspace.setdefault("layout_version", 1)
        workspace.setdefault("presentation", {}).setdefault("selected", None)
    return state


def checksum(value: Any) -> str:
    return hashlib.sha256(stable_json(value)).hexdigest()


def base_state(scale: int) -> dict[str, Any]:
    workspaces = {}
    for index in range(scale):
        workspace_id = f"ws-{index:05d}"
        execution_id = f"exec-{index:05d}-0001"
        workspaces[workspace_id] = {
            "workspace_id": workspace_id,
            "name": f"Workspace {index}",
            "tabs": [f"tab-{index:05d}"],
            "closed": False,
            "presentation": {"attached": True, "selected": execution_id},
            "executions": {
                execution_id: {
                    "execution_id": execution_id,
                    "workspace_id": workspace_id,
                    "live_status": "live",
                    "pty_identity": f"pty-{index:05d}",
                    "history": [
                        {
                            "segment_id": f"seg-{index:05d}-0001",
                            "line_ids": [f"line-{index:05d}-0001"],
                            "payload": f"output-{index}-public",
                            "redacted": False,
                        }
                    ],
                }
            },
        }
    return {"schema_version": 1, "next_execution": scale + 1, "next_segment": scale + 1, "workspaces": workspaces}


def mutate_state(state: dict[str, Any], scale: int) -> None:
    """Apply create/update/reorder/close and history retention operations."""

    for index in range(min(scale, 25)):
        workspace_id = f"ws-{index:05d}"
        workspace = state["workspaces"][workspace_id]
        workspace["name"] += " / updated"
        workspace["tabs"].insert(0, workspace["tabs"].pop())
        execution_id = next(iter(workspace["executions"]))
        history = workspace["executions"][execution_id]["history"]
        segment_sequence = state["next_segment"]
        state["next_segment"] += 1
        history.append(
            {
                "segment_id": f"seg-{index:05d}-{segment_sequence:04d}",
                "line_ids": [f"line-{index:05d}-{segment_sequence:04d}"],
                "payload": f"output-{index}-secret-token",
                "redacted": False,
            }
        )
        # A redaction request covers the newly appended sensitive segment too;
        # no backend may retain a readable secret merely because it was added
        # after the previous checkpoint.
        history[-1]["payload"] = "[redacted]"
        history[-1]["redacted"] = True
        # Retention/redaction are explicit records, never inferred from a row.
        if index % 2 == 0:
            history[0]["payload"] = "[redacted]"
            history[0]["redacted"] = True
        if index % 5 == 0:
            history.pop(0)
        if index % 7 == 0:
            workspace["closed"] = True
            workspace["executions"][execution_id]["live_status"] = "finalized"
            workspace["executions"][execution_id]["pty_identity"] = None

    # A replacement after close must have a fresh identity and cannot resurrect
    # the old PTY/execution.
    closed = state["workspaces"]["ws-00000"]
    replacement_id = f"exec-00000-{state['next_execution']:04d}"
    state["next_execution"] += 1
    closed["executions"][replacement_id] = {
        "execution_id": replacement_id,
        "workspace_id": "ws-00000",
        "live_status": "not_claimed",
        "pty_identity": None,
        "history": [],
    }
    closed["presentation"]["attached"] = False


def append_unredacted_fixture(state: dict[str, Any]) -> None:
    """Create one committed sensitive history payload before redaction."""

    execution = state["workspaces"]["ws-00000"]["executions"]["exec-00000-0001"]
    execution["history"].append(
        {
            "segment_id": "seg-00000-sensitive",
            "line_ids": ["line-00000-sensitive"],
            "payload": "output-0-secret-token",
            "redacted": False,
        }
    )


def validate_recovered(state: dict[str, Any], model: str) -> dict[str, Any]:
    workspaces = state.get("workspaces", {})
    finalized = 0
    not_claimed = 0
    live_claims = 0
    secret_survivors = 0
    unavailable_segments = 0
    duplicate_execution_ids = 0
    execution_ids: set[str] = set()
    for workspace in workspaces.values():
        for execution in workspace.get("executions", {}).values():
            execution_id = execution["execution_id"]
            if execution_id in execution_ids:
                duplicate_execution_ids += 1
            execution_ids.add(execution_id)
            status = execution["live_status"]
            finalized += status == "finalized"
            not_claimed += status == "not_claimed"
            live_claims += status == "live" and execution.get("pty_identity") is not None
            for segment in execution.get("history", []):
                secret_survivors += "secret-token" in segment.get("payload", "")
                unavailable_segments += segment.get("availability") == "unavailable"
    return {
        "model": model,
        "workspace_count": len(workspaces),
        "finalized_execution_count": finalized,
        "not_claimed_execution_count": not_claimed,
        "live_claims_after_runtime_absent": live_claims,
        "secret_history_survivors": secret_survivors,
        "unavailable_history_segments": unavailable_segments,
        "duplicate_execution_ids": duplicate_execution_ids,
        "non_resurrection_ok": live_claims == 0 and duplicate_execution_ids == 0,
        "redaction_and_truncation_ok": secret_survivors == 0,
        "runtime_absent_is_explicit": not_claimed > 0,
    }


def mark_runtime_absent(state: dict[str, Any]) -> dict[str, Any]:
    """Apply the honest-recovery rule: metadata survives, PTYs do not."""

    for workspace in state.get("workspaces", {}).values():
        workspace.setdefault("presentation", {})["attached"] = False
        for execution in workspace.get("executions", {}).values():
            if execution.get("live_status") == "live":
                execution["live_status"] = "not_claimed"
                execution["pty_identity"] = None
    return state


class Backend:
    def __init__(self, root: Path, model: str):
        self.root = root
        self.model = model
        self.current_schema_version = 1

    def validate_write_version(self, state: dict[str, Any]) -> None:
        incoming = int(state["schema_version"])
        if incoming < self.current_schema_version:
            raise RuntimeError(
                f"schema downgrade refused: current={self.current_schema_version} incoming={incoming}"
            )
        self.current_schema_version = incoming

    def commit(self, state: dict[str, Any], failpoint: str | None = None) -> None:
        raise NotImplementedError

    def recover(self) -> dict[str, Any]:
        raise NotImplementedError

    def corrupt_last_commit(self) -> None:
        raise NotImplementedError

    def migrate(self) -> float:
        start = now_ns()
        state = self.recover()
        state["schema_version"] = 2
        for workspace in state["workspaces"].values():
            workspace["layout_version"] = 2
        self.commit(state)
        return (now_ns() - start) / 1e6

    def close(self) -> None:
        pass

    def size_bytes(self) -> int:
        return sum(path.stat().st_size for path in self.root.rglob("*") if path.is_file())

    def raw_secret_bytes(self) -> int:
        return sum(path.read_bytes().count(b"secret-token") for path in self.root.rglob("*") if path.is_file())


class SqliteBackend(Backend):
    """Typed metadata comparator; history is represented by explicit rows."""

    def __init__(self, root: Path, model: str):
        super().__init__(root, model)
        self.path = root / "workspace.sqlite3"
        self.db = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        self.db.executescript(
            """
            CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, tabs_json TEXT NOT NULL,
                closed INTEGER NOT NULL, attached INTEGER NOT NULL,
                selected TEXT, layout_version INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL,
                live_status TEXT NOT NULL, pty_identity TEXT
            );
            CREATE TABLE IF NOT EXISTS history_segments (
                execution_id TEXT NOT NULL, segment_id TEXT NOT NULL,
                line_ids_json TEXT NOT NULL, payload TEXT NOT NULL,
                redacted INTEGER NOT NULL, PRIMARY KEY(execution_id, segment_id)
            );
            """
        )

    def commit(self, state: dict[str, Any], failpoint: str | None = None) -> None:
        self.validate_write_version(state)
        self.db.execute("BEGIN IMMEDIATE")
        try:
            self.db.execute("DELETE FROM workspaces")
            self.db.execute("DELETE FROM executions")
            self.db.execute("DELETE FROM history_segments")
            self.db.executemany(
                "INSERT INTO workspaces(id,name,tabs_json,closed,attached,selected,layout_version) VALUES (?,?,?,?,?,?,?)",
                [
                    (
                        workspace_id,
                        workspace["name"],
                        json.dumps(workspace["tabs"], sort_keys=True),
                        int(workspace["closed"]),
                        int(workspace["presentation"]["attached"]),
                        workspace["presentation"].get("selected"),
                        workspace.get("layout_version", 1),
                    )
                    for workspace_id, workspace in state["workspaces"].items()
                ],
            )
            self.db.executemany(
                "INSERT INTO executions(id,workspace_id,live_status,pty_identity) VALUES (?,?,?,?)",
                [
                    (
                        execution["execution_id"],
                        workspace_id,
                        execution["live_status"],
                        execution.get("pty_identity"),
                    )
                    for workspace_id, workspace in state["workspaces"].items()
                    for execution in workspace["executions"].values()
                ],
            )
            self.db.executemany(
                "INSERT INTO history_segments(execution_id,segment_id,line_ids_json,payload,redacted) VALUES (?,?,?,?,?)",
                [
                    (
                        execution["execution_id"],
                        segment["segment_id"],
                        json.dumps(segment["line_ids"], sort_keys=True),
                        segment["payload"],
                        int(segment["redacted"]),
                    )
                    for workspace in state["workspaces"].values()
                    for execution in workspace["executions"].values()
                    for segment in execution.get("history", [])
                    if segment.get("availability", "available") == "available"
                ],
            )
            self.db.execute(
                "INSERT INTO meta(key,value) VALUES ('state',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (json.dumps({"schema_version": state["schema_version"], "next_execution": state["next_execution"], "next_segment": state["next_segment"]}),),
            )
            self.db.execute("COMMIT")
        except Exception:
            self.db.execute("ROLLBACK")
            raise

    def recover(self) -> dict[str, Any]:
        metadata = json.loads(self.db.execute("SELECT value FROM meta WHERE key='state'").fetchone()[0])
        self.current_schema_version = int(metadata["schema_version"])
        workspaces = {}
        for row in self.db.execute(
            "SELECT id,name,tabs_json,closed,attached,selected,layout_version FROM workspaces ORDER BY id"
        ):
            workspace_id, name, tabs_json, closed, attached, selected, layout_version = row
            workspaces[workspace_id] = {
                "workspace_id": workspace_id,
                "name": name,
                "tabs": json.loads(tabs_json),
                "closed": bool(closed),
                "presentation": {"attached": bool(attached), "selected": selected},
                "layout_version": layout_version,
                "executions": {},
            }
        execution_workspace = {}
        for execution_id, workspace_id, live_status, pty_identity in self.db.execute(
            "SELECT id,workspace_id,live_status,pty_identity FROM executions ORDER BY id"
        ):
            execution_workspace[execution_id] = workspace_id
            workspaces[workspace_id]["executions"][execution_id] = {
                "execution_id": execution_id,
                "workspace_id": workspace_id,
                "live_status": live_status,
                "pty_identity": pty_identity,
                "history": [],
            }
        for execution_id, segment_id, line_ids_json, payload, redacted in self.db.execute(
            "SELECT execution_id,segment_id,line_ids_json,payload,redacted FROM history_segments ORDER BY execution_id,segment_id"
        ):
            workspaces[execution_workspace[execution_id]]["executions"][execution_id]["history"].append(
                {
                    "segment_id": segment_id,
                    "line_ids": json.loads(line_ids_json),
                    "payload": payload,
                    "redacted": bool(redacted),
                }
            )
        return {**metadata, "workspaces": workspaces}

    def corrupt_last_commit(self) -> None:
        # An uncommitted transaction is discarded by close/reopen; this models
        # a process crash in the commit window without fabricating a live claim.
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("DELETE FROM workspaces WHERE id='ws-00000'")
        self.db.close()
        self.db = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")

    def close(self) -> None:
        self.db.close()


class JournalBackend(Backend):
    def __init__(self, root: Path, model: str):
        super().__init__(root, model)
        self.path = root / "events.jsonl"
        self.sequence = 0

    @staticmethod
    def _events_for_state(state: dict[str, Any]) -> list[dict[str, Any]]:
        events = []
        for workspace_id, workspace in sorted(state["workspaces"].items()):
            events.append({
                "kind": "workspace_upsert", "workspace_id": workspace_id,
                "name": workspace["name"], "tabs": workspace["tabs"],
                "closed": workspace["closed"], "presentation": workspace["presentation"],
                "layout_version": workspace.get("layout_version", 1),
            })
            for execution in sorted(workspace["executions"].values(), key=lambda item: item["execution_id"]):
                events.append({
                    "kind": "execution_upsert", "execution_id": execution["execution_id"],
                    "workspace_id": workspace_id, "live_status": execution["live_status"],
                    "pty_identity": execution.get("pty_identity"),
                })
                events.append({"kind": "history_clear", "execution_id": execution["execution_id"]})
                for segment in execution.get("history", []):
                    if segment.get("availability", "available") == "available":
                        events.append({
                            "kind": "history_upsert", "execution_id": execution["execution_id"],
                            "segment_id": segment["segment_id"], "line_ids": segment["line_ids"],
                            "payload": segment["payload"], "redacted": segment["redacted"],
                        })
        return events

    @staticmethod
    def _apply_events(state: dict[str, Any], events: list[dict[str, Any]], schema_version: int, next_execution: int, next_segment: int) -> None:
        workspaces = state.setdefault("workspaces", {})
        for event in events:
            kind = event["kind"]
            if kind == "workspace_upsert":
                workspaces[event["workspace_id"]] = {
                    "workspace_id": event["workspace_id"], "name": event["name"],
                    "tabs": event["tabs"], "closed": event["closed"],
                    "presentation": event["presentation"], "layout_version": event.get("layout_version", 1),
                    "executions": workspaces.get(event["workspace_id"], {}).get("executions", {}),
                }
            elif kind == "execution_upsert":
                workspaces[event["workspace_id"]]["executions"][event["execution_id"]] = {
                    "execution_id": event["execution_id"], "workspace_id": event["workspace_id"],
                    "live_status": event["live_status"], "pty_identity": event.get("pty_identity"), "history": [],
                }
            elif kind == "history_clear":
                for workspace in workspaces.values():
                    execution = workspace["executions"].get(event["execution_id"])
                    if execution is not None:
                        execution["history"] = []
                        break
            elif kind == "history_upsert":
                for workspace in workspaces.values():
                    execution = workspace["executions"].get(event["execution_id"])
                    if execution is not None:
                        execution["history"].append({
                            "segment_id": event["segment_id"], "line_ids": event["line_ids"],
                            "payload": event["payload"], "redacted": event["redacted"],
                        })
                        break
        state["schema_version"] = schema_version
        state["next_execution"] = next_execution
        state["next_segment"] = next_segment

    def commit(self, state: dict[str, Any], failpoint: str | None = None) -> None:
        self.validate_write_version(state)
        self.sequence += 1
        events = self._events_for_state(state)
        records = [{"record": "event", "seq": self.sequence, "event": event} for event in events]
        records.append({
            "record": "commit", "seq": self.sequence, "schema_version": state["schema_version"],
            "next_execution": state["next_execution"], "next_segment": state["next_segment"], "event_count": len(events),
            "events_checksum": checksum(events),
        })
        with self.path.open("ab") as handle:
            for record in records:
                record["checksum"] = checksum({key: value for key, value in record.items() if key != "checksum"})
                handle.write(stable_json(record) + b"\n")
            handle.flush()
            os.fsync(handle.fileno())

    def recover(self) -> dict[str, Any]:
        if not self.path.exists():
            raise RuntimeError("journal missing")
        latest = {"schema_version": 1, "next_execution": 0, "next_segment": 0, "workspaces": {}}
        pending = []
        valid_end = 0
        latest_seq = 0
        with self.path.open("rb") as handle:
            for raw in handle:
                try:
                    record = json.loads(raw)
                    expected = checksum({key: value for key, value in record.items() if key != "checksum"})
                    if record.get("checksum") != expected:
                        break
                    if record.get("record") == "event":
                        pending.append(record["event"])
                    elif record.get("record") == "commit":
                        if record["event_count"] != len(pending) or record["events_checksum"] != checksum(pending):
                            break
                        self._apply_events(latest, pending, int(record["schema_version"]), int(record["next_execution"]), int(record["next_segment"]))
                        latest_seq = max(latest_seq, int(record["seq"]))
                        pending = []
                        valid_end = handle.tell()
                    else:
                        break
                except (ValueError, KeyError, UnicodeDecodeError):
                    break
        if valid_end == 0:
            raise RuntimeError("no valid journal commit")
        # Quarantine the torn/corrupt suffix before accepting new commits.
        # Otherwise a later valid event would remain hidden behind the first
        # invalid record on every restart.
        with self.path.open("r+b") as handle:
            handle.truncate(valid_end)
            handle.flush()
            os.fsync(handle.fileno())
        self.sequence = latest_seq
        self.current_schema_version = int(latest["schema_version"])
        return latest

    def corrupt_last_commit(self) -> None:
        with self.path.open("ab") as handle:
            handle.write(b'{"seq":999,"op":"replace_state","state":')
            handle.flush()
            os.fsync(handle.fileno())


class SnapshotBackend(Backend):
    def __init__(self, root: Path, model: str):
        super().__init__(root, model)
        self.path = root / "workspace.json"

    def commit(self, state: dict[str, Any], failpoint: str | None = None) -> None:
        self.validate_write_version(state)
        atomic_write(self.path, stable_json(state) + b"\n")

    def recover(self) -> dict[str, Any]:
        with self.path.open("rb") as handle:
            state = json.loads(handle.read())
        self.current_schema_version = int(state["schema_version"])
        return state

    def corrupt_last_commit(self) -> None:
        # Atomic replacement means a torn temporary file is ignored.
        temporary = self.path.with_name(self.path.name + ".tmp")
        temporary.write_bytes(b'{"schema_version":2,"workspaces":')
        fsync_file(temporary)


class HybridBackend(Backend):
    """Typed metadata plus immutable content-addressed history segments."""

    def __init__(self, root: Path, model: str):
        super().__init__(root, model)
        self.dbpath = root / "metadata.sqlite3"
        self.segment_dir = root / "segments"
        self.segment_dir.mkdir(exist_ok=True)
        self.db = sqlite3.connect(self.dbpath, isolation_level=None, check_same_thread=False)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        self.db.executescript(
            """
            CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, tabs_json TEXT NOT NULL,
                closed INTEGER NOT NULL, attached INTEGER NOT NULL,
                selected TEXT, layout_version INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL,
                live_status TEXT NOT NULL, pty_identity TEXT
            );
            CREATE TABLE IF NOT EXISTS history_refs (
                execution_id TEXT NOT NULL, ordinal INTEGER NOT NULL,
                storage_id TEXT, source_segment_id TEXT NOT NULL,
                first_line_id TEXT, last_line_id TEXT, content_checksum TEXT,
                availability TEXT NOT NULL, reason TEXT,
                PRIMARY KEY(execution_id, ordinal)
            );
            """
        )

    @staticmethod
    def _object_for(segment: dict[str, Any]) -> tuple[str, bytes, dict[str, Any]]:
        payload = {
            "format_version": 1,
            "source_segment_id": segment["segment_id"],
            "line_ids": segment["line_ids"],
            "payload": segment["payload"],
            "redacted": segment["redacted"],
        }
        content_checksum = checksum(payload)
        storage_id = f"segment-v1-{content_checksum}"
        envelope = {**payload, "content_checksum": content_checksum}
        return storage_id, stable_json(envelope) + b"\n", envelope

    def _refs_for_state(self, state: dict[str, Any]) -> tuple[dict[str, Any], list[tuple[Any, ...]], set[str]]:
        metadata = {"schema_version": state["schema_version"], "next_execution": state["next_execution"], "next_segment": state["next_segment"]}
        refs = []
        referenced = set()
        for workspace_id, workspace in state["workspaces"].items():
            for execution in workspace["executions"].values():
                for ordinal, segment in enumerate(execution.get("history", [])):
                    if segment.get("availability", "available") != "available":
                        refs.append((execution["execution_id"], ordinal, None, segment["segment_id"], None, None, None, "unavailable", segment.get("reason", "unavailable")))
                        continue
                    storage_id, _encoded, envelope = self._object_for(segment)
                    referenced.add(storage_id)
                    refs.append((
                        execution["execution_id"], ordinal, storage_id, segment["segment_id"],
                        segment["line_ids"][0] if segment["line_ids"] else None,
                        segment["line_ids"][-1] if segment["line_ids"] else None,
                        envelope["content_checksum"], "available", None,
                    ))
        return metadata, refs, referenced

    def commit(self, state: dict[str, Any], failpoint: str | None = None) -> None:
        self.validate_write_version(state)
        metadata, refs, referenced = self._refs_for_state(state)
        objects = {}
        for workspace in state["workspaces"].values():
            for execution in workspace["executions"].values():
                for segment in execution.get("history", []):
                    if segment.get("availability", "available") == "available":
                        storage_id, encoded, _envelope = self._object_for(segment)
                        objects[storage_id] = encoded
        if failpoint == "before_object":
            raise RuntimeError("fault injected before object durable write")
        for storage_id, encoded in objects.items():
            path = self.segment_dir / (storage_id + ".json")
            if path.exists() and path.read_bytes() != encoded:
                raise RuntimeError("immutable segment identity collision")
            if not path.exists():
                atomic_write(path, encoded)
        if failpoint == "after_object_before_manifest":
            raise RuntimeError("fault injected after object durable write before manifest")
        self.db.execute("BEGIN IMMEDIATE")
        try:
            self.db.execute("DELETE FROM workspaces")
            self.db.execute("DELETE FROM executions")
            self.db.execute("DELETE FROM history_refs")
            self.db.execute(
                "INSERT INTO meta(key,value) VALUES ('state',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (json.dumps(metadata, sort_keys=True),),
            )
            self.db.executemany(
                "INSERT INTO workspaces(id,name,tabs_json,closed,attached,selected,layout_version) VALUES (?,?,?,?,?,?,?)",
                [
                    (workspace_id, workspace["name"], json.dumps(workspace["tabs"], sort_keys=True), int(workspace["closed"]),
                     int(workspace["presentation"]["attached"]), workspace["presentation"].get("selected"), workspace.get("layout_version", 1))
                    for workspace_id, workspace in state["workspaces"].items()
                ],
            )
            self.db.executemany(
                "INSERT INTO executions(id,workspace_id,live_status,pty_identity) VALUES (?,?,?,?)",
                [
                    (execution["execution_id"], workspace_id, execution["live_status"], execution.get("pty_identity"))
                    for workspace_id, workspace in state["workspaces"].items()
                    for execution in workspace["executions"].values()
                ],
            )
            self.db.executemany(
                "INSERT INTO history_refs(execution_id,ordinal,storage_id,source_segment_id,first_line_id,last_line_id,content_checksum,availability,reason) VALUES (?,?,?,?,?,?,?,?,?)",
                refs,
            )
            self.db.execute("COMMIT")
        except Exception:
            self.db.execute("ROLLBACK")
            raise
        if failpoint == "after_manifest":
            raise RuntimeError("fault injected after manifest publication")
        for path in self.segment_dir.glob("segment-v1-*.json"):
            if path.stem not in referenced:
                path.unlink()

    def recover(self) -> dict[str, Any]:
        metadata = json.loads(self.db.execute("SELECT value FROM meta WHERE key='state'").fetchone()[0])
        self.current_schema_version = int(metadata["schema_version"])
        workspaces = {}
        for row in self.db.execute("SELECT id,name,tabs_json,closed,attached,selected,layout_version FROM workspaces ORDER BY id"):
            workspace_id, name, tabs_json, closed, attached, selected, layout_version = row
            workspaces[workspace_id] = {
                "workspace_id": workspace_id, "name": name, "tabs": json.loads(tabs_json), "closed": bool(closed),
                "presentation": {"attached": bool(attached), "selected": selected}, "layout_version": layout_version, "executions": {},
            }
        execution_workspace = {}
        for execution_id, workspace_id, live_status, pty_identity in self.db.execute("SELECT id,workspace_id,live_status,pty_identity FROM executions ORDER BY id"):
            execution_workspace[execution_id] = workspace_id
            workspaces[workspace_id]["executions"][execution_id] = {
                "execution_id": execution_id, "workspace_id": workspace_id, "live_status": live_status,
                "pty_identity": pty_identity, "history": [],
            }
        for row in self.db.execute("SELECT execution_id,storage_id,source_segment_id,first_line_id,last_line_id,content_checksum,availability,reason FROM history_refs ORDER BY execution_id,ordinal"):
            execution_id, storage_id, source_id, first_line, last_line, expected_checksum, availability, reason = row
            segment = {"segment_id": source_id, "line_ids": [first_line] if first_line else [], "payload": "", "redacted": False}
            if availability == "available":
                path = self.segment_dir / (storage_id + ".json")
                try:
                    envelope = json.loads(path.read_bytes())
                    payload = {key: envelope[key] for key in ("format_version", "source_segment_id", "line_ids", "payload", "redacted")}
                    actual_checksum = checksum(payload)
                    if envelope.get("content_checksum") != actual_checksum or actual_checksum != expected_checksum or storage_id != f"segment-v1-{actual_checksum}":
                        raise ValueError("checksum or identity mismatch")
                    if envelope["source_segment_id"] != source_id or (envelope["line_ids"] and (envelope["line_ids"][0] != first_line or envelope["line_ids"][-1] != last_line)):
                        raise ValueError("manifest range or identity mismatch")
                    segment = {"segment_id": source_id, "line_ids": envelope["line_ids"], "payload": envelope["payload"], "redacted": envelope["redacted"]}
                except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
                    segment["availability"] = "unavailable"
                    segment["reason"] = str(error)
            else:
                segment["availability"] = "unavailable"
                segment["reason"] = reason or "unavailable"
            workspaces[execution_workspace[execution_id]]["executions"][execution_id]["history"].append(segment)
        return {**metadata, "workspaces": workspaces}

    def corrupt_last_commit(self) -> None:
        temporary = self.segment_dir / "segment-v1-torn.json.tmp"
        temporary.write_bytes(b'{"format_version":1,"source_segment_id":"torn","payload":')
        fsync_file(temporary)

    def close(self) -> None:
        self.db.close()


def backend_for(root: Path, model: str) -> Backend:
    return {"sqlite": SqliteBackend, "journal": JournalBackend, "snapshot": SnapshotBackend, "hybrid": HybridBackend}[model](root, model)


def observe_concurrent(backend: Backend, state: dict[str, Any]) -> dict[str, Any]:
    """Observe from an independent connection while one writer commits."""

    stop = threading.Event()
    observations: list[dict[str, Any]] = []
    failures: list[str] = []
    reader_backend = backend_for(backend.root, backend.model)
    expected = {checksum(canonical_state(mark_runtime_absent(clone(state))))}

    def writer() -> None:
        for _ in range(4):
            try:
                backend.commit(state)
            except Exception as error:  # pragma: no cover - diagnostic report
                failures.append(type(error).__name__)
        stop.set()

    def reader_loop() -> None:
        while not stop.is_set():
            try:
                observations.append(mark_runtime_absent(reader_backend.recover()))
            except (OSError, RuntimeError, sqlite3.Error) as error:
                failures.append(type(error).__name__)

    reader_thread = threading.Thread(target=reader_loop)
    writer_thread = threading.Thread(target=writer)
    reader_thread.start()
    writer_thread.start()
    writer_thread.join()
    stop.set()
    reader_thread.join()
    valid = all(
        validate_recovered(item, backend.model)["non_resurrection_ok"]
        and validate_recovered(item, backend.model)["redaction_and_truncation_ok"]
        and checksum(canonical_state(item)) in expected
        for item in observations
    )
    reader_backend.close()
    return {
        "observations": len(observations),
        "failures": failures,
        "independent_reader_connection": True,
        "all_observations_were_committed_states": valid and bool(observations),
    }


def percentile(values: list[float], fraction: float) -> float:
    if not values:
        raise ValueError("percentile requires at least one sample")
    ordered = sorted(values)
    rank = max(1, min(len(ordered), int((len(ordered) * fraction) + 0.999999)))
    return ordered[rank - 1]


def _run_model_once(model: str, scale: int, commits: int, model_root: Path) -> dict[str, Any]:
    model_root.mkdir()
    backend = backend_for(model_root, model)
    state = base_state(scale)
    append_unredacted_fixture(state)
    backend.commit(state)
    commit_ms = []
    for _ in range(commits):
        mutate_state(state, scale)
        start = now_ns()
        backend.commit(state)
        commit_ms.append((now_ns() - start) / 1e6)
    before_corruption = backend.recover()
    backend.corrupt_last_commit()
    recovery_start = now_ns()
    recovered = mark_runtime_absent(backend.recover())
    # Persist the recovery disposition before migration so a later restart does
    # not accidentally reassert a stale live claim.
    backend.commit(recovered)
    recovery_ms = (now_ns() - recovery_start) / 1e6
    migration_ms = backend.migrate()
    validation = validate_recovered(recovered, model)
    post_migration = mark_runtime_absent(backend.recover())
    downgrade_refused = False
    downgrade = clone(post_migration)
    downgrade["schema_version"] = 1
    try:
        backend.commit(downgrade)
    except RuntimeError as error:
        downgrade_refused = "schema downgrade refused" in str(error)
    # A second client observes only a committed state; a failed/torn tail must
    # never expose a partial workspace graph.
    concurrent_observation = observe_concurrent(backend, post_migration)
    result = {
        "model": model,
        "scale_workspaces": scale,
        "commits": commits,
        "commit_ms_p50": percentile(commit_ms, 0.50),
        "commit_ms_p95": percentile(commit_ms, 0.95),
        "commit_samples": commit_ms,
        "recovery_ms": recovery_ms,
        "migration_ms": migration_ms,
        "post_migration_schema_version": post_migration["schema_version"],
        "post_migration_recovery": validate_recovered(post_migration, model),
        "disk_bytes": backend.size_bytes(),
        "raw_secret_artifacts_after_redaction": backend.raw_secret_bytes(),
        "before_corruption_workspace_count": len(before_corruption["workspaces"]),
        "recovery": validation,
        "concurrent_committed_observation": concurrent_observation,
        "migration_downgrade_refused": downgrade_refused,
    }
    backend.close()
    return result


def run_hybrid_faults(scale: int, root: Path) -> dict[str, Any]:
    """Exercise object/manifest ordering and explicit unavailable segments."""

    root.mkdir(parents=True, exist_ok=True)

    def fresh(name: str) -> tuple[HybridBackend, dict[str, Any]]:
        work_root = root / name
        work_root.mkdir()
        backend = HybridBackend(work_root, "hybrid")
        state = base_state(max(2, scale))
        append_unredacted_fixture(state)
        backend.commit(state)
        return backend, state

    phases = {}
    for phase in ("before_object", "after_object_before_manifest", "after_manifest"):
        backend, state = fresh("fault-" + phase)
        baseline = clone(backend.recover())
        candidate = clone(state)
        mutate_state(candidate, max(2, scale))
        try:
            backend.commit(candidate, failpoint=phase)
            raised = False
        except RuntimeError:
            raised = True
        observed = backend.recover()
        phases[phase] = {
            "raised": raised,
            "manifest_visible": checksum(canonical_state(observed)) == checksum(canonical_state(candidate)),
            "old_manifest_preserved": checksum(canonical_state(observed)) == checksum(canonical_state(baseline)),
            "orphan_object_count": len(list(backend.segment_dir.glob("segment-v1-*.json"))) - len(
                {row[0] for row in backend.db.execute("SELECT storage_id FROM history_refs WHERE storage_id IS NOT NULL")}
                )
            if phase == "after_object_before_manifest"
            else 0,
        }
        backend.close()

    backend, state = fresh("validation")
    before_objects = {path.name: path.read_bytes() for path in backend.segment_dir.glob("segment-v1-*.json")}
    before_refs = {row[0] + ".json" for row in backend.db.execute("SELECT storage_id FROM history_refs WHERE storage_id IS NOT NULL")}
    candidate = clone(state)
    mutate_state(candidate, max(2, scale))
    backend.commit(candidate)
    after_objects = {path.name: path.read_bytes() for path in backend.segment_dir.glob("segment-v1-*.json")}
    after_refs = {row[0] + ".json" for row in backend.db.execute("SELECT storage_id FROM history_refs WHERE storage_id IS NOT NULL")}
    retained_old = before_refs & after_refs
    immutable_preserved = all(after_objects[name] == before_objects[name] for name in retained_old)
    refs = list(backend.db.execute("SELECT storage_id FROM history_refs WHERE storage_id IS NOT NULL ORDER BY execution_id,ordinal"))
    missing_path = backend.segment_dir / (refs[0][0] + ".json")
    missing_path.unlink()
    missing_state = backend.recover()
    missing_unavailable = validate_recovered(missing_state, "hybrid")["unavailable_history_segments"]

    corrupt_backend, corrupt_state = fresh("corrupt")
    refs = list(corrupt_backend.db.execute("SELECT storage_id FROM history_refs WHERE storage_id IS NOT NULL ORDER BY execution_id,ordinal"))
    corrupt_path = corrupt_backend.segment_dir / (refs[0][0] + ".json")
    corrupt_path.write_bytes(b"not-a-valid-segment")
    corrupt_state_observed = corrupt_backend.recover()
    corrupt_unavailable = validate_recovered(corrupt_state_observed, "hybrid")["unavailable_history_segments"]
    backend.close()
    corrupt_backend.close()
    return {
        "fault_phases": phases,
        "immutable_existing_objects_not_overwritten": immutable_preserved,
        "retained_old_object_count": len(retained_old),
        "missing_segment_explicit_unavailable_count": missing_unavailable,
        "corrupt_segment_explicit_unavailable_count": corrupt_unavailable,
        "missing_or_corrupt_never_silently_disappear": missing_unavailable > 0 and corrupt_unavailable > 0,
    }


def run_model(model: str, scale: int, commits: int, repetitions: int, root: Path) -> dict[str, Any]:
    runs = [_run_model_once(model, scale, commits, root / f"{model}-run-{index:03d}") for index in range(repetitions)]
    commit_samples = [sample for run in runs for sample in run["commit_samples"]]
    recovery_samples = [run["recovery_ms"] for run in runs]
    migration_samples = [run["migration_ms"] for run in runs]
    return {
        "model": model,
        "scale_workspaces": scale,
        "commits_per_repetition": commits,
        "repetitions": repetitions,
        "commit_sample_count": len(commit_samples),
        "percentile_method": "nearest-rank",
        "commit_ms_p50": percentile(commit_samples, 0.50),
        "commit_ms_p95": percentile(commit_samples, 0.95),
        "recovery_ms_p50": percentile(recovery_samples, 0.50),
        "recovery_ms_p95": percentile(recovery_samples, 0.95),
        "migration_ms_p50": percentile(migration_samples, 0.50),
        "migration_ms_p95": percentile(migration_samples, 0.95),
        "disk_bytes_p50": percentile([run["disk_bytes"] for run in runs], 0.50),
        "raw_secret_artifacts_max": max(run["raw_secret_artifacts_after_redaction"] for run in runs),
        "all_migrations_refused_downgrade": all(run["migration_downgrade_refused"] for run in runs),
        "all_recoveries_non_resurrecting": all(run["recovery"]["non_resurrection_ok"] for run in runs),
        "all_recoveries_logically_redacted": all(run["recovery"]["redaction_and_truncation_ok"] for run in runs),
        "all_independent_observers_safe": all(
            run["concurrent_committed_observation"]["independent_reader_connection"]
            and run["concurrent_committed_observation"]["all_observations_were_committed_states"]
            and not run["concurrent_committed_observation"]["failures"]
            for run in runs
        ),
        "total_independent_observations": sum(run["concurrent_committed_observation"]["observations"] for run in runs),
        "fault_injection": run_hybrid_faults(scale, root / f"{model}-faults") if model == "hybrid" else None,
    }
    return result


def run_hot_path_overlap(model: str, scale: int, root: Path) -> dict[str, Any]:
    """Synthetic no-I/O hot loop with/without a background persistence writer."""

    model_root = root / (model + "-hot")
    model_root.mkdir()
    backend = backend_for(model_root, model)
    state = base_state(scale)
    stop = threading.Event()
    writes = 0

    def writer() -> None:
        nonlocal writes
        while not stop.is_set():
            backend.commit(state)
            writes += 1

    def hot_tick() -> int:
        # Deliberately does not call persistence; this represents a bounded
        # terminal mutation counter, not a real PTY/VT latency claim.
        return 1 + 1

    cold = []
    for _ in range(1000):
        start = now_ns()
        hot_tick()
        cold.append((now_ns() - start) / 1e6)
    thread = threading.Thread(target=writer)
    thread.start()
    busy = []
    for _ in range(1000):
        start = now_ns()
        hot_tick()
        busy.append((now_ns() - start) / 1e6)
    stop.set()
    thread.join()
    return {
        "model": model,
        "hot_path_p95_without_writer_ms": percentile(cold, 0.95),
        "hot_path_p95_with_writer_ms": percentile(busy, 0.95),
        "background_commits": writes,
        "interpretation": "synthetic scheduler/I-O overlap only; not PTY/VT product latency",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", nargs="+", choices=MODEL_NAMES, default=list(MODEL_NAMES))
    parser.add_argument("--scale", type=int, default=100)
    parser.add_argument("--commits", type=int, default=3)
    parser.add_argument("--repetitions", type=int, default=20)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.scale < 1 or args.commits < 1 or args.repetitions < 1:
        parser.error("--scale, --commits and --repetitions must be positive")
    with tempfile.TemporaryDirectory(prefix="seyal-issue-687-") as temporary:
        root = Path(temporary)
        rows = [run_model(model, args.scale, args.commits, args.repetitions, root) for model in args.models]
        hot = [run_hot_path_overlap(model, min(args.scale, 10), root) for model in args.models]
    output = {
        "experiment": "issue-687-durable-persistence-comparison",
        "research_only": True,
        "models": rows,
        "hot_path_overlap": hot,
        "environment": {
            "python": os.sys.version.split()[0],
            "sqlite": sqlite3.sqlite_version,
            "platform": os.uname().sysname + " " + os.uname().release,
        },
    }
    encoded = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    print(encoded, end="")


if __name__ == "__main__":
    main()
