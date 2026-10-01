//! Raw REST helpers for the endpoints the SDK does not wrap.
//!
//! Dakera v0.12 answers every error with a JSON body
//! (`{"error", "code", "status", "details"}`), carries `Retry-After` on every
//! `503` and uses `413` (over a size limit or a hard quota), `501` (a feature
//! that is switched off or a configuration the server cannot serve) and `403`
//! (a key that lacks the scope, or is pinned to namespaces, for a node-wide
//! route). Every non-2xx answer becomes an [`ApiHttpError`] that keeps those
//! fields, so the CLI can say what to do instead of printing raw JSON.

use std::fmt;

use anyhow::Result;
use dakera_client::reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde_json::Value;

use crate::context::Context;

/// A non-2xx answer from the server.
#[derive(Debug, Clone)]
pub struct ApiHttpError {
    /// HTTP status code.
    pub status: u16,
    /// The status with its reason phrase (`403 Forbidden`).
    pub status_text: String,
    /// The server's machine-readable error code (`INSUFFICIENT_SCOPE`, ...).
    pub code: Option<String>,
    /// The server's human-readable message.
    pub message: String,
    /// The server's `details` field.
    pub details: Option<String>,
    /// Seconds from the `Retry-After` header (every v0.12 `503` carries one).
    pub retry_after_secs: Option<u64>,
}

impl ApiHttpError {
    /// Build the error from a status, the `Retry-After` header and the body.
    pub fn from_parts(status: StatusCode, retry_after_secs: Option<u64>, body: &str) -> Self {
        let parsed: Option<Value> = serde_json::from_str(body).ok();
        let field = |name: &str| -> Option<String> {
            parsed
                .as_ref()
                .and_then(|v| v.get(name))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let status_text = status.to_string();
        // The authentication middleware's 401 carries `message`; every other
        // error carries `error`; the startup gate's 503 carries `reason`.
        let message = field("message")
            .or_else(|| field("error"))
            .or_else(|| field("reason"))
            .unwrap_or_else(|| {
                let trimmed = body.trim();
                if trimmed.is_empty() {
                    status_text.clone()
                } else {
                    trimmed.to_string()
                }
            });
        Self {
            status: status.as_u16(),
            status_text,
            code: field("code"),
            message,
            details: field("details"),
            retry_after_secs,
        }
    }

    fn details_contain(&self, needle: &str) -> bool {
        match &self.details {
            Some(d) => d.contains(needle),
            None => false,
        }
    }

    /// What the operator can do about this error, when the server's answer
    /// has a well-known meaning in v0.12.
    pub fn hint(&self) -> Option<String> {
        let code = self.code.as_deref().unwrap_or("");
        if self.status == 503 || self.status == 429 {
            return Some(match self.retry_after_secs {
                Some(secs) => format!("the server asks to retry in {secs}s (Retry-After)"),
                None => HINT_BUSY.to_string(),
            });
        }
        let pinned = self.details_contain("namespace: *");
        let super_admin = self.details_contain("required: super_admin");
        let hint = match (self.status, code) {
            (401, _) => HINT_AUTH,
            (403, "NAMESPACE_ACCESS_DENIED") if pinned => HINT_PINNED,
            (403, "NAMESPACE_ACCESS_DENIED") => HINT_NAMESPACE,
            (403, "INSUFFICIENT_SCOPE") if super_admin => HINT_SUPER_ADMIN,
            (403, "INSUFFICIENT_SCOPE") => HINT_SCOPE,
            (403, "CROSS_ORIGIN_REQUEST_REFUSED") => HINT_CROSS_ORIGIN,
            (413, "QUOTA_EXCEEDED") => HINT_QUOTA,
            (413, _) => HINT_TOO_LARGE,
            (501, "FEATURE_DISABLED") => HINT_DISABLED,
            (501, _) => HINT_UNSUPPORTED,
            _ => return None,
        };
        Some(hint.to_string())
    }
}

const HINT_AUTH: &str = "authentication failed: set DAKERA_API_KEY to a valid, unexpired API key";
const HINT_BUSY: &str = "the server is busy or starting; retry shortly";
const HINT_PINNED: &str = "this API key is pinned to namespaces; since v0.12 such a key is \
     refused on node-wide routes (/admin/*: backups, encryption, quotas, config). Use a key that \
     is not pinned to a namespace.";
const HINT_NAMESPACE: &str =
    "this API key cannot reach that namespace; use a key whose namespace list includes it";
const HINT_SUPER_ADMIN: &str = "this needs a global super_admin key: since v0.12 backup \
     download, upload and restore are refused to admin keys (a backup bundle carries every API \
     key hash).";
const HINT_SCOPE: &str =
    "this API key's scope is too low; `required` in the details names the scope the route needs";
const HINT_CROSS_ORIGIN: &str = "authentication is off on the server and the request looked like \
     a web page of another origin; turn authentication on or call the server from a client";
const HINT_QUOTA: &str =
    "a hard namespace quota is exceeded (enforced since v0.12); see `dk admin quotas-get`";
const HINT_TOO_LARGE: &str = "the request body is larger than the server accepts \
     (DAKERA_MAX_BODY_SIZE, or DAKERA_ATTACHMENT_MAX_BYTES for an attachment; `dk capabilities` \
     shows the attachment limit)";
const HINT_DISABLED: &str = "this feature is switched off on the server; the message names the \
     environment variable that turns it on (`dk capabilities` shows what is enabled)";
const HINT_UNSUPPORTED: &str = "the server's configuration cannot serve this request; the \
     details name the settings to change";

impl fmt::Display for ApiHttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request failed ({})", self.status_text)?;
        if let Some(code) = &self.code {
            write!(f, " [{code}]")?;
        }
        write!(f, ": {}", self.message)?;
        if let Some(details) = &self.details {
            write!(f, " ({details})")?;
        }
        if let Some(hint) = self.hint() {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiHttpError {}

/// Send a request, mapping a transport failure to a readable error.
pub async fn send(req: RequestBuilder, method: &Method, path: &str) -> Result<Response> {
    match req.send().await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            if e.is_connect() || e.is_timeout() {
                anyhow::bail!("Connection error: {method} {path}: {e}")
            }
            anyhow::bail!("Failed to {method} {path}: {e}")
        }
    }
}

/// The `Retry-After` header of a response, in seconds.
fn retry_after_of(resp: &Response) -> Option<u64> {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// The answer of a probe route, whatever its status.
pub struct Probe {
    pub status: u16,
    pub retry_after_secs: Option<u64>,
    /// The JSON body (`null` when it is not JSON).
    pub body: Value,
    /// The body as text.
    pub text: String,
}

impl Probe {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// This answer as an [`ApiHttpError`] (for a non-2xx status).
    pub fn into_error(self) -> ApiHttpError {
        let fallback = StatusCode::INTERNAL_SERVER_ERROR;
        let status = StatusCode::from_u16(self.status).unwrap_or(fallback);
        ApiHttpError::from_parts(status, self.retry_after_secs, &self.text)
    }
}

/// GET `path` and return its status, `Retry-After` and body without turning a
/// non-2xx answer into an error: `/health` and `/health/ready` answer `503`
/// with a JSON body while the server starts.
pub async fn probe(ctx: &Context, path: &str) -> Result<Probe> {
    let client = crate::commands::authed_client();
    let req = client.get(format!("{}{}", ctx.url, path));
    let t = ctx.log_request("GET", path);
    let sent = send(req, &Method::GET, path).await;
    ctx.log_response(t, if sent.is_ok() { "OK" } else { "ERR" });
    let resp = sent?;
    let status = resp.status().as_u16();
    let retry_after_secs = retry_after_of(&resp);
    let text = resp.text().await.unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(Probe {
        status,
        retry_after_secs,
        body,
        text,
    })
}

/// Turn a non-2xx response into an [`ApiHttpError`]; pass a 2xx one through.
pub async fn ensure_success(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let retry_after = retry_after_of(&resp);
    let body = resp.text().await.unwrap_or_default();
    let err = ApiHttpError::from_parts(status, retry_after, &body);
    Err(anyhow::Error::new(err))
}

/// Read a response body as JSON; an empty body is `{}`.
pub async fn json_of(resp: Response) -> Result<Value> {
    let text = resp.text().await?;
    if text.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let parsed: Result<Value, _> = serde_json::from_str(&text);
    parsed.map_err(|e| anyhow::anyhow!("Failed to parse response JSON: {e}"))
}

/// One JSON request. `body` is sent as JSON when present.
pub async fn request_json(
    url: &str,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> Result<Value> {
    let client = crate::commands::authed_client();
    let mut req = client.request(method.clone(), format!("{url}{path}"));
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = send(req, &method, path).await?;
    let resp = ensure_success(resp).await?;
    json_of(resp).await
}

/// [`request_json`] with the verbose request/response log lines.
pub async fn request_json_logged(
    ctx: &Context,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> Result<Value> {
    let t = ctx.log_request(method.as_str(), path);
    let result = request_json(&ctx.url, method, path, body).await;
    ctx.log_response(t, if result.is_ok() { "OK" } else { "ERR" });
    result
}

/// GET `path` and return the raw bytes of a successful answer.
pub async fn get_bytes(ctx: &Context, path: &str) -> Result<Vec<u8>> {
    let client = crate::commands::authed_client();
    let req = client.get(format!("{}{}", ctx.url, path));
    let t = ctx.log_request("GET", path);
    let resp = send(req, &Method::GET, path).await;
    let result = match resp {
        Ok(r) => ensure_success(r).await,
        Err(e) => Err(e),
    };
    ctx.log_response(t, if result.is_ok() { "OK" } else { "ERR" });
    let bytes = result?.bytes().await?;
    Ok(bytes.to_vec())
}

/// POST raw bytes with a `Content-Type` and return the JSON answer.
pub async fn post_bytes(
    ctx: &Context,
    path: &str,
    content_type: &str,
    data: Vec<u8>,
) -> Result<Value> {
    let client = crate::commands::authed_client();
    let req = client
        .post(format!("{}{}", ctx.url, path))
        .header("Content-Type", content_type)
        .body(data);
    let t = ctx.log_request("POST", path);
    let resp = send(req, &Method::POST, path).await;
    let result = match resp {
        Ok(r) => ensure_success(r).await,
        Err(e) => Err(e),
    };
    ctx.log_response(t, if result.is_ok() { "OK" } else { "ERR" });
    json_of(result?).await
}

/// A string field of a JSON object.
pub fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// An unsigned integer field of a JSON object.
pub fn u64_of(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64())
}

/// A boolean field of a JSON object.
pub fn bool_of(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|x| x.as_bool())
}

/// A string addressed by a JSON pointer (`/error/code`).
pub fn ptr_str<'a>(v: &'a Value, pointer: &str) -> Option<&'a str> {
    v.pointer(pointer).and_then(|x| x.as_str())
}

/// An unsigned integer addressed by a JSON pointer (`/resources/open_fds`).
pub fn ptr_u64(v: &Value, pointer: &str) -> Option<u64> {
    v.pointer(pointer).and_then(|x| x.as_u64())
}

/// Percent-encode one URL path segment. `:` stays as it is (an attachment
/// reference is `sha256:<hex>`; a colon is a valid path character).
pub fn segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The error an answer with this status, code and details becomes.
    fn answer(status: u16, code: &str, error: &str, details: Option<&str>) -> ApiHttpError {
        let body = json!({ "error": error, "code": code, "status": status, "details": details });
        let status = StatusCode::from_u16(status).unwrap();
        ApiHttpError::from_parts(status, None, &body.to_string())
    }

    fn raw(status: u16, retry: Option<u64>, body: &str) -> ApiHttpError {
        let status = StatusCode::from_u16(status).unwrap();
        ApiHttpError::from_parts(status, retry, body)
    }

    #[test]
    fn parses_the_v012_error_body() {
        let details = Some("required: super_admin, actual: admin");
        let e = answer(403, "INSUFFICIENT_SCOPE", "Insufficient scope", details);
        assert_eq!(e.status, 403);
        assert_eq!(e.code.as_deref(), Some("INSUFFICIENT_SCOPE"));
        assert_eq!(e.message, "Insufficient scope");
        let text = e.to_string();
        assert!(text.contains("[INSUFFICIENT_SCOPE]"));
        assert!(text.contains("super_admin"));
        assert!(text.contains("hint:"));
    }

    #[test]
    fn auth_middleware_401_uses_message() {
        let body = json!({
            "error": "authentication_error",
            "message": "API key required",
            "code": "AUTHENTICATION_REQUIRED",
            "status": 401
        });
        let e = raw(401, None, &body.to_string());
        assert_eq!(e.message, "API key required");
        assert!(e.hint().unwrap().contains("DAKERA_API_KEY"));
    }

    #[test]
    fn namespace_pinned_key_hint() {
        let code = "NAMESPACE_ACCESS_DENIED";
        let e = answer(403, code, "Access denied", Some("namespace: *"));
        let hint = e.hint().unwrap();
        assert!(hint.contains("pinned"));
        assert!(hint.contains("node-wide"));
    }

    #[test]
    fn namespace_denied_for_one_namespace_has_its_own_hint() {
        let code = "NAMESPACE_ACCESS_DENIED";
        let e = answer(403, code, "Access denied", Some("namespace: a"));
        assert!(e.hint().unwrap().contains("cannot reach that namespace"));
    }

    #[test]
    fn super_admin_hint_for_backup_routes() {
        let details = Some("required: super_admin, actual: admin");
        let e = answer(403, "INSUFFICIENT_SCOPE", "Insufficient scope", details);
        assert!(e.hint().unwrap().contains("global super_admin key"));
    }

    #[test]
    fn lower_scope_hint_when_not_super_admin() {
        let details = Some("required: admin, actual: read");
        let e = answer(403, "INSUFFICIENT_SCOPE", "Insufficient scope", details);
        assert!(e.hint().unwrap().contains("scope is too low"));
    }

    #[test]
    fn retry_after_on_503() {
        let body = json!({ "error": "the server is starting", "code": "SERVICE_UNAVAILABLE" });
        let e = raw(503, Some(5), &body.to_string());
        assert_eq!(e.retry_after_secs, Some(5));
        assert!(e.to_string().contains("retry in 5s"));
    }

    #[test]
    fn a_503_without_retry_after_still_gets_a_hint() {
        let e = answer(503, "SERVICE_UNAVAILABLE", "busy", None);
        assert!(e.hint().unwrap().contains("retry shortly"));
    }

    #[test]
    fn payload_too_large_and_quota_are_told_apart() {
        let big = answer(413, "PAYLOAD_TOO_LARGE", "too big", None);
        assert!(big.hint().unwrap().contains("DAKERA_ATTACHMENT_MAX_BYTES"));
        let quota = answer(413, "QUOTA_EXCEEDED", "quota", None);
        assert!(quota.hint().unwrap().contains("quota"));
    }

    #[test]
    fn feature_disabled_and_not_implemented_501() {
        let details = Some("set DAKERA_ATTACHMENTS=1");
        let off = answer(501, "FEATURE_DISABLED", "disabled", details);
        assert!(off.hint().unwrap().contains("switched off"));
        assert_eq!(off.details.as_deref(), Some("set DAKERA_ATTACHMENTS=1"));
        let cfg = answer(501, "NOT_IMPLEMENTED", "unsupported", None);
        assert!(cfg.hint().unwrap().contains("configuration"));
    }

    #[test]
    fn non_json_body_is_kept_as_the_message() {
        let e = raw(502, None, "bad gateway from proxy");
        assert_eq!(e.message, "bad gateway from proxy");
        assert!(e.code.is_none());
        assert!(e.hint().is_none());
    }

    #[test]
    fn startup_gate_body_uses_reason() {
        let body = json!({ "ready": false, "starting": true, "reason": "loading models" });
        let e = raw(503, Some(5), &body.to_string());
        assert_eq!(e.message, "loading models");
    }

    #[test]
    fn empty_body_falls_back_to_the_status() {
        let e = raw(404, None, "");
        assert_eq!(e.message, "404 Not Found");
    }

    #[test]
    fn segment_encodes_reserved_characters() {
        assert_eq!(segment("sha256:ab"), "sha256:ab");
        assert_eq!(segment("my-ns_1.x"), "my-ns_1.x");
        assert_eq!(segment("a b/c"), "a%20b%2Fc");
    }
}
