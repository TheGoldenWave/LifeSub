use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::asr::manifest::{
    ArtifactInstallMode, ConfigCompatibilityError, DeviceRequirement, DirectInstallConstraints,
    InstallConstraints, QualificationPolicy, RegistryValidationError, RequiredInstallFile,
    RuntimeRequirement, canonical_bundle_payload, model_registry, vad_manifest,
    validate_qwen_config_shape, validate_registry,
};

#[test]
fn runtime_language_catalog_excludes_pseudo_values_and_qwen06_explicit_languages() {
    let registry = model_registry();
    let whisper = registry.model("whisper-tiny").unwrap();
    assert!(!whisper.supported_languages.contains(&"multilingual"));

    let qwen06 = registry.model("qwen3-asr-0.6b-int8-2026-03-25").unwrap();
    assert_eq!(qwen06.supported_languages, &["auto"]);

    let qwen17 = registry.model("qwen3-asr-1.7b").unwrap();
    assert!(qwen17.supported_languages.contains(&"zh"));
    assert!(qwen17.supported_languages.contains(&"en"));
}
use crate::asr::model_lookup::{
    DeviceSupport, InstallationQualification, ModelLookup, ModelLookupContext,
};
use crate::domain::AsrProviderKind;

const QWEN17_ID: &str = "qwen3-asr-1.7b";
const QWEN17_HF_REVISION: &str = "bcd2b5b7f32b480ab5790554cfa8347f246a14f3";
const QWEN17_BUNDLE_SHA256: &str =
    "8a5c16d08be3c49e638689b6438a9a3be9d5d732e49f904d2c0666d5229c995a";
const MAX_REDIRECTS: usize = 10;
const CONNECT_TIMEOUT_SECONDS: u64 = 10;
const PROBE_TIMEOUT_SECONDS: u64 = 30;
const FULL_DOWNLOAD_TIMEOUT_SECONDS: u64 = 6 * 60 * 60;
const PROVIDER_API_BODY_LIMIT: u64 = 4 * 1024 * 1024;
const QWEN_RUNTIME_FOR_TEST: RuntimeRequirement = RuntimeRequirement::QwenCandleMetal {
    crate_name: "qwen3-asr",
    crate_version: "0.2.2",
    git_url: "https://github.com/alan890104/qwen3-asr-rs.git",
    git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc",
    cargo_feature: "metal",
    target_os: "macos",
    target_arch: "aarch64",
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct GoldenArchiveInstallContract {
    max_scanned_entries: u64,
    max_written_file_bytes: u64,
    max_total_written_bytes: u64,
    required_files: BTreeSet<String>,
}

#[test]
fn shipping_registry_has_six_installable_models_and_one_vad() {
    let registry = model_registry();
    assert_eq!(validate_registry(registry, vad_manifest()), Ok(()));
    assert_eq!(registry.models().len(), 6);

    let mut ids = HashSet::new();
    for model in registry.models() {
        assert!(ids.insert(model.id), "duplicate model id: {}", model.id);
        assert!(!model.manifest_version.trim().is_empty());
        assert!(!model.display_name.trim().is_empty());
        assert!(!model.supported_languages.is_empty());
        assert!(!model.source.repository_url.trim().is_empty());
        assert!(!model.source.model_card_url.trim().is_empty());
        assert!(!model.source.license_spdx.trim().is_empty());
        assert!(!model.source.provenance.trim().is_empty());
        assert!(!model.bundle.artifacts.is_empty());
        assert!(!model.bundle.required_paths.is_empty());
        assert_hex_sha256(model.bundle.identity_sha256);

        for artifact in model.bundle.artifacts {
            assert_artifact_contract(artifact);
        }
        for required in model.bundle.required_paths {
            assert_normalized_relative_path(required);
        }

        let expected_policy = if model.id == QWEN17_ID {
            QualificationPolicy::RuntimeSmokeRequired
        } else {
            QualificationPolicy::StructuralWithPinnedRuntime
        };
        assert_eq!(model.qualification_policy, expected_policy);
    }

    let vad = vad_manifest();
    assert_eq!(
        vad.qualification_policy,
        QualificationPolicy::StructuralWithPinnedRuntime
    );
    assert_eq!(vad.bundle.artifacts.len(), 1);
    assert_eq!(vad.bundle.required_paths, &["silero_vad.onnx"]);
    assert_artifact_contract(&vad.bundle.artifacts[0]);
}

#[test]
fn sherpa_manifests_share_the_task1_trusted_native_build_identity() {
    let pinned = crate::asr::pinned_sherpa_runtime_identity();
    assert_eq!(pinned.version, "1.13.5");
    assert_eq!(
        pinned.git_commit,
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
    assert_eq!(
        pinned.native_archive_sha256,
        "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
    );
    assert_eq!(pinned.build_id, "sherpa-onnx-v1.13.5-osx-arm64-static-lib");

    for model in model_registry()
        .models()
        .iter()
        .filter(|model| model.id != QWEN17_ID)
    {
        assert_eq!(
            model.runtime,
            RuntimeRequirement::SherpaOnnx {
                crate_version: pinned.version,
                git_commit: pinned.git_commit,
                native_archive_sha256: pinned.native_archive_sha256,
                build_id: pinned.build_id,
                cargo_feature: "static",
            }
        );
        let payload: serde_json::Value =
            serde_json::from_str(&canonical_bundle_payload(model).unwrap()).unwrap();
        assert_eq!(
            payload["runtime_requirement"],
            serde_json::json!({
                "build_id": pinned.build_id,
                "cargo_feature": "static",
                "crate": "sherpa-onnx",
                "git_commit": pinned.git_commit,
                "native_archive_sha256": pinned.native_archive_sha256,
                "version": pinned.version,
            })
        );
    }
    assert_eq!(
        vad_manifest().runtime,
        RuntimeRequirement::SherpaOnnx {
            crate_version: pinned.version,
            git_commit: pinned.git_commit,
            native_archive_sha256: pinned.native_archive_sha256,
            build_id: pinned.build_id,
            cargo_feature: "static",
        }
    );
}

#[cfg(feature = "asr-runtime")]
#[test]
fn observed_sherpa_identity_carries_trusted_wrapper_build_metadata() {
    let observed = crate::asr::verify_runtime_identity().unwrap();
    let pinned = crate::asr::pinned_sherpa_runtime_identity();
    assert_eq!(observed.version(), pinned.version);
    assert_eq!(observed.observed_git_sha1(), "3dc7c569");
    assert_eq!(observed.pinned_git_sha1(), pinned.git_commit);
    assert_eq!(
        observed.native_archive_sha256(),
        pinned.native_archive_sha256
    );
    assert_eq!(observed.build_id(), pinned.build_id);
    assert!(observed.matches_pinned(pinned));
}

#[cfg(feature = "asr-runtime")]
#[test]
fn runtime_identity_rejects_spoofed_native_build_attestation() {
    let pinned = crate::asr::pinned_sherpa_runtime_identity();
    assert_eq!(
        crate::asr::verify_runtime_identity_values_with_build(
            pinned.version,
            "3dc7c569",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            pinned.build_id,
            "1",
        ),
        Err(
            crate::asr::RuntimeIdentityError::NativeArchiveSha256Mismatch {
                observed: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
                pinned: pinned.native_archive_sha256,
            }
        )
    );
    assert_eq!(
        crate::asr::verify_runtime_identity_values_with_build(
            pinned.version,
            pinned.git_commit,
            pinned.native_archive_sha256,
            "spoofed-build",
            "1",
        ),
        Err(crate::asr::RuntimeIdentityError::BuildIdMismatch {
            observed: "spoofed-build".to_owned(),
            pinned: pinned.build_id,
        })
    );
    assert_eq!(
        crate::asr::verify_runtime_identity_values_with_build(
            pinned.version,
            "3dc7c569",
            pinned.native_archive_sha256,
            pinned.build_id,
            "0",
        ),
        Err(
            crate::asr::RuntimeIdentityError::BuildAttestationUnverified {
                observed: "0".to_owned(),
            }
        )
    );
}

#[test]
fn task1_shell_and_rust_build_attestation_contracts_match() {
    let fetcher = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/fetch-sherpa-runtime.sh"
    ));
    let wrapper = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/with-sherpa-runtime.sh"
    ));
    let build_rs = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
    for contract in [fetcher, wrapper, build_rs] {
        assert!(
            contract.contains("339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44")
        );
        assert!(contract.contains("sherpa-onnx-v1.13.5-osx-arm64-static-lib"));
    }
    assert!(fetcher.contains("schema=lifesub.sherpa-runtime-attestation.v1"));
    assert!(wrapper.contains("LIFESUB_SHERPA_RUNTIME_ATTESTATION_FILE"));
    assert!(build_rs.contains("cargo:rustc-env=LIFESUB_SHERPA_ARCHIVE_SHA256"));
    assert!(build_rs.contains("cargo:rustc-env=LIFESUB_SHERPA_BUILD_ID"));
}

#[test]
fn registry_freezes_downloaded_archive_hashes_and_required_contents() {
    let registry = model_registry();
    let expected = [
        (
            "sense-voice-small-int8-2024-07-17",
            163_002_883,
            "7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e",
            &["model.int8.onnx", "tokens.txt", "test_wavs/zh.wav"][..],
        ),
        (
            "whisper-tiny",
            116_204_861,
            "c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1",
            &[
                "tiny-encoder.onnx",
                "tiny-decoder.onnx",
                "tiny-tokens.txt",
                "test_wavs/0.wav",
                "test_wavs/1.wav",
                "test_wavs/8k.wav",
                "test_wavs/trans.txt",
            ][..],
        ),
        (
            "whisper-base",
            207_557_382,
            "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
            &[
                "base-encoder.onnx",
                "base-decoder.onnx",
                "base-tokens.txt",
                "test_wavs/0.wav",
                "test_wavs/1.wav",
                "test_wavs/8k.wav",
                "test_wavs/trans.txt",
            ][..],
        ),
        (
            "whisper-small",
            639_387_718,
            "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
            &[
                "small-encoder.onnx",
                "small-decoder.onnx",
                "small-tokens.txt",
                "test_wavs/0.wav",
                "test_wavs/1.wav",
                "test_wavs/8k.wav",
                "test_wavs/trans.txt",
            ][..],
        ),
        (
            "qwen3-asr-0.6b-int8-2026-03-25",
            878_702_423,
            "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96",
            &[
                "conv_frontend.onnx",
                "encoder.int8.onnx",
                "decoder.int8.onnx",
                "tokenizer/vocab.json",
                "tokenizer/merges.txt",
                "tokenizer/tokenizer_config.json",
                "test_wavs/codeswitch.wav",
                "test_wavs/transcript.txt",
            ][..],
        ),
    ];

    for (model_id, bytes, sha256, required_paths) in expected {
        let model = registry.model(model_id).unwrap();
        let artifact = &model.bundle.artifacts[0];
        assert_eq!(artifact.bytes, bytes);
        assert_eq!(artifact.sha256, sha256);
        assert_eq!(artifact.install_mode, ArtifactInstallMode::ExtractTarBz2);
        for required_path in required_paths {
            assert!(model.bundle.required_paths.contains(required_path));
        }
    }
}

#[test]
fn install_constraints_freeze_exact_archive_and_direct_whitelists() {
    let expected_archives = [
        (
            "sense-voice-small-int8-2024-07-17",
            12,
            239_233_841,
            240_500_355,
            7,
        ),
        ("whisper-tiny", 11, 114_505_801, 153_794_272, 7),
        ("whisper-base", 11, 196_548_998, 293_277_543, 7),
        ("whisper-small", 11, 559_127_829, 970_298_212, 7),
        (
            "qwen3-asr-0.6b-int8-2026-03-25",
            27,
            755_914_231,
            1_000_089_273,
            22,
        ),
    ];
    let mut archive_file_count = 0;
    for (model_id, entries, max_file, total, file_count) in expected_archives {
        let model = model_registry().model(model_id).unwrap();
        let InstallConstraints::Archive(constraints) = model.bundle.install_constraints else {
            panic!("{model_id} did not expose archive constraints");
        };
        assert_eq!(constraints.max_scanned_entries, entries);
        assert_eq!(constraints.max_written_file_bytes, max_file);
        assert_eq!(constraints.max_total_written_bytes, total);
        assert_eq!(constraints.required_files.len(), file_count);
        archive_file_count += constraints.required_files.len();
        assert_install_inventory_matches_required_paths(
            model.bundle.required_paths,
            constraints.required_files,
        );
    }
    assert_eq!(archive_file_count, 50);

    let qwen06 = model_registry()
        .model("qwen3-asr-0.6b-int8-2026-03-25")
        .unwrap();
    let InstallConstraints::Archive(qwen06) = qwen06.bundle.install_constraints else {
        unreachable!();
    };
    for expected in [
        RequiredInstallFile {
            path: "tokenizer/merges.txt",
            bytes: 1_671_853,
            sha256: "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
        },
        RequiredInstallFile {
            path: "tokenizer/tokenizer_config.json",
            bytes: 12_487,
            sha256: "4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c",
        },
        RequiredInstallFile {
            path: "tokenizer/vocab.json",
            bytes: 2_776_833,
            sha256: "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
        },
    ] {
        assert!(qwen06.required_files.contains(&expected));
    }

    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    let InstallConstraints::Direct(qwen17_constraints) = qwen17.bundle.install_constraints else {
        panic!("Qwen 1.7B did not expose direct constraints");
    };
    assert_eq!(qwen17_constraints.required_files.len(), 5);
    assert_eq!(qwen17_constraints.max_written_file_bytes, 4_220_320_824);
    assert_eq!(qwen17_constraints.max_total_written_bytes, 4_710_022_180);
    assert_install_inventory_matches_required_paths(
        qwen17.bundle.required_paths,
        qwen17_constraints.required_files,
    );

    let InstallConstraints::Direct(vad) = vad_manifest().bundle.install_constraints else {
        panic!("VAD did not expose direct constraints");
    };
    assert_eq!(vad.required_files.len(), 1);
    assert_eq!(vad.max_written_file_bytes, 643_854);
    assert_eq!(vad.max_total_written_bytes, 643_854);
}

#[test]
fn independent_golden_inventory_matches_manifest_bidirectionally() {
    let mut golden = golden_archive_install_contracts();
    let mut manifest_models = BTreeSet::new();
    for model in model_registry().models() {
        let InstallConstraints::Archive(constraints) = model.bundle.install_constraints else {
            continue;
        };
        assert!(manifest_models.insert(model.id));
        let expected = golden
            .remove(model.id)
            .unwrap_or_else(|| panic!("golden fixture omitted archive model {}", model.id));
        assert_eq!(archive_contract_from_manifest(constraints), expected);
    }
    assert_eq!(manifest_models.len(), 5);
    assert!(
        golden.is_empty(),
        "golden fixture has unknown models: {golden:?}"
    );
}

#[test]
fn cached_official_archives_match_manifest_and_golden_when_configured() {
    let Some(cache) = std::env::var_os("LIFESUB_MODEL_ARCHIVE_CACHE") else {
        return;
    };
    verify_cached_archive_contracts(Path::new(&cache)).unwrap();
}

#[test]
fn install_constraints_are_explicitly_excluded_from_qwen17_jcs_v2() {
    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    let canonical = canonical_bundle_payload(qwen17).unwrap();
    assert_eq!(qwen17.manifest_version, "2");
    assert_eq!(qwen17.bundle.identity_sha256, QWEN17_BUNDLE_SHA256);
    assert!(!canonical.contains("install_constraints"));
    assert!(!canonical.contains("max_scanned_entries"));
    assert!(!canonical.contains("max_total_written_bytes"));
    assert_eq!(
        hex::encode(Sha256::digest(canonical.as_bytes())),
        QWEN17_BUNDLE_SHA256
    );
}

#[test]
fn validator_rejects_every_archive_install_constraint_field_drift() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let InstallConstraints::Archive(canonical) = source.bundle.install_constraints else {
        unreachable!();
    };
    let mut mutations = Vec::new();
    let mut value = canonical;
    value.max_scanned_entries -= 1;
    mutations.push(value);
    let mut value = canonical;
    value.max_written_file_bytes += 1;
    mutations.push(value);
    let mut value = canonical;
    value.max_total_written_bytes += 1;
    mutations.push(value);

    let mut files = canonical.required_files.to_vec();
    files[0].path = "unsafe\\path";
    let mut value = canonical;
    value.required_files = Box::leak(files.into_boxed_slice());
    mutations.push(value);
    let mut files = canonical.required_files.to_vec();
    files[0].bytes += 1;
    let mut value = canonical;
    value.required_files = Box::leak(files.into_boxed_slice());
    mutations.push(value);
    let mut files = canonical.required_files.to_vec();
    files[0].sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut value = canonical;
    value.required_files = Box::leak(files.into_boxed_slice());
    mutations.push(value);

    for constraints in mutations {
        let mut model = *source;
        model.bundle.install_constraints = InstallConstraints::Archive(constraints);
        assert!(validate_single_model(model).is_err());
    }
}

#[test]
fn validator_rejects_every_direct_install_constraint_field_drift() {
    let source = model_registry().model(QWEN17_ID).unwrap();
    let InstallConstraints::Direct(canonical) = source.bundle.install_constraints else {
        unreachable!();
    };
    let mut max_file = *source;
    max_file.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        max_written_file_bytes: canonical.max_written_file_bytes + 1,
        ..canonical
    });
    assert!(validate_single_model(max_file).is_err());
    let mut total = *source;
    total.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        max_total_written_bytes: canonical.max_total_written_bytes + 1,
        ..canonical
    });
    assert!(validate_single_model(total).is_err());
    let mut files = canonical.required_files.to_vec();
    files[0].sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut inventory = *source;
    inventory.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        required_files: Box::leak(files.into_boxed_slice()),
        ..canonical
    });
    assert!(validate_single_model(inventory).is_err());
    let mut files = canonical.required_files.to_vec();
    files[0].path = "unsafe\\path";
    let mut path = *source;
    path.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        required_files: Box::leak(files.into_boxed_slice()),
        ..canonical
    });
    assert!(validate_single_model(path).is_err());
    let mut files = canonical.required_files.to_vec();
    files[0].bytes += 1;
    let mut bytes = *source;
    bytes.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        required_files: Box::leak(files.into_boxed_slice()),
        ..canonical
    });
    assert!(validate_single_model(bytes).is_err());

    let vad_source = *vad_manifest();
    let InstallConstraints::Direct(vad_constraints) = vad_source.bundle.install_constraints else {
        unreachable!();
    };
    let mut vad = vad_source;
    vad.bundle.install_constraints = InstallConstraints::Direct(DirectInstallConstraints {
        max_total_written_bytes: vad_constraints.max_total_written_bytes + 1,
        ..vad_constraints
    });
    assert_vad_error(vad, RegistryValidationError::InvalidManifestField);
}

#[test]
fn qwen17_bundle_freezes_official_mixed_source_identity() {
    let model = model_registry().model(QWEN17_ID).unwrap();
    assert_eq!(model.provider, AsrProviderKind::Qwen3Asr);
    assert_eq!(model.manifest_version, "2");
    assert_eq!(model.bundle.identity_sha256, QWEN17_BUNDLE_SHA256);
    assert_eq!(model.bundle.artifacts.len(), 5);
    assert!(matches!(
        model.runtime,
        RuntimeRequirement::QwenCandleMetal {
            crate_name: "qwen3-asr",
            crate_version: "0.2.2",
            git_url: "https://github.com/alan890104/qwen3-asr-rs.git",
            git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc",
            cargo_feature: "metal",
            target_os: "macos",
            target_arch: "aarch64",
        }
    ));
    assert_eq!(
        model.device,
        DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major: 14,
            minimum_memory_gib: 24,
        }
    );

    let tokenizer = model
        .bundle
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "qwen17-tokenizer")
        .unwrap();
    assert_eq!(tokenizer.revision, QWEN17_HF_REVISION);
    assert_eq!(tokenizer.bytes, 11_429_653);
    assert_eq!(
        tokenizer.sha256,
        "fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05"
    );
    assert!(tokenizer.resolved_url.contains(QWEN17_HF_REVISION));
    assert!(!tokenizer.source_endpoint.contains('?'));
    assert_eq!(
        hugging_face_discovery_endpoint(tokenizer).unwrap(),
        format!("{}?blobs=true", tokenizer.source_endpoint)
    );
    assert_eq!(tokenizer.license_spdx, "Apache-2.0");

    for artifact in model
        .bundle
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact_id != "qwen17-tokenizer")
    {
        assert_eq!(
            artifact.revision,
            "d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec"
        );
        let expected_hosts = if artifact.bytes > 20_000_000 {
            &["cdn-lfs-cn-1.modelscope.cn", "www.modelscope.cn"][..]
        } else {
            &["www.modelscope.cn"][..]
        };
        assert_eq!(artifact.redirect_hosts, expected_hosts);
        assert_eq!(artifact.license_spdx, "Apache-2.0");
    }
}

#[test]
fn qwen17_jcs_payload_matches_golden_bytes_and_identity() {
    let model = model_registry().model(QWEN17_ID).unwrap();
    let canonical = canonical_bundle_payload(model).unwrap();
    let fixture = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/models/qwen17-bundle-v2.json"
    ));

    assert_eq!(canonical.as_bytes(), fixture);
    assert!(!canonical.contains("?blobs=true"));
    assert_eq!(model.bundle.identity_sha256, QWEN17_BUNDLE_SHA256);
}

#[test]
fn qwen17_rejects_both_version_one_draft_identities() {
    let shipping = canonical_bundle_payload(model_registry().model(QWEN17_ID).unwrap()).unwrap();
    let post_discovery_v1 =
        shipping.replace("\"manifest_version\":\"2\"", "\"manifest_version\":\"1\"");
    assert_eq!(
        hex::encode(Sha256::digest(post_discovery_v1.as_bytes())),
        "26fea093b01541244dcb170fe3dbc33854d07c770ea60dadcba806bfb0e23ea5"
    );
    let pre_discovery_v1 = post_discovery_v1.replace(
        "[\"cdn-lfs-cn-1.modelscope.cn\",\"www.modelscope.cn\"]",
        "[\"www.modelscope.cn\"]",
    );
    assert_eq!(
        hex::encode(Sha256::digest(pre_discovery_v1.as_bytes())),
        "8279d22e1b2ae8fe71473bf28c2edd9cd37c4ee212641c8c0d13d2641745fc61"
    );
    assert_ne!(
        model_registry()
            .model(QWEN17_ID)
            .unwrap()
            .bundle
            .identity_sha256,
        "26fea093b01541244dcb170fe3dbc33854d07c770ea60dadcba806bfb0e23ea5"
    );
    assert!(
        !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/models/qwen17-bundle-v1.json"
        ))
        .exists()
    );
}

#[test]
fn qwen_config_contract_requires_original_thinker_shape() {
    let original = br#"{"thinker_config":{"audio_config":{},"text_config":{}}}"#;
    let hf_only = br#"{"audio_config":{},"text_config":{}}"#;

    assert_eq!(validate_qwen_config_shape(original), Ok(()));
    assert_eq!(
        validate_qwen_config_shape(hf_only),
        Err(ConfigCompatibilityError::MissingThinkerConfig)
    );
}

#[test]
fn model_lookup_exposes_device_install_and_qualification_states() {
    let registry = model_registry();
    let context_free = registry.lookup(QWEN17_ID).unwrap();
    assert!(context_free.selectable);
    assert!(!context_free.installable);
    assert!(!context_free.executable);
    assert_eq!(
        context_free.reason_code.as_deref(),
        Some("model_context_required")
    );

    let cases = [
        (
            ModelLookupContext::new(
                DeviceSupport::Unsupported,
                InstallationQualification::NotInstalled,
            ),
            (true, false, false, Some("model_device_unsupported")),
        ),
        (
            ModelLookupContext::new(
                DeviceSupport::Compatible,
                InstallationQualification::NotInstalled,
            ),
            (true, true, false, Some("model_not_installed")),
        ),
        (
            ModelLookupContext::new(
                DeviceSupport::Compatible,
                InstallationQualification::InstalledUnqualified,
            ),
            (true, true, false, Some("model_runtime_unqualified")),
        ),
        (
            ModelLookupContext::new(
                DeviceSupport::Compatible,
                InstallationQualification::RuntimeQualified,
            ),
            (true, true, true, None),
        ),
    ];

    for (context, expected) in cases {
        let capabilities = registry.lookup_with_context(QWEN17_ID, context).unwrap();
        assert_eq!(
            (
                capabilities.selectable,
                capabilities.installable,
                capabilities.executable,
                capabilities.reason_code.as_deref(),
            ),
            expected
        );
    }
}

#[test]
fn cargo_and_notice_contract_pin_qwen_metal_runtime_closure() {
    let cargo_toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let cargo_lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));

    assert!(cargo_toml.contains(
        "qwen3-asr = { git = \"https://github.com/alan890104/qwen3-asr-rs.git\", rev = \"c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc\", version = \"=0.2.2\", default-features = false, features = [\"metal\"], optional = true }"
    ));
    assert!(cargo_toml.contains("asr-runtime = [\"dep:sherpa-onnx\"]"));
    assert!(cargo_toml.contains(
        "candle-core = { version = \"=0.9.2\", default-features = false, features = [\"metal\"], optional = true }"
    ));
    assert!(cargo_toml.contains("asr-qwen17-runtime = [\"dep:qwen3-asr\", \"dep:candle-core\"]"));
    assert!(cargo_toml.contains("\"asr-qwen17-runtime\""));
    assert!(cargo_lock.contains(
        "git+https://github.com/alan890104/qwen3-asr-rs.git?rev=c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc#c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc"
    ));
}

#[test]
fn cargo_feature_contract_is_semantically_pinned() {
    let manifest: toml::Value = toml::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/Cargo.toml"
    )))
    .unwrap();
    let features = manifest["features"].as_table().unwrap();
    assert_eq!(toml_array(features, "default"), Vec::<String>::new());
    assert_eq!(toml_array(features, "asr-runtime"), ["dep:sherpa-onnx"]);
    assert_eq!(
        toml_array(features, "asr-qwen17-runtime"),
        ["dep:qwen3-asr", "dep:candle-core"]
    );
    let desktop = toml_array(features, "desktop");
    assert!(desktop.contains(&"asr-runtime".to_owned()));
    assert!(desktop.contains(&"asr-qwen17-runtime".to_owned()));

    let qwen = &manifest["target"]["cfg(all(target_os = \"macos\", target_arch = \"aarch64\"))"]["dependencies"]
        ["qwen3-asr"];
    let candle = &manifest["target"]["cfg(all(target_os = \"macos\", target_arch = \"aarch64\"))"]
        ["dependencies"]["candle-core"];
    assert!(manifest["dependencies"].get("qwen3-asr").is_none());
    let targets = manifest["target"].as_table().unwrap();
    assert_eq!(targets.len(), 1);
    assert!(targets.contains_key("cfg(all(target_os = \"macos\", target_arch = \"aarch64\"))"));
    assert_eq!(
        qwen["git"].as_str(),
        Some("https://github.com/alan890104/qwen3-asr-rs.git")
    );
    assert_eq!(
        qwen["rev"].as_str(),
        Some("c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc")
    );
    assert_eq!(qwen["version"].as_str(), Some("=0.2.2"));
    assert_eq!(qwen["optional"].as_bool(), Some(true));
    assert_eq!(qwen["default-features"].as_bool(), Some(false));
    assert_eq!(
        qwen["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["metal"]
    );
    assert_eq!(candle["version"].as_str(), Some("=0.9.2"));
    assert_eq!(candle["optional"].as_bool(), Some(true));
    assert_eq!(candle["default-features"].as_bool(), Some(false));
    assert_eq!(
        candle["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["metal"]
    );

    let lock: toml::Value = toml::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/Cargo.lock"
    )))
    .unwrap();
    assert_locked_package(
        &lock,
        "qwen3-asr",
        "0.2.2",
        Some("c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc"),
    );
    assert_locked_package(&lock, "candle-core", "0.9.2", None);
    assert_locked_package(&lock, "candle-nn", "0.9.2", None);
}

#[test]
#[ignore = "set LIFESUB_VERIFY_CARGO_GRAPH=1 to invoke locked cargo metadata/tree"]
fn cargo_metadata_and_feature_tree_match_the_pinned_contract() {
    if std::env::var_os("LIFESUB_VERIFY_CARGO_GRAPH").is_none() {
        return;
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let metadata = Command::new(&cargo)
        .args([
            "metadata",
            "--manifest-path",
            manifest,
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let qwen = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "qwen3-asr")
        .unwrap();
    assert_eq!(qwen["version"], "0.2.2");
    assert!(
        qwen["source"]
            .as_str()
            .unwrap()
            .contains("c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc")
    );
    let notices = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../THIRD_PARTY_NOTICES.md"
    ));
    assert_eq!(
        notice_rows(
            notices,
            "<!-- BEGIN LIFESUB_RUNTIME_CLOSURE_V1 -->",
            "<!-- END LIFESUB_RUNTIME_CLOSURE_V1 -->",
        ),
        metadata_qwen_notice_rows(&metadata)
    );

    let apple = Command::new(&cargo)
        .args([
            "metadata",
            "--manifest-path",
            manifest,
            "--locked",
            "--all-features",
            "--filter-platform",
            "aarch64-apple-darwin",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap();
    assert!(apple.status.success());
    let apple: serde_json::Value = serde_json::from_slice(&apple.stdout).unwrap();
    let apple_lifesub = apple["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"].as_str().unwrap().contains("lifesub@0.1.0"))
        .unwrap();
    assert!(
        apple_lifesub["deps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| dependency["pkg"]
                .as_str()
                .unwrap()
                .contains("qwen3-asr@0.2.2"))
    );

    let no_default = cargo_tree(&cargo, manifest, None);
    assert!(!no_default.contains("qwen3-asr"));
    assert!(!no_default.contains("sherpa-onnx"));
    let desktop = cargo_tree(&cargo, manifest, Some("desktop"));
    assert!(desktop.contains("qwen3-asr feature \"metal\""));
    assert!(desktop.contains("candle-core feature \"metal\""));
    assert!(desktop.contains("candle-nn feature \"metal\""));
    assert!(desktop.contains("sherpa-onnx feature \"static\""));
    assert!(!desktop.contains("qwen3-asr feature \"cuda\""));
    assert!(!desktop.contains("qwen3-asr feature \"hub\""));

    let unsupported = Command::new(&cargo)
        .args([
            "metadata",
            "--manifest-path",
            manifest,
            "--locked",
            "--all-features",
            "--filter-platform",
            "x86_64-unknown-linux-gnu",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap();
    assert!(unsupported.status.success());
    let unsupported: serde_json::Value = serde_json::from_slice(&unsupported.stdout).unwrap();
    let lifesub = unsupported["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"].as_str().unwrap().contains("lifesub@0.1.0"))
        .unwrap();
    assert!(
        !lifesub["deps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| dependency["pkg"]
                .as_str()
                .unwrap()
                .contains("qwen3-asr@0.2.2"))
    );
}

#[test]
fn notices_reconcile_manifest_artifacts_and_locked_runtime_closure_bidirectionally() {
    let notices = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../THIRD_PARTY_NOTICES.md"
    ));
    let artifact_rows = notice_rows(
        notices,
        "<!-- BEGIN LIFESUB_ARTIFACT_NOTICES_V1 -->",
        "<!-- END LIFESUB_ARTIFACT_NOTICES_V1 -->",
    );
    let runtime_rows = notice_rows(
        notices,
        "<!-- BEGIN LIFESUB_RUNTIME_CLOSURE_V1 -->",
        "<!-- END LIFESUB_RUNTIME_CLOSURE_V1 -->",
    );

    let mut expected_artifacts = BTreeSet::new();
    for artifact in model_registry()
        .models()
        .iter()
        .flat_map(|model| model.bundle.artifacts)
        .chain(vad_manifest().bundle.artifacts)
    {
        expected_artifacts.insert(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            artifact.artifact_id,
            artifact.source_repository,
            artifact.source_model,
            artifact.revision,
            artifact.license_spdx,
            artifact.sha256,
            artifact.provenance
        ));
    }
    assert_eq!(artifact_rows, expected_artifacts);

    let lock: toml::Value = toml::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/Cargo.lock"
    )))
    .unwrap();
    let expected_runtime = locked_qwen_closure(&lock);
    let actual_runtime = runtime_rows
        .iter()
        .map(|row| {
            let columns = row.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 5, "invalid runtime notice row: {row}");
            assert!(!columns[3].is_empty(), "missing license: {row}");
            assert!(!columns[4].is_empty(), "missing repository: {row}");
            format!("{}\t{}\t{}", columns[0], columns[1], columns[2])
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_runtime, expected_runtime);
}

#[test]
fn metadata_notice_closure_preserves_license_and_repository_fields() {
    let metadata = serde_json::json!({
        "packages": [
            {"id":"root","name":"qwen3-asr","version":"0.2.2","source":"git+exact","license":"MIT","repository":"https://example.test/qwen"},
            {"id":"dep","name":"candle-core","version":"0.9.2","source":"registry+exact","license":"MIT OR Apache-2.0","repository":"https://example.test/candle"}
        ],
        "resolve": {"nodes": [
            {"id":"root","deps":[{"pkg":"dep"}]},
            {"id":"dep","deps":[]}
        ]}
    });
    assert_eq!(
        metadata_qwen_notice_rows(&metadata),
        BTreeSet::from([
            "candle-core\t0.9.2\tregistry+exact\tMIT OR Apache-2.0\thttps://example.test/candle"
                .to_owned(),
            "qwen3-asr\t0.2.2\tgit+exact\tMIT\thttps://example.test/qwen".to_owned(),
        ])
    );
}

#[test]
fn validator_rejects_empty_registries() {
    let error = validate_registry(
        &crate::asr::manifest::ModelRegistry::new(&[]),
        vad_manifest(),
    );
    assert_eq!(error, Err(RegistryValidationError::EmptyRegistry));
}

#[test]
fn validator_rejects_non_normalized_https_hosts() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let mut artifact = source.bundle.artifacts[0];
    artifact.resolved_url =
        "https://GitHub.com/k2-fsa/sherpa-onnx/releases/download/asr-models/archive.tar.bz2";
    let artifacts = Box::leak(Box::new([artifact]));
    let mut model = *source;
    model.bundle.artifacts = artifacts;
    let models = Box::leak(Box::new([model]));

    assert_eq!(
        validate_registry(
            &crate::asr::manifest::ModelRegistry::new(models),
            vad_manifest()
        ),
        Err(RegistryValidationError::InvalidArtifact)
    );
}

#[test]
fn validator_rejects_non_default_https_ports() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let mut artifact = source.bundle.artifacts[0];
    artifact.resolved_url =
        "https://github.com:8443/k2-fsa/sherpa-onnx/releases/download/asr-models/archive.tar.bz2";
    let mut model = *source;
    model.bundle.artifacts = Box::leak(Box::new([artifact]));
    assert_single_model_error(model, RegistryValidationError::InvalidArtifact);
}

#[test]
fn validator_rejects_single_artifact_identity_drift() {
    let mut model = *model_registry().model("whisper-tiny").unwrap();
    model.bundle.identity_sha256 =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    assert_single_model_error(model, RegistryValidationError::InvalidBundleIdentity);
}

#[test]
fn validator_rejects_incomplete_vad_contract() {
    let mut vad = *vad_manifest();
    vad.id = "";
    assert_eq!(
        validate_registry(model_registry(), Box::leak(Box::new(vad))),
        Err(RegistryValidationError::InvalidManifestField)
    );
}

#[test]
fn validator_rejects_noncanonical_vad_identity() {
    let mut vad = *vad_manifest();
    vad.id = "silero-vad-unpinned";
    vad.manifest_version = "2";
    assert_eq!(
        validate_registry(model_registry(), Box::leak(Box::new(vad))),
        Err(RegistryValidationError::InvalidManifestField)
    );
}

#[test]
fn vad_manifest_freezes_official_sherpa_source_defaults() {
    let vad = vad_manifest();
    assert_eq!(vad.sherpa_onnx_version, "1.13.5");
    assert_eq!(
        vad.sherpa_onnx_commit,
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
    assert_eq!(
        vad.silero_config_source_header,
        "sherpa-onnx/csrc/silero-vad-model-config.h"
    );
    assert_eq!(
        vad.vad_config_source_header,
        "sherpa-onnx/csrc/vad-model-config.h"
    );
    assert_eq!(vad.threshold.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        vad.min_silence_duration_seconds.to_bits(),
        0.5_f32.to_bits()
    );
    assert_eq!(
        vad.min_speech_duration_seconds.to_bits(),
        0.25_f32.to_bits()
    );
    assert_eq!(
        vad.max_speech_duration_seconds.to_bits(),
        20.0_f32.to_bits()
    );
    assert_eq!(vad.window_size_samples, 512);
    assert_eq!(vad.sample_rate_hz, 16_000);
    assert_eq!(vad.num_threads, 1);
    assert_eq!(vad.provider, "cpu");
}

#[test]
fn validator_rejects_every_vad_default_and_provenance_mutation() {
    let canonical = *vad_manifest();
    let mut mutations = Vec::new();

    let mut value = canonical;
    value.threshold = 0.4;
    mutations.push(value);
    let mut value = canonical;
    value.min_silence_duration_seconds = 0.4;
    mutations.push(value);
    let mut value = canonical;
    value.min_speech_duration_seconds = 0.2;
    mutations.push(value);
    let mut value = canonical;
    value.max_speech_duration_seconds = 19.0;
    mutations.push(value);
    let mut value = canonical;
    value.window_size_samples = 256;
    mutations.push(value);
    let mut value = canonical;
    value.sample_rate_hz = 8_000;
    mutations.push(value);
    let mut value = canonical;
    value.num_threads = 2;
    mutations.push(value);
    let mut value = canonical;
    value.provider = "cuda";
    mutations.push(value);
    let mut value = canonical;
    value.sherpa_onnx_version = "1.13.4";
    mutations.push(value);
    let mut value = canonical;
    value.sherpa_onnx_commit = "3dc7c569";
    mutations.push(value);
    let mut value = canonical;
    value.silero_config_source_header = "";
    mutations.push(value);
    let mut value = canonical;
    value.vad_config_source_header = "";
    mutations.push(value);

    for mutation in mutations {
        assert_vad_error(mutation, RegistryValidationError::InvalidManifestField);
    }
}

#[test]
fn validator_rejects_nonfinite_or_invalid_vad_ranges() {
    let mut nan = *vad_manifest();
    nan.threshold = f32::NAN;
    assert_vad_error(nan, RegistryValidationError::InvalidManifestField);
    let mut infinity = *vad_manifest();
    infinity.max_speech_duration_seconds = f32::INFINITY;
    assert_vad_error(infinity, RegistryValidationError::InvalidManifestField);
    let mut invalid_range = *vad_manifest();
    invalid_range.max_speech_duration_seconds = 0.1;
    assert_vad_error(invalid_range, RegistryValidationError::InvalidManifestField);
}

#[test]
fn validator_rejects_vad_artifact_or_required_file_drift() {
    let canonical = *vad_manifest();
    let mut artifact = canonical.bundle.artifacts[0];
    artifact.sha256 = "malformed";
    let artifacts = Box::leak(Box::new([artifact]));
    let mut malformed_hash = canonical;
    malformed_hash.bundle.artifacts = artifacts;
    assert_vad_error(
        malformed_hash,
        RegistryValidationError::InvalidManifestField,
    );

    let mut missing_required_file = canonical;
    missing_required_file.bundle.required_paths = &[];
    assert_vad_error(
        missing_required_file,
        RegistryValidationError::InvalidManifestField,
    );
}

#[test]
fn vad_manifest_excludes_lifesub_orchestration_parameters() {
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/asr/manifest.rs"));
    assert!(!source.contains("speech_padding_milliseconds"));
    assert!(!source.contains("orchestration_max_window_seconds"));
}

#[test]
fn validator_rejects_runtime_device_and_policy_drift() {
    let mut runtime = *model_registry().model("whisper-tiny").unwrap();
    runtime.runtime = RuntimeRequirement::SherpaOnnx {
        crate_version: "1.13.5",
        git_commit: "",
        native_archive_sha256: "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44",
        build_id: "sherpa-onnx-v1.13.5-osx-arm64-static-lib",
        cargo_feature: "static",
    };
    assert_single_model_error(runtime, RegistryValidationError::InvalidManifestField);

    let mut archive = *model_registry().model("whisper-tiny").unwrap();
    archive.runtime = RuntimeRequirement::SherpaOnnx {
        crate_version: "1.13.5",
        git_commit: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5",
        native_archive_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        build_id: "sherpa-onnx-v1.13.5-osx-arm64-static-lib",
        cargo_feature: "static",
    };
    assert_single_model_error(archive, RegistryValidationError::InvalidManifestField);

    let mut build = *model_registry().model("whisper-tiny").unwrap();
    build.runtime = RuntimeRequirement::SherpaOnnx {
        crate_version: "1.13.5",
        git_commit: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5",
        native_archive_sha256: "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44",
        build_id: "untrusted-build",
        cargo_feature: "static",
    };
    assert_single_model_error(build, RegistryValidationError::InvalidManifestField);

    let mut device = *model_registry().model("whisper-tiny").unwrap();
    device.device = DeviceRequirement::AppleSiliconMetal {
        minimum_macos_major: 13,
        minimum_memory_gib: 16,
    };
    assert_single_model_error(device, RegistryValidationError::InvalidManifestField);

    let mut policy = *model_registry().model("whisper-tiny").unwrap();
    policy.qualification_policy = QualificationPolicy::RuntimeSmokeRequired;
    assert_single_model_error(policy, RegistryValidationError::InvalidManifestField);
}

#[test]
fn validator_rejects_non_normalized_or_duplicate_languages() {
    let mut model = *model_registry().model("whisper-tiny").unwrap();
    model.supported_languages = Box::leak(Box::new(["auto", "EN", "auto"]));
    assert_single_model_error(model, RegistryValidationError::InvalidManifestField);
}

#[test]
fn validator_rejects_redirect_allowlist_that_omits_download_host() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let mut artifact = source.bundle.artifacts[0];
    artifact.redirect_hosts = &["release-assets.githubusercontent.com"];
    let artifacts = Box::leak(Box::new([artifact]));
    let mut model = *source;
    model.bundle.artifacts = artifacts;
    assert_single_model_error(model, RegistryValidationError::InvalidArtifact);
}

#[test]
fn github_artifacts_allowlist_the_provider_api_and_download_redirect_chain() {
    for model in model_registry()
        .models()
        .iter()
        .filter(|model| model.id != QWEN17_ID)
    {
        assert_eq!(
            model.bundle.artifacts[0].redirect_hosts,
            &[
                "api.github.com",
                "github.com",
                "release-assets.githubusercontent.com",
            ]
        );
    }
}

#[test]
fn validator_rejects_incomplete_single_archive_required_paths() {
    let mut model = *model_registry().model("whisper-tiny").unwrap();
    model.bundle.required_paths = &["tiny-encoder.onnx", "tiny-decoder.onnx"];
    assert_single_model_error(model, RegistryValidationError::InvalidRequiredPath);
}

#[test]
fn validator_rejects_shipping_metadata_drift_with_same_id_and_version() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let mut display = *source;
    display.display_name = "Whisper Tiny Drift";
    assert_single_model_error(display, RegistryValidationError::InvalidManifestField);

    let mut artifact = source.bundle.artifacts[0];
    artifact.provenance = "different conversion provenance";
    let artifacts = Box::leak(Box::new([artifact]));
    let mut provenance = *source;
    provenance.bundle.artifacts = artifacts;
    assert_single_model_error(provenance, RegistryValidationError::InvalidManifestField);
}

#[test]
fn validator_freezes_every_single_archive_shipping_field() {
    for source in model_registry()
        .models()
        .iter()
        .filter(|model| model.bundle.artifacts.len() == 1)
    {
        let mut mutations = Vec::new();

        let mut model = *source;
        model.id = "unknown-shipping-id";
        mutations.push(model);
        let mut model = *source;
        model.manifest_version = "999";
        mutations.push(model);
        let mut model = *source;
        model.display_name = "metadata drift";
        mutations.push(model);
        let mut model = *source;
        model.provider = if source.provider == AsrProviderKind::Qwen3Asr {
            AsrProviderKind::Whisper
        } else {
            AsrProviderKind::Qwen3Asr
        };
        mutations.push(model);
        let mut model = *source;
        model.supported_languages = &["auto", "drift"];
        mutations.push(model);
        let mut model = *source;
        model.source.repository_url = "https://example.test/repository";
        mutations.push(model);
        let mut model = *source;
        model.source.model_card_url = "https://example.test/model-card";
        mutations.push(model);
        let mut model = *source;
        model.source.license_spdx = "LicenseRef-Drift";
        mutations.push(model);
        let mut model = *source;
        model.source.provenance = "source provenance drift";
        mutations.push(model);
        let mut model = *source;
        model.runtime = QWEN_RUNTIME_FOR_TEST;
        mutations.push(model);
        let mut model = *source;
        model.device = DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major: 14,
            minimum_memory_gib: 24,
        };
        mutations.push(model);
        let mut model = *source;
        model.qualification_policy = QualificationPolicy::RuntimeSmokeRequired;
        mutations.push(model);
        let mut model = *source;
        model.bundle.required_paths = &["drift/path"];
        mutations.push(model);

        let canonical_artifact = source.bundle.artifacts[0];
        let artifact_mutations = [
            artifact_with(canonical_artifact, |artifact| {
                artifact.artifact_id = "artifact-drift"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.source_repository = "https://example.test/repository"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.source_model = "model-drift"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.source_endpoint = "https://example.test/provider-api"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.resolved_url = "https://example.test/archive.tar.bz2"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.revision = "revision-drift"
            }),
            artifact_with(canonical_artifact, |artifact| artifact.bytes += 1),
            artifact_with(canonical_artifact, |artifact| {
                artifact.sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.required_path = "root-drift"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.install_mode = ArtifactInstallMode::Direct
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.license_spdx = "MIT-0"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.provenance = "artifact drift"
            }),
            artifact_with(canonical_artifact, |artifact| {
                artifact.redirect_hosts = &["example.test"]
            }),
        ];
        for artifact in artifact_mutations {
            let mut model = *source;
            model.bundle.artifacts = Box::leak(Box::new([artifact]));
            mutations.push(model);
        }

        for mutation in mutations {
            assert!(
                validate_registry(
                    &crate::asr::manifest::ModelRegistry::new(Box::leak(Box::new([mutation]))),
                    vad_manifest(),
                )
                .is_err(),
                "accepted shipping metadata drift for {}",
                source.id
            );
        }
    }
}

#[test]
fn validator_rejects_uppercase_sha_and_unsafe_path_bytes() {
    let source = model_registry().model("whisper-tiny").unwrap();
    let uppercase = source.bundle.artifacts[0].sha256.to_ascii_uppercase();
    let mut artifact = source.bundle.artifacts[0];
    artifact.sha256 = Box::leak(uppercase.into_boxed_str());
    let artifacts = Box::leak(Box::new([artifact]));
    let mut model = *source;
    model.bundle.artifacts = artifacts;
    model.bundle.identity_sha256 = artifact.sha256;
    assert_single_model_error(model, RegistryValidationError::InvalidBundleIdentity);

    let mut artifact = source.bundle.artifacts[0];
    artifact.required_path = "unsafe\\path\u{0001}";
    let artifacts = Box::leak(Box::new([artifact]));
    let mut model = *source;
    model.bundle.artifacts = artifacts;
    assert_single_model_error(model, RegistryValidationError::InvalidArtifact);
}

#[test]
fn range_probe_requires_manifest_total_size_for_200_and_206() {
    let artifact = model_registry()
        .model(QWEN17_ID)
        .unwrap()
        .bundle
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "qwen17-weights-00002")
        .unwrap();
    let mut partial = reqwest::header::HeaderMap::new();
    partial.insert(
        reqwest::header::CONTENT_RANGE,
        reqwest::header::HeaderValue::from_static("bytes 0-0/478200688"),
    );
    assert!(
        validate_transport_size(reqwest::StatusCode::PARTIAL_CONTENT, &partial, artifact).is_ok()
    );
    partial.insert(
        reqwest::header::CONTENT_RANGE,
        reqwest::header::HeaderValue::from_static("bytes 0-0/1"),
    );
    assert!(
        validate_transport_size(reqwest::StatusCode::PARTIAL_CONTENT, &partial, artifact).is_err()
    );
    partial.insert(
        reqwest::header::CONTENT_RANGE,
        reqwest::header::HeaderValue::from_static("malformed/478200688"),
    );
    assert!(
        validate_transport_size(reqwest::StatusCode::PARTIAL_CONTENT, &partial, artifact).is_err()
    );

    let missing = reqwest::header::HeaderMap::new();
    assert!(validate_transport_size(reqwest::StatusCode::OK, &missing, artifact).is_err());
    let mut wrong = reqwest::header::HeaderMap::new();
    wrong.insert(
        reqwest::header::CONTENT_LENGTH,
        reqwest::header::HeaderValue::from_static("1"),
    );
    assert!(validate_transport_size(reqwest::StatusCode::OK, &wrong, artifact).is_err());
    let mut exact = reqwest::header::HeaderMap::new();
    exact.insert(
        reqwest::header::CONTENT_LENGTH,
        reqwest::header::HeaderValue::from_static("478200688"),
    );
    assert!(validate_transport_size(reqwest::StatusCode::OK, &exact, artifact).is_ok());
}

#[test]
fn partial_content_probe_body_must_contain_exactly_one_byte() {
    assert!(validate_partial_probe_body(&mut std::io::Cursor::new([7_u8])).is_ok());
    assert!(validate_partial_probe_body(&mut std::io::Cursor::new([])).is_err());
    assert!(validate_partial_probe_body(&mut std::io::Cursor::new([7_u8, 8_u8])).is_err());
}

#[test]
fn redirect_url_policy_requires_https_without_credentials_on_effective_port_443() {
    let allowed = HashSet::from(["cdn.example.test".to_owned()]);
    assert!(redirect_url_allowed(
        &reqwest::Url::parse("https://cdn.example.test/model").unwrap(),
        &allowed,
    ));
    for rejected in [
        "http://cdn.example.test/model",
        "https://user@cdn.example.test/model",
        "https://cdn.example.test:8443/model",
        "https://other.example.test/model",
    ] {
        assert!(!redirect_url_allowed(
            &reqwest::Url::parse(rejected).unwrap(),
            &allowed,
        ));
    }
}

#[test]
fn provider_metadata_reader_rejects_bodies_larger_than_four_mib() {
    assert_eq!(
        read_json_body_limited(std::io::Cursor::new(br#"{"ok":true}"#)).unwrap(),
        serde_json::json!({"ok": true})
    );
    let oversized = vec![b' '; usize::try_from(PROVIDER_API_BODY_LIMIT).unwrap() + 1];
    assert!(read_json_body_limited(std::io::Cursor::new(oversized)).is_err());
}

#[test]
fn redirect_policy_rejects_disallowed_intermediate_before_allowed_final_host() {
    let start_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let start_port = start_listener.local_addr().unwrap().port();
    let middle_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let middle_port = middle_listener.local_addr().unwrap().port();
    middle_listener.set_nonblocking(true).unwrap();
    let start_server = std::thread::spawn(move || {
        let (mut stream, _) = start_listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).unwrap();
        write!(
            stream,
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{middle_port}/middle\r\nContent-Length: 0\r\n\r\n"
        )
        .unwrap();
    });
    let middle_server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        while std::time::Instant::now() < deadline {
            match middle_listener.accept() {
                Ok((mut stream, _)) => {
                    write!(
                        stream,
                        "HTTP/1.1 302 Found\r\nLocation: http://localhost:{start_port}/final\r\nContent-Length: 0\r\n\r\n"
                    )
                    .unwrap();
                    return true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(error) => panic!("middle listener failed: {error}"),
            }
        }
        false
    });
    let client = client_for_redirect_hosts(&["localhost"], RequestProfile::Probe).unwrap();
    let error = client
        .get(format!("http://localhost:{start_port}/start"))
        .send()
        .unwrap_err();
    start_server.join().unwrap();
    assert!(!middle_server.join().unwrap());
    assert!(format!("{error:?}").contains("redirect URL violates the artifact allowlist policy"));
}

#[test]
fn hugging_face_discovery_rejects_commit_and_blob_identity_drift() {
    let tokenizer = model_registry()
        .model(QWEN17_ID)
        .unwrap()
        .bundle
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "qwen17-tokenizer")
        .unwrap();
    let valid = serde_json::json!({
        "sha": QWEN17_HF_REVISION,
        "siblings": [{
            "rfilename": "tokenizer.json",
            "blobId": "9684da909ac4869cee4a3b6a6679194964b22ac6",
            "size": 11_429_653,
            "lfs": {
                "sha256": "fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05",
                "size": 11_429_653,
                "pointerSize": 133
            }
        }]
    });
    assert_eq!(verify_hugging_face_discovery(&valid, tokenizer), Ok(()));

    let mut wrong_commit = valid.clone();
    wrong_commit["sha"] = serde_json::Value::String("floating-main".to_owned());
    assert!(verify_hugging_face_discovery(&wrong_commit, tokenizer).is_err());
    let mut wrong_blob = valid;
    wrong_blob["siblings"][0]["lfs"]["sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(verify_hugging_face_discovery(&wrong_blob, tokenizer).is_err());
}

#[test]
fn github_release_discovery_rejects_asset_identity_drift() {
    let artifact = &model_registry()
        .model("whisper-tiny")
        .unwrap()
        .bundle
        .artifacts[0];
    let asset_id = 179_373_699_u64;
    let valid = serde_json::json!({
        "tag_name": "asr-models",
        "assets": [{
            "id": asset_id,
            "name": artifact.source_model,
            "size": artifact.bytes,
            "browser_download_url": artifact.resolved_url
        }]
    });
    assert_eq!(verify_github_release_discovery(&valid, artifact), Ok(()));
    let mut drift = valid;
    drift["assets"][0]["id"] = serde_json::Value::from(asset_id + 1);
    assert!(verify_github_release_discovery(&drift, artifact).is_err());
}

#[test]
#[ignore = "set LIFESUB_VERIFY_UPSTREAM=1 to re-download and inspect pinned assets"]
fn re_downloads_and_verifies_upstream_manifest_assets() {
    if std::env::var_os("LIFESUB_VERIFY_UPSTREAM").is_none() {
        return;
    }
    verify_upstream_registry().unwrap();
}

#[test]
#[ignore = "set LIFESUB_VERIFY_PROVIDER_DISCOVERY=1 to call official provider APIs"]
fn verifies_official_provider_discovery_without_large_downloads() {
    if std::env::var_os("LIFESUB_VERIFY_PROVIDER_DISCOVERY").is_none() {
        return;
    }
    let client = upstream_client().unwrap();
    verify_github_and_modelscope_discovery(&client).unwrap();
    verify_hugging_face_discovery_download(&client).unwrap();
}

#[test]
#[ignore = "set LIFESUB_VERIFY_PROVIDER_DISCOVERY=1 to call GitHub and ModelScope APIs"]
fn verifies_github_and_modelscope_provider_discovery() {
    if std::env::var_os("LIFESUB_VERIFY_PROVIDER_DISCOVERY").is_none() {
        return;
    }
    verify_github_and_modelscope_discovery(&upstream_client().unwrap()).unwrap();
}

#[test]
#[ignore = "set LIFESUB_VERIFY_HF_OFFICIAL=1 to call the official Hugging Face API"]
fn verifies_hugging_face_official_discovery_and_tokenizer() {
    if std::env::var_os("LIFESUB_VERIFY_HF_OFFICIAL").is_none() {
        return;
    }
    verify_hugging_face_discovery_download(&upstream_client().unwrap()).unwrap();
}

fn verify_github_and_modelscope_discovery(
    client: &reqwest::blocking::Client,
) -> Result<(), String> {
    for artifact in model_registry()
        .models()
        .iter()
        .filter(|model| model.id != QWEN17_ID)
        .map(|model| &model.bundle.artifacts[0])
        .chain(vad_manifest().bundle.artifacts)
    {
        verify_github_release_api(client, artifact)?;
    }
    verify_qwen17_provider_metadata(client)?;
    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    for artifact in qwen17
        .bundle
        .artifacts
        .iter()
        .filter(|artifact| artifact.bytes > 20_000_000)
    {
        let report = verify_large_qwen_artifact(client, artifact, true)?;
        if !report.metadata_verified || !report.transport_size_route_verified {
            return Err(format!(
                "incomplete large-artifact evidence for {}",
                artifact.artifact_id
            ));
        }
        if std::env::var_os("LIFESUB_VERIFY_QWEN17_FULL_DOWNLOAD").is_some()
            && !report.full_hash_verified
        {
            return Err(format!(
                "full hash evidence missing for {}",
                artifact.artifact_id
            ));
        }
    }
    Ok(())
}

fn verify_hugging_face_discovery_download(
    client: &reqwest::blocking::Client,
) -> Result<(), String> {
    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    let tokenizer = qwen17
        .bundle
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "qwen17-tokenizer")
        .unwrap();
    verify_hugging_face_provider_api(client, tokenizer)?;
    download_artifact(client, tokenizer)?;
    Ok(())
}

fn verify_upstream_registry() -> Result<(), String> {
    let client = upstream_client()?;

    for model in model_registry()
        .models()
        .iter()
        .filter(|model| model.id != QWEN17_ID)
    {
        let artifact = &model.bundle.artifacts[0];
        verify_github_release_api(&client, artifact)?;
        let mut downloaded = download_artifact(&client, artifact)?;
        let InstallConstraints::Archive(constraints) = model.bundle.install_constraints else {
            return Err(format!("archive model {} has direct constraints", model.id));
        };
        let observed =
            inspect_archive_install_contract(&mut downloaded, artifact.required_path, constraints)?;
        let golden = golden_archive_install_contracts()
            .remove(model.id)
            .ok_or_else(|| format!("golden fixture omitted {}", model.id))?;
        if observed != golden || observed != archive_contract_from_manifest(constraints) {
            return Err(format!("archive install contract drift for {}", model.id));
        }
    }

    let vad = vad_manifest();
    verify_github_release_api(&client, &vad.bundle.artifacts[0])?;
    download_artifact(&client, &vad.bundle.artifacts[0])?;
    verify_qwen17_provider_metadata(&client)?;
    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    let tokenizer = qwen17
        .bundle
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "qwen17-tokenizer")
        .unwrap();
    verify_hugging_face_provider_api(&client, tokenizer)?;
    for artifact in qwen17
        .bundle
        .artifacts
        .iter()
        .filter(|artifact| artifact.bytes > 20_000_000)
    {
        let report = verify_large_qwen_artifact(&client, artifact, true)?;
        if !report.metadata_verified || !report.transport_size_route_verified {
            return Err(format!(
                "incomplete large-artifact evidence for {}",
                artifact.artifact_id
            ));
        }
        if std::env::var_os("LIFESUB_VERIFY_QWEN17_FULL_DOWNLOAD").is_some()
            && !report.full_hash_verified
        {
            return Err(format!(
                "full hash evidence missing for {}",
                artifact.artifact_id
            ));
        }
    }
    for artifact in qwen17
        .bundle
        .artifacts
        .iter()
        .filter(|artifact| artifact.bytes <= 20_000_000)
    {
        let mut downloaded = download_artifact(&client, artifact)?;
        if artifact.required_path == "config.json" {
            downloaded
                .seek(SeekFrom::Start(0))
                .map_err(|error| error.to_string())?;
            let mut config = Vec::new();
            downloaded
                .read_to_end(&mut config)
                .map_err(|error| error.to_string())?;
            validate_qwen_config_shape(&config)
                .map_err(|error| format!("pinned Qwen config shape mismatch: {error:?}"))?;
            if validate_qwen_config_shape(br#"{"audio_config":{},"text_config":{}}"#).is_ok() {
                return Err("hf-only Qwen config was accepted".to_owned());
            }
        }
    }
    Ok(())
}

fn upstream_client() -> Result<reqwest::blocking::Client, String> {
    client_for_redirect_hosts(&[], RequestProfile::Probe)
}

#[derive(Clone, Copy)]
enum RequestProfile {
    Probe,
    FullDownload,
}

fn client_for_redirect_hosts(
    allowed_hosts: &[&str],
    profile: RequestProfile,
) -> Result<reqwest::blocking::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("LifeSub-manifest-verifier/0.2"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json, application/octet-stream"),
    );
    let allowed_hosts = allowed_hosts
        .iter()
        .map(|host| (*host).to_owned())
        .collect::<HashSet<_>>();
    let redirect_policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("redirect limit exceeded");
        }
        if redirect_url_allowed(attempt.url(), &allowed_hosts) {
            attempt.follow()
        } else {
            attempt.error("redirect URL violates the artifact allowlist policy")
        }
    });
    let timeout = match profile {
        RequestProfile::Probe => std::time::Duration::from_secs(PROBE_TIMEOUT_SECONDS),
        RequestProfile::FullDownload => {
            std::time::Duration::from_secs(FULL_DOWNLOAD_TIMEOUT_SECONDS)
        }
    };
    let mut builder = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .redirect(redirect_policy)
        .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
        .timeout(timeout);
    if let Some(ip) = std::env::var_os("LIFESUB_HF_RESOLVE_IP") {
        let ip = ip
            .to_str()
            .ok_or_else(|| "LIFESUB_HF_RESOLVE_IP is not UTF-8".to_owned())?
            .parse()
            .map_err(|error| format!("invalid LIFESUB_HF_RESOLVE_IP: {error}"))?;
        builder = builder.resolve("huggingface.co", std::net::SocketAddr::new(ip, 443));
    }
    builder.build().map_err(|error| error.to_string())
}

fn get_json(
    client: &reqwest::blocking::Client,
    endpoint: &str,
) -> Result<serde_json::Value, String> {
    let response = client
        .get(endpoint)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("provider API {endpoint}: {error:?}"))?;
    if !response.status().is_success() {
        return Err(format!("provider API returned {}", response.status()));
    }
    let endpoint_host = reqwest::Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    if response.url().host_str() != endpoint_host.as_deref() {
        return Err("provider API response host drift".to_owned());
    }
    read_json_body_limited(response)
}

fn read_json_body_limited(reader: impl Read) -> Result<serde_json::Value, String> {
    let mut body = Vec::new();
    reader
        .take(PROVIDER_API_BODY_LIMIT + 1)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if u64::try_from(body.len()).map_err(|error| error.to_string())? > PROVIDER_API_BODY_LIMIT {
        return Err("provider metadata exceeded 4 MiB".to_owned());
    }
    serde_json::from_slice(&body).map_err(|error| error.to_string())
}

fn redirect_url_allowed(url: &reqwest::Url, allowed_hosts: &HashSet<String>) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url
            .host_str()
            .is_some_and(|host| allowed_hosts.contains(host))
}

fn ensure_url_allowed(url: &str, allowed_hosts: &[&str]) -> Result<(), String> {
    let url = reqwest::Url::parse(url).map_err(|error| format!("invalid URL: {error}"))?;
    let allowed_hosts = allowed_hosts
        .iter()
        .map(|host| (*host).to_owned())
        .collect::<HashSet<_>>();
    if redirect_url_allowed(&url, &allowed_hosts) {
        Ok(())
    } else {
        Err(format!("URL violates the artifact allowlist policy: {url}"))
    }
}

fn verify_hugging_face_provider_api(
    _client: &reqwest::blocking::Client,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<(), String> {
    let endpoint = hugging_face_discovery_endpoint(artifact)?;
    ensure_url_allowed(&endpoint, artifact.redirect_hosts)?;
    let client = client_for_redirect_hosts(artifact.redirect_hosts, RequestProfile::Probe)?;
    let discovery = get_json(&client, &endpoint)?;
    verify_hugging_face_discovery(&discovery, artifact)
}

fn hugging_face_discovery_endpoint(
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<String, String> {
    let mut endpoint = reqwest::Url::parse(artifact.source_endpoint)
        .map_err(|error| format!("invalid Hugging Face source endpoint: {error}"))?;
    if endpoint.query().is_some() {
        return Err("canonical Hugging Face source endpoint must not contain a query".to_owned());
    }
    endpoint.query_pairs_mut().append_pair("blobs", "true");
    Ok(endpoint.to_string())
}

fn verify_hugging_face_discovery(
    discovery: &serde_json::Value,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<(), String> {
    if discovery.get("sha").and_then(serde_json::Value::as_str) != Some(artifact.revision) {
        return Err("Hugging Face revision drift".to_owned());
    }
    let tokenizer = discovery
        .get("siblings")
        .and_then(serde_json::Value::as_array)
        .and_then(|siblings| {
            siblings.iter().find(|sibling| {
                sibling.get("rfilename").and_then(serde_json::Value::as_str)
                    == Some(artifact.required_path)
            })
        })
        .ok_or_else(|| "Hugging Face API omitted tokenizer.json".to_owned())?;
    let blob_id = tokenizer
        .get("blobId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Hugging Face API omitted tokenizer blobId".to_owned())?;
    if blob_id != "9684da909ac4869cee4a3b6a6679194964b22ac6"
        || tokenizer.get("size").and_then(serde_json::Value::as_u64) != Some(artifact.bytes)
        || tokenizer
            .pointer("/lfs/size")
            .and_then(serde_json::Value::as_u64)
            != Some(artifact.bytes)
        || tokenizer
            .pointer("/lfs/sha256")
            .and_then(serde_json::Value::as_str)
            != Some(artifact.sha256)
    {
        return Err("Hugging Face tokenizer blob identity drift".to_owned());
    }
    Ok(())
}

fn verify_github_release_api(
    _client: &reqwest::blocking::Client,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<(), String> {
    ensure_url_allowed(artifact.source_endpoint, artifact.redirect_hosts)?;
    let client = client_for_redirect_hosts(artifact.redirect_hosts, RequestProfile::Probe)?;
    let discovery = get_json(&client, artifact.source_endpoint)?;
    verify_github_release_discovery(&discovery, artifact)
}

fn verify_github_release_discovery(
    discovery: &serde_json::Value,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<(), String> {
    if discovery
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        != Some("asr-models")
    {
        return Err("GitHub release tag drift".to_owned());
    }
    let expected_id = artifact
        .revision
        .strip_prefix("github-release-asset:")
        .ok_or_else(|| "GitHub asset revision has invalid shape".to_owned())?
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    let asset = discovery
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .and_then(|assets| {
            assets.iter().find(|asset| {
                asset.get("name").and_then(serde_json::Value::as_str) == Some(artifact.source_model)
            })
        })
        .ok_or_else(|| format!("GitHub release omitted {}", artifact.source_model))?;
    if asset.get("id").and_then(serde_json::Value::as_u64) != Some(expected_id)
        || asset.get("size").and_then(serde_json::Value::as_u64) != Some(artifact.bytes)
        || asset
            .get("browser_download_url")
            .and_then(serde_json::Value::as_str)
            != Some(artifact.resolved_url)
    {
        return Err(format!(
            "GitHub release identity drift for {}",
            artifact.artifact_id
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LargeArtifactVerificationReport {
    metadata_verified: bool,
    transport_size_route_verified: bool,
    full_hash_verified: bool,
}

fn verify_large_qwen_artifact(
    client: &reqwest::blocking::Client,
    artifact: &crate::asr::manifest::ArtifactFile,
    metadata_verified: bool,
) -> Result<LargeArtifactVerificationReport, String> {
    let mut report = probe_large_direct_artifact(client, artifact)?;
    report.metadata_verified = metadata_verified;
    if std::env::var_os("LIFESUB_VERIFY_QWEN17_FULL_DOWNLOAD").is_some() {
        download_artifact(client, artifact)?;
        report.full_hash_verified = true;
    }
    Ok(report)
}

fn probe_large_direct_artifact(
    _client: &reqwest::blocking::Client,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<LargeArtifactVerificationReport, String> {
    ensure_initial_url_host(artifact)?;
    let client = client_for_redirect_hosts(artifact.redirect_hosts, RequestProfile::Probe)?;
    let mut response = client
        .get(artifact.resolved_url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .map_err(|error| format!("range probe {}: {error}", artifact.artifact_id))?;
    if !response.status().is_success() {
        return Err(format!(
            "range probe {} returned {}",
            artifact.artifact_id,
            response.status()
        ));
    }
    ensure_url_allowed(response.url().as_str(), artifact.redirect_hosts)?;
    validate_transport_size(response.status(), response.headers(), artifact)?;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        validate_partial_probe_body(&mut response)?;
    } else if response.status() == reqwest::StatusCode::OK {
        let mut first_byte = [0_u8; 1];
        response
            .read_exact(&mut first_byte)
            .map_err(|error| error.to_string())?;
    }
    Ok(LargeArtifactVerificationReport {
        metadata_verified: false,
        transport_size_route_verified: true,
        full_hash_verified: false,
    })
}

fn validate_partial_probe_body(reader: &mut impl Read) -> Result<(), String> {
    let mut body = Vec::new();
    reader
        .take(2)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if body.len() == 1 {
        Ok(())
    } else {
        Err(format!(
            "206 range probe returned {} bytes instead of exactly one",
            body.len()
        ))
    }
}

fn validate_transport_size(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<(), String> {
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let content_range = headers
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "206 response omitted Content-Range".to_owned())?;
        let total = content_range
            .strip_prefix("bytes 0-0/")
            .and_then(|value| value.parse::<u64>().ok());
        if total == Some(artifact.bytes) {
            return Ok(());
        }
        return Err(format!("Content-Range drift for {}", artifact.artifact_id));
    }
    if status == reqwest::StatusCode::OK {
        let content_length = headers
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| "200 response omitted valid Content-Length".to_owned())?;
        if content_length == artifact.bytes {
            return Ok(());
        }
        return Err(format!("Content-Length drift for {}", artifact.artifact_id));
    }
    Err(format!("unsupported range probe status {status}"))
}

fn download_artifact(
    _client: &reqwest::blocking::Client,
    artifact: &crate::asr::manifest::ArtifactFile,
) -> Result<tempfile::NamedTempFile, String> {
    ensure_initial_url_host(artifact)?;
    let client = client_for_redirect_hosts(artifact.redirect_hosts, RequestProfile::FullDownload)?;
    let mut response = client
        .get(artifact.resolved_url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("download {}: {error}", artifact.artifact_id))?;
    ensure_url_allowed(response.url().as_str(), artifact.redirect_hosts)?;

    let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| error.to_string())?;
        hasher.update(&buffer[..count]);
        bytes += u64::try_from(count).map_err(|error| error.to_string())?;
        if bytes > artifact.bytes {
            return Err(format!(
                "download exceeded byte limit for {}",
                artifact.artifact_id
            ));
        }
    }
    let sha256 = hex::encode(hasher.finalize());
    if bytes != artifact.bytes || sha256 != artifact.sha256 {
        return Err(format!(
            "identity mismatch for {}: {bytes}/{sha256}",
            artifact.artifact_id
        ));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    Ok(file)
}

fn ensure_initial_url_host(artifact: &crate::asr::manifest::ArtifactFile) -> Result<(), String> {
    ensure_url_allowed(artifact.resolved_url, artifact.redirect_hosts)
}

fn verify_cached_archive_contracts(cache: &Path) -> Result<(), String> {
    let mut golden = golden_archive_install_contracts();
    for model in model_registry().models() {
        let InstallConstraints::Archive(constraints) = model.bundle.install_constraints else {
            continue;
        };
        let artifact = &model.bundle.artifacts[0];
        let path = cache.join(artifact.source_model);
        let mut file = std::fs::File::open(&path)
            .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
        let observed =
            inspect_archive_install_contract(&mut file, artifact.required_path, constraints)?;
        let expected = golden
            .remove(model.id)
            .ok_or_else(|| format!("golden fixture omitted {}", model.id))?;
        if observed != expected {
            return Err(format!("cached archive contract drift for {}", model.id));
        }
    }
    if !golden.is_empty() {
        return Err(format!("unmatched golden archive contracts: {golden:?}"));
    }
    Ok(())
}

fn inspect_archive_install_contract(
    file: &mut impl Read,
    expected_root: &str,
    constraints: crate::asr::manifest::ArchiveInstallConstraints,
) -> Result<GoldenArchiveInstallContract, String> {
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let required = constraints
        .required_files
        .iter()
        .map(|file| (file.path, file))
        .collect::<BTreeMap<_, _>>();
    let mut seen = HashSet::new();
    let mut observed_required = BTreeSet::new();
    let mut scanned = 0_u64;
    let mut max_written = 0_u64;
    let mut total_written = 0_u64;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        scanned = scanned
            .checked_add(1)
            .ok_or_else(|| "archive entry count overflow".to_owned())?;
        if scanned > constraints.max_scanned_entries {
            return Err("archive scanned-entry bound exceeded".to_owned());
        }
        let path = entry.path().map_err(|error| error.to_string())?;
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("unsafe archive path: {}", path.display()));
        }
        let relative = path
            .strip_prefix(expected_root)
            .map_err(|_| format!("archive entry escaped root: {}", path.display()))?;
        let normalized = relative
            .to_str()
            .ok_or_else(|| "non-UTF-8 archive path".to_owned())?
            .to_owned();
        if normalized.contains('\\') || normalized.chars().any(char::is_control) {
            return Err(format!("unsafe archive path bytes: {normalized:?}"));
        }
        if !seen.insert(normalized.clone()) {
            return Err(format!("duplicate archive path: {normalized}"));
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(format!("special archive entry: {normalized}"));
        }
        let Some(expected) = required.get(normalized.as_str()) else {
            continue;
        };
        if entry.size() != expected.bytes {
            return Err(format!("archive file size drift: {normalized}"));
        }
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            let count = entry.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
            bytes = bytes
                .checked_add(u64::try_from(count).map_err(|error| error.to_string())?)
                .ok_or_else(|| "archive file size overflow".to_owned())?;
            if bytes > expected.bytes {
                return Err(format!("archive file exceeded declared size: {normalized}"));
            }
        }
        let sha256 = hex::encode(hasher.finalize());
        if bytes != expected.bytes || sha256 != expected.sha256 {
            return Err(format!("archive file identity drift: {normalized}"));
        }
        total_written = total_written
            .checked_add(bytes)
            .ok_or_else(|| "archive written total overflow".to_owned())?;
        max_written = max_written.max(bytes);
        observed_required.insert(format!("{normalized}\t{bytes}\t{sha256}"));
    }
    let observed = GoldenArchiveInstallContract {
        max_scanned_entries: scanned,
        max_written_file_bytes: max_written,
        max_total_written_bytes: total_written,
        required_files: observed_required,
    };
    if observed != archive_contract_from_manifest(constraints) {
        return Err("archive required inventory or bounds drift".to_owned());
    }
    Ok(observed)
}

fn verify_qwen17_provider_metadata(_client: &reqwest::blocking::Client) -> Result<(), String> {
    let qwen17 = model_registry().model(QWEN17_ID).unwrap();
    let metadata_artifact = &qwen17.bundle.artifacts[0];
    let endpoint = metadata_artifact.source_endpoint;
    ensure_url_allowed(endpoint, metadata_artifact.redirect_hosts)?;
    let client =
        client_for_redirect_hosts(metadata_artifact.redirect_hosts, RequestProfile::Probe)?;
    let response = client
        .get(endpoint)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    ensure_url_allowed(response.url().as_str(), metadata_artifact.redirect_hosts)?;
    let response = read_json_body_limited(response)?;
    let files = response
        .pointer("/Data/Files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "ModelScope metadata omitted Data.Files".to_owned())?;
    for artifact in qwen17
        .bundle
        .artifacts
        .iter()
        .filter(|artifact| artifact.source_model == "Qwen/Qwen3-ASR-1.7B")
    {
        let metadata = files
            .iter()
            .find(|file| {
                file.get("Path").and_then(serde_json::Value::as_str) == Some(artifact.required_path)
            })
            .ok_or_else(|| format!("ModelScope metadata omitted {}", artifact.required_path))?;
        if metadata.get("Revision").and_then(serde_json::Value::as_str) != Some(artifact.revision)
            || metadata.get("Size").and_then(serde_json::Value::as_u64) != Some(artifact.bytes)
            || metadata.get("Sha256").and_then(serde_json::Value::as_str) != Some(artifact.sha256)
        {
            return Err(format!(
                "ModelScope identity drift for {}",
                artifact.artifact_id
            ));
        }
    }
    Ok(())
}

fn toml_array(table: &toml::value::Table, key: &str) -> Vec<String> {
    table[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

fn assert_locked_package(
    lock: &toml::Value,
    name: &str,
    version: &str,
    source_fragment: Option<&str>,
) {
    let package = lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| {
            package["name"].as_str() == Some(name) && package["version"].as_str() == Some(version)
        })
        .unwrap();
    if let Some(source_fragment) = source_fragment {
        assert!(
            package["source"]
                .as_str()
                .unwrap()
                .contains(source_fragment)
        );
    }
    if name == "qwen3-asr" {
        let dependencies = package["dependencies"].as_array().unwrap();
        assert!(
            !dependencies.iter().any(|dependency| {
                matches!(dependency.as_str(), Some("reqwest") | Some("cudarc"))
            })
        );
    }
}

fn cargo_tree(cargo: &std::ffi::OsStr, manifest: &str, features: Option<&str>) -> String {
    let mut command = Command::new(cargo);
    command.args([
        "tree",
        "--manifest-path",
        manifest,
        "--locked",
        "-e",
        "features",
        "--no-default-features",
        "--target",
        "aarch64-apple-darwin",
    ]);
    if let Some(features) = features {
        command.args(["--features", features]);
    }
    let output = command.output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn notice_rows(notices: &str, begin: &str, end: &str) -> BTreeSet<String> {
    let section = notices
        .split_once(begin)
        .and_then(|(_, suffix)| suffix.split_once(end).map(|(section, _)| section))
        .unwrap_or_else(|| panic!("missing notice section {begin}"));
    section
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn locked_qwen_closure(lock: &toml::Value) -> BTreeSet<String> {
    let packages = lock["package"].as_array().unwrap();
    let root = packages
        .iter()
        .position(|package| {
            package["name"].as_str() == Some("qwen3-asr")
                && package["version"].as_str() == Some("0.2.2")
        })
        .unwrap();
    let mut queue = VecDeque::from([root]);
    let mut visited = HashSet::new();
    let mut closure = BTreeSet::new();
    while let Some(index) = queue.pop_front() {
        if !visited.insert(index) {
            continue;
        }
        let package = &packages[index];
        closure.insert(format!(
            "{}\t{}\t{}",
            package["name"].as_str().unwrap(),
            package["version"].as_str().unwrap(),
            package["source"].as_str().unwrap_or("path")
        ));
        for dependency in package
            .get("dependencies")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let dependency = dependency.as_str().unwrap();
            let identity = dependency.split(" (").next().unwrap();
            let mut parts = identity.split_whitespace();
            let name = parts.next().unwrap();
            let version = parts.next();
            for (candidate_index, candidate) in packages.iter().enumerate() {
                if candidate["name"].as_str() == Some(name)
                    && version.is_none_or(|version| candidate["version"].as_str() == Some(version))
                {
                    queue.push_back(candidate_index);
                }
            }
        }
    }
    closure
}

fn metadata_qwen_notice_rows(metadata: &serde_json::Value) -> BTreeSet<String> {
    let packages = metadata["packages"].as_array().unwrap();
    let nodes = metadata["resolve"]["nodes"].as_array().unwrap();
    let root = packages
        .iter()
        .find(|package| package["name"] == "qwen3-asr" && package["version"] == "0.2.2")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut queue = VecDeque::from([root]);
    let mut visited = HashSet::new();
    let mut rows = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let package = packages
            .iter()
            .find(|package| package["id"].as_str() == Some(&id))
            .unwrap();
        let name = package["name"].as_str().unwrap();
        let version = package["version"].as_str().unwrap();
        let source = package["source"].as_str().unwrap_or("path");
        let license = package["license"].as_str().unwrap_or("NOASSERTION");
        let repository = package["repository"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("https://crates.io/crates/{name}/{version}"));
        rows.insert(format!(
            "{name}\t{version}\t{source}\t{license}\t{repository}"
        ));
        let node = nodes
            .iter()
            .find(|node| node["id"].as_str() == Some(&id))
            .unwrap();
        for dependency in node["deps"].as_array().unwrap() {
            queue.push_back(dependency["pkg"].as_str().unwrap().to_owned());
        }
    }
    rows
}

fn assert_install_inventory_matches_required_paths(
    required_paths: &[&str],
    required_files: &[RequiredInstallFile],
) {
    assert_eq!(required_paths.len(), required_files.len());
    let paths = required_files
        .iter()
        .map(|file| file.path)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        required_paths.iter().copied().collect::<BTreeSet<_>>(),
        paths
    );
    for file in required_files {
        assert_normalized_relative_path(file.path);
        assert!(file.bytes > 0);
        assert_hex_sha256(file.sha256);
    }
}

fn validate_single_model(
    model: crate::asr::manifest::ModelManifest,
) -> Result<(), RegistryValidationError> {
    validate_registry(
        &crate::asr::manifest::ModelRegistry::new(Box::leak(Box::new([model]))),
        vad_manifest(),
    )
}

fn golden_archive_install_contracts() -> BTreeMap<String, GoldenArchiveInstallContract> {
    let fixture = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/models/sherpa-install-inventory-v1.tsv"
    ));
    let mut contracts = BTreeMap::new();
    let mut file_count = 0;
    for (index, line) in fixture.lines().enumerate() {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 5, "invalid golden row {}", index + 1);
        match columns[0] {
            "archive" => {
                let previous = contracts.insert(
                    columns[1].to_owned(),
                    GoldenArchiveInstallContract {
                        max_scanned_entries: columns[2].parse().unwrap(),
                        max_written_file_bytes: columns[3].parse().unwrap(),
                        max_total_written_bytes: columns[4].parse().unwrap(),
                        required_files: BTreeSet::new(),
                    },
                );
                assert!(
                    previous.is_none(),
                    "duplicate golden archive {}",
                    columns[1]
                );
            }
            "file" => {
                let contract = contracts
                    .get_mut(columns[1])
                    .unwrap_or_else(|| panic!("file row precedes archive row: {}", columns[1]));
                let row = format!("{}\t{}\t{}", columns[2], columns[3], columns[4]);
                assert!(
                    contract.required_files.insert(row),
                    "duplicate golden file row"
                );
                file_count += 1;
            }
            kind => panic!("unknown golden row type {kind}"),
        }
    }
    assert_eq!(contracts.len(), 5);
    assert_eq!(file_count, 50);
    contracts
}

fn archive_contract_from_manifest(
    constraints: crate::asr::manifest::ArchiveInstallConstraints,
) -> GoldenArchiveInstallContract {
    GoldenArchiveInstallContract {
        max_scanned_entries: constraints.max_scanned_entries,
        max_written_file_bytes: constraints.max_written_file_bytes,
        max_total_written_bytes: constraints.max_total_written_bytes,
        required_files: constraints
            .required_files
            .iter()
            .map(|file| format!("{}\t{}\t{}", file.path, file.bytes, file.sha256))
            .collect(),
    }
}

fn artifact_with(
    mut artifact: crate::asr::manifest::ArtifactFile,
    mutate: impl FnOnce(&mut crate::asr::manifest::ArtifactFile),
) -> crate::asr::manifest::ArtifactFile {
    mutate(&mut artifact);
    artifact
}

fn assert_single_model_error(
    model: crate::asr::manifest::ModelManifest,
    expected: RegistryValidationError,
) {
    let models = Box::leak(Box::new([model]));
    assert_eq!(
        validate_registry(
            &crate::asr::manifest::ModelRegistry::new(models),
            vad_manifest()
        ),
        Err(expected)
    );
}

fn assert_vad_error(vad: crate::asr::manifest::VadManifest, expected: RegistryValidationError) {
    assert_eq!(
        validate_registry(model_registry(), Box::leak(Box::new(vad))),
        Err(expected)
    );
}

fn assert_artifact_contract(artifact: &crate::asr::manifest::ArtifactFile) {
    assert!(!artifact.artifact_id.trim().is_empty());
    assert!(artifact.source_repository.starts_with("https://"));
    assert!(!artifact.source_model.trim().is_empty());
    assert!(artifact.source_endpoint.starts_with("https://"));
    assert!(artifact.resolved_url.starts_with("https://"));
    assert!(!artifact.revision.trim().is_empty());
    assert!(artifact.bytes > 0);
    assert_hex_sha256(artifact.sha256);
    assert_normalized_relative_path(artifact.required_path);
    assert!(artifact.required);
    assert!(!artifact.license_spdx.trim().is_empty());
    assert!(!artifact.provenance.trim().is_empty());
    assert!(!artifact.redirect_hosts.is_empty());
    for host in artifact.redirect_hosts {
        assert_eq!(*host, host.to_ascii_lowercase());
        assert!(!host.contains('*'));
    }
}

fn assert_normalized_relative_path(path: &str) {
    assert!(!path.is_empty());
    assert!(!path.starts_with('/'));
    assert!(!path.ends_with('/'));
    assert!(!path.split('/').any(|component| component.is_empty()));
    assert!(!path.split('/').any(|component| component == ".."));
}

fn assert_hex_sha256(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(value, "0".repeat(64));
}
