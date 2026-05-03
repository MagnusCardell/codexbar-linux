use crate::model::KeyringState;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecryptorBackend {
    Fake,
    Plain,
    SecretService,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecryptionStatus {
    NotNeeded,
    Succeeded,
    Failed,
    Unavailable,
    Locked,
    PromptRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeDecryptorMode {
    Success,
    Failure,
    Unavailable,
    Locked,
    PromptRequired,
}

impl Default for FakeDecryptorMode {
    fn default() -> Self {
        Self::Success
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretServiceProbeStatus {
    Unavailable,
    Locked,
    PromptRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserDecryptorMode {
    Plain,
    Unavailable,
    SecretServiceProbe(SecretServiceProbeStatus),
    Fake(FakeDecryptorMode),
}

impl Default for BrowserDecryptorMode {
    fn default() -> Self {
        Self::Plain
    }
}

impl BrowserDecryptorMode {
    pub fn fake(mode: FakeDecryptorMode) -> Self {
        Self::Fake(mode)
    }

    pub fn backend(self) -> DecryptorBackend {
        match self {
            Self::Plain => DecryptorBackend::Plain,
            Self::Unavailable => DecryptorBackend::Unavailable,
            Self::SecretServiceProbe(_) => DecryptorBackend::SecretService,
            Self::Fake(_) => DecryptorBackend::Fake,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecryptError {
    Unavailable,
    Locked,
    PromptRequired,
    Failed,
}

impl DecryptError {
    pub fn keyring_state(self) -> KeyringState {
        match self {
            Self::Unavailable => KeyringState::Unavailable,
            Self::Locked | Self::PromptRequired => KeyringState::Locked,
            Self::Failed => KeyringState::Unlocked,
        }
    }
}

pub trait CookieDecryptor {
    fn backend(&self) -> DecryptorBackend;

    fn decrypt(&self, encrypted_value: &[u8]) -> Result<String, DecryptError>;
}

#[derive(Clone, Debug)]
pub struct BrowserCookieDecryptor {
    mode: BrowserDecryptorMode,
}

impl BrowserCookieDecryptor {
    pub fn new(mode: BrowserDecryptorMode) -> Self {
        Self { mode }
    }
}

impl CookieDecryptor for BrowserCookieDecryptor {
    fn backend(&self) -> DecryptorBackend {
        self.mode.backend()
    }

    fn decrypt(&self, encrypted_value: &[u8]) -> Result<String, DecryptError> {
        match self.mode {
            BrowserDecryptorMode::Plain | BrowserDecryptorMode::Unavailable => {
                Err(DecryptError::Unavailable)
            }
            BrowserDecryptorMode::SecretServiceProbe(status) => match status {
                SecretServiceProbeStatus::Unavailable => Err(DecryptError::Unavailable),
                SecretServiceProbeStatus::Locked => Err(DecryptError::Locked),
                SecretServiceProbeStatus::PromptRequired => Err(DecryptError::PromptRequired),
            },
            BrowserDecryptorMode::Fake(mode) => {
                FakeCookieDecryptor::new(mode).decrypt(encrypted_value)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FakeCookieDecryptor {
    mode: FakeDecryptorMode,
}

impl FakeCookieDecryptor {
    pub fn new(mode: FakeDecryptorMode) -> Self {
        Self { mode }
    }
}

impl CookieDecryptor for FakeCookieDecryptor {
    fn backend(&self) -> DecryptorBackend {
        DecryptorBackend::Fake
    }

    fn decrypt(&self, encrypted_value: &[u8]) -> Result<String, DecryptError> {
        match self.mode {
            FakeDecryptorMode::Success => {
                if encrypted_value.starts_with(b"v10") || encrypted_value.starts_with(b"v11") {
                    Ok("fixture-decrypted-value".to_string())
                } else {
                    Err(DecryptError::Failed)
                }
            }
            FakeDecryptorMode::Failure => Err(DecryptError::Failed),
            FakeDecryptorMode::Unavailable => Err(DecryptError::Unavailable),
            FakeDecryptorMode::Locked => Err(DecryptError::Locked),
            FakeDecryptorMode::PromptRequired => Err(DecryptError::PromptRequired),
        }
    }
}
