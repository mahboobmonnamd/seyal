#!/usr/bin/env python3
from pathlib import Path

path = Path("crates/seyal-runtime/src/runtime.rs")
text = path.read_text()

old = '''                ReadOutcome::Bytes(0) | ReadOutcome::WouldBlock => {\n                    drain_complete = matches!(\n                        self.entries.get(&id).map(|entry| entry.lifecycle),\n                        Some(Lifecycle::DrainingAfterPrimaryExit { .. })\n                    );\n                    break;\n                }\n'''
new = '''                ReadOutcome::Bytes(0) | ReadOutcome::WouldBlock => {\n                    // A temporary empty nonblocking PTY read is not end-of-file.\n                    // After the primary process exits, descendants or the PTY\n                    // discipline may still make final tail bytes readable during\n                    // the bounded final-drain window. Only EOF may finalize early;\n                    // otherwise keep the execution until the drain deadline.\n                    break;\n                }\n'''
if text.count(old) != 1:
    raise SystemExit(f"service_reads anchor count={text.count(old)}")
text = text.replace(old, new, 1)

old = '''                Some(Lifecycle::DrainingAfterPrimaryExit { exit, .. }) => {\n                    let _ = exit;\n                    self.finalize(id)?;\n                }\n'''
new = '''                Some(Lifecycle::DrainingAfterPrimaryExit { exit, .. }) => {\n                    let _ = exit;\n                    // Close the final-drain race: readiness and the deadline may\n                    // become observable in the same scheduling turn. Give the PTY\n                    // one last bounded production read before publishing the final\n                    // display and completing execution metadata. EOF may finalize\n                    // inside service_reads; otherwise the deadline remains the hard\n                    // upper bound and finalize retires the execution below.\n                    self.service_reads(id)?;\n                    if self.entries.contains_key(&id) {\n                        self.finalize(id)?;\n                    }\n                }\n'''
if text.count(old) != 1:
    raise SystemExit(f"deadline anchor count={text.count(old)}")
path.write_text(text.replace(old, new, 1))
