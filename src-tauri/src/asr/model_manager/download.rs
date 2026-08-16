use super::fs_support::*;
use super::*;

pub(super) fn execute_reqwest(
    t: &ReqwestTransport,
    r: &DownloadRequest,
) -> Result<DownloadResponse, ManagerError> {
    let mut current = reqwest::Url::parse(&r.url)
        .map_err(|_| ManagerError::invalid_source("invalid artifact URL"))?;
    validate_shipping(&current, &r.redirect_hosts)?;
    for _ in 0..=MAX_REDIRECTS {
        let mut b = t.client.get(current.clone());
        if let Some(s) = r.range_start {
            b = b.header(reqwest::header::RANGE, format!("bytes={s}-"));
        }
        if let Some(v) = &r.if_range {
            b = b.header(reqwest::header::IF_RANGE, v);
        }
        let response = b.send().map_err(|e| ManagerError::network(e.to_string()))?;
        if response.status().is_redirection() {
            let loc = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| ManagerError::network("redirect missing Location"))?;
            current = current
                .join(loc)
                .map_err(|_| ManagerError::invalid_source("invalid redirect URL"))?;
            validate_shipping(&current, &r.redirect_hosts)?;
            continue;
        }
        let headers = response
            .headers()
            .iter()
            .filter_map(|(n, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (n.as_str().to_ascii_lowercase(), v.to_owned()))
            })
            .collect();
        return Ok(DownloadResponse {
            status: response.status().as_u16(),
            final_url: current.to_string(),
            headers,
            body: Box::new(response),
        });
    }
    Err(ManagerError::invalid_source("too many redirects"))
}
fn allowed(u: &reqwest::Url, h: &[String]) -> Result<(), ManagerError> {
    let host = u
        .host_str()
        .ok_or_else(|| ManagerError::invalid_source("URL missing host"))?;
    if h.iter().any(|v| v.eq_ignore_ascii_case(host)) {
        Ok(())
    } else {
        Err(ManagerError::invalid_source("redirect host rejected"))
    }
}
fn validate_shipping(u: &reqwest::Url, h: &[String]) -> Result<(), ManagerError> {
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() {
        return Err(ManagerError::invalid_source(
            "shipping downloads require HTTPS",
        ));
    }
    allowed(u, h)
}
fn validate_response(v: &str, h: &[String]) -> Result<(), ManagerError> {
    let u =
        reqwest::Url::parse(v).map_err(|_| ManagerError::invalid_source("invalid response URL"))?;
    if u.scheme() == "http" && !matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "::1")) {
        return Err(ManagerError::invalid_source("non-loopback HTTP rejected"));
    }
    allowed(&u, h)
}
fn header<'a>(h: &'a BTreeMap<String, String>, n: &str) -> Option<&'a str> {
    h.get(n)
        .or_else(|| h.get(&n.to_ascii_lowercase()))
        .map(String::as_str)
}

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    pub fn new(root: impl AsRef<Path>, transport: T, catalog: C) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            anchored_root: None,
            transport,
            catalog,
            observed_sherpa_runtime: None,
            execution_leases: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            #[cfg(test)]
            available_space_override: None,
            #[cfg(test)]
            delete_marker_fault: None,
            #[cfg(test)]
            install_fault: None,
            #[cfg(test)]
            available_space_sequence: None,
        }
    }
    pub fn new_anchored(root: impl AsRef<Path>, root_dir: File, transport: T, catalog: C) -> Self {
        let mut manager = Self::new(root, transport, catalog);
        manager.anchored_root = Some(std::sync::Arc::new(root_dir));
        manager
    }
    pub fn with_sherpa_runtime_identity(mut self, v: FullSherpaRuntimeIdentity) -> Self {
        self.observed_sherpa_runtime = Some(v);
        self
    }
    #[cfg(test)]
    pub(crate) fn catalog(&self) -> &C {
        &self.catalog
    }
    #[cfg(test)]
    pub(crate) fn with_available_space_for_test(mut self, v: u64) -> Self {
        self.available_space_override = Some(v);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_available_space_sequence_for_test(mut self, v: Vec<u64>) -> Self {
        self.available_space_sequence = Some(std::sync::Arc::new(std::sync::Mutex::new(v)));
        self
    }
    #[cfg(test)]
    pub(crate) fn with_delete_marker_fault_for_test(mut self, v: DeleteMarkerFault) -> Self {
        self.delete_marker_fault = Some(v);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_install_fault_for_test(mut self, v: InstallFault) -> Self {
        self.install_fault = Some(v);
        self
    }
    #[cfg(test)]
    pub(crate) fn required_additional_free_for_test(
        &self,
        p: &ModelInstallPlan,
        d: Option<&str>,
    ) -> Result<u64, ManagerError> {
        self.required_additional_free(p, d)
    }
    pub fn download_model<F: Fn() -> bool>(
        &self,
        id: &str,
        d: &DeviceProfile,
        c: F,
    ) -> Result<String, ManagerError> {
        validate_component("model_id", id)?;
        let p = resolve_current_plan(id)?;
        self.download_only(&p, d, c)
    }
    pub fn download_and_install_model<F: Fn() -> bool>(
        &self,
        id: &str,
        d: &DeviceProfile,
        c: F,
    ) -> Result<StoredInstallation, ManagerError> {
        validate_component("model_id", id)?;
        let p = resolve_current_plan(id)?;
        self.download_and_install(&p, d, c)
    }
    pub(crate) fn download_only<F: Fn() -> bool>(
        &self,
        p: &ModelInstallPlan,
        d: &DeviceProfile,
        c: F,
    ) -> Result<String, ManagerError> {
        validate_device(&p.device, d)?;
        validate_plan(p)?;
        self.preflight_disk(p, None)?;
        let id = self.catalog.begin_download(p)?;
        self.download_existing(p, &id, c)?;
        Ok(id)
    }
    pub(crate) fn retry_download<F: Fn() -> bool>(
        &self,
        p: &ModelInstallPlan,
        id: &str,
        d: &DeviceProfile,
        c: F,
    ) -> Result<(), ManagerError> {
        validate_component("download_id", id)?;
        validate_device(&p.device, d)?;
        validate_plan(p)?;
        self.preflight_disk(p, Some(id))?;
        self.download_existing(p, id, c)
    }
    pub fn retry_model_download<F: Fn() -> bool>(
        &self,
        m: &str,
        id: &str,
        d: &DeviceProfile,
        c: F,
    ) -> Result<(), ManagerError> {
        validate_component("model_id", m)?;
        self.retry_download(&resolve_current_plan(m)?, id, d, c)
    }
    fn download_existing<F: Fn() -> bool>(
        &self,
        p: &ModelInstallPlan,
        id: &str,
        c: F,
    ) -> Result<(), ManagerError> {
        fs::create_dir_all(self.download_dir(id))?;
        self.catalog.set_download_state(id, "downloading", None)?;
        for a in &p.artifacts {
            if let Err(e) = self.download_artifact(id, a, &c) {
                let s = if e.code() == "model_download_cancelled" {
                    "cancelled"
                } else {
                    "failed"
                };
                self.catalog.set_download_state(id, s, Some(e.code()))?;
                return Err(e);
            }
        }
        self.catalog.set_download_state(id, "verifying", None)?;
        Ok(())
    }
    fn download_artifact<F: Fn() -> bool>(
        &self,
        id: &str,
        a: &ArtifactPlan,
        c: &F,
    ) -> Result<(), ManagerError> {
        let path = self
            .download_dir(id)
            .join(format!("{}.part", a.artifact_id));
        let identity = source_identity(a);
        let cp = self.catalog.checkpoint(id, &a.artifact_id)?;
        let valid = cp.as_ref().filter(|x| {
            x.source_identity == identity
                && x.expected_bytes == a.expected_bytes
                && x.temp_path == path
        });
        let mut offset = valid.map(|x| x.downloaded_bytes).unwrap_or(0);
        let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if valid.is_none() {
            if len > 0 {
                let f = OpenOptions::new().write(true).open(&path)?;
                f.set_len(0)?;
                f.sync_all()?;
            }
            offset = 0;
        } else if len > offset {
            let f = OpenOptions::new().write(true).open(&path)?;
            f.set_len(offset)?;
            f.sync_all()?;
        } else if len < offset {
            offset = len;
            self.save_progress(
                id,
                a,
                &identity,
                &path,
                offset,
                valid.and_then(|x| x.etag.as_deref()),
                valid.and_then(|x| x.last_modified.as_deref()),
                "downloading",
            )?;
        }
        if offset == a.expected_bytes && sha256_file(&path)? == a.expected_sha256 {
            self.save_verified(id, a, path)?;
            return Ok(());
        }
        if offset == a.expected_bytes {
            offset = 0;
        }
        let old_etag = cp.as_ref().and_then(|x| x.etag.clone());
        let old_mod = cp.as_ref().and_then(|x| x.last_modified.clone());
        let mut start = offset;
        for attempt in 0..2 {
            if c() {
                return Err(ManagerError::new(
                    "model_download_cancelled",
                    "download cancelled",
                ));
            }
            let req = DownloadRequest {
                url: a.url.clone(),
                range_start: (start > 0).then_some(start),
                if_range: (start > 0)
                    .then_some(old_etag.clone().or(old_mod.clone()))
                    .flatten(),
                redirect_hosts: a.redirect_hosts.clone(),
            };
            let mut r = self.transport.execute(&req)?;
            validate_response(&r.final_url, &a.redirect_hosts)?;
            let etag = header(&r.headers, "etag").map(str::to_owned);
            let modified = header(&r.headers, "last-modified").map(str::to_owned);
            let changed = r.status == 206
                && start > 0
                && ((old_etag.is_some() && old_etag != etag)
                    || (old_etag.is_none() && old_mod.is_some() && old_mod != modified));
            if changed && attempt == 0 {
                start = 0;
                continue;
            }
            let append = match r.status {
                206 if start > 0 => {
                    let expected = format!(
                        "bytes {start}-{}/{total}",
                        a.expected_bytes - 1,
                        total = a.expected_bytes
                    );
                    if header(&r.headers, "content-range") != Some(&expected) {
                        return Err(ManagerError::network("invalid Content-Range"));
                    }
                    true
                }
                200 => false,
                _ => return Err(ManagerError::network("unexpected HTTP status")),
            };
            let expected = if append {
                a.expected_bytes - start
            } else {
                a.expected_bytes
            };
            let declared = header(&r.headers, "content-length")
                .ok_or_else(|| ManagerError::network("missing Content-Length"))?
                .parse::<u64>()
                .map_err(|_| ManagerError::network("invalid Content-Length"))?;
            if declared != expected {
                return Err(ManagerError::network("incorrect Content-Length"));
            }
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .truncate(!append)
                .open(&path)?;
            let base = if append { start } else { 0 };
            let written = self.stream(
                id,
                a,
                &identity,
                &path,
                base,
                etag.as_deref(),
                modified.as_deref(),
                &mut r.body,
                &mut f,
                expected,
                c,
            )?;
            f.flush()?;
            f.sync_all()?;
            let final_bytes = base
                .checked_add(written)
                .ok_or_else(|| ManagerError::network("response byte count overflow"))?;
            self.save_progress(
                id,
                a,
                &identity,
                &path,
                final_bytes,
                etag.as_deref(),
                modified.as_deref(),
                "downloaded",
            )?;
            if written != expected {
                return Err(ManagerError::network("response body length mismatch"));
            }
            if final_bytes != a.expected_bytes || sha256_file(&path)? != a.expected_sha256 {
                return Err(ManagerError::integrity("artifact hash mismatch"));
            }
            self.save_verified(id, a, path)?;
            return Ok(());
        }
        Err(ManagerError::network("artifact validators changed"))
    }
    #[allow(clippy::too_many_arguments)]
    fn stream<F: Fn() -> bool>(
        &self,
        id: &str,
        a: &ArtifactPlan,
        identity: &str,
        path: &Path,
        base: u64,
        etag: Option<&str>,
        modified: Option<&str>,
        reader: &mut dyn Read,
        writer: &mut File,
        expected: u64,
        c: &F,
    ) -> Result<u64, ManagerError> {
        let mut b = [0u8; COPY_BUFFER_BYTES];
        let mut written = 0u64;
        let mut checkpointed = 0u64;
        loop {
            if c() {
                writer.flush()?;
                writer.sync_data()?;
                self.save_progress(
                    id,
                    a,
                    identity,
                    path,
                    base + written,
                    etag,
                    modified,
                    "cancelled",
                )?;
                return Err(ManagerError::new(
                    "model_download_cancelled",
                    "download cancelled",
                ));
            }
            let n = match reader.read(&mut b) {
                Ok(n) => n,
                Err(e) => {
                    writer.flush()?;
                    writer.sync_data()?;
                    self.save_progress(
                        id,
                        a,
                        identity,
                        path,
                        base + written,
                        etag,
                        modified,
                        "downloading",
                    )?;
                    return if c() {
                        Err(ManagerError::new(
                            "model_download_cancelled",
                            "download cancelled while waiting for response bytes",
                        ))
                    } else {
                        Err(ManagerError::network(e.to_string()))
                    };
                }
            };
            if n == 0 {
                writer.flush()?;
                writer.sync_data()?;
                self.save_progress(
                    id,
                    a,
                    identity,
                    path,
                    base + written,
                    etag,
                    modified,
                    "downloading",
                )?;
                return Ok(written);
            }
            let next = written
                .checked_add(n as u64)
                .ok_or_else(|| ManagerError::network("response body length overflow"))?;
            if next > expected {
                writer.flush()?;
                writer.sync_data()?;
                self.save_progress(
                    id,
                    a,
                    identity,
                    path,
                    base + written,
                    etag,
                    modified,
                    "downloading",
                )?;
                return Err(ManagerError::network(
                    "response body exceeds declared length",
                ));
            }
            writer.write_all(&b[..n])?;
            written = next;
            if written
                .checked_sub(checkpointed)
                .ok_or_else(|| ManagerError::network("checkpoint accounting underflow"))?
                >= CHECKPOINT_INTERVAL_BYTES
            {
                writer.flush()?;
                writer.sync_data()?;
                self.save_progress(
                    id,
                    a,
                    identity,
                    path,
                    base + written,
                    etag,
                    modified,
                    "downloading",
                )?;
                checkpointed = written;
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn save_progress(
        &self,
        id: &str,
        a: &ArtifactPlan,
        identity: &str,
        path: &Path,
        bytes: u64,
        etag: Option<&str>,
        modified: Option<&str>,
        state: &str,
    ) -> Result<(), ManagerError> {
        self.catalog.save_checkpoint(
            id,
            &ArtifactCheckpoint {
                artifact_id: a.artifact_id.clone(),
                source_identity: identity.to_owned(),
                downloaded_bytes: bytes,
                expected_bytes: a.expected_bytes,
                temp_path: path.to_path_buf(),
                etag: etag.map(str::to_owned),
                last_modified: modified.map(str::to_owned),
                verified_sha256: None,
                state: state.to_owned(),
            },
        )
    }
    fn save_verified(&self, id: &str, a: &ArtifactPlan, path: PathBuf) -> Result<(), ManagerError> {
        self.catalog.save_checkpoint(
            id,
            &ArtifactCheckpoint {
                artifact_id: a.artifact_id.clone(),
                source_identity: source_identity(a),
                downloaded_bytes: a.expected_bytes,
                expected_bytes: a.expected_bytes,
                temp_path: path,
                etag: None,
                last_modified: None,
                verified_sha256: Some(a.expected_sha256.clone()),
                state: "verified".to_owned(),
            },
        )
    }
    pub(super) fn download_dir(&self, id: &str) -> PathBuf {
        self.root.join("downloads").join(id)
    }
}
