#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
COLLECTOR="$PROJECT_ROOT/scripts/collect-native-capture-report.py"
TEMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEMP_ROOT"' EXIT

APP="$TEMP_ROOT/LifeSub.app"
mkdir -p "$APP/Contents/MacOS"
printf 'app-binary' > "$APP/Contents/MacOS/lifesub"
printf 'helper-binary' > "$APP/Contents/MacOS/lifesub-capture-helper"

CATALOG="$TEMP_ROOT/lifesub.sqlite3"
sqlite3 "$CATALOG" <<'SQL'
CREATE TABLE capture_sources(id TEXT PRIMARY KEY, session_id TEXT, source TEXT, device_uid TEXT, device_name TEXT, channel_count INTEGER, sample_rate_hz INTEGER, created_at TEXT);
CREATE TABLE chunks(id TEXT PRIMARY KEY, session_id TEXT, source TEXT, path TEXT, sha256 TEXT, byte_length INTEGER, session_offset_ms INTEGER, duration_ms INTEGER, integrity_state TEXT);
CREATE TABLE capture_chunk_timing(chunk_id TEXT PRIMARY KEY, capture_source_id TEXT, source_sequence INTEGER, source_sample_start INTEGER, source_sample_end INTEGER, session_start_ms INTEGER, session_end_ms INTEGER, monotonic_start_ns INTEGER, monotonic_end_ns INTEGER, utc_start_at TEXT, utc_end_at TEXT, discontinuity_count INTEGER, dropped_frame_count INTEGER, created_at TEXT);
CREATE TABLE asr_jobs(id TEXT PRIMARY KEY, session_id TEXT, chunk_id TEXT, state TEXT, created_at TEXT);
CREATE TABLE provider_receipts(id TEXT PRIMARY KEY, job_id TEXT, chunk_id TEXT);
CREATE TABLE revisions(id TEXT PRIMARY KEY, session_id TEXT, number INTEGER, provider TEXT, created_at TEXT);
CREATE TABLE revision_receipts(revision_id TEXT, receipt_id TEXT);
CREATE TABLE segments(id TEXT PRIMARY KEY, revision_id TEXT, start_ms INTEGER, end_ms INTEGER, source TEXT, text TEXT);
INSERT INTO capture_sources VALUES('src-mic','rec-1','microphone','built-in-uid','MacBook Pro Microphone',1,16000,'2026-08-27T00:00:00Z');
INSERT INTO chunks VALUES('chk-1','rec-1','microphone','audio/chk-1.wav','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',3244,0,100,'available');
INSERT INTO chunks VALUES('chk-1b','rec-1','microphone','audio/chk-1b.wav','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',1916844,100,59900,'available');
INSERT INTO capture_chunk_timing VALUES('chk-1','src-mic',0,0,1600,0,100,1,100000001,'2026-08-27T00:00:00Z','2026-08-27T00:00:00.1Z',0,0,'2026-08-27T00:00:00.1Z');
INSERT INTO capture_chunk_timing VALUES('chk-1b','src-mic',1,1600,960000,100,60000,100000001,60000000001,'2026-08-27T00:00:00.1Z','2026-08-27T00:01:00Z',0,0,'2026-08-27T00:01:00Z');
INSERT INTO asr_jobs VALUES('asr-1','rec-1','chk-1','succeeded','2026-08-27T00:01:00Z');
INSERT INTO provider_receipts VALUES('receipt-1','asr-1','chk-1');
INSERT INTO revisions VALUES('tr-1','rec-1',1,'sense_voice','2026-08-27T00:01:01Z');
INSERT INTO revision_receipts VALUES('tr-1','receipt-1');
INSERT INTO segments VALUES('seg-1','tr-1',0,1000,'microphone','collector fixture');
SQL

OBSERVATION="$TEMP_ROOT/observation.json"
cat > "$OBSERVATION" <<'JSON'
{
  "schema": "lifesub.native-capture-observation.v1",
  "scenario": "mic-only",
  "permissions": {"microphone": "granted", "screen_recording": "not_requested"},
  "devices": [{"source": "microphone", "device_id": "built-in-uid", "device_name": "MacBook Pro Microphone"}],
  "microphone_probe": {"uid": "built-in-uid", "name": "MacBook Pro Microphone", "transport": "built_in", "result": "compatible", "input_sample_rate_hz": 44100, "input_channel_count": 1, "read_back_uid": "built-in-uid", "first_frame_sample_count": 512, "elapsed_ms": 12},
  "microphone_binding": {"enumerated": [{"uid": "built-in-uid", "name": "MacBook Pro Microphone", "is_connected": true, "transport": "built_in"}], "ui_selected_uid": "built-in-uid", "ui_selected_name": "MacBook Pro Microphone", "persisted_before_restart": {"uid": "built-in-uid", "name": "MacBook Pro Microphone"}, "persisted_after_restart": {"uid": "built-in-uid", "name": "MacBook Pro Microphone"}, "start_command_uid": "built-in-uid", "helper_resolved_uid": "built-in-uid", "helper_resolved_name": "MacBook Pro Microphone", "audio_unit_readback_uid": "built-in-uid", "source_started_uid": "built-in-uid", "source_started_name": "MacBook Pro Microphone"},
  "peak_rss_mib": 512,
  "checks": {"search": true, "playback": true, "export": true, "restart_recovery": true},
  "evidence": {}
}
JSON

OUTPUT="$TEMP_ROOT/reports"
python3 "$COLLECTOR" \
  --scenario mic-only \
  --session-id rec-1 \
  --app "$APP" \
  --catalog "$CATALOG" \
  --observation "$OBSERVATION" \
  --output-dir "$OUTPUT"

python3 - "$OUTPUT/mic-only.json" "$APP" <<'PY'
import hashlib, json, pathlib, sys
report = json.load(open(sys.argv[1]))
app = pathlib.Path(sys.argv[2])
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert report["schema"] == "lifesub.native-capture-acceptance.v3"
assert report["app_sha256"] == digest(app / "Contents/MacOS/lifesub")
assert report["helper_sha256"] == digest(app / "Contents/MacOS/lifesub-capture-helper")
assert report["sources"][0]["capture_source_id"] == "src-mic"
assert report["chunks"][0]["chunk_id"] == "chk-1"
assert report["asr"] == {"job_id": "asr-1", "receipt_id": "receipt-1", "revision_id": "tr-1"}
assert report["microphone_binding"]["catalog_uid"] == "built-in-uid"
assert report["microphone_binding"]["catalog_name"] == "MacBook Pro Microphone"
assert report["metrics"]["duration_seconds"] == 60
PY
"$PROJECT_ROOT/scripts/verify-native-capture.sh" --verify-report-file "$OUTPUT/mic-only.json"

sqlite3 "$CATALOG" <<'SQL'
INSERT INTO capture_sources VALUES('src-system','rec-1','system_audio','display-1','Display 1',1,16000,'2026-08-27T00:00:00Z');
INSERT INTO chunks VALUES('chk-2','rec-1','system_audio','audio/chk-2.wav','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',1916844,0,59900,'available');
INSERT INTO capture_chunk_timing VALUES('chk-2','src-system',0,0,958400,100,59900,100000001,59900000001,'2026-08-27T00:00:00.1Z','2026-08-27T00:00:59.9Z',0,0,'2026-08-27T00:01:00Z');
SQL

python3 - "$OBSERVATION" "$TEMP_ROOT/dual-source.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
data["scenario"] = "dual-source"
data["devices"].append({"source": "system_audio", "device_id": "display-1", "device_name": "Display 1"})
json.dump(data, open(sys.argv[2], "w"))
PY
python3 "$COLLECTOR" --scenario dual-source --session-id rec-1 --app "$APP" --catalog "$CATALOG" --observation "$TEMP_ROOT/dual-source.json" --output-dir "$OUTPUT"
"$PROJECT_ROOT/scripts/verify-native-capture.sh" --verify-report-file "$OUTPUT/dual-source.json"
python3 - "$OUTPUT/dual-source.json" <<'PY'
import json, sys
report = json.load(open(sys.argv[1]))
assert abs(report["metrics"]["duration_seconds"] - 59.9) < 0.001
assert abs(report["metrics"]["drift_p95_ms"] - 100) < 0.001, report["metrics"]
PY

python3 - "$OBSERVATION" "$TEMP_ROOT/forged.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
data["sources"] = []
json.dump(data, open(sys.argv[2], "w"))
PY
if python3 "$COLLECTOR" --scenario mic-only --session-id rec-1 --app "$APP" --catalog "$CATALOG" --observation "$TEMP_ROOT/forged.json" --output-dir "$OUTPUT" >/dev/null 2>&1; then
  echo "collector must reject observation overrides of Catalog facts" >&2
  exit 1
fi

python3 - "$OBSERVATION" "$TEMP_ROOT/system-only-forged.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
data["scenario"] = "system-only"
json.dump(data, open(sys.argv[2], "w"))
PY
if python3 "$COLLECTOR" --scenario system-only --session-id rec-1 --app "$APP" --catalog "$CATALOG" --observation "$TEMP_ROOT/system-only-forged.json" --output-dir "$OUTPUT" >/dev/null 2>&1; then
  echo "collector must reject microphone facts without a microphone source" >&2
  exit 1
fi

echo "collect-native-capture-report tests passed"
