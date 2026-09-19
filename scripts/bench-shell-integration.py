#!/usr/bin/env python3
"""Tier 1 shell-integration hook-cost harness (ADR-009 mechanism 8).

Launches the same zsh binary with the same isolated HOME/.zshrc under each
terminal's *own* zsh integration script inside one headless PTY, and measures

  (a) startup_us          fork -> first prompt-ready signal
  (b) prompt_to_prompt_us Enter on `true` -> next prompt-ready signal

as nearest-rank p50/p95/p99 over N iterations x R runs. Integration scripts:

  off      no integration (own baseline)
  seyal    crates/seyal-runtime/assets/shell-integration/zsh/.zshenv, nonce
           over an inherited pipe exactly as Runtime does it
  ghostty  fetched at benchmark time from ghostty-org/ghostty at a pinned
           revision (GPLv3; never vendored) - also covers libghostty (cmux)
  warp     zsh_body.sh extracted from the installed Warp.app binary, loaded
           the way Warp does it (ZLE off, bootstrapped after the user's rc)

A variant that cannot be obtained or does not bootstrap on this host is
recorded as ENVIRONMENT_UNSUPPORTED; that is not a pass.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import pty
import re
import select
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEYAL_ZSHENV = ROOT / "crates/seyal-runtime/assets/shell-integration/zsh/.zshenv"
GHOSTTY_RAW = "https://raw.githubusercontent.com/ghostty-org/ghostty/{rev}/src/shell-integration/zsh/{name}"
GHOSTTY_API = "https://api.github.com/repos/ghostty-org/ghostty/commits/main"
NONCE = "ab" * 16
USER_RC = "PROMPT='PS%% '\nalias ll='ls -l'\n"

# Prompt-ready signals, one per variant, matched on raw PTY output.
READY = {
    "off": re.compile(rb"PS% "),
    "seyal": re.compile(rb"\x1b\]133;A;" + NONCE.encode() + rb"\x07"),
    "ghostty": re.compile(rb"\x1b\]133;[AP]"),  # A or P prompt marker
    # Warp hex-encodes its DCS JSON: this is `{"hook": "Precmd` in hex.
    "warp": re.compile(rb"\x1bP\$d7b22686f6f6b223a2022507265636d64"),
}


def percentile(samples: list[float], p: float) -> float:
    ordered = sorted(samples)
    rank = max(1, int(round(p / 100.0 * len(ordered) + 0.5)))
    return ordered[min(rank, len(ordered)) - 1]


class Variant:
    def __init__(self, name: str, workdir: Path):
        self.name = name
        self.workdir = workdir
        self.env: dict[str, str] = {}
        self.pass_fds: tuple[int, ...] = ()
        self.reason: str | None = None

    def prepare(self, args) -> None:
        home = self.workdir / "home"
        home.mkdir(parents=True, exist_ok=True)
        (home / ".zshrc").write_text(USER_RC)
        self.env = {
            "HOME": str(home),
            "PATH": "/usr/bin:/bin",
            "TERM": "xterm-256color",
            "LANG": "C",
            "HISTFILE": str(home / ".zsh_history"),
        }
        zdotdir = self.workdir / "zdotdir"
        zdotdir.mkdir(parents=True, exist_ok=True)
        os.chmod(zdotdir, 0o700)
        if self.name == "off":
            return
        if self.name == "seyal":
            shutil.copy(SEYAL_ZSHENV, zdotdir / ".zshenv")
            os.chmod(zdotdir / ".zshenv", 0o600)
            self.env["ZDOTDIR"] = str(zdotdir)
            return
        if self.name == "ghostty":
            rev = args.ghostty_rev
            if rev == "main":
                with urllib.request.urlopen(GHOSTTY_API, timeout=20) as response:
                    rev = json.load(response)["sha"]
            for name in (".zshenv", "ghostty-integration"):
                with urllib.request.urlopen(GHOSTTY_RAW.format(rev=rev, name=name), timeout=20) as response:
                    (zdotdir / name).write_bytes(response.read())
            self.env["ZDOTDIR"] = str(zdotdir)
            self.env["GHOSTTY_SHELL_FEATURES"] = ""
            self.env["TERM_PROGRAM"] = "ghostty"
            self.reason = f"ghostty-rev={rev}"
            return
        if self.name == "warp":
            binary = Path(args.warp_app) / "Contents/MacOS/stable"
            if not binary.exists():
                raise RuntimeError(f"Warp binary not found at {binary}")
            data = binary.read_bytes()
            start_marker = b"bundled/bootstrap/zsh_body.sh"
            end_marker = b"bundled/bootstrap/zsh_init_shell.sh"
            start = data.find(start_marker)
            end = data.find(end_marker, start) if start >= 0 else -1
            if start < 0 or end < 0:
                raise RuntimeError("could not locate zsh_body.sh inside the Warp binary")
            body = data[start + len(start_marker) : end]
            # The resource table has no separators: drop the bytes preceding
            # the script's first line and anything after its final `fi`.
            first = body.find(b"# Note that WARP_SESSION_ID")
            last = body.rfind(b"\nfi\n")
            if first < 0 or last < 0:
                raise RuntimeError("Warp zsh_body.sh boundaries not recognized")
            body = body[first : last + len(b"\nfi\n")]
            (zdotdir / "warp_zsh_body.sh").write_bytes(body)
            version = subprocess.run(
                ["defaults", "read", str(Path(args.warp_app) / "Contents/Info.plist"), "CFBundleShortVersionString"],
                capture_output=True, text=True, check=False,
            ).stdout.strip()
            # Warp types `unsetopt ZLE`, sets its session variables, then
            # evaluates the body after the user's rc files. Mirror that order
            # with a deferred precmd, the same way Seyal and Ghostty defer.
            (zdotdir / ".zshenv").write_text(
                "builtin unset ZDOTDIR\n"
                "[[ -r \"$HOME/.zshenv\" ]] && builtin source \"$HOME/.zshenv\"\n"
                "if [[ -o interactive ]]; then\n"
                "  unsetopt ZLE\n"
                "  _bench_warp_init() {\n"
                "    precmd_functions=(${precmd_functions:#_bench_warp_init})\n"
                f"    builtin source '{zdotdir / 'warp_zsh_body.sh'}'\n"
                "  }\n"
                "  typeset -ga precmd_functions\n"
                "  precmd_functions+=(_bench_warp_init)\n"
                "fi\n"
            )
            self.env.update({
                "ZDOTDIR": str(zdotdir),
                "WARP_SESSION_ID": "1",
                "WARP_IS_LOCAL_SHELL_SESSION": "1",
                "WARP_HONOR_PS1": "0",
                "TERM_PROGRAM": "WarpTerminal",
            })
            self.reason = f"warp-version={version or 'unknown'}"
            return
        raise RuntimeError(f"unknown variant {self.name}")

    def spawn(self):
        env = dict(self.env)
        pass_fds: list[int] = []
        if self.name == "seyal":
            read_end, write_end = os.pipe()
            os.write(write_end, (NONCE + "\n").encode())
            os.close(write_end)
            os.set_inheritable(read_end, True)
            env["SEYAL_NONCE_FD"] = str(read_end)
            pass_fds.append(read_end)
        t0 = time.perf_counter_ns()
        pid, fd = pty.fork()
        if pid == 0:
            os.environ.clear()
            os.environ.update(env)
            os.execvp("/bin/zsh", ["/bin/zsh", "-i"])
        for extra in pass_fds:
            os.close(extra)
        return pid, fd, t0


def read_until(fd: int, pattern: re.Pattern, deadline_s: float) -> tuple[bool, bytes]:
    buffer = b""
    deadline = time.perf_counter() + deadline_s
    while time.perf_counter() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if not ready:
            continue
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            return False, buffer
        if not chunk:
            return False, buffer
        buffer += chunk
        if pattern.search(buffer):
            return True, buffer
    return False, buffer


def measure(variant: Variant, iterations: int) -> tuple[float, list[float]] | None:
    ready = READY[variant.name]
    pid, fd, t0 = variant.spawn()
    try:
        ok, _ = read_until(fd, ready, 15.0)
        if not ok:
            return None
        startup_us = (time.perf_counter_ns() - t0) / 1000.0
        # Let late startup output (mode switches, second prompt draws) settle.
        read_until(fd, re.compile(rb"(?!x)x"), 0.2)
        samples: list[float] = []
        for _ in range(iterations):
            t = time.perf_counter_ns()
            os.write(fd, b"true\r")
            ok, _ = read_until(fd, ready, 5.0)
            if not ok:
                return None
            samples.append((time.perf_counter_ns() - t) / 1000.0)
        return startup_us, samples
    finally:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except OSError:
            pass
        os.close(fd)


def host_description() -> str:
    chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
    return f"{chip or platform.machine()} macOS {platform.mac_ver()[0]} zsh={zsh_version()}"


def zsh_version() -> str:
    return subprocess.run(["/bin/zsh", "--version"], capture_output=True, text=True).stdout.strip().split()[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--variants", default="off,seyal,ghostty,warp")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--iterations", type=int, default=200)
    parser.add_argument("--ghostty-rev", default="main")
    parser.add_argument("--warp-app", default="/Applications/Warp.app")
    parser.add_argument("--noise-pct", type=float, default=2.0)
    parser.add_argument("--noise-us", type=float, default=100.0)
    parser.add_argument("--out", default="")
    args = parser.parse_args()

    commit = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short=12", "HEAD"], capture_output=True, text=True).stdout.strip()
    host = host_description()
    lines: list[str] = []

    def emit(line: str) -> None:
        print(line)
        lines.append(line)

    emit(f"[seyal shell-integration benchmark] tier=1 commit={commit} host=\"{host}\" runs={args.runs} iterations={args.iterations} percentile_method=nearest-rank noise_band=max({args.noise_pct}%,{args.noise_us}us)")
    results: dict[str, dict[str, float] | None] = {}
    with tempfile.TemporaryDirectory(prefix="seyal-si-bench-") as tmp:
        for name in args.variants.split(","):
            variant = Variant(name, Path(tmp) / name)
            try:
                variant.prepare(args)
            except Exception as error:  # noqa: BLE001 - reported, not hidden
                emit(f"[seyal shell-integration benchmark] variant={name} status=ENVIRONMENT_UNSUPPORTED reason=\"{error}\"")
                results[name] = None
                continue
            startups: list[float] = []
            p2p: list[float] = []
            failed = False
            for _ in range(args.runs):
                outcome = measure(variant, args.iterations)
                if outcome is None:
                    failed = True
                    break
                startup_us, samples = outcome
                startups.append(startup_us)
                p2p.extend(samples)
            if failed:
                emit(f"[seyal shell-integration benchmark] variant={name} status=ENVIRONMENT_UNSUPPORTED reason=\"prompt-ready signal not observed\" {variant.reason or ''}")
                results[name] = None
                continue
            summary = {
                "startup_p50": percentile(startups, 50), "startup_p95": percentile(startups, 95), "startup_p99": percentile(startups, 99),
                "p2p_p50": percentile(p2p, 50), "p2p_p95": percentile(p2p, 95), "p2p_p99": percentile(p2p, 99),
                "p2p_mean": statistics.fmean(p2p),
            }
            results[name] = summary
            emit(
                f"[seyal shell-integration benchmark] variant={name} status=measured "
                f"startup_us p50={summary['startup_p50']:.0f} p95={summary['startup_p95']:.0f} p99={summary['startup_p99']:.0f} samples={len(startups)} "
                f"prompt_to_prompt_us p50={summary['p2p_p50']:.0f} p95={summary['p2p_p95']:.0f} p99={summary['p2p_p99']:.0f} mean={summary['p2p_mean']:.0f} samples={len(p2p)} "
                f"{variant.reason or ''}".rstrip()
            )

    seyal = results.get("seyal")
    if seyal:
        for name in ("ghostty", "warp"):
            other = results.get(name)
            if not other:
                emit(f"[seyal shell-integration benchmark] verdict vs {name}: ENVIRONMENT_UNSUPPORTED (not a pass)")
                continue
            verdicts = []
            for metric in ("startup", "p2p"):
                for pct in ("p50", "p95"):
                    mine = seyal[f"{metric}_{pct}"]
                    theirs = other[f"{metric}_{pct}"]
                    band = max(theirs * args.noise_pct / 100.0, args.noise_us)
                    ok = mine <= theirs + band
                    verdicts.append(f"{metric}_{pct}={'ok' if ok else 'WORSE'}({mine:.0f}<={theirs:.0f}+{band:.0f})")
            overall = "PASS" if all("WORSE" not in v for v in verdicts) else "FAIL"
            emit(f"[seyal shell-integration benchmark] verdict vs {name}: {overall} " + " ".join(verdicts))
        off = results.get("off")
        if off:
            delta = seyal["p2p_p50"] - off["p2p_p50"]
            emit(f"[seyal shell-integration benchmark] own_baseline prompt_to_prompt_p50 seyal-off={delta:+.0f}us startup_p50 seyal-off={seyal['startup_p50'] - off['startup_p50']:+.0f}us")

    if args.out:
        Path(args.out).write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
