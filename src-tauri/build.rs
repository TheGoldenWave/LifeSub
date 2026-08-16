use std::path::{Path, PathBuf};
use std::{fs::File, io::Read};

use sha2::{Digest, Sha256};

const RUNTIME_VERSION: &str = "1.13.5";
const RUNTIME_GIT_COMMIT: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";
const ARCHIVE_NAME: &str = "sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2";
const ARCHIVE_SIZE: u64 = 19_862_746;
const ARCHIVE_SHA256: &str = "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44";
const BUILD_ID: &str = "sherpa-onnx-v1.13.5-osx-arm64-static-lib";
const ATTESTATION_FILE_NAME: &str = ".lifesub-sherpa-runtime-attestation-v1";
const ATTESTATION_ENV: &str = "LIFESUB_SHERPA_RUNTIME_ATTESTATION_FILE";

fn main() {
    println!("cargo:rerun-if-env-changed={ATTESTATION_ENV}");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_ARCHIVE_DIR");
    println!("cargo:rerun-if-env-changed=LIFESUB_SHERPA_ARCHIVE_SHA256");
    println!("cargo:rerun-if-env-changed=LIFESUB_SHERPA_BUILD_ID");
    println!("cargo:rerun-if-env-changed=LIFESUB_SHERPA_VERIFIED");

    if std::env::var_os("CARGO_FEATURE_ASR_RUNTIME").is_some() {
        verify_native_runtime_attestation();
        println!("cargo:rustc-env=LIFESUB_SHERPA_ARCHIVE_SHA256={ARCHIVE_SHA256}");
        println!("cargo:rustc-env=LIFESUB_SHERPA_BUILD_ID={BUILD_ID}");
        println!("cargo:rustc-env=LIFESUB_SHERPA_VERIFIED=1");
    }

    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}

fn verify_native_runtime_attestation() {
    require_env("LIFESUB_SHERPA_VERIFIED", "1");
    require_env("LIFESUB_SHERPA_ARCHIVE_SHA256", ARCHIVE_SHA256);
    require_env("LIFESUB_SHERPA_BUILD_ID", BUILD_ID);

    let archive_dir = canonical_regular_directory_env_path("SHERPA_ONNX_ARCHIVE_DIR");
    let attestation_path = canonical_regular_file_env_path(ATTESTATION_ENV);
    assert_eq!(attestation_path.parent(), Some(archive_dir.as_path()));
    assert_eq!(
        attestation_path.file_name().and_then(|name| name.to_str()),
        Some(ATTESTATION_FILE_NAME)
    );

    let expected = format!(
        "schema=lifesub.sherpa-runtime-attestation.v1\nversion={RUNTIME_VERSION}\ngit_commit={RUNTIME_GIT_COMMIT}\narchive_name={ARCHIVE_NAME}\narchive_size={ARCHIVE_SIZE}\narchive_sha256={ARCHIVE_SHA256}\nbuild_id={BUILD_ID}\n"
    );
    let actual = std::fs::read_to_string(&attestation_path)
        .unwrap_or_else(|error| panic!("failed to read sherpa runtime attestation: {error}"));
    assert_eq!(actual, expected, "sherpa runtime attestation drift");

    let archive_path = archive_dir.join(ARCHIVE_NAME);
    let metadata = symlink_metadata_regular_file(&archive_path, "attested sherpa archive");
    assert_eq!(metadata.len(), ARCHIVE_SIZE, "attested archive size drift");
    assert_eq!(
        sha256_file(&archive_path),
        ARCHIVE_SHA256,
        "attested archive SHA-256 drift"
    );
    println!("cargo:rerun-if-changed={}", attestation_path.display());
    println!("cargo:rerun-if-changed={}", archive_path.display());
}

fn require_env(name: &str, expected: &str) {
    let actual = std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for the asr-runtime feature"));
    assert_eq!(
        actual, expected,
        "{name} does not match the trusted runtime"
    );
}

fn canonical_regular_directory_env_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name)
        .unwrap_or_else(|| panic!("{name} is required for the asr-runtime feature"));
    let path = Path::new(&value);
    let metadata = std::fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("failed to inspect {name}: {error}"));
    assert!(
        !metadata.file_type().is_symlink(),
        "{name} must not be a symlink"
    );
    assert!(metadata.is_dir(), "{name} must be a directory");
    std::fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("failed to canonicalize {name}: {error}"))
}

fn canonical_regular_file_env_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name)
        .unwrap_or_else(|| panic!("{name} is required for the asr-runtime feature"));
    let path = Path::new(&value);
    symlink_metadata_regular_file(path, name);
    std::fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("failed to canonicalize {name}: {error}"))
}

fn symlink_metadata_regular_file(path: &Path, label: &str) -> std::fs::Metadata {
    let metadata = std::fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("failed to inspect {label}: {error}"));
    assert!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    assert!(metadata.is_file(), "{label} must be a regular file");
    metadata
}

fn sha256_file(path: &Path) -> String {
    let mut file = File::open(path)
        .unwrap_or_else(|error| panic!("failed to open attested sherpa archive: {error}"));
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .unwrap_or_else(|error| panic!("failed to hash attested sherpa archive: {error}"));
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    format!("{:x}", hasher.finalize())
}
