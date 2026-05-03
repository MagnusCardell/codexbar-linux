use std::fmt;

use crate::model::KeyringState;

use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;
use sha2::{Digest, Sha256};

type Aes128CbcDecryptor = cbc::Decryptor<Aes128>;

const CHROMIUM_V10_PREFIX: &[u8] = b"v10";
const CHROMIUM_V11_PREFIX: &[u8] = b"v11";
const CHROMIUM_V20_PREFIX: &[u8] = b"v20";
const CHROMIUM_V24_PREFIX: &[u8] = b"v24";
const CHROMIUM_LINUX_BASIC_PASSWORD: &[u8] = b"peanuts";
const CHROMIUM_LINUX_BASIC_SALT: &[u8] = b"saltysalt";
const CHROMIUM_LINUX_BASIC_ITERATIONS: u32 = 1;
const CHROMIUM_AES_BLOCK_LEN: usize = 16;
const CHROMIUM_HOST_KEY_HASH_LEN: usize = 32;
const CHROMIUM_V24_DB_VERSION: i64 = 24;
const CHROMIUM_LINUX_BASIC_IV: [u8; CHROMIUM_AES_BLOCK_LEN] = [b' '; CHROMIUM_AES_BLOCK_LEN];

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
    UnsupportedFormat,
    MalformedCiphertext,
    WrongKey,
    InvalidMaterial,
    HeaderTooLarge,
    TooManyCookies,
}

impl DecryptError {
    pub fn keyring_state(self) -> KeyringState {
        match self {
            Self::Unavailable => KeyringState::Unavailable,
            Self::Locked | Self::PromptRequired => KeyringState::Locked,
            Self::Failed
            | Self::UnsupportedFormat
            | Self::MalformedCiphertext
            | Self::WrongKey
            | Self::InvalidMaterial
            | Self::HeaderTooLarge
            | Self::TooManyCookies => KeyringState::Unlocked,
        }
    }
}

pub trait CookieDecryptor {
    fn backend(&self) -> DecryptorBackend;

    fn decrypt(
        &self,
        encrypted_value: &[u8],
        context: CookieDecryptContext<'_>,
    ) -> Result<String, DecryptError>;
}

#[derive(Clone, Copy)]
pub struct CookieDecryptContext<'a> {
    pub host_key: &'a str,
    pub db_version: Option<i64>,
}

impl fmt::Debug for CookieDecryptContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieDecryptContext")
            .field("host_key", &"[redacted]")
            .field("db_version", &self.db_version)
            .finish()
    }
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

    fn decrypt(
        &self,
        encrypted_value: &[u8],
        context: CookieDecryptContext<'_>,
    ) -> Result<String, DecryptError> {
        match self.mode {
            BrowserDecryptorMode::Plain => PlainCookieDecryptor.decrypt(encrypted_value, context),
            BrowserDecryptorMode::Unavailable => Err(DecryptError::Unavailable),
            BrowserDecryptorMode::SecretServiceProbe(status) => match status {
                SecretServiceProbeStatus::Unavailable => Err(DecryptError::Unavailable),
                SecretServiceProbeStatus::Locked => Err(DecryptError::Locked),
                SecretServiceProbeStatus::PromptRequired => Err(DecryptError::PromptRequired),
            },
            BrowserDecryptorMode::Fake(mode) => {
                FakeCookieDecryptor::new(mode).decrypt(encrypted_value, context)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PlainCookieDecryptor;

impl CookieDecryptor for PlainCookieDecryptor {
    fn backend(&self) -> DecryptorBackend {
        DecryptorBackend::Plain
    }

    fn decrypt(
        &self,
        encrypted_value: &[u8],
        context: CookieDecryptContext<'_>,
    ) -> Result<String, DecryptError> {
        decrypt_chromium_linux_basic_v10(encrypted_value, context)
    }
}

fn decrypt_chromium_linux_basic_v10(
    encrypted_value: &[u8],
    context: CookieDecryptContext<'_>,
) -> Result<String, DecryptError> {
    let ciphertext = match encrypted_value.strip_prefix(CHROMIUM_V10_PREFIX) {
        Some(ciphertext) => ciphertext,
        None if encrypted_value.starts_with(CHROMIUM_V11_PREFIX) => {
            return Err(DecryptError::Unavailable);
        }
        None if is_known_unsupported_chromium_prefix(encrypted_value) => {
            return Err(DecryptError::UnsupportedFormat);
        }
        None => return Err(DecryptError::Failed),
    };
    if ciphertext.len() < CHROMIUM_AES_BLOCK_LEN || ciphertext.len() % CHROMIUM_AES_BLOCK_LEN != 0 {
        return Err(DecryptError::MalformedCiphertext);
    }

    let mut key = [0_u8; CHROMIUM_AES_BLOCK_LEN];
    pbkdf2_hmac::<Sha1>(
        CHROMIUM_LINUX_BASIC_PASSWORD,
        CHROMIUM_LINUX_BASIC_SALT,
        CHROMIUM_LINUX_BASIC_ITERATIONS,
        &mut key,
    );
    let mut buffer = ciphertext.to_vec();
    let plaintext = Aes128CbcDecryptor::new_from_slices(&key, &CHROMIUM_LINUX_BASIC_IV)
        .map_err(|_| DecryptError::Failed)?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| DecryptError::MalformedCiphertext)?;
    let plaintext = strip_chromium_host_key_hash_if_required(plaintext, context)?;
    String::from_utf8(plaintext.to_vec()).map_err(|_| DecryptError::WrongKey)
}

fn is_known_unsupported_chromium_prefix(encrypted_value: &[u8]) -> bool {
    encrypted_value.starts_with(CHROMIUM_V20_PREFIX)
        || encrypted_value.starts_with(CHROMIUM_V24_PREFIX)
}

fn strip_chromium_host_key_hash_if_required<'a>(
    plaintext: &'a [u8],
    context: CookieDecryptContext<'_>,
) -> Result<&'a [u8], DecryptError> {
    if context
        .db_version
        .is_none_or(|version| version < CHROMIUM_V24_DB_VERSION)
    {
        return Ok(plaintext);
    }
    let Some((host_hash, value)) = plaintext.split_at_checked(CHROMIUM_HOST_KEY_HASH_LEN) else {
        return Err(DecryptError::MalformedCiphertext);
    };
    let expected = Sha256::digest(context.host_key.as_bytes());
    if host_hash != expected.as_slice() {
        return Err(DecryptError::WrongKey);
    }
    Ok(value)
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

    fn decrypt(
        &self,
        encrypted_value: &[u8],
        _context: CookieDecryptContext<'_>,
    ) -> Result<String, DecryptError> {
        match self.mode {
            FakeDecryptorMode::Success => {
                if encrypted_value.starts_with(CHROMIUM_V10_PREFIX)
                    || encrypted_value.starts_with(CHROMIUM_V11_PREFIX)
                {
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

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;

    type Aes128CbcEncryptor = cbc::Encryptor<Aes128>;

    #[test]
    fn plain_backend_decrypts_v10_basic_cookie_with_v24_host_hash() {
        let encrypted = encrypt_v10_basic_for_test(
            b"codex-cookie-value",
            CookieDecryptContext {
                host_key: ".chatgpt.com",
                db_version: Some(24),
            },
        );
        let decryptor = BrowserCookieDecryptor::new(BrowserDecryptorMode::Plain);

        let value = decryptor
            .decrypt(
                &encrypted,
                CookieDecryptContext {
                    host_key: ".chatgpt.com",
                    db_version: Some(24),
                },
            )
            .expect("v10 decrypt");

        assert_eq!(value, "codex-cookie-value");
        let debug = format!("{decryptor:?}");
        assert!(!debug.contains("peanuts"));
        assert!(!debug.contains("codex-cookie-value"));
    }

    #[test]
    fn plain_backend_rejects_v10_basic_cookie_with_wrong_v24_host_hash() {
        let encrypted = encrypt_v10_basic_for_test(
            b"codex-cookie-value",
            CookieDecryptContext {
                host_key: ".chatgpt.com",
                db_version: Some(24),
            },
        );
        let decryptor = BrowserCookieDecryptor::new(BrowserDecryptorMode::Plain);

        assert_eq!(
            decryptor.decrypt(
                &encrypted,
                CookieDecryptContext {
                    host_key: ".openai.com",
                    db_version: Some(24),
                },
            ),
            Err(DecryptError::WrongKey)
        );
    }

    #[test]
    fn plain_backend_rejects_v10_with_invalid_lengths() {
        let decryptor = BrowserCookieDecryptor::new(BrowserDecryptorMode::Plain);

        for encrypted in [b"v10".as_slice(), b"v10short".as_slice()] {
            assert_eq!(
                decryptor.decrypt(
                    encrypted,
                    CookieDecryptContext {
                        host_key: ".chatgpt.com",
                        db_version: Some(24),
                    },
                ),
                Err(DecryptError::MalformedCiphertext)
            );
        }
    }

    #[test]
    fn plain_backend_rejects_v10_with_bad_padding() {
        let mut encrypted = encrypt_v10_basic_for_test(
            b"codex-cookie-value",
            CookieDecryptContext {
                host_key: ".chatgpt.com",
                db_version: Some(24),
            },
        );
        *encrypted.last_mut().expect("last byte") ^= 0x01;
        let decryptor = BrowserCookieDecryptor::new(BrowserDecryptorMode::Plain);

        assert_eq!(
            decryptor.decrypt(
                &encrypted,
                CookieDecryptContext {
                    host_key: ".chatgpt.com",
                    db_version: Some(24),
                },
            ),
            Err(DecryptError::MalformedCiphertext)
        );
    }

    #[test]
    fn plain_backend_classifies_known_unsupported_prefixes_as_safe_failures() {
        let decryptor = BrowserCookieDecryptor::new(BrowserDecryptorMode::Plain);

        assert_eq!(
            decryptor.decrypt(
                b"v11fixture",
                CookieDecryptContext {
                    host_key: ".chatgpt.com",
                    db_version: Some(24),
                },
            ),
            Err(DecryptError::Unavailable)
        );
        for encrypted in [b"v20fixture".as_slice(), b"v24fixture".as_slice()] {
            assert_eq!(
                decryptor.decrypt(
                    encrypted,
                    CookieDecryptContext {
                        host_key: ".chatgpt.com",
                        db_version: Some(24),
                    },
                ),
                Err(DecryptError::UnsupportedFormat)
            );
        }
    }

    fn encrypt_v10_basic_for_test(value: &[u8], context: CookieDecryptContext<'_>) -> Vec<u8> {
        let mut key = [0_u8; CHROMIUM_AES_BLOCK_LEN];
        pbkdf2_hmac::<Sha1>(
            CHROMIUM_LINUX_BASIC_PASSWORD,
            CHROMIUM_LINUX_BASIC_SALT,
            CHROMIUM_LINUX_BASIC_ITERATIONS,
            &mut key,
        );
        let mut plaintext = Vec::new();
        if context
            .db_version
            .is_some_and(|version| version >= CHROMIUM_V24_DB_VERSION)
        {
            plaintext.extend_from_slice(Sha256::digest(context.host_key.as_bytes()).as_slice());
        }
        plaintext.extend_from_slice(value);

        let padded_len = ((plaintext.len() / CHROMIUM_AES_BLOCK_LEN) + 1) * CHROMIUM_AES_BLOCK_LEN;
        let mut buffer = vec![0_u8; padded_len];
        buffer[..plaintext.len()].copy_from_slice(&plaintext);
        let ciphertext = Aes128CbcEncryptor::new_from_slices(&key, &CHROMIUM_LINUX_BASIC_IV)
            .expect("fixed key and IV lengths")
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .expect("test encryption");

        let mut encrypted = CHROMIUM_V10_PREFIX.to_vec();
        encrypted.extend_from_slice(ciphertext);
        encrypted
    }
}
