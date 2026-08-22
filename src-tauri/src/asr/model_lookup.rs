use crate::domain::{AsrLanguage, AsrProviderKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCapabilities {
    pub provider: AsrProviderKind,
    pub languages: Vec<String>,
    pub selectable: bool,
    pub installable: bool,
    pub executable: bool,
    pub reason_code: Option<String>,
}

impl ModelCapabilities {
    pub fn new<I, S>(
        provider: AsrProviderKind,
        languages: I,
        selectable: bool,
        installable: bool,
        executable: bool,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            provider,
            languages: languages
                .into_iter()
                .map(|language| language.as_ref().to_owned())
                .collect(),
            selectable,
            installable,
            executable,
            reason_code: None,
        }
    }

    pub fn supports_language(&self, language: &AsrLanguage) -> bool {
        self.languages
            .iter()
            .any(|supported| supported == language.as_str())
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        self.reason_code = Some(reason_code.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceSupport {
    Compatible,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationQualification {
    NotInstalled,
    InstalledUnqualified,
    RuntimeQualified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelLookupContext {
    pub device: DeviceSupport,
    pub installation: InstallationQualification,
}

impl ModelLookupContext {
    pub const fn new(device: DeviceSupport, installation: InstallationQualification) -> Self {
        Self {
            device,
            installation,
        }
    }
}

pub trait ModelLookup {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities>;

    fn lookup_with_context(
        &self,
        model_id: &str,
        context: ModelLookupContext,
    ) -> Option<ModelCapabilities> {
        let mut capabilities = self.lookup(model_id)?;
        match (context.device, context.installation) {
            (DeviceSupport::Unsupported, _) => {
                capabilities.installable = false;
                capabilities.executable = false;
                capabilities.reason_code = Some("model_device_unsupported".to_owned());
            }
            (DeviceSupport::Compatible, InstallationQualification::NotInstalled) => {
                capabilities.installable = true;
                capabilities.executable = false;
                capabilities.reason_code = Some("model_not_installed".to_owned());
            }
            (DeviceSupport::Compatible, InstallationQualification::InstalledUnqualified) => {
                capabilities.installable = true;
                capabilities.executable = false;
                capabilities.reason_code = Some("model_runtime_unqualified".to_owned());
            }
            (DeviceSupport::Compatible, InstallationQualification::RuntimeQualified) => {
                capabilities.installable = true;
                capabilities.executable = true;
                capabilities.reason_code = None;
            }
        }
        Some(capabilities)
    }
}
