#!/usr/bin/env python3
from __future__ import annotations

import os
import re
from pathlib import Path

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()

HOT_FUNCTIONS = {
    "crates/seyal-terminal/src/terminal.rs": ["feed", "finish_input"],
    "crates/seyal-runtime/src/runtime.rs": ["poll_once", "drain_control", "service_reads", "service_writes"],
    "crates/seyal-runtime/src/input.rs": ["try_submit"],
}

FORBIDDEN = {
    "blocking lock": ("Mutex<", "RwLock<", ".lock()", ".read()", ".write()"),
    "thread/process hop": ("thread::spawn", "std::thread", "process::Command", "Command::new("),
    "blocking sleep": ("thread::sleep", "std::thread::sleep"),
    "serialization": ("serde_json", "json!(", "bincode", "postcard"),
    "network/filesystem I/O": ("TcpStream", "UnixStream", "std::fs", "File::open", "File::create"),
    "avoidable allocation": ("Vec::new()", "vec![", ".to_vec()", ".to_owned()", "String::new()", "String::from(", "format!("),
    "unbounded channel": ("mpsc::channel(", "channel::<"),
}


def extract_function(source: str, name: str) -> str | None:
    match = re.search(rf"\bfn\s+{re.escape(name)}\s*\([^)]*\)[^{{]*\{{", source, re.S)
    if not match:
        return None
    start = match.start()
    brace = source.find("{", match.start())
    depth = 0
    for index in range(brace, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    return None


def main() -> None:
    errors: list[str] = []
    for relpath, functions in HOT_FUNCTIONS.items():
        path = ROOT / relpath
        if not path.exists():
            errors.append(f"missing guarded hot-path file: {relpath}")
            continue
        source = path.read_text(encoding="utf-8")
        for function in functions:
            body = extract_function(source, function)
            if body is None:
                errors.append(f"missing guarded hot-path function: {relpath}::{function}")
                continue
            for category, patterns in FORBIDDEN.items():
                for pattern in patterns:
                    if pattern in body:
                        errors.append(
                            f"{relpath}::{function} contains forbidden {category} primitive {pattern!r}"
                        )

    if errors:
        print("Hot-path performance guardrail violations:")
        for error in errors:
            print(f"  {error}")
        raise SystemExit(1)

    print("Hot-path performance guardrails passed.")


if __name__ == "__main__":
    main()
