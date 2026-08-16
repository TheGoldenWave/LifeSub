use std::ffi::CString;
use std::fs::{self, File};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::asr::manifest::{ModelManifest, RuntimeRequirement, model_registry};
use crate::asr::model_manager::{DeviceProfile, ExecutableInstallationLease};
pub use crate::asr::settings::AsrProviderOptions as ProviderOptions;
use crate::domain::{AsrErrorCode, AsrProviderKind};

const REQUIRED_SAMPLE_RATE_HZ: u32 = 16_000;
const MAX_AUDIO_SAMPLES: usize = 25 * REQUIRED_SAMPLE_RATE_HZ as usize;
const QWEN06_MODEL_ID: &str = "qwen3-asr-0.6b-int8-2026-03-25";
const QWEN06_TOKENIZER_FILES: &[&str] = &[
    "tokenizer/merges.txt",
    "tokenizer/tokenizer_config.json",
    "tokenizer/vocab.json",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendKind {
    SenseVoiceSherpa,
    WhisperSherpa,
    Qwen06Sherpa,
    Qwen17CandleMetal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFamily {
    SherpaOnnx,
    QwenCandleMetal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    pub os: String,
    pub arch: String,
    pub backend: String,
    pub device_index: u16,
    pub macos_major: u16,
    pub memory_gib: u16,
    pub chip: String,
}

impl DeviceIdentity {
    pub fn current() -> Self {
        let profile = DeviceProfile::current();
        let backend = if profile.metal_available {
            "metal"
        } else {
            "unsupported"
        };
        Self::from_profile(&profile, backend)
    }

    fn is_qwen17_device(&self) -> bool {
        self.os == "macos"
            && self.arch == "aarch64"
            && self.backend == "metal"
            && self.device_index == 0
            && self.macos_major >= 14
            && self.memory_gib >= 24
            && self.chip == "M4"
    }

    fn from_profile(profile: &DeviceProfile, backend: &str) -> Self {
        Self {
            os: profile.os.clone(),
            arch: profile.arch.clone(),
            backend: backend.to_owned(),
            device_index: 0,
            macos_major: profile.macos_major,
            memory_gib: profile.memory_gib,
            chip: profile.chip.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QualificationEvidence {
    pub marker_identity: String,
    pub device: DeviceIdentity,
}

impl QualificationEvidence {
    #[cfg(test)]
    pub(crate) fn matching(manifest: &ModelManifest, device: DeviceIdentity) -> Self {
        Self {
            marker_identity: qualification_identity(manifest, &device),
            device,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderRequest {
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub language: String,
    pub num_threads: u16,
    pub options: ProviderOptions,
    pub qualification: QualificationEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSelection {
    pub language: String,
    pub num_threads: u16,
    pub options: ProviderOptions,
}

impl ProviderSelection {
    pub fn new(language: impl Into<String>, num_threads: u16, options: ProviderOptions) -> Self {
        Self {
            language: language.into(),
            num_threads,
            options,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(model_id: &str) -> Self {
        let manifest = model_registry().model(model_id).unwrap();
        Self {
            language: "auto".to_owned(),
            num_threads: 2,
            options: match manifest.provider {
                AsrProviderKind::SenseVoice => ProviderOptions::SenseVoice { use_itn: true },
                AsrProviderKind::Whisper => ProviderOptions::Whisper {
                    task: crate::asr::settings::WhisperTask::Transcribe,
                },
                AsrProviderKind::Qwen3Asr => ProviderOptions::Qwen3Asr,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeRequest {
    pub backend: BackendKind,
    pub runtime: RuntimeFamily,
    pub install_dir: PathBuf,
    pub required_files: Vec<PathBuf>,
    pub language: Option<String>,
    pub use_itn: Option<bool>,
    pub whisper_task: Option<String>,
    pub num_threads: u16,
    pub device: DeviceIdentity,
    pub runtime_identity: RuntimeExecutionIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionIdentity {
    pub runtime_name: String,
    pub version: String,
    pub git_commit: String,
    pub native_archive_sha256: Option<String>,
    pub build_id: Option<String>,
    pub backend: String,
    pub target_os: String,
    pub target_arch: String,
    pub device_index: Option<u16>,
}

impl RuntimeExecutionIdentity {
    pub(crate) fn sherpa() -> Self {
        let pinned = crate::asr::pinned_sherpa_runtime_identity();
        Self {
            runtime_name: "sherpa-onnx".to_owned(),
            version: pinned.version.to_owned(),
            git_commit: pinned.git_commit.to_owned(),
            native_archive_sha256: Some(pinned.native_archive_sha256.to_owned()),
            build_id: Some(pinned.build_id.to_owned()),
            backend: "cpu".to_owned(),
            target_os: std::env::consts::OS.to_owned(),
            target_arch: std::env::consts::ARCH.to_owned(),
            device_index: None,
        }
    }

    pub(crate) fn qwen17(device: &DeviceIdentity) -> Self {
        Self {
            runtime_name: "qwen3-asr+candle-core".to_owned(),
            version: "qwen3-asr/0.2.2;candle-core/0.9.2".to_owned(),
            git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc".to_owned(),
            native_archive_sha256: None,
            build_id: None,
            backend: "metal".to_owned(),
            target_os: device.os.clone(),
            target_arch: device.arch.clone(),
            device_index: Some(device.device_index),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderIdentity {
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub backend: BackendKind,
    pub runtime: RuntimeFamily,
    pub device: DeviceIdentity,
    pub execution: RuntimeExecutionIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderError {
    code: AsrErrorCode,
    detail: String,
}

impl ProviderError {
    pub fn new(code: AsrErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub const fn code(&self) -> AsrErrorCode {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    fn invalid(detail: impl Into<String>) -> Self {
        Self::new(AsrErrorCode::InvalidProviderParameter, detail)
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ProviderError {}

#[derive(Clone, Copy, Debug)]
pub struct AudioSlice<'a> {
    samples: &'a [f32],
    sample_rate_hz: u32,
}

impl<'a> AudioSlice<'a> {
    pub fn new(samples: &'a [f32], sample_rate_hz: u32) -> Result<Self, ProviderError> {
        if sample_rate_hz != REQUIRED_SAMPLE_RATE_HZ
            || samples.is_empty()
            || samples.len() > MAX_AUDIO_SAMPLES
        {
            return Err(ProviderError::invalid(
                "audio must be non-empty 16 kHz PCM and at most 25 seconds",
            ));
        }
        Ok(Self {
            samples,
            sample_rate_hz,
        })
    }

    pub const fn samples(self) -> &'a [f32] {
        self.samples
    }

    pub const fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(crate) trait NativeBackend: Send {
    fn transcribe(&mut self, audio: AudioSlice<'_>) -> Result<String, ProviderError>;
}

pub(crate) trait NativeBackendFactory: Clone + Send + Sync + 'static {
    fn create(&self, request: &NativeRequest) -> Result<Box<dyn NativeBackend>, ProviderError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct ProductionBackendFactory;

impl NativeBackendFactory for ProductionBackendFactory {
    fn create(&self, request: &NativeRequest) -> Result<Box<dyn NativeBackend>, ProviderError> {
        match request.backend {
            BackendKind::SenseVoiceSherpa => {
                #[cfg(feature = "asr-runtime")]
                {
                    create_sherpa_backend(crate::asr::sense_voice::sherpa_config(request))
                }
                #[cfg(not(feature = "asr-runtime"))]
                {
                    runtime_disabled("sherpa")
                }
            }
            BackendKind::WhisperSherpa => {
                #[cfg(feature = "asr-runtime")]
                {
                    create_sherpa_backend(crate::asr::whisper::sherpa_config(request))
                }
                #[cfg(not(feature = "asr-runtime"))]
                {
                    runtime_disabled("sherpa")
                }
            }
            BackendKind::Qwen06Sherpa => {
                #[cfg(feature = "asr-runtime")]
                {
                    create_sherpa_backend(crate::asr::qwen3_asr::sherpa_config(request))
                }
                #[cfg(not(feature = "asr-runtime"))]
                {
                    runtime_disabled("sherpa")
                }
            }
            BackendKind::Qwen17CandleMetal => create_qwen17_backend(request),
        }
    }
}

#[cfg(not(feature = "asr-runtime"))]
fn runtime_disabled(name: &str) -> Result<Box<dyn NativeBackend>, ProviderError> {
    Err(ProviderError::new(
        AsrErrorCode::ProviderInitializationFailed,
        format!("{name} runtime feature is disabled"),
    ))
}

#[cfg(feature = "asr-runtime")]
struct SherpaBackend {
    recognizer: sherpa_onnx::OfflineRecognizer,
}

#[cfg(feature = "asr-runtime")]
impl NativeBackend for SherpaBackend {
    fn transcribe(&mut self, audio: AudioSlice<'_>) -> Result<String, ProviderError> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(audio.sample_rate_hz() as i32, audio.samples());
        self.recognizer.decode(&stream);
        stream
            .get_result()
            .map(|result| result.text)
            .ok_or_else(|| {
                ProviderError::new(
                    AsrErrorCode::TranscriptionFailed,
                    "sherpa returned no result",
                )
            })
    }
}

#[cfg(feature = "asr-runtime")]
fn create_sherpa_backend(
    config: sherpa_onnx::OfflineRecognizerConfig,
) -> Result<Box<dyn NativeBackend>, ProviderError> {
    let recognizer = sherpa_onnx::OfflineRecognizer::create(&config).ok_or_else(|| {
        ProviderError::new(
            AsrErrorCode::ProviderInitializationFailed,
            "failed to create sherpa recognizer",
        )
    })?;
    Ok(Box::new(SherpaBackend { recognizer }))
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
struct Qwen17Backend {
    inference: qwen3_asr::AsrInference,
    language: Option<String>,
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
pub(crate) trait Qwen17RuntimeBoundary {
    type Device;

    fn create_metal_device(&self, device_index: usize) -> Result<Self::Device, String>;
    fn load(
        &self,
        request: &NativeRequest,
        device: Self::Device,
    ) -> Result<Box<dyn NativeBackend>, String>;
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
#[derive(Clone, Copy, Debug, Default)]
struct ProductionQwen17Runtime;

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
impl Qwen17RuntimeBoundary for ProductionQwen17Runtime {
    type Device = candle_core::Device;

    fn create_metal_device(&self, device_index: usize) -> Result<Self::Device, String> {
        if device_index != 0 {
            return Err("Qwen 1.7B requires Metal device 0".to_owned());
        }
        crate::asr::qwen3_asr::create_metal_device().map_err(|error| error.to_string())
    }

    fn load(
        &self,
        request: &NativeRequest,
        device: Self::Device,
    ) -> Result<Box<dyn NativeBackend>, String> {
        let inference = qwen3_asr::AsrInference::load(&request.install_dir, device)
            .map_err(|error| error.to_string())?;
        Ok(Box::new(Qwen17Backend {
            inference,
            language: request.language.clone(),
        }))
    }
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
impl NativeBackend for Qwen17Backend {
    fn transcribe(&mut self, audio: AudioSlice<'_>) -> Result<String, ProviderError> {
        let mut options = qwen3_asr::TranscribeOptions::default();
        options.language = self.language.clone();
        self.inference
            .transcribe_samples(audio.samples(), options)
            .map(|result| result.text)
            .map_err(|error| {
                ProviderError::new(AsrErrorCode::TranscriptionFailed, error.to_string())
            })
    }
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
fn create_qwen17_backend(request: &NativeRequest) -> Result<Box<dyn NativeBackend>, ProviderError> {
    create_qwen17_backend_with_runtime(request, &ProductionQwen17Runtime)
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
pub(crate) fn create_qwen17_backend_with_runtime<R: Qwen17RuntimeBoundary>(
    request: &NativeRequest,
    runtime: &R,
) -> Result<Box<dyn NativeBackend>, ProviderError> {
    let device = runtime
        .create_metal_device(0)
        .map_err(|error| ProviderError::new(AsrErrorCode::ProviderInitializationFailed, error))?;
    runtime
        .load(request, device)
        .map_err(|error| ProviderError::new(AsrErrorCode::ProviderInitializationFailed, error))
}

#[cfg(not(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
)))]
fn create_qwen17_backend(
    _request: &NativeRequest,
) -> Result<Box<dyn NativeBackend>, ProviderError> {
    Err(ProviderError::new(
        AsrErrorCode::ProviderInitializationFailed,
        "Qwen 1.7B Candle/Metal runtime is unavailable",
    ))
}

pub trait AsrProvider: Send {
    fn identity(&self) -> &ProviderIdentity;
    fn transcribe(
        &mut self,
        audio: AudioSlice<'_>,
        token: &CancellationToken,
    ) -> Result<String, ProviderError>;
}

pub struct Provider {
    identity: ProviderIdentity,
    backend: Box<dyn NativeBackend>,
    _execution_files: Vec<File>,
    _execution_aliases: Vec<PrivateAliasDirectory>,
    execution_lease: Option<ExecutableInstallationLease>,
}

struct PrivateAliasDirectory {
    path: PathBuf,
}

impl PrivateAliasDirectory {
    fn for_qwen06_tokenizer(
        installation: &ExecutableInstallationLease,
        execution_files: &mut Vec<File>,
    ) -> Result<Self, ProviderError> {
        let mut template = std::env::temp_dir()
            .join("lifesub-qwen06-tokenizer-XXXXXX")
            .as_os_str()
            .as_bytes()
            .to_vec();
        template.push(0);
        let created = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
        if created.is_null() {
            return Err(ProviderError::new(
                AsrErrorCode::ProviderInitializationFailed,
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let path = PathBuf::from(
            CString::from_vec_with_nul(template)
                .map_err(|_| ProviderError::invalid("invalid tokenizer alias path"))?
                .to_string_lossy()
                .into_owned(),
        );
        let alias = Self { path };
        fs::set_permissions(&alias.path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            ProviderError::new(
                AsrErrorCode::ProviderInitializationFailed,
                error.to_string(),
            )
        })?;
        for relative in QWEN06_TOKENIZER_FILES {
            let (file, fd_path) = installation
                .open_execution_path(std::path::Path::new(relative))
                .map_err(provider_storage_error)?;
            let name = std::path::Path::new(relative)
                .file_name()
                .ok_or_else(|| ProviderError::invalid("invalid tokenizer file name"))?;
            symlink(&fd_path, alias.path.join(name)).map_err(|error| {
                ProviderError::new(
                    AsrErrorCode::ProviderInitializationFailed,
                    error.to_string(),
                )
            })?;
            execution_files.push(file);
        }
        Ok(alias)
    }
}

impl Drop for PrivateAliasDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Provider {
    #[cfg(test)]
    pub(crate) fn inventory_validation_count_for_test(&self) -> usize {
        self.execution_lease
            .as_ref()
            .map_or(0, ExecutableInstallationLease::validation_count)
    }
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Provider")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl AsrProvider for Provider {
    fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    fn transcribe(
        &mut self,
        audio: AudioSlice<'_>,
        token: &CancellationToken,
    ) -> Result<String, ProviderError> {
        if token.is_cancelled() {
            return Err(ProviderError::new(
                AsrErrorCode::Cancelled,
                "cancelled before native inference",
            ));
        }
        let text = self.backend.transcribe(audio)?;
        if token.is_cancelled() {
            return Err(ProviderError::new(
                AsrErrorCode::Cancelled,
                "cancelled after native inference",
            ));
        }
        if text.trim().is_empty() {
            return Err(ProviderError::new(
                AsrErrorCode::TranscriptionFailed,
                "native inference returned empty text",
            ));
        }
        Ok(text)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProviderFactory<F> {
    backends: F,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProductionProviderFactory {
    inner: ProviderFactory<ProductionBackendFactory>,
}

impl ProductionProviderFactory {
    pub const fn new() -> Self {
        Self {
            inner: ProviderFactory::new(ProductionBackendFactory),
        }
    }

    pub fn create(
        &self,
        installation: ExecutableInstallationLease,
        selection: ProviderSelection,
    ) -> Result<Provider, ProviderError> {
        self.inner.create_verified(installation, selection)
    }
}

impl<F: NativeBackendFactory> ProviderFactory<F> {
    pub(crate) fn create_verified(
        &self,
        installation: ExecutableInstallationLease,
        selection: ProviderSelection,
    ) -> Result<Provider, ProviderError> {
        let plan = installation.plan();
        let manifest = model_registry()
            .model(&plan.model_id)
            .ok_or_else(|| ProviderError::invalid("unknown shipping model"))?;
        let backend = if matches!(manifest.runtime, RuntimeRequirement::QwenCandleMetal { .. }) {
            "metal"
        } else {
            "cpu"
        };
        let device = DeviceIdentity::from_profile(installation.device(), backend);
        if matches!(manifest.runtime, RuntimeRequirement::QwenCandleMetal { .. }) {
            let current = DeviceIdentity::current();
            if current != device || !current.is_qwen17_device() {
                return Err(ProviderError::new(
                    AsrErrorCode::ModelCapabilityUnavailable,
                    "current device no longer matches qualified Qwen device",
                ));
            }
            crate::asr::runtime_qualifier::verify_runtime_qualification_marker(
                manifest,
                installation.install_dir(),
                installation.runtime_identity_json().ok_or_else(|| {
                    ProviderError::new(
                        AsrErrorCode::ModelCapabilityUnavailable,
                        "qualified runtime identity is missing",
                    )
                })?,
                device.clone(),
            )
            .map_err(|error| {
                ProviderError::new(AsrErrorCode::ModelCapabilityUnavailable, error.to_string())
            })?;
        }
        let request = ProviderRequest {
            provider: manifest.provider,
            model_id: manifest.id.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: installation.install_dir().to_path_buf(),
            language: selection.language,
            num_threads: selection.num_threads,
            options: selection.options,
            qualification: QualificationEvidence {
                marker_identity: qualification_identity(manifest, &device),
                device,
            },
        };
        if matches!(manifest.runtime, RuntimeRequirement::QwenCandleMetal { .. }) {
            let mut provider = self.create(request)?;
            provider.execution_lease = Some(installation);
            return Ok(provider);
        }
        installation
            .revalidate_execution_boundary()
            .map_err(provider_storage_error)?;
        if !installation.is_fd_anchored() {
            let mut provider = self.create(request)?;
            provider.execution_lease = Some(installation);
            return Ok(provider);
        }
        let mut native = native_request(&request, manifest)?;
        let mut execution_files = Vec::with_capacity(native.required_files.len());
        let mut execution_aliases = Vec::new();
        let execution_paths = native
            .required_files
            .iter()
            .enumerate()
            .map(|(index, nominal)| {
                if manifest.id == QWEN06_MODEL_ID && index == 3 {
                    let alias = PrivateAliasDirectory::for_qwen06_tokenizer(
                        &installation,
                        &mut execution_files,
                    )?;
                    let path = alias.path.clone();
                    execution_aliases.push(alias);
                    return Ok(path);
                }
                let relative = nominal
                    .strip_prefix(installation.install_dir())
                    .map_err(|_| {
                        ProviderError::new(
                            AsrErrorCode::ModelIntegrityFailed,
                            "native execution path escaped the verified installation",
                        )
                    })?;
                let (file, path) = installation
                    .open_execution_path(relative)
                    .map_err(provider_storage_error)?;
                execution_files.push(file);
                Ok(path)
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        native.install_dir = installation.install_dir().to_path_buf();
        native.required_files = execution_paths;
        let backend = self
            .backends
            .create(&native)
            .map_err(|error| ProviderError::new(error.code(), error.detail().to_owned()))?;
        installation
            .revalidate_execution_boundary()
            .map_err(provider_storage_error)?;
        let mut provider = provider_from_native(
            manifest,
            native,
            backend,
            execution_files,
            execution_aliases,
        );
        provider.execution_lease = Some(installation);
        Ok(provider)
    }

    pub(crate) const fn new(backends: F) -> Self {
        Self { backends }
    }

    pub(crate) fn create(&self, request: ProviderRequest) -> Result<Provider, ProviderError> {
        let manifest = model_registry()
            .model(&request.model_id)
            .ok_or_else(|| ProviderError::invalid("unknown shipping model"))?;
        validate_identity(&request, manifest)?;
        let native = native_request(&request, manifest)?;
        let backend = self
            .backends
            .create(&native)
            .map_err(|error| ProviderError::new(error.code(), error.detail().to_owned()))?;
        Ok(provider_from_native(
            manifest,
            native,
            backend,
            Vec::new(),
            Vec::new(),
        ))
    }
}

fn native_request(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<NativeRequest, ProviderError> {
    match request.model_id.as_str() {
        "sense-voice-small-int8-2024-07-17" => {
            crate::asr::sense_voice::native_request(request, manifest)
        }
        "whisper-tiny" | "whisper-base" | "whisper-small" => {
            crate::asr::whisper::native_request(request, manifest)
        }
        "qwen3-asr-0.6b-int8-2026-03-25" | "qwen3-asr-1.7b" => {
            crate::asr::qwen3_asr::native_request(request, manifest)
        }
        _ => Err(ProviderError::invalid(
            "model has no production provider dispatch",
        )),
    }
}

fn provider_from_native(
    manifest: &ModelManifest,
    native: NativeRequest,
    backend: Box<dyn NativeBackend>,
    execution_files: Vec<File>,
    execution_aliases: Vec<PrivateAliasDirectory>,
) -> Provider {
    Provider {
        identity: ProviderIdentity {
            provider: manifest.provider,
            model_id: manifest.id.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            backend: native.backend,
            runtime: native.runtime,
            device: native.device,
            execution: native.runtime_identity,
        },
        backend,
        _execution_files: execution_files,
        _execution_aliases: execution_aliases,
        execution_lease: None,
    }
}

fn provider_storage_error(error: crate::asr::model_manager::ManagerError) -> ProviderError {
    ProviderError::new(AsrErrorCode::ModelIntegrityFailed, error.to_string())
}

fn validate_identity(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<(), ProviderError> {
    if request.provider != manifest.provider
        || request.manifest_version != manifest.manifest_version
        || request.bundle_identity != manifest.bundle.identity_sha256
        || request.install_dir.as_os_str().is_empty()
        || request.num_threads == 0
    {
        return Err(ProviderError::invalid(
            "provider/model/bundle identity mismatch",
        ));
    }
    if request.qualification.marker_identity
        != qualification_identity(manifest, &request.qualification.device)
    {
        return Err(ProviderError::new(
            AsrErrorCode::ModelCapabilityUnavailable,
            "runtime qualification is missing or stale",
        ));
    }
    if matches!(manifest.runtime, RuntimeRequirement::QwenCandleMetal { .. })
        && !request.qualification.device.is_qwen17_device()
    {
        return Err(ProviderError::new(
            AsrErrorCode::ModelCapabilityUnavailable,
            "Qwen 1.7B requires macOS arm64 Metal device 0",
        ));
    }
    Ok(())
}

pub(crate) fn qualification_identity(manifest: &ModelManifest, device: &DeviceIdentity) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        manifest.id,
        manifest.manifest_version,
        manifest.bundle.identity_sha256,
        device.os,
        device.arch,
        device.backend,
        device.device_index,
        device.macos_major,
        device.memory_gib,
        device.chip,
    )
}
