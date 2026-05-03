use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionFinding {
    pub code: &'static str,
}

pub fn validate_public_json_text(text: &str) -> Result<(), RedactionFinding> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        validate_public_json_value(&value)?;
    }
    let lower = text.to_ascii_lowercase();
    let forbidden = [
        ("authorization_header", "authorization:"),
        ("set_cookie_header", "set-cookie"),
        ("bearer_token", "bearer "),
        ("openai_api_key", "sk-"),
        ("github_token", "ghp_"),
        ("slack_token", "xoxb-"),
        ("access_token", "access_token"),
        ("access_token", "accesstoken"),
        ("refresh_token", "refresh_token"),
        ("refresh_token", "refreshtoken"),
        ("session_token", "session_token"),
        ("session_key", "sessionkey"),
        ("raw_payload", "\"rawpayload\""),
        ("raw_response", "\"rawresponse\""),
        ("home_path", "/home/"),
        ("local_share_path", "~/.local/share"),
        ("auth_json_path", "auth.json"),
        ("browser_cookie_db", "network/cookies"),
        ("browser_login_db", "login data"),
        ("browser_profile_path", ".config/chrom"),
        ("browser_profile_path", ".mozilla/firefox"),
    ];
    for (code, needle) in forbidden {
        if lower.contains(needle) {
            return Err(RedactionFinding { code });
        }
    }

    if contains_raw_email(text) {
        return Err(RedactionFinding { code: "raw_email" });
    }

    Ok(())
}

pub fn validate_public_json_value(value: &Value) -> Result<(), RedactionFinding> {
    validate_value_recursive(value)?;
    let text = serde_json::to_string(value).map_err(|_| RedactionFinding {
        code: "json_serialization",
    })?;
    validate_public_json_text_without_json_parse(&text)
}

pub fn contains_null(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.iter().any(contains_null),
        Value::Object(map) => map.values().any(contains_null),
        _ => false,
    }
}

fn contains_raw_email(text: &str) -> bool {
    for token in text.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
    }) {
        let Some(at) = token.find('@') else {
            continue;
        };
        if token.contains("***@") || token.contains("masked") {
            continue;
        }
        let (left, right) = token.split_at(at);
        let domain = &right[1..];
        if !left.is_empty()
            && domain.contains('.')
            && domain
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        {
            return true;
        }
    }
    false
}

fn validate_public_json_text_without_json_parse(text: &str) -> Result<(), RedactionFinding> {
    let lower = text.to_ascii_lowercase();
    for (code, needle) in [
        ("authorization_header", "authorization:"),
        ("set_cookie_header", "set-cookie"),
        ("bearer_token", "bearer "),
        ("openai_api_key", "sk-"),
        ("github_token", "ghp_"),
        ("slack_token", "xoxb-"),
        ("home_path", "/home/"),
        ("local_share_path", "~/.local/share"),
        ("auth_json_path", "auth.json"),
    ] {
        if lower.contains(needle) {
            return Err(RedactionFinding { code });
        }
    }
    if contains_raw_email(text) {
        return Err(RedactionFinding { code: "raw_email" });
    }
    Ok(())
}

fn validate_value_recursive(value: &Value) -> Result<(), RedactionFinding> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                validate_public_key(key)?;
                validate_value_recursive(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_value_recursive(item)?;
            }
        }
        Value::String(value) => validate_public_string(value)?,
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn validate_public_key(key: &str) -> Result<(), RedactionFinding> {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if matches!(
        normalized.as_str(),
        "accountemaildisplay"
            | "accountemailhash"
            | "accountorganizationdisplay"
            | "accountorganizationhash"
            | "provideraccountidhash"
    ) {
        return Ok(());
    }
    if normalized.contains("header") {
        return Err(RedactionFinding { code: "secret_key" });
    }
    if normalized.contains("url")
        && (normalized.starts_with("raw")
            || normalized.starts_with("request")
            || normalized.starts_with("response")
            || normalized.starts_with("final"))
    {
        return Err(RedactionFinding {
            code: "raw_url_key",
        });
    }
    let code = match normalized.as_str() {
        "authorization" | "cookie" | "cookies" | "setcookie" | "xapikey" | "headers" => {
            Some("secret_key")
        }
        "raw" | "rawpayload" | "rawresponse" | "rawoutput" | "stdout" | "stderr" | "stdouttext"
        | "stderrtext" | "stdoutjson" | "stderrjson" => Some("raw_payload"),
        "accesstoken" | "refreshtoken" | "sessiontoken" | "sessionkey" | "apikey" | "password"
        | "secret" => Some("token_key"),
        "accountemail" | "signedinemail" | "email" => Some("raw_email_key"),
        "accountorganization" | "organization" | "provideraccountid" => Some("raw_identity_key"),
        _ => None,
    };
    if let Some(code) = code {
        return Err(RedactionFinding { code });
    }
    Ok(())
}

fn validate_public_string(value: &str) -> Result<(), RedactionFinding> {
    let lower = value.to_ascii_lowercase();
    for (code, needle) in [
        ("authorization_header", "authorization:"),
        ("set_cookie_header", "set-cookie"),
        ("cookie_header", "cookie:"),
        ("bearer_token", "bearer "),
        ("openai_api_key", "sk-"),
        ("github_token", "ghp_"),
        ("slack_token", "xoxb-"),
        ("access_token", "access_token"),
        ("access_token", "accesstoken"),
        ("refresh_token", "refresh_token"),
        ("refresh_token", "refreshtoken"),
        ("session_token", "session_token"),
        ("session_token", "sessiontoken"),
        ("session_key", "session_key"),
        ("session_key", "sessionkey"),
        ("home_path", "/home/"),
        ("local_share_path", "~/.local/share"),
        ("auth_json_path", "auth.json"),
    ] {
        if lower.contains(needle) {
            return Err(RedactionFinding { code });
        }
    }
    for name in ["api_key", "apikey"] {
        if lower.contains(&format!("{name}=")) || lower.contains(&format!("{name}:")) {
            return Err(RedactionFinding { code: "api_key" });
        }
    }
    if contains_raw_email(value) {
        return Err(RedactionFinding { code: "raw_email" });
    }
    Ok(())
}
