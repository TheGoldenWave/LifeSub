fn compressed_tar(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
    let mut builder = tar::Builder::new(encoder);
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_mode(0o644);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        builder.append_data(&mut header, path, *bytes).unwrap();
    }
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}

fn traversal_tar() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_size(1);
    header.set_cksum();
    builder
        .append_data(&mut header, "root/safe", b"x".as_slice())
        .unwrap();
    let mut bytes = builder.into_inner().unwrap();
    bytes[..100].fill(0);
    bytes[..9].copy_from_slice(b"../escape");
    bytes[148..156].fill(b' ');
    let checksum = bytes[..512]
        .iter()
        .map(|byte| u64::from(*byte))
        .sum::<u64>();
    let encoded = format!("{checksum:06o}\0 ");
    bytes[148..156].copy_from_slice(encoded.as_bytes());
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
    encoder.write_all(&bytes).unwrap();
    encoder.finish().unwrap()
}

fn archive_contract(
    max_scanned_entries: u64,
    max_written_file_bytes: u64,
    max_total_written_bytes: u64,
    required: &[(&str, &[u8])],
) -> InstallContract {
    InstallContract::Archive {
        archive_root: "root".to_owned(),
        max_scanned_entries,
        max_written_file_bytes,
        max_total_written_bytes,
        required_files: required
            .iter()
            .map(|(path, bytes)| RequiredInstalledFile {
                path: (*path).to_owned(),
                bytes: bytes.len() as u64,
                sha256: hex::encode(sha2::Sha256::digest(bytes)),
            })
            .collect(),
    }
}

#[test]
fn archive_symlink_and_hardlink_entries_are_rejected() {
    for kind in [tar::EntryType::Symlink, tar::EntryType::Link] {
        let directory = TempDir::new().unwrap();
        let archive = directory.path().join("archive.tar.bz2");
        fs::write(&archive, compressed_tar(&[("root/unsafe", kind, b"")])).unwrap();
        let destination = directory.path().join("out");
        fs::create_dir(&destination).unwrap();

        let contract = archive_contract(1, 1, 1, &[("required.bin", b"x")]);
        let error = extract_tar_bz2_safely(&archive, &destination, &contract).unwrap_err();

        assert_eq!(error.code(), "model_structural_incompatible");
    }
}

#[test]
fn archive_path_traversal_entry_is_rejected() {
    let directory = TempDir::new().unwrap();
    let archive = directory.path().join("archive.tar.bz2");
    fs::write(&archive, traversal_tar()).unwrap();
    let destination = directory.path().join("out");
    fs::create_dir(&destination).unwrap();
    let contract = archive_contract(1, 1, 1, &[("safe", b"x")]);

    let error = extract_tar_bz2_safely(&archive, &destination, &contract).unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert!(!directory.path().join("escape").exists());
}

#[test]
fn archive_duplicate_paths_are_rejected() {
    let directory = TempDir::new().unwrap();
    let archive = directory.path().join("archive.tar.bz2");
    fs::write(
        &archive,
        compressed_tar(&[
            ("root/same.bin", tar::EntryType::Regular, b"a"),
            ("root/same.bin", tar::EntryType::Regular, b"b"),
        ]),
    )
    .unwrap();
    let destination = directory.path().join("out");
    fs::create_dir(&destination).unwrap();

    let contract = archive_contract(2, 1, 1, &[("same.bin", b"a")]);
    let error = extract_tar_bz2_safely(&archive, &destination, &contract).unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
}

#[test]
fn archive_expansion_limit_is_enforced_before_write() {
    let directory = TempDir::new().unwrap();
    let archive = directory.path().join("archive.tar.bz2");
    fs::write(
        &archive,
        compressed_tar(&[("root/large.bin", tar::EntryType::Regular, b"12345")]),
    )
    .unwrap();
    let destination = directory.path().join("out");
    fs::create_dir(&destination).unwrap();

    let contract = archive_contract(1, 4, 5, &[("large.bin", b"12345")]);
    let error = extract_tar_bz2_safely(&archive, &destination, &contract).unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert!(!destination.join("large.bin").exists());
}

fn exact_small_archive() -> (Vec<u8>, InstallContract) {
    let model = b"abc".as_slice();
    let tokens = b"de".as_slice();
    (
        compressed_tar(&[
            ("root", tar::EntryType::Directory, b""),
            ("root/model.onnx", tar::EntryType::Regular, model),
            ("root/tokens.txt", tar::EntryType::Regular, tokens),
            ("root/README.md", tar::EntryType::Regular, b"skip"),
        ]),
        archive_contract(4, 3, 5, &[("model.onnx", model), ("tokens.txt", tokens)]),
    )
}

#[test]
fn archive_scans_all_entries_but_writes_only_exact_whitelist() {
    let directory = TempDir::new().unwrap();
    let archive = directory.path().join("archive.tar.bz2");
    let (bytes, contract) = exact_small_archive();
    fs::write(&archive, bytes).unwrap();
    let destination = directory.path().join("out");
    fs::create_dir(&destination).unwrap();

    let written = extract_tar_bz2_safely(&archive, &destination, &contract).unwrap();

    assert_eq!(written, 5);
    assert_eq!(fs::read(destination.join("model.onnx")).unwrap(), b"abc");
    assert_eq!(fs::read(destination.join("tokens.txt")).unwrap(), b"de");
    assert!(!destination.join("README.md").exists());
    assert!(!destination.join("root").exists());
}

#[test]
fn archive_contract_drift_fails_closed_for_each_bound_and_required_field() {
    let directory = TempDir::new().unwrap();
    let archive = directory.path().join("archive.tar.bz2");
    let (bytes, contract) = exact_small_archive();
    fs::write(&archive, bytes).unwrap();

    let mut mutations = Vec::new();
    let mut entries = contract.clone();
    if let InstallContract::Archive {
        max_scanned_entries,
        ..
    } = &mut entries
    {
        *max_scanned_entries = 3;
    }
    mutations.push(entries);
    let mut per_file = contract.clone();
    if let InstallContract::Archive {
        max_written_file_bytes,
        ..
    } = &mut per_file
    {
        *max_written_file_bytes = 2;
    }
    mutations.push(per_file);
    let mut total = contract.clone();
    if let InstallContract::Archive {
        max_total_written_bytes,
        ..
    } = &mut total
    {
        *max_total_written_bytes = 4;
    }
    mutations.push(total);
    let mut path = contract.clone();
    path.required_files_mut()[0].path = "missing.onnx".to_owned();
    mutations.push(path);
    let mut size = contract.clone();
    size.required_files_mut()[0].bytes = 4;
    mutations.push(size);
    let mut hash = contract;
    hash.required_files_mut()[0].sha256 = "0".repeat(64);
    mutations.push(hash);

    for (index, mutation) in mutations.into_iter().enumerate() {
        let destination = directory.path().join(format!("out-{index}"));
        fs::create_dir(&destination).unwrap();
        assert!(
            extract_tar_bz2_safely(&archive, &destination, &mutation).is_err(),
            "accepted mutation {index}"
        );
        assert!(!destination.join(".lifesub-structural.json").exists());
    }
}

fn response(url: &str, body: &[u8]) -> Result<DownloadResponse, ManagerError> {
    Ok(DownloadResponse {
        status: 200,
        final_url: url.to_owned(),
        headers: [("content-length".to_owned(), body.len().to_string())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(body.to_vec())),
    })
}

fn direct_artifact(id: &str, path: &str, body: &[u8]) -> ArtifactPlan {
    ArtifactPlan {
        artifact_id: id.to_owned(),
        source_repository: "repo".to_owned(),
        source_model: "model".to_owned(),
        url: format!("http://127.0.0.1/{id}"),
        revision: "revision".to_owned(),
        expected_bytes: body.len() as u64,
        expected_sha256: hex::encode(sha2::Sha256::digest(body)),
        required_path: path.to_owned(),
        install_mode: InstallMode::Direct,
        redirect_hosts: vec!["127.0.0.1".to_owned()],
        license_spdx: "Apache-2.0".to_owned(),
        provenance: "fixture".to_owned(),
    }
}

fn simple_sherpa_plan(model_id: &str, body: &[u8]) -> ModelInstallPlan {
    let artifact = direct_artifact("model", "model.onnx", body);
    ModelInstallPlan {
        model_id: model_id.to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact],
    }
}

fn sherpa_identity() -> FullSherpaRuntimeIdentity {
    let pinned = crate::asr::pinned_sherpa_runtime_identity();
    FullSherpaRuntimeIdentity {
        version: pinned.version.to_owned(),
        git_commit: pinned.git_commit.to_owned(),
        native_archive_sha256: pinned.native_archive_sha256.to_owned(),
        build_id: pinned.build_id.to_owned(),
    }
}
