fn anchored_direct_install_fixture(
    root: &std::path::Path,
) -> (
    ModelManager<ScriptedTransport, MemoryCatalog>,
    ModelInstallPlan,
    String,
) {
    let body = b"model";
    let plan = simple_sherpa_plan("anchored-direct", body);
    let manager = ModelManager::new_anchored(
        root,
        std::fs::File::open(root).unwrap(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        MemoryCatalog::default(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    let download_id = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap();
    (manager, plan, download_id)
}

#[test]
fn production_direct_install_stays_on_held_root_after_nominal_root_swap() {
    let parent = TempDir::new().unwrap();
    let root = parent.path().join("data");
    let held = parent.path().join("held");
    fs::create_dir(&root).unwrap();
    let (manager, plan, download_id) = anchored_direct_install_fixture(&root);

    fs::rename(&root, &held).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("sentinel"), b"replacement").unwrap();

    let installation = manager
        .install_verified_download(&plan, &download_id)
        .unwrap();

    assert_eq!(installation.install_dir, root.join("models/asr/whisper/anchored-direct/1-bundle"));
    assert_eq!(
        fs::read(held.join("models/asr/whisper/anchored-direct/1-bundle/model.onnx")).unwrap(),
        b"model"
    );
    assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"replacement");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn production_direct_install_never_replaces_competing_final_directory() {
    let root = TempDir::new().unwrap();
    let (manager, plan, download_id) = anchored_direct_install_fixture(root.path());
    let final_dir = root
        .path()
        .join("models/asr/whisper/anchored-direct/1-bundle");
    fs::create_dir_all(&final_dir).unwrap();
    fs::write(final_dir.join("sentinel"), b"competitor").unwrap();

    let error = manager
        .install_verified_download(&plan, &download_id)
        .unwrap_err();

    assert_eq!(error.code(), "model_install_conflict");
    assert_eq!(fs::read(final_dir.join("sentinel")).unwrap(), b"competitor");
}

#[test]
fn production_direct_install_rejects_staging_name_replacement_before_publish() {
    let root = TempDir::new().unwrap();
    let (manager, plan, download_id) = anchored_direct_install_fixture(root.path());
    let staging = root.path().join("staging/anchored-direct-1-download-1");
    let displaced = root.path().join("staging/displaced");
    let staging_for_hook = staging.clone();
    let displaced_for_hook = displaced.clone();
    let replacement_inode = std::rc::Rc::new(std::cell::Cell::new(0));
    let replacement_inode_for_hook = replacement_inode.clone();
    crate::asr::model_manager::set_before_install_stage_claim_for_test(move || {
        use std::os::unix::fs::MetadataExt;

        fs::rename(&staging_for_hook, &displaced_for_hook).unwrap();
        fs::create_dir(&staging_for_hook).unwrap();
        fs::write(staging_for_hook.join("sentinel"), b"competitor").unwrap();
        replacement_inode_for_hook.set(fs::metadata(&staging_for_hook).unwrap().ino());
    });

    let error = manager
        .install_verified_download(&plan, &download_id)
        .unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert_eq!(fs::read(staging.join("sentinel")).unwrap(), b"competitor");
    assert_eq!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&staging).unwrap()),
        replacement_inode.get()
    );
    assert_eq!(fs::read_dir(displaced).unwrap().count(), 0);
    assert!(
        !root
            .path()
            .join("models/asr/whisper/anchored-direct/1-bundle")
            .exists()
    );
}

#[test]
fn production_direct_install_never_overwrites_competing_structural_marker() {
    let root = TempDir::new().unwrap();
    let (manager, plan, download_id) = anchored_direct_install_fixture(root.path());
    let root_path = root.path().to_path_buf();
    crate::asr::model_manager::set_before_install_marker_publish_for_test(move || {
        let staging = root_path.join("staging");
        let claim = fs::read_dir(staging)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".lifesub-stage-"))
            })
            .unwrap();
        fs::write(claim.join(".lifesub-structural.json"), b"sentinel").unwrap();
    });

    let error = manager
        .install_verified_download(&plan, &download_id)
        .unwrap_err();

    assert_eq!(error.code(), "model_install_conflict");
    assert!(
        !root
            .path()
            .join("models/asr/whisper/anchored-direct/1-bundle")
            .exists()
    );
    let claim = fs::read_dir(root.path().join("staging"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.join(".lifesub-structural.json").exists())
        .unwrap();
    assert_eq!(
        fs::read(claim.join(".lifesub-structural.json")).unwrap(),
        b"sentinel"
    );
}

#[test]
fn anchored_directory_creation_syncs_each_parent_and_deepest_directory() {
    let root = TempDir::new().unwrap();
    crate::asr::model_manager::clear_ensure_dir_sync_trace_for_test();
    let manager = ModelManager::new_anchored(
        root.path(),
        std::fs::File::open(root.path()).unwrap(),
        ScriptedTransport::default(),
        MemoryCatalog::default(),
    );

    manager
        .ensure_anchored_directory_for_test(std::path::Path::new("one/two/three"))
        .unwrap();

    assert_eq!(
        crate::asr::model_manager::take_ensure_dir_sync_trace_for_test(),
        vec!["", "one", "one/two", "one/two/three"]
    );
}
