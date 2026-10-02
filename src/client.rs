//! HTTP client of the Almena API (`/api/v1`): JSON in, JSON out, the bearer
//! token when there is one, and the API's error codes kept as they come.

use std::fmt;
use std::time::Duration;

use anyhow::Context as _;
use reqwest::Method;
use reqwest::blocking::{RequestBuilder, Response};
use serde_json::Value;

use crate::cli::VERSION;

pub struct Client {
    http: reqwest::blocking::Client,
    base: url::Url,
    token: Option<String>,
    verbose: bool,
}

/// A refusal from the API: its HTTP status and the stable code in `detail`.
#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    /// Field by field, for a request the API could not take as it was.
    pub problems: Vec<String>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.code, self.status)?;
        if let Some(hint) = hint(&self.code) {
            write!(f, ": {hint}")?;
        }
        for problem in &self.problems {
            write!(f, "\n  {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// What to do about the refusals people meet most.
fn hint(code: &str) -> Option<&'static str> {
    Some(match code {
        "not_authenticated" => "not signed in, or the sign-in has ended; run `almena auth login`",
        "not_a_member" | "tenant_not_found" => "not a tenant of yours; see `almena tenant list`",
        "not_an_admin" => "only the tenant's admins may do this",
        "not_a_signer" => {
            "you do not sign as the tenant; see `almena tenant get` (its signing flow) \
             and link a wallet with `almena account link-wallet`"
        }
        "no_signers" => "nobody who signs as the tenant has linked a wallet",
        "request_expired" => "the wallet did not answer in time; try again",
        "invalid_code" => "wrong or expired code",
        "too_many_attempts" => "too many wrong codes; ask for a new one",
        "sign_in_required" => "API tokens are made from a sign-in, not with another token",
        "invalid_request" => "the API could not take the request as it was",
        _ => return None,
    })
}

impl Client {
    pub fn new(base: &str, token: Option<String>, verbose: bool) -> anyhow::Result<Self> {
        let mut base = url::Url::parse(base).with_context(|| format!("not a URL: {base}"))?;
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        let http = reqwest::blocking::Client::builder()
            .user_agent(format!("almena/{VERSION}"))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            base,
            token,
            verbose,
        })
    }

    /// `/api/v1/{segments…}`, each segment escaped.
    pub fn url(&self, segments: &[&str]) -> url::Url {
        let mut url = self.base.clone();
        {
            let mut path = url.path_segments_mut().expect("an http(s) URL has a path");
            path.pop_if_empty().extend(["api", "v1"]).extend(segments);
        }
        url
    }

    pub fn get(&self, segments: &[&str]) -> anyhow::Result<Value> {
        self.json(self.request(Method::GET, segments))
    }

    pub fn get_query(&self, segments: &[&str], query: &[(&str, String)]) -> anyhow::Result<Value> {
        self.json(self.request(Method::GET, segments).query(query))
    }

    pub fn post(&self, segments: &[&str], body: &Value) -> anyhow::Result<Value> {
        self.json(self.request(Method::POST, segments).json(body))
    }

    pub fn put(&self, segments: &[&str], body: &Value) -> anyhow::Result<Value> {
        self.json(self.request(Method::PUT, segments).json(body))
    }

    pub fn patch(&self, segments: &[&str], body: &Value) -> anyhow::Result<Value> {
        self.json(self.request(Method::PATCH, segments).json(body))
    }

    pub fn delete(&self, segments: &[&str]) -> anyhow::Result<Value> {
        self.json(self.request(Method::DELETE, segments))
    }

    /// A file as the API serves it, and the name it gives it.
    pub fn download(&self, segments: &[&str]) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        let response = self.send(self.request(Method::GET, segments))?;
        let name = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .and_then(filename);
        Ok((response.bytes()?.to_vec(), name))
    }

    fn request(&self, method: Method, segments: &[&str]) -> RequestBuilder {
        let request = self.http.request(method, self.url(segments));
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn send(&self, request: RequestBuilder) -> anyhow::Result<Response> {
        let request = request.build()?;
        if self.verbose {
            eprintln!("> {} {}", request.method(), request.url());
        }
        let host = request.url().host_str().unwrap_or_default().to_owned();
        let response = self
            .http
            .execute(request)
            .with_context(|| format!("could not reach {host}"))?;
        if self.verbose {
            eprintln!("< {}", response.status());
        }
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status().as_u16();
        let body: Value = response.json().unwrap_or(Value::Null);
        Err(refusal(status, &body).into())
    }

    fn json(&self, request: RequestBuilder) -> anyhow::Result<Value> {
        let response = self.send(request)?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        let text = response.text()?;
        if text.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context("the API answered with something other than JSON")
    }
}

/// The API's error body, `{"detail": …}`: a code, an object with a `code`, or
/// (a request it could not take) FastAPI's list of problems.
pub fn refusal(status: u16, body: &Value) -> ApiError {
    let detail = &body["detail"];
    let (code, problems) = match detail {
        Value::String(code) => (code.clone(), Vec::new()),
        Value::Object(object) => {
            let code = object
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("refused")
                .to_owned();
            let problems = object
                .iter()
                .filter(|(key, _)| key.as_str() != "code")
                .map(|(key, value)| format!("{key}: {}", compact(value)))
                .collect();
            (code, problems)
        }
        Value::Array(items) => {
            let problems = items
                .iter()
                .map(|item| {
                    let place = item["loc"]
                        .as_array()
                        .map(|loc| {
                            loc.iter()
                                .skip(1)
                                .map(|part| part.as_str().map_or(part.to_string(), str::to_owned))
                                .collect::<Vec<_>>()
                                .join(".")
                        })
                        .unwrap_or_default();
                    format!("{place}: {}", item["msg"].as_str().unwrap_or("invalid"))
                })
                .collect();
            ("invalid_request".to_owned(), problems)
        }
        _ => (format!("http_{status}"), Vec::new()),
    };
    ApiError {
        status,
        code,
        problems,
    }
}

fn compact(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The file name in `Content-Disposition: attachment; filename="…"`.
fn filename(header: &str) -> Option<String> {
    let start = header.find("filename=")? + "filename=".len();
    let name = header[start..].split(';').next()?.trim().trim_matches('"');
    // Only the name: never a path to write elsewhere.
    let name = name.rsplit(['/', '\\']).next()?;
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn urls_escape_their_segments() {
        let client = Client::new("https://api.example.org", None, false).unwrap();
        let url = client.url(&["tenants", "a b/c", "issuers"]);
        assert_eq!(
            url.as_str(),
            "https://api.example.org/api/v1/tenants/a%20b%2Fc/issuers"
        );
        let client = Client::new("http://localhost:8000/base/", None, false).unwrap();
        assert_eq!(
            client.url(&["auth", "me"]).as_str(),
            "http://localhost:8000/base/api/v1/auth/me"
        );
    }

    #[test]
    fn refusals_keep_the_apis_code() {
        let error = refusal(403, &json!({"detail": "not_a_signer"}));
        assert_eq!(error.code, "not_a_signer");
        assert!(error.to_string().contains("almena account link-wallet"));

        let error = refusal(
            422,
            &json!({"detail": {"code": "claims_invalid", "errors": {"x": 1}}}),
        );
        assert_eq!(error.code, "claims_invalid");
        assert_eq!(error.problems, vec![r#"errors: {"x":1}"#]);

        let error = refusal(
            422,
            &json!({"detail": [{"loc": ["body", "name"], "msg": "Field required"}]}),
        );
        assert_eq!(error.code, "invalid_request");
        assert_eq!(error.problems, vec!["name: Field required"]);

        assert_eq!(refusal(502, &Value::Null).code, "http_502");
    }

    #[test]
    fn file_names_are_names_only() {
        assert_eq!(
            filename(r#"attachment; filename="id.pdf""#).as_deref(),
            Some("id.pdf")
        );
        assert_eq!(
            filename("attachment; filename=../../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(filename("attachment"), None);
    }
}
