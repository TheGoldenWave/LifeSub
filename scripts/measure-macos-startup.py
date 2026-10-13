#!/usr/bin/env python3
"""Measure time from launching a packaged LifeSub binary to its first visible window."""

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_APP = ROOT / "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LifeSub.app"
COUNTER_SOURCE = ROOT / "scripts/measure-window-count.swift"
POLL_SECONDS = 0.05
STOP_TIMEOUT_SECONDS = 5


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def window_count(counter, pid):
    result = subprocess.run(
        [str(counter), str(pid)], capture_output=True, text=True, timeout=2, check=True
    )
    return int(result.stdout.strip())


def measure_once(executable, counter, timeout_seconds):
    with tempfile.TemporaryDirectory(prefix="lifesub-startup-") as isolated_home:
        home = Path(isolated_home)
        (home / "Library/Application Support").mkdir(parents=True)
        environment = os.environ.copy()
        environment["HOME"] = isolated_home
        with (home / "launch.log").open("wb") as log:
            started = time.perf_counter()
            process = subprocess.Popen(
                [str(executable)], env=environment, stdout=log, stderr=subprocess.STDOUT
            )
            visible_window_ms = None
            try:
                while time.perf_counter() - started < timeout_seconds:
                    if process.poll() is not None:
                        break
                    if window_count(counter, process.pid) > 0:
                        visible_window_ms = round((time.perf_counter() - started) * 1000, 3)
                        break
                    time.sleep(POLL_SECONDS)
                return {
                    "visible_window_ms": visible_window_ms,
                    "data_directory_created": (
                        home / "Library/Application Support/com.goldenwave.lifesub"
                    ).is_dir(),
                    "exited_before_window": process.poll() is not None,
                    "complete": visible_window_ms is not None,
                }
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=STOP_TIMEOUT_SECONDS)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=STOP_TIMEOUT_SECONDS)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout-seconds", type=float, default=10)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "darwin" or args.runs < 1 or args.timeout_seconds <= 0:
        parser.error("requires macOS, positive runs, and positive timeout")

    executable = args.app.resolve() / "Contents/MacOS/lifesub"
    if not executable.is_file():
        parser.error(f"missing packaged executable: {executable}")

    with tempfile.TemporaryDirectory(prefix="lifesub-window-counter-") as counter_dir:
        counter = Path(counter_dir) / "window-count"
        subprocess.run(["swiftc", str(COUNTER_SOURCE), "-o", str(counter)], check=True)
        runs = [measure_once(executable, counter, args.timeout_seconds) for _ in range(args.runs)]

    report = {
        "schema": "lifesub.startup-window-baseline.v1",
        "app_executable_sha256": sha256_file(executable),
        "host": {"os": platform.mac_ver()[0], "arch": platform.machine()},
        "metric": "process launch to first on-screen layer-0 window; not UI interactivity",
        "poll_interval_ms": int(POLL_SECONDS * 1000),
        "runs": runs,
        "complete": all(run["complete"] and run["data_directory_created"] for run in runs),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"startup window baseline: {len(runs)} runs, complete={report['complete']}")
    for index, run in enumerate(runs, 1):
        print(f"  run {index}: {run['visible_window_ms']} ms")
    return 0 if report["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
