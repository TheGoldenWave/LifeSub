use crate::domain::AsrProviderKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCapabilities {
    pub provider: AsrProviderKind,
    pub languages: Vec<String>,
    pub selectable: bool,
    pub installable: bool,
    pub executable: bool,
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
        }
    }

    pub fn supports_language(&self, language: &str) -> bool {
        self.languages.iter().any(|supported| supported == language)
    }
}

pub trait ModelLookup {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities>;
}
