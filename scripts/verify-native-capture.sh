#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --verify-report-dir) mode="directory" ;;
  --verify-report-file) mode="file" ;;
  *)
    echo "Usage: $0 --verify-report-dir <directory> | --verify-report-file <report.json>" >&2
    exit 64
    ;;
esac
[[ -n "${2:-}" ]] || {
  echo "missing native capture report path" >&2
  exit 64
}

python3 - "$mode" "$2" <<'PY'
import json, os, re, sys

mode, target = sys.argv[1:]
all_scenarios = ["mic-only", "system-only", "dual-source", "permission-denied",
 "permission-allow-revoke", "settings-permission-roundtrip", "device-disconnect-reconnect",
 "helper-crash-recovery", "restart-recovery", "sleep-wake", "search-playback-export",
 "dual-source-30m", "microphone-built-in-selection", "microphone-external-selection",
 "microphone-prestart-disconnect", "microphone-active-disconnect",
 "microphone-restart-persistence", "microphone-no-device-ui", "system-only-no-microphone"]
all_scenarios.append("microphone-same-uid-recovery")
if mode == "file":
    require_path = os.path.abspath(target)
    root = os.path.dirname(require_path)
    scenario = os.path.basename(require_path).removesuffix(".json")
    if scenario not in all_scenarios:
        raise SystemExit(f"unknown report scenario: {scenario}")
    scenarios = [scenario]
else:
    root = target
    scenarios = all_scenarios
sha = re.compile(r"^[0-9a-f]{64}$")
commit_re = re.compile(r"^[0-9a-f]{40}$")
canonical_thresholds = {"drift_p95_ms_max": 250, "dropped_frames_max": 0,
                        "peak_rss_mib_max": 2048, "chunk_duration_seconds_max": 600}

def require(value, message):
    if not value: raise SystemExit(message)

def validate_probe(probe, expected_uid, expected_name, expected_result, scenario):
    require(isinstance(probe, dict), f"{scenario}: missing microphone probe")
    require(expected_uid and expected_name, f"{scenario}: empty probe identity")
    require(probe.get("uid") == expected_uid and probe.get("name") == expected_name, f"{scenario}: probe identity")
    require(probe.get("transport") in {"built_in", "usb", "bluetooth", "aggregate", "virtual", "unknown"}, f"{scenario}: probe transport")
    require(probe.get("result") == expected_result, f"{scenario}: probe result")
    require(isinstance(probe.get("elapsed_ms"), int) and probe["elapsed_ms"] >= 0, f"{scenario}: probe elapsed")
    if expected_result == "compatible":
        require(isinstance(probe.get("input_sample_rate_hz"), int) and probe["input_sample_rate_hz"] > 0, f"{scenario}: probe sample rate")
        require(isinstance(probe.get("input_channel_count"), int) and probe["input_channel_count"] > 0, f"{scenario}: probe channels")
        require(probe.get("read_back_uid") == expected_uid, f"{scenario}: probe readback UID")
        require(isinstance(probe.get("first_frame_sample_count"), int) and probe["first_frame_sample_count"] > 0, f"{scenario}: probe first frame")
    else:
        require(probe.get("first_frame_sample_count") == 0, f"{scenario}: failed probe claimed frames")

candidate = None
for scenario in scenarios:
    path = os.path.join(root, scenario + ".json")
    require(os.path.isfile(path), f"missing report: {scenario}")
    report = json.load(open(path))
    require(report.get("schema") == "lifesub.native-capture-acceptance.v3", f"{scenario}: schema")
    require(report.get("scenario") == scenario and report.get("passed") is True, f"{scenario}: result")
    require(report.get("environment", {}).get("os") == "macos", f"{scenario}: target OS")
    require(report["environment"].get("os_major", 0) >= 14, f"{scenario}: macOS version")
    require(report["environment"].get("arch") == "arm64", f"{scenario}: target arch")
    require(report.get("thresholds") == canonical_thresholds, f"{scenario}: thresholds drift")
    source = report.get("source") or {}
    identity = (report.get("app_sha256"), report.get("helper_sha256"), source.get("worktree"),
                source.get("commit"), source.get("dirty_digest"))
    require(sha.fullmatch(identity[0] or "") and sha.fullmatch(identity[1] or ""), f"{scenario}: binary hash")
    require(identity[2] and commit_re.fullmatch(identity[3] or "") and sha.fullmatch(identity[4] or ""), f"{scenario}: source identity")
    candidate = identity if candidate is None else candidate
    require(identity == candidate, f"{scenario}: mixed candidate identity")
    permissions = report.get("permissions") or {}
    require(permissions.get("microphone") in {"granted", "denied", "revoked", "not_requested"}, f"{scenario}: mic permission")
    require(permissions.get("screen_recording") in {"granted", "denied", "revoked", "not_requested"}, f"{scenario}: screen permission")
    evidence = report.get("evidence") or {}

    sources = report.get("sources") or []
    chunks = report.get("chunks") or []
    if scenario == "permission-denied":
        require(evidence.get("start_result") == "rejected", "permission-denied: start was not rejected")
        require("denied" in permissions.values(), "permission-denied: no denial")
        require(not sources and not chunks and report.get("asr") is None, "permission-denied: evidence was published")
        binding = report.get("microphone_binding") or {}
        selected_uid = binding.get("ui_selected_uid")
        selected_name = binding.get("ui_selected_name")
        persisted = {"uid": selected_uid, "name": selected_name}
        require(selected_uid and selected_name, "permission-denied: selected microphone identity")
        require(binding.get("persisted_before_restart") == persisted and binding.get("persisted_after_restart") == persisted, "permission-denied: persisted microphone identity")
        require(binding.get("start_command_uid") == selected_uid, "permission-denied: start microphone identity")
        validate_probe(report.get("microphone_probe"), selected_uid, selected_name, "permission_denied", scenario)
        continue

    if scenario == "microphone-prestart-disconnect":
        binding = report.get("microphone_binding") or {}
        enumerated = binding.get("enumerated") or []
        selected_uid = binding.get("ui_selected_uid")
        selected_name = binding.get("ui_selected_name")
        persisted_before = binding.get("persisted_before_restart") or {}
        persisted_after = binding.get("persisted_after_restart") or {}
        require(evidence.get("start_result") == "rejected", f"{scenario}: start result")
        require(evidence.get("error_code") == "selected_microphone_unavailable", f"{scenario}: stable error")
        require(all(evidence.get(key) == 0 for key in ("microphone_source_rows", "microphone_chunk_rows", "publication_rows")), f"{scenario}: failure published evidence")
        require(not sources and not chunks and report.get("asr") is None, f"{scenario}: failure rows")
        require(selected_uid and selected_name and binding.get("start_command_uid") == selected_uid, f"{scenario}: requested identity")
        require(persisted_before == {"uid": selected_uid, "name": selected_name} and persisted_after == persisted_before, f"{scenario}: persisted identity")
        require(any(row.get("uid") == selected_uid and row.get("name") == selected_name and row.get("is_connected") is False for row in enumerated), f"{scenario}: disconnect evidence")
        validate_probe(report.get("microphone_probe"), selected_uid, selected_name, "unavailable", scenario)
        continue

    if scenario == "microphone-no-device-ui":
        require(not report.get("devices") and not sources and not chunks and report.get("asr") is None, f"{scenario}: unexpected capture evidence")
        require(report.get("microphone_binding") is None, f"{scenario}: forged microphone binding")
        require(report.get("microphone_probe") is None, f"{scenario}: forged microphone probe")
        require(evidence == {"injected_fixture": True, "ui_state": "empty", "start_blocked": True, "start_command_sent": False}, f"{scenario}: empty UI evidence")
        continue

    expected_sources = {"mic-only": {"microphone"}, "system-only": {"system_audio"},
                        "dual-source": {"microphone", "system_audio"},
                        "dual-source-30m": {"microphone", "system_audio"},
                        "microphone-built-in-selection": {"microphone"},
                        "microphone-external-selection": {"microphone"},
                        "microphone-active-disconnect": {"microphone"},
                        "microphone-restart-persistence": {"microphone"},
                        "microphone-same-uid-recovery": {"microphone"},
                        "system-only-no-microphone": {"system_audio"}}.get(scenario)
    observed_sources = {row.get("source") for row in sources}
    if expected_sources is not None: require(observed_sources == expected_sources, f"{scenario}: wrong source set")
    if scenario in {"system-only", "system-only-no-microphone"}:
        require(report.get("microphone_binding") is None and report.get("microphone_probe") is None, f"{scenario}: unexpected microphone evidence")
    devices = report.get("devices") or []
    require(sources and chunks and devices, f"{scenario}: missing capture evidence")
    device_sources = {}
    device_names = {}
    for device in devices:
        require(device.get("source") in {"microphone", "system_audio"}, f"{scenario}: device source")
        require(device.get("device_id") and device.get("device_name"), f"{scenario}: device identity")
        device_sources[device["device_id"]] = device["source"]
        device_names[device["device_id"]] = device["device_name"]
    source_ids = set()
    source_rows = {}
    for row in sources:
        require(row.get("capture_source_id") and row.get("sample_rate_hz") == 16000, f"{scenario}: source identity")
        require(row["capture_source_id"] not in source_rows, f"{scenario}: duplicate capture_source_id")
        require(row.get("device_id") in device_sources, f"{scenario}: source device reference")
        require(device_sources[row["device_id"]] == row.get("source"), f"{scenario}: source device kind")
        require(row.get("device_name") == device_names[row["device_id"]], f"{scenario}: source device name")
        require(0 <= row.get("sample_start", -1) < row.get("sample_end", -1), f"{scenario}: source sample range")
        require(0 <= row.get("monotonic_start_ns", -1) < row.get("monotonic_end_ns", -1), f"{scenario}: source host range")
        source_ids.add(row["capture_source_id"])
        source_rows[row["capture_source_id"]] = row

    microphone_rows = [row for row in sources if row.get("source") == "microphone"]
    if microphone_rows:
        require(len(microphone_rows) == 1, f"{scenario}: ambiguous microphone source")
        binding = report.get("microphone_binding") or {}
        selected_uid = binding.get("ui_selected_uid")
        selected_name = binding.get("ui_selected_name")
        require(selected_uid and selected_name, f"{scenario}: UI microphone selection")
        enumerated = binding.get("enumerated") or []
        matching_enumerated = [row for row in enumerated if row.get("uid") == selected_uid]
        require(len(matching_enumerated) == 1, f"{scenario}: enumerated microphone identity")
        require(matching_enumerated[0].get("name") == selected_name and matching_enumerated[0].get("is_connected") is True, f"{scenario}: enumerated microphone state")
        require((binding.get("persisted_before_restart") or {}) == {"uid": selected_uid, "name": selected_name}, f"{scenario}: persisted selection before restart")
        require((binding.get("persisted_after_restart") or {}) == {"uid": selected_uid, "name": selected_name}, f"{scenario}: persisted selection after restart")
        require(all(binding.get(key) == selected_uid for key in ("start_command_uid", "helper_resolved_uid", "audio_unit_readback_uid", "source_started_uid", "catalog_uid")), f"{scenario}: microphone UID binding")
        require(all(binding.get(key) == selected_name for key in ("helper_resolved_name", "source_started_name", "catalog_name")), f"{scenario}: microphone name binding")
        validate_probe(report.get("microphone_probe"), selected_uid, selected_name, "compatible", scenario)
        microphone_row = microphone_rows[0]
        require(microphone_row.get("device_id") == selected_uid and microphone_row.get("device_name") == selected_name, f"{scenario}: Catalog microphone source identity")
    sequences = {source_id: [] for source_id in source_ids}
    for chunk in chunks:
        require(chunk.get("chunk_id") and sha.fullmatch(chunk.get("sha256", "")), f"{scenario}: chunk identity")
        require(chunk.get("capture_source_id") in source_ids, f"{scenario}: orphan chunk")
        source_row = source_rows[chunk["capture_source_id"]]
        require(chunk.get("source") == source_row["source"], f"{scenario}: chunk source mismatch")
        require(0 <= chunk.get("sample_start", -1) < chunk.get("sample_end", -1), f"{scenario}: chunk sample range")
        require(source_row["sample_start"] <= chunk["sample_start"] < chunk["sample_end"] <= source_row["sample_end"], f"{scenario}: chunk outside source range")
        require(0 < chunk.get("duration_seconds", 0) <= canonical_thresholds["chunk_duration_seconds_max"], f"{scenario}: chunk rotation")
        expected_samples = chunk["duration_seconds"] * source_row["sample_rate_hz"]
        require(abs((chunk["sample_end"] - chunk["sample_start"]) - expected_samples) <= 1, f"{scenario}: chunk duration/sample mismatch")
        sequences[chunk["capture_source_id"]].append(chunk.get("source_sequence"))
    for source_id, values in sequences.items():
        require(sorted(values) == list(range(len(values))), f"{scenario}: non-contiguous source sequence")
        source_chunks = sorted((row for row in chunks if row["capture_source_id"] == source_id), key=lambda row: row["source_sequence"])
        require(source_chunks[0]["sample_start"] == source_rows[source_id]["sample_start"], f"{scenario}: source head gap")
        require(source_chunks[-1]["sample_end"] == source_rows[source_id]["sample_end"], f"{scenario}: source tail gap")
        require(all(left["sample_end"] == right["sample_start"] for left, right in zip(source_chunks, source_chunks[1:])), f"{scenario}: source sample discontinuity")
    asr = report.get("asr") or {}
    require(all(asr.get(k) for k in ("job_id", "receipt_id", "revision_id")), f"{scenario}: ASR identity")
    metrics = report.get("metrics") or {}
    envelopes = [
        (row["monotonic_start_ns"], row["monotonic_end_ns"])
        for row in source_rows.values()
    ]
    recomputed_drift_ms = 0
    if len(envelopes) > 1:
        baseline = envelopes[0]
        recomputed_drift_ms = max(
            max(abs(baseline[0] - envelope[0]), abs(baseline[1] - envelope[1]))
            for envelope in envelopes[1:]
        ) / 1_000_000
    reported_drift_ms = metrics.get("drift_p95_ms", -1)
    require(0 <= recomputed_drift_ms <= 250, f"{scenario}: drift")
    require(abs(reported_drift_ms - recomputed_drift_ms) <= 1, f"{scenario}: reported drift mismatch")
    require(metrics.get("dropped_frames") == 0, f"{scenario}: dropped frames")
    require(0 < metrics.get("peak_rss_mib", 0) <= 2048, f"{scenario}: memory")
    common_duration = min(
        (row["sample_end"] - row["sample_start"]) / row["sample_rate_hz"]
        for row in source_rows.values()
    )
    require(abs(metrics.get("duration_seconds", 0) - common_duration) <= 1 / 16000, f"{scenario}: common duration mismatch")
    checks = report.get("checks") or {}
    require(all(checks.get(k) is True for k in ("search", "playback", "export", "restart_recovery")), f"{scenario}: user-path check")

    scenario_requirements = {
      "permission-allow-revoke": evidence.get("permission_transitions") == ["denied", "granted", "revoked"] and evidence.get("after_revoke_start") == "rejected",
      "settings-permission-roundtrip": evidence.get("actions") == ["open_settings", "grant", "return_app"] and evidence.get("final_state") == "granted",
      "device-disconnect-reconnect": evidence.get("interruption_observed") is True and evidence.get("reconnected_device_id") in device_sources,
      "helper-crash-recovery": evidence.get("crash_observed") is True and isinstance(evidence.get("helper_pid_before"), int) and evidence["helper_pid_before"] > 0 and isinstance(evidence.get("helper_pid_after"), int) and evidence["helper_pid_after"] > 0 and evidence["helper_pid_before"] != evidence["helper_pid_after"] and evidence.get("recovered") is True,
      "restart-recovery": isinstance(evidence.get("app_pid_before"), int) and evidence["app_pid_before"] > 0 and isinstance(evidence.get("app_pid_after"), int) and evidence["app_pid_after"] > 0 and evidence["app_pid_before"] != evidence["app_pid_after"] and evidence.get("recovered") is True,
      "sleep-wake": evidence.get("sleep_observed") is True and evidence.get("wake_observed") is True and evidence.get("recovered") is True,
      "search-playback-export": all(evidence.get(k) is True for k in ("unique_phrase_found", "playback_verified", "export_verified")),
      "microphone-built-in-selection": (report.get("microphone_binding") or {}).get("enumerated", [{}])[0].get("transport") == "built_in",
      "microphone-external-selection": (report.get("microphone_binding") or {}).get("enumerated", [{}])[0].get("transport") in {"usb", "bluetooth"},
      "microphone-active-disconnect": evidence.get("interruption_reason") == "selected_microphone_disconnected" and evidence.get("recoverable") is False and evidence.get("fallback_observed") is False,
      "microphone-restart-persistence": evidence.get("app_restarted") is True and evidence.get("selection_restored") is True,
      "system-only-no-microphone": evidence.get("enumerated_microphone_count") == 0 and evidence.get("start_result") == "started" and not any(row.get("source") == "microphone" for row in devices),
    }
    if scenario in scenario_requirements: require(scenario_requirements[scenario], f"{scenario}: transition evidence")
    if scenario == "microphone-same-uid-recovery":
        recovery = report.get("recovery") or {}
        binding = report.get("microphone_binding") or {}
        selected_uid = binding.get("ui_selected_uid")
        require(recovery.get("reason") == "device_configuration_changed", f"{scenario}: recovery reason")
        require(isinstance(recovery.get("generation"), int) and recovery["generation"] > 0, f"{scenario}: recovery generation")
        require(recovery.get("recovering_uid") == selected_uid and recovery.get("recovered_uid") == selected_uid, f"{scenario}: recovered UID")
        require(isinstance(recovery.get("first_host_time"), int) and recovery["first_host_time"] > 0, f"{scenario}: recovery first frame")
        require(microphone_rows[0]["monotonic_start_ns"] <= recovery["first_host_time"] <= microphone_rows[0]["monotonic_end_ns"], f"{scenario}: recovery first frame range")
        require(recovery.get("fallback_observed") is False, f"{scenario}: fallback observed")
        chunk_by_id = {row.get("chunk_id"): row for row in chunks}
        before = chunk_by_id.get(recovery.get("pre_recovery_chunk_id"))
        after = chunk_by_id.get(recovery.get("post_recovery_chunk_id"))
        require(before and after, f"{scenario}: recovery chunk binding")
        require(before.get("capture_source_id") == after.get("capture_source_id") == microphone_rows[0]["capture_source_id"], f"{scenario}: recovery source identity")
        require(after.get("source_sequence") == before.get("source_sequence") + 1 and before.get("sample_end") == after.get("sample_start"), f"{scenario}: recovery continuity")
        post_asr = recovery.get("post_recovery_asr") or {}
        require(post_asr.get("chunk_id") == after.get("chunk_id") and all(post_asr.get(key) for key in ("job_id", "receipt_id", "revision_id")), f"{scenario}: post-recovery ASR")
    if scenario == "dual-source-30m":
        require(metrics.get("duration_seconds", 0) >= 1800, "dual-source-30m: duration")
        require(all(row["sample_end"] - row["sample_start"] >= 28800000 for row in source_rows.values()), "dual-source-30m: sample span")
        require(all(len(values) >= 3 for values in sequences.values()), "dual-source-30m: insufficient rotation")

print(f"native capture reports verified: {len(scenarios)} scenarios")
PY
