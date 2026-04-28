use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionFinding {
    pub code: &'static str,
}

pub fn validate_public_json_text(text: &str) -> Result<(), RedactionFinding> {
    let lower = text.to_ascii_lowercase();
    let forbidden = [
        ("authorization_header", "authorization:"),
        ("set_cookie_header", "set-cookie"),
        ("bearer_token", "bearer "),
        ("openai_api_key", "sk-"),
        ("access_token", "access_token"),
        ("refresh_token", "refresh_token"),
        ("session_token", "session_token"),
        ("raw_payload", "\"rawpayload\""),
        ("raw_response", "\"rawresponse\""),
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
    let text = serde_json::to_string(value).map_err(|_| RedactionFinding {
        code: "json_serialization",
    })?;
    validate_public_json_text(&text)
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
