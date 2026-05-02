use crate::model::KeyringState;

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
    fn decrypt(&self, encrypted_value: &[u8]) -> Result<String, DecryptError>;
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
    fn decrypt(&self, encrypted_value: &[u8]) -> Result<String, DecryptError> {
        match self.mode {
            FakeDecryptorMode::Success => {
                if encrypted_value.is_empty() {
                    Err(DecryptError::Failed)
                } else {
                    Ok("fixture-decrypted-value".to_string())
                }
            }
            FakeDecryptorMode::Failure => Err(DecryptError::Failed),
            FakeDecryptorMode::Unavailable => Err(DecryptError::Unavailable),
            FakeDecryptorMode::Locked => Err(DecryptError::Locked),
            FakeDecryptorMode::PromptRequired => Err(DecryptError::PromptRequired),
        }
    }
}
