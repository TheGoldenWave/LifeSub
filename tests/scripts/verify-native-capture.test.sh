#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HARNESS="$PROJECT_ROOT/scripts/verify-native-capture.sh"
TEMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEMP_ROOT"' EXIT

write_reports() {
  local report_dir="$1"
  mkdir -p "$report_dir"
  python3 - "$report_dir" <<'PY'
import json, os, sys

root = sys.argv[1]
scenarios = [
    "mic-only", "system-only", "dual-source", "permission-denied",
    "permission-allow-revoke", "settings-permission-roundtrip",
    "device-disconnect-reconnect", "helper-crash-recovery", "restart-recovery",
    "sleep-wake", "search-playback-export", "dual-source-30m",
    "microphone-built-in-selection", "microphone-external-selection",
    "microphone-prestart-disconnect", "microphone-active-disconnect",
    "microphone-restart-persistence", "microphone-no-device-ui",
    "system-only-no-microphone", "microphone-same-uid-recovery",
]
for scenario in scenarios:
    source_kind = "system_audio" if scenario in {"system-only", "system-only-no-microphone"} else "microphone"
    microphone_uid = "external-mic-uid" if scenario == "microphone-external-selection" else "built-in-mic-uid"
    microphone_name = "Bluetooth Headset Microphone" if scenario == "microphone-external-selection" else "MacBook Pro Microphone"
    device_id = "system-device-uid" if source_kind == "system_audio" else microphone_uid
    device_name = "Screen Audio" if source_kind == "system_audio" else microphone_name
    report = {
        "schema": "lifesub.native-capture-acceptance.v3",
        "scenario": scenario,
        "passed": True,
        "app_sha256": "a" * 64,
        "helper_sha256": "b" * 64,
        "source": {"worktree": "/repo", "commit": "c" * 40, "dirty_digest": "d" * 64},
        "environment": {"os": "macos", "os_major": 15, "arch": "arm64"},
        "thresholds": {"drift_p95_ms_max": 250, "dropped_frames_max": 0, "peak_rss_mib_max": 2048, "chunk_duration_seconds_max": 600},
        "permissions": {"microphone": "granted", "screen_recording": "granted"},
        "devices": [{"source": source_kind, "device_id": device_id, "device_name": device_name}],
        "sources": [{"capture_source_id": "src-1", "device_id": device_id, "device_name": device_name, "source": source_kind, "sample_rate_hz": 16000, "sample_start": 0, "sample_end": 960000, "monotonic_start_ns": 1, "monotonic_end_ns": 60000000001}],
        "chunks": [{"chunk_id": "chk-1", "sha256": "e" * 64, "source": source_kind, "capture_source_id": "src-1", "source_sequence": 0, "sample_start": 0, "sample_end": 960000, "duration_seconds": 60}],
        "asr": {"job_id": "asr-1", "receipt_id": "receipt-1", "revision_id": "tr-1"},
        "metrics": {"drift_p95_ms": 0, "dropped_frames": 0, "peak_rss_mib": 650, "duration_seconds": 60},
        "checks": {"search": True, "playback": True, "export": True, "restart_recovery": True},
        "evidence": {},
    }
    if scenario == "permission-denied":
        report["permissions"] = {"microphone": "denied", "screen_recording": "denied"}
        report["sources"] = []
        report["chunks"] = []
        report["asr"] = None
        report["evidence"] = {"start_result": "rejected"}
        report["microphone_probe"] = {
            "uid": microphone_uid, "name": microphone_name, "transport": "built_in", "result": "permission_denied",
            "input_sample_rate_hz": None, "input_channel_count": None,
            "read_back_uid": None, "first_frame_sample_count": 0, "elapsed_ms": 4,
        }
        report["microphone_binding"] = {
            "ui_selected_uid": microphone_uid,
            "ui_selected_name": microphone_name,
            "persisted_before_restart": {"uid": microphone_uid, "name": microphone_name},
            "persisted_after_restart": {"uid": microphone_uid, "name": microphone_name},
            "start_command_uid": microphone_uid,
        }
    if scenario in {"dual-source", "dual-source-30m"}:
        report["devices"].append({"source": "system_audio", "device_id": "system-device-uid", "device_name": "Screen Audio"})
        report["sources"].append({"capture_source_id": "src-2", "device_id": "system-device-uid", "device_name": "Screen Audio", "source": "system_audio", "sample_rate_hz": 16000, "sample_start": 0, "sample_end": 960000, "monotonic_start_ns": 1, "monotonic_end_ns": 60000000001})
        report["chunks"].append({"chunk_id": "chk-2", "sha256": "f" * 64, "source": "system_audio", "capture_source_id": "src-2", "source_sequence": 0, "sample_start": 0, "sample_end": 960000, "duration_seconds": 60})
    if scenario == "dual-source-30m":
        report["metrics"]["duration_seconds"] = 1800
        for source in report["sources"]:
            source["sample_end"] = 28800000
            source["monotonic_end_ns"] = 1800000000001
        report["chunks"] = []
        for source_id, source_name, digest in [("src-1", "microphone", "e"), ("src-2", "system_audio", "f")]:
            for sequence in range(3):
                report["chunks"].append({"chunk_id": f"{source_id}-chk-{sequence}", "sha256": digest * 64, "source": source_name, "capture_source_id": source_id, "source_sequence": sequence, "sample_start": sequence * 9600000, "sample_end": (sequence + 1) * 9600000, "duration_seconds": 600})
    scenario_evidence = {
        "permission-allow-revoke": {"permission_transitions": ["denied", "granted", "revoked"], "after_revoke_start": "rejected"},
        "settings-permission-roundtrip": {"actions": ["open_settings", "grant", "return_app"], "final_state": "granted"},
        "device-disconnect-reconnect": {"interruption_observed": True, "reconnected_device_id": microphone_uid},
        "helper-crash-recovery": {"crash_observed": True, "helper_pid_before": 100, "helper_pid_after": 101, "recovered": True},
        "restart-recovery": {"app_pid_before": 200, "app_pid_after": 201, "recovered": True},
        "sleep-wake": {"sleep_observed": True, "wake_observed": True, "recovered": True},
        "search-playback-export": {"unique_phrase_found": True, "playback_verified": True, "export_verified": True},
    }
    report["evidence"].update(scenario_evidence.get(scenario, {}))
    if any(source["source"] == "microphone" for source in report["sources"]):
        report["microphone_probe"] = {
            "uid": microphone_uid, "name": microphone_name, "transport": "bluetooth" if scenario == "microphone-external-selection" else "built_in", "result": "compatible",
            "input_sample_rate_hz": 44100, "input_channel_count": 1,
            "read_back_uid": microphone_uid, "first_frame_sample_count": 512, "elapsed_ms": 12,
        }
        report["microphone_binding"] = {
            "enumerated": [{
                "uid": microphone_uid,
                "name": microphone_name,
                "is_connected": True,
                "transport": "bluetooth" if scenario == "microphone-external-selection" else "built_in",
            }],
            "ui_selected_uid": microphone_uid,
            "ui_selected_name": microphone_name,
            "persisted_before_restart": {"uid": microphone_uid, "name": microphone_name},
            "persisted_after_restart": {"uid": microphone_uid, "name": microphone_name},
            "start_command_uid": microphone_uid,
            "helper_resolved_uid": microphone_uid,
            "helper_resolved_name": microphone_name,
            "audio_unit_readback_uid": microphone_uid,
            "source_started_uid": microphone_uid,
            "source_started_name": microphone_name,
            "catalog_uid": microphone_uid,
            "catalog_name": microphone_name,
        }
    if scenario == "microphone-prestart-disconnect":
        report["sources"] = []
        report["chunks"] = []
        report["asr"] = None
        report["devices"] = []
        report["evidence"] = {
            "start_result": "rejected",
            "error_code": "selected_microphone_unavailable",
            "microphone_source_rows": 0,
            "microphone_chunk_rows": 0,
            "publication_rows": 0,
        }
        report["microphone_binding"] = {
            "enumerated": [{"uid": microphone_uid, "name": microphone_name, "is_connected": False, "transport": "built_in"}],
            "ui_selected_uid": microphone_uid,
            "ui_selected_name": microphone_name,
            "persisted_before_restart": {"uid": microphone_uid, "name": microphone_name},
            "persisted_after_restart": {"uid": microphone_uid, "name": microphone_name},
            "start_command_uid": microphone_uid,
        }
        report["microphone_probe"] = {
            "uid": microphone_uid, "name": microphone_name, "transport": "built_in", "result": "unavailable",
            "input_sample_rate_hz": None, "input_channel_count": None,
            "read_back_uid": None, "first_frame_sample_count": 0, "elapsed_ms": 8,
        }
    if scenario == "microphone-active-disconnect":
        report["evidence"].update({
            "interruption_reason": "selected_microphone_disconnected",
            "recoverable": False,
            "fallback_observed": False,
        })
    if scenario == "microphone-restart-persistence":
        report["evidence"].update({"app_restarted": True, "selection_restored": True})
    if scenario == "microphone-no-device-ui":
        report["devices"] = []
        report["sources"] = []
        report["chunks"] = []
        report["asr"] = None
        report["microphone_binding"] = None
        report["microphone_probe"] = None
        report["evidence"] = {
            "injected_fixture": True,
            "ui_state": "empty",
            "start_blocked": True,
            "start_command_sent": False,
        }
    if scenario == "system-only-no-microphone":
        report["evidence"].update({"enumerated_microphone_count": 0, "start_result": "started"})
    if scenario == "microphone-same-uid-recovery":
        report["chunks"] = [
            {"chunk_id": "chk-before", "sha256": "e" * 64, "source": "microphone", "capture_source_id": "src-1", "source_sequence": 0, "sample_start": 0, "sample_end": 480000, "duration_seconds": 30},
            {"chunk_id": "chk-after", "sha256": "f" * 64, "source": "microphone", "capture_source_id": "src-1", "source_sequence": 1, "sample_start": 480000, "sample_end": 960000, "duration_seconds": 30},
        ]
        report["recovery"] = {
            "reason": "device_configuration_changed",
            "generation": 1,
            "recovering_uid": microphone_uid,
            "recovered_uid": microphone_uid,
            "first_host_time": 30000000001,
            "pre_recovery_chunk_id": "chk-before",
            "post_recovery_chunk_id": "chk-after",
            "fallback_observed": False,
            "post_recovery_asr": {"job_id": "asr-after", "receipt_id": "receipt-after", "revision_id": "tr-after", "chunk_id": "chk-after"},
        }
    with open(os.path.join(root, f"{scenario}.json"), "w") as handle:
        json.dump(report, handle)
PY
}

VALID="$TEMP_ROOT/valid"
write_reports "$VALID"
"$HARNESS" --verify-report-dir "$VALID"

MISSING_ID="$TEMP_ROOT/missing-id"
cp -R "$VALID" "$MISSING_ID"
python3 - "$MISSING_ID/mic-only.json" <<'PY'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
del data["sources"][0]["capture_source_id"]
json.dump(data, open(path, "w"))
PY
if "$HARNESS" --verify-report-dir "$MISSING_ID" >/dev/null 2>&1; then
  echo "missing capture_source_id must fail" >&2
  exit 1
fi

HIGH_DRIFT="$TEMP_ROOT/high-drift"
cp -R "$VALID" "$HIGH_DRIFT"
python3 - "$HIGH_DRIFT/dual-source-30m.json" <<'PY'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
data["metrics"]["drift_p95_ms"] = 251
json.dump(data, open(path, "w"))
PY
if "$HARNESS" --verify-report-dir "$HIGH_DRIFT" >/dev/null 2>&1; then
  echo "drift above threshold must fail" >&2
  exit 1
fi

mutate_must_fail() {
  local name="$1"
  local file="$2"
  local expression="$3"
  local directory="$TEMP_ROOT/$name"
  cp -R "$VALID" "$directory"
  python3 - "$directory/$file" "$expression" <<'PY'
import json, sys
path, expression = sys.argv[1:]
data = json.load(open(path))
exec(expression, {}, {"data": data})
json.dump(data, open(path, "w"))
PY
  if "$HARNESS" --verify-report-dir "$directory" >/dev/null 2>&1; then
    echo "$name mutation must fail" >&2
    exit 1
  fi
}

mutate_must_fail denied-published permission-denied.json 'data["chunks"] = [{"chunk_id":"forged","sha256":"e"*64}]'
mutate_must_fail wrong-system-source system-only.json 'data["sources"][0]["source"] = "microphone"'
mutate_must_fail mixed-candidate sleep-wake.json 'data["app_sha256"] = "9"*64'
mutate_must_fail single-long-chunk dual-source-30m.json 'data["chunks"] = data["chunks"][:1]; data["chunks"][0]["duration_seconds"] = 1800'
mutate_must_fail wrong-arch mic-only.json 'data["environment"]["arch"] = "x86_64"'
mutate_must_fail empty-device mic-only.json 'data["devices"][0]["device_id"] = ""'
mutate_must_fail source-device-kind mic-only.json 'data["devices"][0]["source"] = "system_audio"'
mutate_must_fail chunk-source-mismatch mic-only.json 'data["chunks"][0]["source"] = "system_audio"'
mutate_must_fail chunk-outside-source mic-only.json 'data["chunks"][0]["sample_end"] = data["sources"][0]["sample_end"] + 1'
mutate_must_fail missing-helper-pid helper-crash-recovery.json 'del data["evidence"]["helper_pid_before"]'
mutate_must_fail unknown-reconnect-device device-disconnect-reconnect.json 'data["evidence"]["reconnected_device_id"] = "unknown-device"'
mutate_must_fail fake-long-duration dual-source-30m.json '[(row.update({"sample_end": row["sample_start"] + 3})) for row in data["sources"]]; [(row.update({"sample_start": 0, "sample_end": 1})) for row in data["chunks"]]'
mutate_must_fail duplicate-source-id dual-source-30m.json 'data["sources"][1]["capture_source_id"] = data["sources"][0]["capture_source_id"]; data["chunks"] = [row for row in data["chunks"] if row["source"] == "system_audio"]; [(row.update({"capture_source_id": data["sources"][0]["capture_source_id"]})) for row in data["chunks"]]'
mutate_must_fail selected-uid-fallback microphone-built-in-selection.json 'data["microphone_binding"]["audio_unit_readback_uid"] = "fallback-default-uid"'
mutate_must_fail helper-name-mismatch microphone-external-selection.json 'data["microphone_binding"]["helper_resolved_name"] = "Other Headset"'
mutate_must_fail source-started-uid-mismatch microphone-built-in-selection.json 'data["microphone_binding"]["source_started_uid"] = "forged-uid"'
mutate_must_fail catalog-name-mismatch microphone-built-in-selection.json 'data["microphone_binding"]["catalog_name"] = "System Default"'
mutate_must_fail persisted-restart-mismatch microphone-restart-persistence.json 'data["microphone_binding"]["persisted_after_restart"]["uid"] = "different-uid"'
mutate_must_fail prestart-published-source microphone-prestart-disconnect.json 'data["evidence"]["microphone_source_rows"] = 1'
mutate_must_fail active-disconnect-fallback microphone-active-disconnect.json 'data["evidence"]["fallback_observed"] = True'
mutate_must_fail no-device-start-sent microphone-no-device-ui.json 'data["evidence"]["start_command_sent"] = True'
mutate_must_fail system-only-microphone-present system-only-no-microphone.json 'data["devices"].append({"source":"microphone","device_id":"unexpected","device_name":"Unexpected"})'
mutate_must_fail missing-probe-uid mic-only.json 'del data["microphone_probe"]["uid"]'
mutate_must_fail missing-denied-probe-identity permission-denied.json 'data["microphone_probe"]["uid"] = None; data["microphone_probe"]["name"] = None'
mutate_must_fail forged-denied-probe-identity permission-denied.json 'data["microphone_probe"]["uid"] = "invented-uid"; data["microphone_probe"]["name"] = "Invented Mic"'
mutate_must_fail forged-system-only-probe system-only.json 'data["microphone_probe"] = {"uid":"invented-uid","name":"Invented Mic","transport":"virtual","result":"compatible","input_sample_rate_hz":48000,"input_channel_count":2,"read_back_uid":"invented-uid","first_frame_sample_count":512,"elapsed_ms":1}; data["microphone_binding"] = {"ui_selected_uid":"invented-uid"}'
mutate_must_fail invalid-probe-transport mic-only.json 'data["microphone_probe"]["transport"] = "airplay"'
mutate_must_fail probe-readback-mismatch microphone-built-in-selection.json 'data["microphone_probe"]["read_back_uid"] = "other-uid"'
mutate_must_fail probe-zero-frame microphone-external-selection.json 'data["microphone_probe"]["first_frame_sample_count"] = 0'
mutate_must_fail recovery-zero-generation microphone-same-uid-recovery.json 'data["recovery"]["generation"] = 0'
mutate_must_fail recovery-uid-mismatch microphone-same-uid-recovery.json 'data["recovery"]["recovered_uid"] = "other-uid"'
mutate_must_fail recovery-missing-post-chunk microphone-same-uid-recovery.json 'data["chunks"] = data["chunks"][:1]'
mutate_must_fail recovery-fallback microphone-same-uid-recovery.json 'data["recovery"]["fallback_observed"] = True'
mutate_must_fail forged-low-drift dual-source.json 'data["sources"][1]["monotonic_start_ns"] += 300000000; data["sources"][1]["monotonic_end_ns"] += 300000000; data["metrics"]["drift_p95_ms"] = 0'

echo "verify-native-capture shell tests passed"
