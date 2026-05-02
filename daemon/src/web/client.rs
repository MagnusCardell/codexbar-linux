use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct WebRequest {
    url: String,
    user_agent: String,
    accept: String,
    accept_language: Option<String>,
    session_header: Option<String>,
    timeout: Duration,
    response_size_limit: usize,
}

impl WebRequest {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            user_agent: "codexbar-linux/0 web-fixture-boundary".to_string(),
            accept: "text/html,application/json;q=0.9,*/*;q=0.1".to_string(),
            accept_language: Some("en-US,en;q=0.8".to_string()),
            session_header: None,
            timeout: Duration::from_secs(15),
            response_size_limit: 512 * 1024,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn response_size_limit(&self) -> usize {
        self.response_size_limit
    }

    pub fn session_material_attached(&self) -> bool {
        self.session_header.is_some()
    }

    pub fn session_material_bytes(&self) -> usize {
        self.session_header.as_ref().map_or(0, String::len)
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_response_size_limit(mut self, limit: usize) -> Self {
        self.response_size_limit = limit;
        self
    }

    pub(crate) fn with_session_header(mut self, value: String) -> Self {
        self.session_header = Some(value);
        self
    }
}

impl fmt::Debug for WebRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebRequest")
            .field("url", &self.url)
            .field("user_agent", &self.user_agent)
            .field("accept", &self.accept)
            .field("accept_language", &self.accept_language)
            .field(
                "session_material_attached",
                &self.session_material_attached(),
            )
            .field("session_material_bytes", &self.session_material_bytes())
            .field("timeout", &self.timeout)
            .field("response_size_limit", &self.response_size_limit)
            .finish()
    }
}

#[derive(Clone)]
pub struct WebResponse {
    status: u16,
    final_url: String,
    redirect_url: Option<String>,
    body: Vec<u8>,
    content_type: Option<String>,
}

impl WebResponse {
    pub fn new(status: u16, final_url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            final_url: final_url.into(),
            redirect_url: None,
            body: body.into(),
            content_type: None,
        }
    }

    pub fn with_redirect(mut self, redirect_url: impl Into<String>) -> Self {
        self.redirect_url = Some(redirect_url.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn final_url(&self) -> &str {
        &self.final_url
    }

    pub fn redirect_url(&self) -> Option<&str> {
        self.redirect_url.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

impl fmt::Debug for WebResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebResponse")
            .field("status", &self.status)
            .field("final_url", &self.final_url)
            .field("redirect_url", &self.redirect_url)
            .field("body_bytes", &self.body.len())
            .field("body", &"[redacted]")
            .field("content_type", &self.content_type)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebClientError {
    Timeout,
    TransportUnavailable,
}

pub trait WebClient {
    fn request(&self, request: WebRequest) -> Result<WebResponse, WebClientError>;
}

#[derive(Clone, Debug)]
pub struct FakeWebClient {
    outcome: Arc<Mutex<FakeWebOutcome>>,
    requests: Arc<Mutex<Vec<FakeRecordedRequest>>>,
}

impl FakeWebClient {
    pub fn responding(response: WebResponse) -> Self {
        Self {
            outcome: Arc::new(Mutex::new(FakeWebOutcome::Response(response))),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(error: WebClientError) -> Self {
        Self {
            outcome: Arc::new(Mutex::new(FakeWebOutcome::Error(error))),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn timeout() -> Self {
        Self::failing(WebClientError::Timeout)
    }

    pub fn response(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self::responding(WebResponse::new(
            status,
            "https://chatgpt.com/codex/settings/usage",
            body,
        ))
    }

    pub fn redirected_response(
        status: u16,
        redirect_url: impl Into<String>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self::responding(
            WebResponse::new(status, "https://chatgpt.com/codex/settings/usage", body)
                .with_redirect(redirect_url),
        )
    }

    pub fn codex_fixture(fixture: CodexWebFixture) -> Self {
        match fixture {
            CodexWebFixture::Success => {
                Self::response(200, codex_success_body().as_bytes().to_vec())
            }
            CodexWebFixture::LoginRequired | CodexWebFixture::CookieRejected => {
                Self::response(200, codex_login_required_body().as_bytes().to_vec())
            }
            CodexWebFixture::AccountMismatch => {
                Self::response(200, codex_account_mismatch_body().as_bytes().to_vec())
            }
            CodexWebFixture::Non200 => Self::response(
                503,
                include_str!("../../fixtures/web/codex/non_200.json")
                    .as_bytes()
                    .to_vec(),
            ),
            CodexWebFixture::RateLimited => Self::response(
                429,
                include_str!("../../fixtures/web/codex/non_200.json")
                    .as_bytes()
                    .to_vec(),
            ),
            CodexWebFixture::ParseError => {
                Self::response(200, codex_parse_error_body().as_bytes().to_vec())
            }
            CodexWebFixture::RedirectWrongHost => Self::redirected_response(
                200,
                "https://codex-test.example.invalid/callback",
                codex_success_body().as_bytes().to_vec(),
            ),
        }
    }

    pub fn requests(&self) -> Vec<FakeRecordedRequest> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }

    pub fn request_count(&self) -> usize {
        self.requests().len()
    }
}

impl WebClient for FakeWebClient {
    fn request(&self, request: WebRequest) -> Result<WebResponse, WebClientError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(FakeRecordedRequest {
                url: request.url().to_string(),
                session_material_attached: request.session_material_attached(),
                session_material_bytes: request.session_material_bytes(),
                timeout_ms: request.timeout().as_millis() as u64,
                response_size_limit: request.response_size_limit(),
            });
        }
        match self.outcome.lock() {
            Ok(outcome) => match &*outcome {
                FakeWebOutcome::Response(response) => Ok(response.clone()),
                FakeWebOutcome::Error(error) => Err(*error),
            },
            Err(_) => Err(WebClientError::TransportUnavailable),
        }
    }
}

#[derive(Clone, Debug)]
enum FakeWebOutcome {
    Response(WebResponse),
    Error(WebClientError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeRecordedRequest {
    pub url: String,
    pub session_material_attached: bool,
    pub session_material_bytes: usize,
    pub timeout_ms: u64,
    pub response_size_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexWebFixture {
    Success,
    LoginRequired,
    CookieRejected,
    AccountMismatch,
    Non200,
    RateLimited,
    ParseError,
    RedirectWrongHost,
}

fn codex_success_body() -> &'static str {
    include_str!("../../fixtures/web/codex/dashboard_success.html")
}

fn codex_login_required_body() -> &'static str {
    include_str!("../../fixtures/web/codex/dashboard_login_required.html")
}

fn codex_account_mismatch_body() -> &'static str {
    include_str!("../../fixtures/web/codex/dashboard_account_mismatch.html")
}

fn codex_parse_error_body() -> &'static str {
    include_str!("../../fixtures/web/codex/dashboard_parse_error.html")
}
