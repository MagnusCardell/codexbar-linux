use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::header::{
    HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_TYPE, COOKIE, LOCATION, PRAGMA,
    USER_AGENT,
};
use reqwest::Client;

use crate::browser::session_material::CookieHeader;
use crate::web::policy::CodexWebPolicy;

const REQUEST_HEADER_PROFILE_BROWSER_LIKE: &str = "browser_like";
const BROWSER_LIKE_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const BROWSER_LIKE_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
const BROWSER_LIKE_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";
const BROWSER_LIKE_CACHE_CONTROL: &str = "no-cache";
const BROWSER_LIKE_PRAGMA: &str = "no-cache";
const BROWSER_LIKE_EXTRA_HEADERS: &[(&str, &str)] = &[
    ("Sec-Fetch-Dest", "document"),
    ("Sec-Fetch-Mode", "navigate"),
    ("Sec-Fetch-Site", "none"),
];

#[derive(Clone)]
pub struct WebRequest {
    url: String,
    user_agent: String,
    accept: String,
    accept_language: Option<String>,
    request_header_profile: &'static str,
    session_header: Option<CookieHeader>,
    timeout: Duration,
    response_size_limit: usize,
    policy_validated_redirect: bool,
}

impl WebRequest {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            user_agent: BROWSER_LIKE_USER_AGENT.to_string(),
            accept: BROWSER_LIKE_ACCEPT.to_string(),
            accept_language: Some(BROWSER_LIKE_ACCEPT_LANGUAGE.to_string()),
            request_header_profile: REQUEST_HEADER_PROFILE_BROWSER_LIKE,
            session_header: None,
            timeout: Duration::from_secs(15),
            response_size_limit: 512 * 1024,
            policy_validated_redirect: false,
        }
    }

    pub(crate) fn new_policy_validated_redirect(url: impl Into<String>) -> Self {
        Self {
            policy_validated_redirect: true,
            ..Self::new(url)
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

    pub(crate) fn policy_validated_redirect(&self) -> bool {
        self.policy_validated_redirect
    }

    pub fn request_header_profile(&self) -> &'static str {
        self.request_header_profile
    }

    pub fn session_material_attached(&self) -> bool {
        self.session_header.is_some()
    }

    pub fn session_material_bytes(&self) -> usize {
        self.session_header.as_ref().map_or(0, CookieHeader::len)
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_response_size_limit(mut self, limit: usize) -> Self {
        self.response_size_limit = limit;
        self
    }

    pub(crate) fn with_session_header(mut self, value: CookieHeader) -> Self {
        self.session_header = Some(value);
        self
    }
}

impl fmt::Debug for WebRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebRequest")
            .field("url", &redacted_url_shape(&self.url))
            .field("request_header_profile", &self.request_header_profile)
            .field("policy_validated_redirect", &self.policy_validated_redirect)
            .field(
                "session_material_attached",
                &self.session_material_attached(),
            )
            .field(
                "session_material_size",
                &session_material_size_class(self.session_material_bytes()),
            )
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
    redirect_present: bool,
    redirect_invalid: bool,
    body: Vec<u8>,
    content_type: Option<String>,
}

impl WebResponse {
    pub fn new(status: u16, final_url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            final_url: final_url.into(),
            redirect_url: None,
            redirect_present: false,
            redirect_invalid: false,
            body: body.into(),
            content_type: None,
        }
    }

    pub fn with_redirect(mut self, redirect_url: impl Into<String>) -> Self {
        self.redirect_url = Some(redirect_url.into());
        self.redirect_present = true;
        self.redirect_invalid = false;
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_optional_redirect(mut self, redirect_url: Option<String>) -> Self {
        self.redirect_present = redirect_url.is_some();
        self.redirect_invalid = false;
        self.redirect_url = redirect_url;
        self
    }

    pub fn with_invalid_redirect_for_tests(mut self) -> Self {
        self.redirect_url = None;
        self.redirect_present = true;
        self.redirect_invalid = true;
        self
    }

    pub fn with_optional_content_type(mut self, content_type: Option<String>) -> Self {
        self.content_type = content_type;
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

    pub fn redirect_present(&self) -> bool {
        self.redirect_present
    }

    pub fn redirect_invalid(&self) -> bool {
        self.redirect_invalid
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
            .field("final_url", &redacted_url_shape(&self.final_url))
            .field("redirect_present", &self.redirect_present)
            .field("redirect_invalid", &self.redirect_invalid)
            .field("body_bytes", &self.body.len())
            .field("body", &"[redacted]")
            .field(
                "content_type_class",
                &content_type_class(self.content_type()),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebClientError {
    Timeout,
    ResponseTooLarge,
    TransportUnavailable,
}

pub type WebClientFuture<'a> =
    Pin<Box<dyn Future<Output = Result<WebResponse, WebClientError>> + Send + 'a>>;

pub trait WebClient: Send + Sync {
    fn request(&self, request: WebRequest) -> WebClientFuture<'_>;
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

    pub fn responding_sequence(responses: Vec<WebResponse>) -> Self {
        Self {
            outcome: Arc::new(Mutex::new(FakeWebOutcome::ResponseSequence(responses))),
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
    fn request(&self, request: WebRequest) -> WebClientFuture<'_> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(FakeRecordedRequest {
                url: request.url().to_string(),
                request_header_profile: request.request_header_profile().to_string(),
                session_material_attached: request.session_material_attached(),
                session_material_bytes: request.session_material_bytes(),
                timeout_ms: request.timeout().as_millis() as u64,
                response_size_limit: request.response_size_limit(),
            });
        }
        let result = match self.outcome.lock() {
            Ok(mut outcome) => match &mut *outcome {
                FakeWebOutcome::Response(response) => Ok(response.clone()),
                FakeWebOutcome::ResponseSequence(responses) => {
                    if responses.is_empty() {
                        Err(WebClientError::TransportUnavailable)
                    } else {
                        Ok(responses.remove(0))
                    }
                }
                FakeWebOutcome::Error(error) => Err(*error),
            },
            Err(_) => Err(WebClientError::TransportUnavailable),
        };
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct ReqwestStaticGetClient {
    client: Option<Client>,
}

impl ReqwestStaticGetClient {
    pub fn new() -> Self {
        Self {
            client: Self::build_client().ok(),
        }
    }

    pub fn validate_request_for_tests(request: &WebRequest) -> Result<(), WebClientError> {
        Self::validate_request(request)
    }

    #[doc(hidden)]
    pub fn static_browser_like_headers_for_tests() -> Vec<(&'static str, &'static str)> {
        let mut headers = vec![
            ("User-Agent", BROWSER_LIKE_USER_AGENT),
            ("Accept", BROWSER_LIKE_ACCEPT),
            ("Accept-Language", BROWSER_LIKE_ACCEPT_LANGUAGE),
            ("Cache-Control", BROWSER_LIKE_CACHE_CONTROL),
            ("Pragma", BROWSER_LIKE_PRAGMA),
        ];
        headers.extend_from_slice(BROWSER_LIKE_EXTRA_HEADERS);
        headers
    }

    fn validate_request(request: &WebRequest) -> Result<(), WebClientError> {
        let policy = CodexWebPolicy::new();
        let result = if request.policy_validated_redirect() {
            policy.validate_follow_redirect_url(request.url())
        } else {
            policy.validate_dashboard_url(request.url())
        };
        result.map_err(|_| WebClientError::TransportUnavailable)
    }

    fn build_client() -> Result<Client, WebClientError> {
        Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| WebClientError::TransportUnavailable)
    }

    async fn request_inner(&self, request: WebRequest) -> Result<WebResponse, WebClientError> {
        Self::validate_request(&request)?;
        if request.session_header.is_none() {
            return Err(WebClientError::TransportUnavailable);
        }

        match tokio::time::timeout(request.timeout(), self.send_validated_request(request)).await {
            Ok(result) => result,
            Err(_) => Err(WebClientError::Timeout),
        }
    }

    async fn send_validated_request(
        &self,
        request: WebRequest,
    ) -> Result<WebResponse, WebClientError> {
        let client = self
            .client
            .as_ref()
            .ok_or(WebClientError::TransportUnavailable)?;
        let session_header = request
            .session_header
            .as_ref()
            .ok_or(WebClientError::TransportUnavailable)?;

        let mut builder = client
            .get(request.url())
            .header(USER_AGENT, header_value(&request.user_agent)?)
            .header(ACCEPT, header_value(&request.accept)?)
            .header(CACHE_CONTROL, header_value(BROWSER_LIKE_CACHE_CONTROL)?)
            .header(PRAGMA, header_value(BROWSER_LIKE_PRAGMA)?)
            .header(COOKIE, header_value(session_header.as_str())?);
        if let Some(accept_language) = &request.accept_language {
            builder = builder.header(ACCEPT_LANGUAGE, header_value(accept_language)?);
        }
        for (name, value) in BROWSER_LIKE_EXTRA_HEADERS {
            builder = builder.header(*name, header_value(value)?);
        }

        let response = builder.send().await.map_err(classify_reqwest_error)?;
        let status = response.status().as_u16();
        let final_url = response.url().as_str().to_string();
        let location = response.headers().get(LOCATION);
        let redirect_present = location.is_some();
        let redirect_url = location
            .and_then(|value| value.to_str().ok())
            .and_then(|location| resolve_redirect_url(&final_url, location));
        let redirect_invalid = redirect_present && redirect_url.is_none();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = read_limited_body(response, request.response_size_limit()).await?;

        let mut response = WebResponse::new(status, final_url, body)
            .with_optional_redirect(redirect_url)
            .with_optional_content_type(content_type);
        if redirect_invalid {
            response = response.with_invalid_redirect_for_tests();
        }
        Ok(response)
    }
}

impl Default for ReqwestStaticGetClient {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ReqwestStaticGetClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReqwestStaticGetClient")
            .field("client_available", &self.client.is_some())
            .finish()
    }
}

impl WebClient for ReqwestStaticGetClient {
    fn request(&self, request: WebRequest) -> WebClientFuture<'_> {
        Box::pin(async move { self.request_inner(request).await })
    }
}

fn header_value(value: &str) -> Result<HeaderValue, WebClientError> {
    HeaderValue::from_str(value).map_err(|_| WebClientError::TransportUnavailable)
}

fn classify_reqwest_error(error: reqwest::Error) -> WebClientError {
    if error.is_timeout() {
        WebClientError::Timeout
    } else {
        WebClientError::TransportUnavailable
    }
}

fn resolve_redirect_url(base: &str, location: &str) -> Option<String> {
    let base = url::Url::parse(base).ok()?;
    base.join(location).ok().map(|url| url.to_string())
}

async fn read_limited_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, WebClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(WebClientError::ResponseTooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_reqwest_error)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(WebClientError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn redacted_url_shape(url: &str) -> String {
    let Ok(url) = url::Url::parse(url) else {
        return "[redacted-url]".to_string();
    };
    let host = url.host_str().unwrap_or("unknown");
    let path = url.path();
    format!("{}://{}{}", url.scheme(), host, path)
}

fn content_type_class(content_type: Option<&str>) -> &'static str {
    let Some(content_type) = content_type else {
        return "missing";
    };
    let lower = content_type.to_ascii_lowercase();
    if lower.contains("text/html") {
        "html"
    } else if lower.contains("application/json") || lower.ends_with("+json") {
        "json"
    } else if lower.starts_with("text/") {
        "text"
    } else {
        "other"
    }
}

fn session_material_size_class(bytes: usize) -> &'static str {
    if bytes == 0 {
        "absent"
    } else {
        "present"
    }
}

#[derive(Clone, Debug)]
enum FakeWebOutcome {
    Response(WebResponse),
    ResponseSequence(Vec<WebResponse>),
    Error(WebClientError),
}

#[derive(Clone, Eq, PartialEq)]
pub struct FakeRecordedRequest {
    pub url: String,
    pub request_header_profile: String,
    pub session_material_attached: bool,
    pub session_material_bytes: usize,
    pub timeout_ms: u64,
    pub response_size_limit: usize,
}

impl fmt::Debug for FakeRecordedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeRecordedRequest")
            .field("url", &redacted_url_shape(&self.url))
            .field("request_header_profile", &self.request_header_profile)
            .field("session_material_attached", &self.session_material_attached)
            .field(
                "session_material_size",
                &session_material_size_class(self.session_material_bytes),
            )
            .field("timeout_ms", &self.timeout_ms)
            .field("response_size_limit", &self.response_size_limit)
            .finish()
    }
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
