#!/usr/bin/env python3
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ALLOWED_TYPES = {"component", "concept", "decision", "module", "milestone", "spec", "test"}


def git_blob_sha(path: Path) -> str:
    data = path.read_bytes()
    header = f"blob {len(data)}\0".encode()
    return hashlib.sha1(header + data).hexdigest()


def load_graph(graph_path: Path):
    with graph_path.open("r", encoding="utf-8") as f:
        return json.load(f)


def validate(graph, root: Path):
    errors = []
    if graph.get("authority") != "derived-navigation-only":
        errors.append("graph authority must be derived-navigation-only")

    nodes = graph.get("nodes")
    if not isinstance(nodes, list):
        return ["nodes must be a list"]

    ids = []
    for node in nodes:
        node_id = node.get("id")
        if not isinstance(node_id, str) or not node_id:
            errors.append("every node must have a non-empty string id")
            continue
        ids.append(node_id)
        if node.get("type") not in ALLOWED_TYPES:
            errors.append(f"{node_id}: invalid type {node.get('type')!r}")
        if not node.get("summary"):
            errors.append(f"{node_id}: summary is required")
        sources = node.get("sources") or []
        if not sources:
            errors.append(f"{node_id}: at least one authoritative source is required")
        for source in sources:
            rel = source.get("path")
            expected = source.get("fingerprint")
            if not rel or not expected:
                errors.append(f"{node_id}: source requires path and fingerprint")
                continue
            path = root / rel
            if not path.is_file():
                errors.append(f"{node_id}: missing source {rel}")
                continue
            actual = git_blob_sha(path)
            if actual != expected:
                errors.append(
                    f"{node_id}: stale source {rel}: graph={expected} current={actual}"
                )

    duplicates = sorted({x for x in ids if ids.count(x) > 1})
    for node_id in duplicates:
        errors.append(f"duplicate node id: {node_id}")

    known = set(ids)
    for node in nodes:
        node_id = node.get("id", "<unknown>")
        for relation in node.get("relationships") or []:
            target = relation.get("target")
            if not relation.get("type") or not target:
                errors.append(f"{node_id}: relationship requires type and target")
            elif target not in known:
                errors.append(f"{node_id}: dangling relationship target {target}")
    return errors


def searchable_text(node):
    parts = [node.get("id", ""), node.get("type", ""), node.get("title", ""), node.get("summary", "")]
    parts.extend(node.get("tags") or [])
    return " ".join(parts).lower()


def print_node(node):
    print(f"{node['id']} [{node['type']}] — {node.get('title', '')}")
    print(f"  {node.get('summary', '')}")
    for rel in node.get("relationships") or []:
        print(f"  -> {rel['type']}: {rel['target']}")
    for source in node.get("sources") or []:
        print(f"  source: {source['path']}")


def cmd_validate(args):
    graph = load_graph(args.graph)
    errors = validate(graph, args.root)
    if errors:
        for error in errors:
            print(f"[project-context] ERROR: {error}", file=sys.stderr)
        return 1
    print(f"[project-context] valid: {len(graph['nodes'])} nodes")
    return 0


def cmd_list(args):
    graph = load_graph(args.graph)
    for node in graph.get("nodes", []):
        print(f"{node['id']}\t{node['type']}\t{node.get('title', '')}")
    return 0


def cmd_query(args):
    graph = load_graph(args.graph)
    terms = [t.lower() for t in args.query if t.strip()]
    if not terms:
        print("query requires at least one term", file=sys.stderr)
        return 64
    scored = []
    for node in graph.get("nodes", []):
        text = searchable_text(node)
        score = sum(text.count(term) for term in terms)
        if score:
            scored.append((score, node['id'], node))
    scored.sort(key=lambda x: (-x[0], x[1]))
    if not scored:
        print("[project-context] no matching nodes; search authoritative repository sources directly")
        return 2
    for _, _, node in scored[: args.limit]:
        print_node(node)
    return 0


def cmd_fingerprint(args):
    for rel in args.paths:
        path = args.root / rel
        if not path.is_file():
            print(f"missing: {rel}", file=sys.stderr)
            return 1
        print(f"{git_blob_sha(path)}  {rel}")
    return 0


def parse_args():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="Query and validate Seyal's derived project context graph")
    parser.add_argument("--root", type=Path, default=root)
    parser.add_argument("--graph", type=Path, default=root / ".context" / "graph.json")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("validate")
    p.set_defaults(func=cmd_validate)

    p = sub.add_parser("list")
    p.set_defaults(func=cmd_list)

    p = sub.add_parser("query")
    p.add_argument("query", nargs="+")
    p.add_argument("--limit", type=int, default=8)
    p.set_defaults(func=cmd_query)

    p = sub.add_parser("fingerprint")
    p.add_argument("paths", nargs="+")
    p.set_defaults(func=cmd_fingerprint)
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_args()
    sys.exit(args.func(args))
