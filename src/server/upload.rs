//! *Create Image* › *File* (roadmap 45): the one write that is streamed, so it cannot go
//! through [`super::actions::Exec`] (the SDK's bodies are whole `Vec<u8>`s). The record is
//! created by `OsAction::ImageCreate`; the data follows here, in one of two modes
//! (`[images] upload`):
//!
//! * `proxy`: [`proxy`] receives the browser's `PUT`, checks the session, the write mode and
//!   the size, and forwards the body chunk by chunk to Glance's `PUT /v2/images/{id}/file`.
//!   Nothing is buffered or written to disk.
//! * `direct`: [`ticket`] hands the page the upload URL and the session's scoped token for
//!   that single request, and the browser `PUT`s to Glance itself.
//!
//! Both send only with `[writes] mode = on`, refuse files over `[images] max_upload_gb`, and
//! never log the token (AGENTS.md §1.2, §1.5–6).

use std::sync::OnceLock;

use axum::body::Body;
use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode};
use openstack_sdk::AsyncOpenStack;
use secrecy::ExposeSecret;

use super::config::state;
use super::{os, session};
use openstack_sdk::api::AsyncClient;
use crate::model::{check_id, AppError, UploadTicket, WriteMode};
use crate::prefs::ImageUpload;

/// Glance's upload URL for `id`: the image endpoint from the catalog, which may or may not
/// carry its version (`https://glance:9292` or `…/v2`).
pub fn file_url(endpoint: &str, id: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    let base = if base.ends_with("/v2") { base.to_string() } else { format!("{base}/v2") };
    format!("{base}/images/{id}/file")
}

/// Whether an upload of `bytes` is acceptable under the settings: the mode is on (`want`),
/// the file is not empty and not over `[images] max_upload_gb`.
pub fn allowed(upload: ImageUpload, want: ImageUpload, max_bytes: u64, bytes: u64) -> Result<(), String> {
    if upload != want {
        return Err(format!("file uploads are not enabled in this mode ([images] upload = {upload:?})").to_lowercase());
    }
    if bytes == 0 {
        return Err("the file is empty".into());
    }
    if bytes > max_bytes {
        return Err(format!("the file is larger than [images] max_upload_gb ({} GiB)", max_bytes >> 30));
    }
    Ok(())
}

/// A client for the streamed body: the SDK's TLS settings (`[tls]`), and no total timeout,
/// since a large upload takes as long as it takes.
fn client() -> Result<&'static reqwest::Client, AppError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let cfg = &state().config;
            let mut b = reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(10));
            if let Some(f) = &cfg.ca_file {
                let pem = std::fs::read(f).map_err(|e| format!("[tls] ca_file {f}: {e}"))?;
                for c in reqwest::Certificate::from_pem_bundle(&pem).map_err(|e| e.to_string())? {
                    b = b.add_root_certificate(c);
                }
            }
            if !cfg.verify_tls {
                b = b.danger_accept_invalid_certs(true);
            }
            b.build().map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| AppError::internal(format!("upload client: {e}")))
}

/// The scoped token and Glance's upload URL for `id`.
async fn target(os: &AsyncOpenStack, id: &str) -> Result<(String, String), AppError> {
    let token = os.get_auth_token().ok_or_else(|| AppError::internal("no token for the image service"))?;
    let ep = os
        .get_service_endpoint(&openstack_sdk::types::ServiceType::Image, None)
        .await
        .map_err(super::sfe("image service endpoint"))?;
    Ok((token.expose_secret().to_string(), file_url(ep.url_str(), id)))
}

/// What both modes check before touching Glance: the session, the mode and the settings.
async fn gate(headers: &HeaderMap, id: &str, want: ImageUpload, bytes: u64) -> Result<(session::Session, AsyncOpenStack), (StatusCode, String)> {
    let cfg = &state().config;
    let s = session::from_headers(headers)
        .filter(|s| !s.is_expired(chrono::Utc::now(), cfg.session_timeout, cfg.token_margin))
        .ok_or((StatusCode::UNAUTHORIZED, "sign in again".to_string()))?;
    check_id(id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let img = &cfg.prefs.images;
    allowed(img.upload, want, img.max_upload_bytes(), bytes).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if cfg.writes != WriteMode::On {
        return Err((StatusCode::CONFLICT, "writes are disabled ([writes] mode)".into()));
    }
    let c = os::scoped(&s).await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok((s, c))
}

/// The `Content-Length` of the browser's request, which the size check goes by: a body without
/// one (chunked) is refused, so the limit is known before anything is forwarded.
fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers.get(http::header::CONTENT_LENGTH)?.to_str().ok()?.parse().ok()
}

/// `PUT /upload/image/{id}` (`proxy` mode): streams the request body to Glance.
pub async fn proxy(Path(id): Path<String>, headers: HeaderMap, body: Body) -> Response {
    let Some(len) = declared_length(&headers) else {
        return (StatusCode::LENGTH_REQUIRED, "the upload needs a Content-Length").into_response();
    };
    let (s, c) = match gate(&headers, &id, ImageUpload::Proxy, len).await {
        Ok(x) => x,
        Err((code, msg)) => return (code, msg).into_response(),
    };
    let (token, url) = match target(&c, &id).await {
        Ok(x) => x,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };
    let client = match client() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    // A browser that lies about the length can't send more than it declared: the stream
    // stops at `len` and fails, which aborts the request to Glance.
    let mut seen = 0u64;
    let stream = body.into_data_stream().map(move |chunk| {
        let chunk = chunk.map_err(|e| std::io::Error::other(e.to_string()))?;
        seen += chunk.len() as u64;
        if seen > len {
            return Err(std::io::Error::other("the body is longer than its Content-Length"));
        }
        Ok::<_, std::io::Error>(chunk)
    });
    let started = std::time::Instant::now();
    let sent = client
        .put(&url)
        .header("X-Auth-Token", token)
        .header(http::header::CONTENT_TYPE, "application/octet-stream")
        .header(http::header::CONTENT_LENGTH, len)
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await;
    let client_ip = session::client_ip_of(&headers);
    match sent {
        Ok(r) if r.status().is_success() => {
            tracing::info!(user = %s.user, project = %s.project_name, client = %client_ip, image = %id, bytes = len, secs = started.elapsed().as_secs(), "image upload (proxy)");
            super::fetch::forget_lookups(&os::scoped_key(&s.token, &s.project_id, s.region.as_deref())).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(r) => {
            let status = r.status();
            let why = r.text().await.unwrap_or_default();
            tracing::warn!(user = %s.user, client = %client_ip, image = %id, %status, "image upload (proxy) refused by the image service");
            (StatusCode::BAD_GATEWAY, format!("the image service answered {status}: {}", why.chars().take(300).collect::<String>())).into_response()
        }
        Err(e) => {
            tracing::warn!(user = %s.user, client = %client_ip, image = %id, "image upload (proxy) failed: {e}");
            (StatusCode::BAD_GATEWAY, format!("the upload failed: {e}")).into_response()
        }
    }
}

/// `direct` mode: the URL and the scoped token for one `PUT` of `bytes` to `id`'s data.
/// The token only travels in this response, and is never logged.
pub async fn ticket(headers: &HeaderMap, id: &str, bytes: u64) -> Result<UploadTicket, AppError> {
    let (s, c) = gate(headers, id, ImageUpload::Direct, bytes).await.map_err(|(_, e)| AppError::invalid(e))?;
    let (token, url) = target(&c, id).await?;
    tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip_of(headers), image = %id, bytes, "image upload (direct): ticket issued");
    Ok(UploadTicket { url, token })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::image_upload_path;

    #[test]
    fn glances_upload_url_is_versioned_once() {
        let want = "https://glance.example:9292/v2/images/abc/file";
        assert_eq!(file_url("https://glance.example:9292", "abc"), want);
        assert_eq!(file_url("https://glance.example:9292/", "abc"), want);
        assert_eq!(file_url("https://glance.example:9292/v2/", "abc"), want, "discovery may add the version");
    }

    #[test]
    fn uploads_follow_the_mode_and_the_size_limit() {
        let gib = 1u64 << 30;
        let ok = |mode, want, bytes| allowed(mode, want, 2 * gib, bytes);
        assert!(ok(ImageUpload::Proxy, ImageUpload::Proxy, gib).is_ok());
        assert!(ok(ImageUpload::Proxy, ImageUpload::Proxy, 2 * gib).is_ok(), "the limit itself fits");
        assert!(ok(ImageUpload::Proxy, ImageUpload::Proxy, 2 * gib + 1).unwrap_err().contains("max_upload_gb"));
        assert!(ok(ImageUpload::Proxy, ImageUpload::Proxy, 0).is_err(), "empty file");
        assert!(ok(ImageUpload::Off, ImageUpload::Proxy, 1).is_err(), "off");
        assert!(ok(ImageUpload::Direct, ImageUpload::Proxy, 1).is_err(), "the proxy route is not open in direct mode");
        assert!(ok(ImageUpload::Proxy, ImageUpload::Direct, 1).is_err(), "no tickets in proxy mode");
    }

    #[test]
    fn the_browser_must_declare_the_length() {
        let mut h = HeaderMap::new();
        assert_eq!(declared_length(&h), None);
        h.insert(http::header::CONTENT_LENGTH, "1048576".parse().unwrap());
        assert_eq!(declared_length(&h), Some(1_048_576));
        assert_eq!(image_upload_path("abc"), "/upload/image/abc");
    }
}
