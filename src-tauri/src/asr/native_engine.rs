use crate::asr::audio::{WorkingAudio, decode_to_working_audio_from_file};
use crate::asr::job::{Clock, ExecutionSnapshot, VadExecutionSnapshot};
use crate::asr::model_manager::ExecutableInstallationLease;
use crate::asr::provider::{
    AsrProvider, AudioSlice, CancellationToken, ProductionProviderFactory, Provider,
    ProviderSelection,
};
use crate::asr::receipt::ProviderReceiptError;
use crate::asr::vad::{EvidenceUtterance, partition_without_vad};
#[cfg(feature = "asr-runtime")]
use crate::asr::vad::{
    SherpaSpeechDetector, SpeechDetector, VadRuntimeConfig, VerifiedVadModel,
    partition_detector_cores,
};
use crate::asr::worker::{
    ExecutionEngine, ExecutionFailure, ExecutionRun, ExecutionSuccess, RunPollResult,
};
use crate::domain::AsrErrorCode;
use crate::domain::{AudioSource, TranscriptSegmentPublication};
use crate::domain::{DataDestination, ProviderOutcome, ProviderReceipt, ProviderReceiptDraft};
use crate::service::CoreRuntime;
use chrono::{DateTime, Utc};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, RecvTimeoutError},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeAsrEngineError {
    ModelIdentityMismatch,
    ModelLeaseUnavailable,
    InputIdentityMismatch,
    InputUnavailable,
    Provider { code: AsrErrorCode },
    Cancelled,
}

pub(crate) struct NativeAsrEngine;

pub(crate) struct ProductionNativeAsrEngine {
    runtime: Arc<CoreRuntime>,
}

pub(crate) struct NativePrepared {
    pub(crate) snapshot: ExecutionSnapshot,
    pub(crate) prepared: Option<PreparedNativeAsr>,
}

pub(crate) struct NativeExecutionRun {
    receiver: Receiver<Result<ExecutionSuccess, ExecutionFailure>>,
    cancellation: CancellationToken,
}

impl ExecutionRun for NativeExecutionRun {
    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Result<ExecutionSuccess, ExecutionFailure>, RunPollResult> {
        if self.cancellation.is_cancelled() {
            return Ok(Err(ExecutionFailure {
                code: AsrErrorCode::Cancelled,
                summary: "native ASR run cancelled".to_owned(),
            }));
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => Ok(result),
            Err(RecvTimeoutError::Timeout) => Err(RunPollResult::Pending),
            Err(RecvTimeoutError::Disconnected) => Ok(Err(ExecutionFailure {
                code: AsrErrorCode::RecoveryRequired,
                summary: "native ASR run exited without a result".to_owned(),
            })),
        }
    }

    fn cancel(&self) {
        self.cancellation.cancel();
    }
}

impl ProductionNativeAsrEngine {
    pub(crate) fn new(runtime: Arc<CoreRuntime>) -> Self {
        Self { runtime }
    }
}

impl<C: Clock> ExecutionEngine<C> for ProductionNativeAsrEngine {
    type Prepared = NativePrepared;
    type Run = NativeExecutionRun;

    fn prepare(&self, snapshot: &ExecutionSnapshot) -> Result<Self::Prepared, ExecutionFailure> {
        Ok(NativePrepared {
            snapshot: snapshot.clone(),
            prepared: None,
        })
    }

    fn start(
        &self,
        mut prepared: Self::Prepared,
        started_at: DateTime<Utc>,
    ) -> Result<Self::Run, ExecutionFailure> {
        let (sender, receiver) = mpsc::channel();
        let cancellation = CancellationToken::new();
        let cancellation_for_thread = cancellation.clone();
        let runtime = self.runtime.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let mut native = match prepared.prepared.take() {
                    Some(prepared) => prepared,
                    None => NativeAsrEngine::prepare(&runtime, &prepared.snapshot)
                        .map_err(native_failure)?,
                };
                if cancellation_for_thread.is_cancelled() {
                    return Err(native_failure(NativeAsrEngineError::Cancelled));
                }
                let texts = native
                    .transcribe_all_windows(&cancellation_for_thread, || Ok(()))
                    .map_err(native_failure)?;
                native
                    .build_success(&prepared.snapshot, texts, started_at, Utc::now())
                    .map_err(native_failure)
            })();
            if cancellation_for_thread.is_cancelled() {
                let _ = sender.send(Err(ExecutionFailure {
                    code: AsrErrorCode::Cancelled,
                    summary: "native ASR run cancelled".to_owned(),
                }));
            } else {
                let _ = sender.send(result);
            }
        });
        Ok(NativeExecutionRun {
            receiver,
            cancellation,
        })
    }
}

fn native_failure(error: NativeAsrEngineError) -> ExecutionFailure {
    let code = match error {
        NativeAsrEngineError::Cancelled => AsrErrorCode::Cancelled,
        NativeAsrEngineError::InputIdentityMismatch => AsrErrorCode::InputIntegrityFailed,
        NativeAsrEngineError::InputUnavailable => AsrErrorCode::InputUnavailable,
        NativeAsrEngineError::ModelLeaseUnavailable => AsrErrorCode::ModelCapabilityUnavailable,
        NativeAsrEngineError::ModelIdentityMismatch => AsrErrorCode::ModelIntegrityFailed,
        NativeAsrEngineError::Provider { code } => code,
    };
    ExecutionFailure {
        code,
        summary: format!("native ASR engine failed: {error:?}"),
    }
}

pub(crate) struct PreparedNativeAsr {
    pub(crate) provider: Provider,
    pub(crate) audio: WorkingAudio,
    pub(crate) utterances: Vec<EvidenceUtterance>,
    pub(crate) executed_vad: Option<VadExecutionSnapshot>,
}

impl PreparedNativeAsr {
    pub(crate) fn build_success(
        &self,
        snapshot: &ExecutionSnapshot,
        texts: Vec<String>,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> Result<ExecutionSuccess, NativeAsrEngineError> {
        if snapshot.vad != self.executed_vad {
            return Err(NativeAsrEngineError::ModelIdentityMismatch);
        }
        if texts.len() != self.utterances.len() {
            return Err(NativeAsrEngineError::InputUnavailable);
        }
        let mut segments = Vec::with_capacity(texts.len());
        for (utterance, text) in self.utterances.iter().zip(texts) {
            segments.push(self.publication(
                snapshot.chunk.source,
                &snapshot.chunk.id,
                utterance,
                text,
            )?);
        }
        let receipt = self
            .receipt(snapshot, started_at, finished_at)
            .map_err(|_| NativeAsrEngineError::Provider {
                code: AsrErrorCode::ReceiptInvalid,
            })?;
        Ok(ExecutionSuccess { receipt, segments })
    }

    pub(crate) fn transcribe_all_windows<F>(
        &mut self,
        token: &CancellationToken,
        mut renew: F,
    ) -> Result<Vec<String>, NativeAsrEngineError>
    where
        F: FnMut() -> Result<(), NativeAsrEngineError>,
    {
        let mut texts = Vec::with_capacity(self.utterances.len());
        for utterance_index in 0..self.utterances.len() {
            let windows = self.utterances[utterance_index].windows.clone();
            if windows.is_empty() {
                return Err(NativeAsrEngineError::InputUnavailable);
            }
            let mut window_texts = Vec::with_capacity(windows.len());
            for window in windows {
                if token.is_cancelled() {
                    return Err(NativeAsrEngineError::Cancelled);
                }
                renew()?;
                window_texts.push(self.transcribe_window(
                    window.inference.start,
                    window.inference.end,
                    token,
                )?);
            }
            texts.push(window_texts.join(" "));
        }
        Ok(texts)
    }

    pub(crate) fn receipt(
        &self,
        snapshot: &ExecutionSnapshot,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> Result<ProviderReceipt, ProviderReceiptError> {
        let identity = self.provider.identity();
        let vad = self.executed_vad.as_ref();
        ProviderReceiptDraft {
            job_id: snapshot.job_id.clone(),
            chunk_id: snapshot.chunk.id.clone(),
            provider: identity.provider,
            model_id: snapshot.model.model_id.clone(),
            manifest_version: snapshot.model.manifest_version.clone(),
            archive_sha256: snapshot.model.bundle_identity.clone(),
            required_file_hashes_json: snapshot.model.required_file_hashes_json.clone(),
            model_source_json: snapshot.model.model_source_json.clone(),
            vad_model_id: vad.map(|vad| vad.model_id.clone()),
            vad_manifest_version: vad.map(|vad| vad.manifest_version.clone()),
            vad_archive_sha256: vad.map(|vad| vad.bundle_identity.clone()),
            vad_required_file_hashes_json: vad.map(|vad| vad.required_file_hashes_json.clone()),
            runtime_version: identity.execution.version.clone(),
            runtime_build_id: identity
                .execution
                .build_id
                .clone()
                .unwrap_or_else(|| identity.execution.runtime_name.clone()),
            parameters_json: snapshot.parameters.json.clone(),
            input_sha256: snapshot.chunk.sha256.clone(),
            started_at,
            finished_at,
            data_destination: DataDestination::LocalDevice,
            outcome: ProviderOutcome::Succeeded,
        }
        .try_into()
    }

    pub(crate) fn publication(
        &self,
        chunk_source: AudioSource,
        chunk_id: &str,
        utterance: &EvidenceUtterance,
        text: String,
    ) -> Result<TranscriptSegmentPublication, NativeAsrEngineError> {
        let start = utterance.evidence.start;
        let end = utterance.evidence.end;
        if text.trim().is_empty() || start >= end {
            return Err(NativeAsrEngineError::InputUnavailable);
        }
        let start_ms =
            i64::try_from(start / 16).map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        let end_ms =
            i64::try_from(end.div_ceil(16)).map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        Ok(TranscriptSegmentPublication {
            id: format!("seg_{}_{}_{}", chunk_id, start, end),
            chunk_start_ms: start_ms,
            chunk_end_ms: end_ms,
            source: chunk_source,
            text,
        })
    }

    pub(crate) fn transcribe_window(
        &mut self,
        start: u64,
        end: u64,
        token: &CancellationToken,
    ) -> Result<String, NativeAsrEngineError> {
        if token.is_cancelled() {
            return Err(NativeAsrEngineError::Cancelled);
        }
        let start = usize::try_from(start).map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        let end = usize::try_from(end).map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        if start >= end || end > self.audio.samples.len() {
            return Err(NativeAsrEngineError::InputUnavailable);
        }
        let slice = AudioSlice::new(&self.audio.samples[start..end], self.audio.sample_rate_hz)
            .map_err(|error| NativeAsrEngineError::Provider { code: error.code() })?;
        self.provider.transcribe(slice, token).map_err(|error| {
            if error.code() == AsrErrorCode::Cancelled {
                NativeAsrEngineError::Cancelled
            } else {
                NativeAsrEngineError::Provider { code: error.code() }
            }
        })
    }
}

impl NativeAsrEngine {
    pub(crate) fn selection_from_snapshot(snapshot: &ExecutionSnapshot) -> ProviderSelection {
        ProviderSelection::from_snapshot(
            snapshot.parameters.language.as_str(),
            snapshot.parameters.num_threads,
            snapshot.parameters.options.clone(),
        )
    }

    pub(crate) fn prepare(
        runtime: &CoreRuntime,
        snapshot: &ExecutionSnapshot,
    ) -> Result<PreparedNativeAsr, NativeAsrEngineError> {
        let installation = runtime
            .executable_installation(&snapshot.model.model_id)
            .map_err(|_| NativeAsrEngineError::ModelLeaseUnavailable)?;
        Self::validate_snapshot_identity(snapshot, &installation)?;
        let provider = ProductionProviderFactory::new()
            .create(installation, Self::selection_from_snapshot(snapshot))
            .map_err(|_| NativeAsrEngineError::ModelLeaseUnavailable)?;
        let chunk = runtime
            .catalog_ref()
            .chunk(&snapshot.chunk.id)
            .map_err(|_| NativeAsrEngineError::InputUnavailable)?
            .ok_or(NativeAsrEngineError::InputUnavailable)?;
        if chunk.sha256 != snapshot.chunk.sha256 || chunk.byte_length != snapshot.chunk.byte_length
        {
            return Err(NativeAsrEngineError::InputIdentityMismatch);
        }
        let file = runtime
            .open_verified_audio_chunk(&chunk)
            .map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        let extension = std::path::Path::new(&snapshot.chunk.relative_path)
            .extension()
            .and_then(|value| value.to_str());
        let audio = decode_to_working_audio_from_file(file, extension)
            .map_err(|_| NativeAsrEngineError::InputUnavailable)?;
        let (utterances, executed_vad) = Self::prepare_utterances(runtime, snapshot, &audio)?;
        Ok(PreparedNativeAsr {
            provider,
            audio,
            utterances,
            executed_vad,
        })
    }

    fn prepare_utterances(
        runtime: &CoreRuntime,
        snapshot: &ExecutionSnapshot,
        audio: &WorkingAudio,
    ) -> Result<(Vec<EvidenceUtterance>, Option<VadExecutionSnapshot>), NativeAsrEngineError> {
        let Some(vad_snapshot) = snapshot.vad.as_ref() else {
            let utterance = partition_without_vad(&audio.samples)
                .map_err(|_| NativeAsrEngineError::InputUnavailable)?;
            return Ok((vec![utterance], None));
        };

        #[cfg(feature = "asr-runtime")]
        {
            let installation = runtime
                .executable_installation(&vad_snapshot.model_id)
                .map_err(|_| NativeAsrEngineError::ModelLeaseUnavailable)?;
            Self::validate_vad_snapshot_identity(vad_snapshot, &installation)?;
            let model = VerifiedVadModel::new(installation)
                .map_err(|_| NativeAsrEngineError::ModelLeaseUnavailable)?;
            let config = VadRuntimeConfig::from_manifest(crate::asr::manifest::vad_manifest())
                .map_err(|_| NativeAsrEngineError::ModelIdentityMismatch)?;
            let mut detector = SherpaSpeechDetector::new(model, config)
                .map_err(|_| NativeAsrEngineError::ModelLeaseUnavailable)?;
            let cores = detector
                .detect(&audio.samples)
                .map_err(|_| NativeAsrEngineError::InputUnavailable)?;
            let utterances = partition_detector_cores(&audio.samples, &cores)
                .map_err(|_| NativeAsrEngineError::InputUnavailable)?;
            Ok((utterances, Some(vad_snapshot.clone())))
        }

        #[cfg(not(feature = "asr-runtime"))]
        {
            let _ = (runtime, vad_snapshot);
            Err(NativeAsrEngineError::ModelLeaseUnavailable)
        }
    }

    fn validate_vad_snapshot_identity(
        snapshot: &VadExecutionSnapshot,
        installation: &ExecutableInstallationLease,
    ) -> Result<(), NativeAsrEngineError> {
        if snapshot.model_id != installation.model_id()
            || snapshot.manifest_version != installation.manifest_version()
            || snapshot.bundle_identity != installation.bundle_identity()
        {
            return Err(NativeAsrEngineError::ModelIdentityMismatch);
        }
        Ok(())
    }

    pub(crate) fn validate_snapshot_identity(
        snapshot: &ExecutionSnapshot,
        installation: &ExecutableInstallationLease,
    ) -> Result<(), NativeAsrEngineError> {
        if snapshot.model.model_id != installation.model_id()
            || snapshot.model.manifest_version != installation.manifest_version()
            || snapshot.model.bundle_identity != installation.bundle_identity()
        {
            return Err(NativeAsrEngineError::ModelIdentityMismatch);
        }
        if snapshot.chunk.sha256.len() != 64
            || !snapshot
                .chunk
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(NativeAsrEngineError::InputIdentityMismatch);
        }
        Ok(())
    }
}
