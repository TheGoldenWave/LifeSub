#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import platform
import sqlite3
import subprocess
import tempfile
from pathlib import Path

SCHEMA = "lifesub.native-capture-acceptance.v3"
OBSERVATION_SCHEMA = "lifesub.native-capture-observation.v1"
SCENARIOS = {
    "mic-only", "system-only", "dual-source", "permission-denied",
    "permission-allow-revoke", "settings-permission-roundtrip",
    "device-disconnect-reconnect", "helper-crash-recovery", "restart-recovery",
    "sleep-wake", "search-playback-export", "dual-source-30m",
    "microphone-built-in-selection", "microphone-external-selection",
    "microphone-prestart-disconnect", "microphone-active-disconnect",
    "microphone-restart-persistence", "microphone-no-device-ui",
    "system-only-no-microphone", "microphone-same-uid-recovery",
}
NO_SESSION_SCENARIOS = {
    "permission-denied", "microphone-prestart-disconnect", "microphone-no-device-ui",
}
EXPECTED_SOURCES = {
    "mic-only": {"microphone"},
    "system-only": {"system_audio"},
    "dual-source": {"microphone", "system_audio"},
    "dual-source-30m": {"microphone", "system_audio"},
    "microphone-built-in-selection": {"microphone"},
    "microphone-external-selection": {"microphone"},
    "microphone-active-disconnect": {"microphone"},
    "microphone-restart-persistence": {"microphone"},
    "microphone-same-uid-recovery": {"microphone"},
    "system-only-no-microphone": {"system_audio"},
}
NEGATIVE_MICROPHONE_SCENARIOS = {"permission-denied", "microphone-prestart-disconnect"}
OBSERVATION_FIELDS = {
    "schema", "scenario", "permissions", "devices", "microphone_probe",
    "microphone_binding", "peak_rss_mib", "checks", "evidence", "recovery",
}
THRESHOLDS = {
    "drift_p95_ms_max": 250,
    "dropped_frames_max": 0,
    "peak_rss_mib_max": 2048,
    "chunk_duration_seconds_max": 600,
}
BUILD_SOURCE_PATHS = (
    "src", "src-tauri", "package.json", "package-lock.json", "vite.config.ts",
    "scripts/build-capture-helper.sh", "scripts/tauri-verified-cargo-runner.sh",
    "scripts/with-sherpa-runtime.sh", "scripts/collect-native-capture-report.py",
    "scripts/verify-native-capture.sh", "docs/design/tokens/base.json",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def command_bytes(project_root, *args):
    return subprocess.check_output(args, cwd=project_root)


def git_identity(project_root):
    worktree = command_bytes(project_root, "git", "rev-parse", "--show-toplevel").decode().strip()
    commit = command_bytes(project_root, "git", "rev-parse", "HEAD").decode().strip()
    digest = hashlib.sha256()
    pathspec = ("--", *BUILD_SOURCE_PATHS)
    digest.update(command_bytes(
        project_root, "git", "status", "--porcelain=v1", "-z", *pathspec
    ))
    digest.update(command_bytes(
        project_root, "git", "diff", "--no-ext-diff", "--binary", "HEAD", *pathspec
    ))
    untracked = command_bytes(
        project_root, "git", "ls-files", "--others", "--exclude-standard", "-z", *pathspec
    ).split(b"\0")
    for raw_path in sorted(path for path in untracked if path):
        digest.update(raw_path)
        path = project_root / os.fsdecode(raw_path)
        if path.is_file() and not path.is_symlink():
            digest.update(sha256_file(path).encode())
    return {"worktree": worktree, "commit": commit, "dirty_digest": digest.hexdigest()}


def executable(app, names):
    for name in names:
        candidate = app / "Contents" / "MacOS" / name
        if candidate.is_file():
            return candidate
    raise ValueError(f"missing bundled executable: {names[0]}")


def load_observation(path, scenario):
    observation = json.loads(path.read_text())
    require(isinstance(observation, dict), "observation must be an object")
    unknown = sorted(set(observation) - OBSERVATION_FIELDS)
    require(not unknown, f"observation cannot override authoritative fields: {unknown}")
    require(observation.get("schema") == OBSERVATION_SCHEMA, "observation schema")
    require(observation.get("scenario") == scenario, "observation scenario")
    require(isinstance(observation.get("permissions"), dict), "observation permissions")
    require(isinstance(observation.get("devices"), list), "observation devices")
    require(isinstance(observation.get("checks"), dict), "observation checks")
    require(isinstance(observation.get("evidence"), dict), "observation evidence")
    peak_rss = observation.get("peak_rss_mib")
    require(isinstance(peak_rss, (int, float)) and peak_rss > 0, "observation peak RSS")
    return observation


def source_and_chunk_facts(connection, session_id):
    source_rows = connection.execute(
        """
        SELECT cs.id, cs.source, cs.device_uid, cs.device_name, cs.sample_rate_hz,
               MIN(t.source_sample_start), MAX(t.source_sample_end),
               MIN(t.monotonic_start_ns), MAX(t.monotonic_end_ns)
        FROM capture_sources cs
        JOIN capture_chunk_timing t ON t.capture_source_id = cs.id
        JOIN chunks c ON c.id = t.chunk_id AND c.session_id = cs.session_id
        WHERE cs.session_id = ? AND c.integrity_state = 'available'
        GROUP BY cs.id, cs.source, cs.device_uid, cs.device_name, cs.sample_rate_hz
        ORDER BY cs.source, cs.id
        """,
        (session_id,),
    ).fetchall()
    require(source_rows, "session has no available native capture sources")
    sources = [
        {
            "capture_source_id": row[0], "source": row[1], "device_id": row[2],
            "device_name": row[3], "sample_rate_hz": row[4], "sample_start": row[5],
            "sample_end": row[6], "monotonic_start_ns": row[7], "monotonic_end_ns": row[8],
        }
        for row in source_rows
    ]
    sample_rates = {source["sample_rate_hz"] for source in sources}
    require(sample_rates == {16000}, "native source sample rate must be 16 kHz")
    spans = [source["sample_end"] - source["sample_start"] for source in sources]
    duration = min(spans) / 16000
    if duration.is_integer():
        duration = int(duration)

    timing_rows = connection.execute(
        """
        SELECT c.id, c.sha256, c.source, t.capture_source_id, t.source_sequence,
               t.source_sample_start, t.source_sample_end, t.session_start_ms,
               t.session_end_ms, t.dropped_frame_count
        FROM chunks c
        JOIN capture_chunk_timing t ON t.chunk_id = c.id
        WHERE c.session_id = ? AND c.integrity_state = 'available'
        ORDER BY t.capture_source_id, t.source_sequence
        """,
        (session_id,),
    ).fetchall()
    chunks = []
    timing_by_source = {}
    dropped_frames = 0
    for row in timing_rows:
        source = next(item for item in sources if item["capture_source_id"] == row[3])
        sample_span = row[6] - row[5]
        chunk_duration = sample_span / source["sample_rate_hz"]
        if chunk_duration.is_integer():
            chunk_duration = int(chunk_duration)
        chunks.append({
            "chunk_id": row[0], "sha256": row[1], "source": row[2],
            "capture_source_id": row[3], "source_sequence": row[4],
            "sample_start": row[5], "sample_end": row[6],
            "duration_seconds": chunk_duration,
        })
        timing_by_source.setdefault(row[3], {})[row[4]] = (row[7], row[8])
        dropped_frames += row[9]
    require(chunks, "session has no available chunks")
    return sources, chunks, timing_by_source, duration, dropped_frames


def drift_p95_ms(timing_by_source):
    if len(timing_by_source) < 2:
        return 0
    envelopes = [
        (min(start for start, _ in timings.values()), max(end for _, end in timings.values()))
        for timings in timing_by_source.values()
    ]
    differences = []
    baseline = envelopes[0]
    for envelope in envelopes[1:]:
        differences.append(abs(baseline[0] - envelope[0]))
        differences.append(abs(baseline[1] - envelope[1]))
    differences.sort()
    rank = max(0, (95 * len(differences) + 99) // 100 - 1)
    return differences[rank]


def asr_fact(connection, session_id, chunk_id=None):
    parameters = [session_id]
    chunk_filter = ""
    if chunk_id is not None:
        chunk_filter = " AND j.chunk_id = ?"
        parameters.append(chunk_id)
    row = connection.execute(
        f"""
        SELECT j.id, pr.id, rr.revision_id, j.chunk_id
        FROM asr_jobs j
        JOIN provider_receipts pr ON pr.job_id = j.id AND pr.chunk_id = j.chunk_id
        JOIN revision_receipts rr ON rr.receipt_id = pr.id
        JOIN revisions r ON r.id = rr.revision_id AND r.session_id = j.session_id
        WHERE j.session_id = ? AND j.state = 'succeeded'{chunk_filter}
          AND EXISTS(SELECT 1 FROM segments s WHERE s.revision_id = r.id AND TRIM(s.text) != '')
        ORDER BY j.created_at DESC, j.id DESC
        LIMIT 1
        """,
        parameters,
    ).fetchone()
    require(row is not None, "session has no succeeded ASR publication")
    return {
        "job_id": row[0], "receipt_id": row[1], "revision_id": row[2], "chunk_id": row[3]
    }


def bind_devices(observation, sources):
    devices = observation["devices"]
    expected = {(source["source"], source["device_id"], source["device_name"]) for source in sources}
    observed = {(row.get("source"), row.get("device_id"), row.get("device_name")) for row in devices}
    require(observed == expected, "observed devices do not exactly match Catalog sources")
    return devices


def bind_negative_microphone(observation):
    binding = observation.get("microphone_binding")
    probe = observation.get("microphone_probe")
    require(isinstance(binding, dict) and isinstance(probe, dict), "negative microphone observation is incomplete")
    uid = binding.get("ui_selected_uid")
    name = binding.get("ui_selected_name")
    expected = {"uid": uid, "name": name}
    require(uid and name, "negative microphone selection identity")
    require(binding.get("persisted_before_restart") == expected, "negative microphone persisted identity before restart")
    require(binding.get("persisted_after_restart") == expected, "negative microphone persisted identity after restart")
    require(binding.get("start_command_uid") == uid, "negative microphone start identity")
    require(probe.get("uid") == uid and probe.get("name") == name, "negative microphone probe identity")
    return binding, probe


def bind_microphone(observation, sources, scenario):
    microphone_sources = [source for source in sources if source["source"] == "microphone"]
    if not microphone_sources:
        if scenario in NEGATIVE_MICROPHONE_SCENARIOS:
            return bind_negative_microphone(observation)
        require(observation.get("microphone_binding") is None, "scenario cannot include microphone binding")
        require(observation.get("microphone_probe") is None, "scenario cannot include microphone probe")
        return None, None
    require(len(microphone_sources) == 1, "ambiguous microphone source")
    binding = dict(observation.get("microphone_binding") or {})
    probe = observation.get("microphone_probe")
    require(binding and isinstance(probe, dict), "microphone observation is incomplete")
    source = microphone_sources[0]
    binding["catalog_uid"] = source["device_id"]
    binding["catalog_name"] = source["device_name"]
    return binding, probe


def collect(args):
    require(args.scenario in SCENARIOS, "unknown native acceptance scenario")
    observation = load_observation(args.observation, args.scenario)
    if args.scenario in NO_SESSION_SCENARIOS:
        require(args.session_id is None, "negative scenario must not bind a session")
        sources, chunks, timing, duration, dropped = [], [], {}, 0, 0
        asr = None
    else:
        require(args.session_id, "scenario requires a Catalog session")
        with sqlite3.connect(args.catalog) as connection:
            sources, chunks, timing, duration, dropped = source_and_chunk_facts(
                connection, args.session_id
            )
            publication = asr_fact(connection, args.session_id)
            asr = {key: publication[key] for key in ("job_id", "receipt_id", "revision_id")}
            recovery = observation.get("recovery")
            if recovery is not None:
                recovery = dict(recovery)
                post_chunk = recovery.get("post_recovery_chunk_id")
                post_asr = asr_fact(connection, args.session_id, post_chunk)
                recovery["post_recovery_asr"] = {
                    "job_id": post_asr["job_id"], "receipt_id": post_asr["receipt_id"],
                    "revision_id": post_asr["revision_id"], "chunk_id": post_asr["chunk_id"],
                }
                observation["recovery"] = recovery

    expected_sources = EXPECTED_SOURCES.get(args.scenario)
    if expected_sources is not None:
        require({source["source"] for source in sources} == expected_sources, "scenario source set")

    devices = bind_devices(observation, sources) if sources else observation["devices"]
    microphone_binding, microphone_probe = bind_microphone(observation, sources, args.scenario)
    app_executable = executable(args.app, ("lifesub", "LifeSub"))
    helper_executable = executable(args.app, ("lifesub-capture-helper", "capture-helper"))
    machine = platform.machine().lower()
    arch = "arm64" if machine in {"arm64", "aarch64"} else machine
    os_version = platform.mac_ver()[0]
    os_major = int(os_version.split(".")[0]) if os_version else 0
    metrics = {
        "drift_p95_ms": drift_p95_ms(timing),
        "dropped_frames": dropped,
        "peak_rss_mib": observation["peak_rss_mib"],
        "duration_seconds": duration,
    }
    report = {
        "schema": SCHEMA,
        "scenario": args.scenario,
        "passed": True,
        "app_sha256": sha256_file(app_executable),
        "helper_sha256": sha256_file(helper_executable),
        "source": git_identity(args.project_root),
        "environment": {"os": "macos", "os_major": os_major, "arch": arch},
        "thresholds": THRESHOLDS,
        "permissions": observation["permissions"],
        "devices": devices,
        "sources": sources,
        "chunks": chunks,
        "asr": asr,
        "metrics": metrics,
        "checks": observation["checks"],
        "evidence": observation["evidence"],
    }
    if microphone_binding is not None:
        report["microphone_binding"] = microphone_binding
    if microphone_probe is not None:
        report["microphone_probe"] = microphone_probe
    if observation.get("recovery") is not None:
        report["recovery"] = observation["recovery"]
    return report


def write_report(report, output_dir):
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / f"{report['scenario']}.json"
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{report['scenario']}.", suffix=".tmp", dir=output_dir
    )
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(report, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, output_path)
        directory = os.open(output_dir, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except Exception:
        try:
            os.close(descriptor)
        except OSError:
            pass
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise
    return output_path


def parse_args():
    project_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description="Collect one LifeSub native capture v3 report")
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--session-id")
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--observation", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--project-root", type=Path, default=project_root)
    return parser.parse_args()


def main():
    args = parse_args()
    try:
        report = collect(args)
        path = write_report(report, args.output_dir)
    except (OSError, sqlite3.Error, subprocess.CalledProcessError, ValueError) as error:
        raise SystemExit(f"native capture report collection failed: {error}") from error
    print(path)


if __name__ == "__main__":
    main()
