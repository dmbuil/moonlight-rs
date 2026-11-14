//! Server-only code: configuration, the encrypted session cookie, OpenStack
//! connections (gtema's `openstack_sdk`) and the JSON → model mappers.
//!
//! The server is stateless with respect to users: everything needed to serve a
//! request travels in the encrypted session cookie. The in-process caches below
//! only avoid re-validating the same Keystone token on every request and can be
//! dropped at any time.

pub mod actions;
pub mod config;
pub mod console;
pub mod downloads;
pub mod errors;
pub mod fetch;
pub mod heat;
pub mod listen;
pub mod logging;
pub mod mapping;
pub mod os;
pub mod paging;
pub mod session;
pub mod settings;
pub mod throttle;
pub mod upload;
pub mod websso;

use crate::model::AppError;

/// Security headers on every response (axum middleware). HSTS only when the
/// deployment is HTTPS (`[session] secure_cookies`); see [`add_security_headers`].
pub async fn security_headers(mut r: axum::response::Response) -> axum::response::Response {
    add_security_headers(r.headers_mut(), config::state().config.secure_cookies);
    r
}

/// The headers themselves, kept pure for tests. Headers already set win.
///
/// * No framing by another site (clickjacking; Horizon's `DISALLOW_IFRAME_EMBED`,
///   always on): `X-Frame-Options` and CSP `frame-ancestors`.
/// * CSP also forbids plugins (`object-src`) and `<base>` rewriting. No
///   `script-src`: Leptos hydration uses inline scripts.
/// * `nosniff`, and no `Referer` to other origins (URLs carry resource IDs).
/// * `https`: HSTS for a year, so the browser never tries plain HTTP again.
pub fn add_security_headers(h: &mut http::HeaderMap, https: bool) {
    use http::header::{
        CONTENT_SECURITY_POLICY, HeaderValue, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
        X_FRAME_OPTIONS,
    };
    h.entry(X_FRAME_OPTIONS).or_insert(HeaderValue::from_static("DENY"));
    h.entry(CONTENT_SECURITY_POLICY).or_insert(HeaderValue::from_static(
        "frame-ancestors 'none'; object-src 'none'; base-uri 'self'",
    ));
    h.entry(X_CONTENT_TYPE_OPTIONS)
        .or_insert(HeaderValue::from_static("nosniff"));
    h.entry(REFERRER_POLICY)
        .or_insert(HeaderValue::from_static("same-origin"));
    if https {
        h.entry(STRICT_TRANSPORT_SECURITY)
            .or_insert(HeaderValue::from_static("max-age=31536000"));
    }
}

/// Gzip for the static bundles, on the fly; only [`compressible`] responses.
/// A release build made with `cargo leptos build --precompress` has `.br`/`.gz`
/// files that leptos_axum sends as they are, and this layer leaves a response
/// that already has a `Content-Encoding` alone.
///
/// Gzip only, measured on the 44 MB dev WASM: gzip makes 4.5 MB in 0.28 s,
/// brotli (quality 4) 4.0 MB in 2.8 s. Over Tailscale (~3 MB/s) brotli's
/// encoder became the bottleneck, so it is slower overall.
pub fn compression() -> tower_http::compression::CompressionLayer<CompressWhen> {
    use tower_http::compression::CompressionLayer;
    fn when(_: http::StatusCode, _: http::Version, h: &http::HeaderMap, _: &http::Extensions) -> bool {
        compressible(h)
    }
    CompressionLayer::new()
        .no_br()
        .no_deflate()
        .no_zstd()
        .compress_when(when as CompressWhen)
}

/// The predicate type of [`compression`]: tower-http's signature, as a plain fn.
pub type CompressWhen = fn(http::StatusCode, http::Version, &http::HeaderMap, &http::Extensions) -> bool;

/// Whether a response may be compressed: only the WASM, JS and CSS bundles.
/// Pages and server-function results stay plain. They can carry secrets (an
/// application credential secret, a generated private key) next to text a
/// user typed, and compressing both together leaks the secret through the
/// response size (BREACH). Small files don't gain enough to be worth it either.
pub fn compressible(h: &http::HeaderMap) -> bool {
    let ty = h
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ty = ty.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    matches!(
        ty.as_str(),
        "application/wasm" | "text/javascript" | "application/javascript" | "text/css"
    )
}

/// Refuses state-changing requests from another origin: server
/// functions are POSTs, and the session cookie's `SameSite=Lax` doesn't stop a
/// sibling subdomain, nor login CSRF. Horizon uses Django's CSRF token.
pub async fn same_origin_only(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    use axum::response::IntoResponse;
    let cfg = &config::state().config;
    // Keystone posts the WebSSO token from its own origin: that one route
    // checks its nonce cookie instead (`websso::callback`).
    if websso::is_callback(req.method(), req.uri().path(), cfg.websso.is_some())
        || same_origin(req.method(), req.headers(), cfg.public_origin.as_deref())
    {
        return next.run(req).await;
    }
    let client = cfg
        .client_ip_header
        .as_ref()
        .and_then(|n| session::client_ip_from(req.headers(), n, cfg.trusted_proxies))
        .unwrap_or_else(|| "-".into());
    tracing::warn!(method = %req.method(), uri = %req.uri(), %client, "cross-origin request refused");
    (http::StatusCode::FORBIDDEN, "cross-origin request refused").into_response()
}

/// Whether a request may proceed: reads always; anything else when the browser
/// says it is same-origin (`Sec-Fetch-Site: same-origin`, or `none` for one the
/// user typed), or, from a browser without that header, when `Origin` is
/// `public` (`[session] public_url`) or, unset, has the request's `Host`. No
/// header to tell means no.
pub fn same_origin(method: &http::Method, h: &http::HeaderMap, public: Option<&str>) -> bool {
    if matches!(*method, http::Method::GET | http::Method::HEAD | http::Method::OPTIONS) {
        return true;
    }
    let get = |name: &str| h.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
    if let Some(site) = get("sec-fetch-site") {
        return site.eq_ignore_ascii_case("same-origin") || site.eq_ignore_ascii_case("none");
    }
    let Some(origin) = get("origin").filter(|o| !o.eq_ignore_ascii_case("null")) else {
        return false;
    };
    let origin = origin.to_ascii_lowercase();
    match public {
        Some(p) => origin == p,
        None => {
            let host = get("host").map(str::to_ascii_lowercase);
            host.is_some_and(|host| origin.split_once("://").is_some_and(|(_, o)| o == host))
        }
    }
}

/// Converts any displayable error into a classified `AppError`, logging it.
/// The browser only gets the full SDK text with `[ui] show_error_details`
///; otherwise the log keeps it (see [`errors::public`]).
pub fn sfe<E: std::fmt::Display>(context: &'static str) -> impl FnOnce(E) -> AppError {
    move |e| {
        tracing::warn!("{context}: {e}");
        // Before `config::init` (unit tests) there is no setting: keep everything.
        let show = config::try_state().is_none_or(|st| st.config.show_error_details);
        errors::public(errors::classify(context, &e.to_string()), context, show)
    }
}

#[cfg(test)]
mod tests {
    fn headers(pairs: &[(&'static str, &str)]) -> http::HeaderMap {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    http::HeaderName::from_static(k),
                    http::HeaderValue::from_str(v).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn writes_must_come_from_the_same_origin() {
        use http::Method;
        let ok = |m: &Method, h: &[(&'static str, &str)], p: Option<&str>| super::same_origin(m, &headers(h), p);
        assert!(
            ok(&Method::GET, &[("sec-fetch-site", "cross-site")], None),
            "reads pass"
        );
        assert!(ok(&Method::POST, &[("sec-fetch-site", "same-origin")], None));
        assert!(
            ok(&Method::POST, &[("sec-fetch-site", "none")], None),
            "typed by the user"
        );
        assert!(
            !ok(
                &Method::POST,
                &[
                    ("sec-fetch-site", "same-site"),
                    ("origin", "https://evil.example.com"),
                    ("host", "moonlight.example.com")
                ],
                None
            ),
            "a sibling subdomain"
        );
        assert!(!ok(&Method::POST, &[("sec-fetch-site", "cross-site")], None));
        // Browsers without Sec-Fetch-*: Origin against Host, or against the public URL.
        let host = ("host", "moonlight.example.com:8443");
        assert!(ok(
            &Method::POST,
            &[("origin", "https://Moonlight.example.com:8443"), host],
            None
        ));
        assert!(!ok(
            &Method::POST,
            &[("origin", "https://other.example.com"), host],
            None
        ));
        assert!(
            !ok(&Method::POST, &[("origin", "null"), host], None),
            "sandboxed or opaque origins"
        );
        assert!(!ok(&Method::POST, &[host], None), "nothing to tell: refused");
        let proxied = [("origin", "https://cloud.example.com"), ("host", "10.0.0.5:3000")];
        assert!(!ok(&Method::POST, &proxied, None), "a proxy rewrote Host…");
        assert!(
            ok(&Method::POST, &proxied, Some("https://cloud.example.com")),
            "…so [session] public_url is compared instead"
        );
        assert!(!ok(
            &Method::DELETE,
            &[("origin", "https://evil.example"), ("host", "evil.example")],
            Some("https://cloud.example.com")
        ));
    }

    #[test]
    fn only_static_bundles_are_compressed() {
        let ok = |ty: &str| super::compressible(&headers(&[("content-type", ty)]));
        assert!(ok("application/wasm"), "the debug WASM is ~140 MB");
        assert!(ok("text/javascript; charset=utf-8"));
        assert!(ok("application/javascript"));
        assert!(ok("Text/CSS"));
        assert!(
            !ok("text/html; charset=utf-8"),
            "pages can hold secrets next to user input (BREACH)"
        );
        assert!(!ok("application/json"), "server function results, same reason");
        assert!(!ok("image/png"));
        assert!(
            !super::compressible(&http::HeaderMap::new()),
            "no Content-Type: left alone"
        );
    }

    #[test]
    fn responses_carry_the_security_headers() {
        let mut h = http::HeaderMap::new();
        super::add_security_headers(&mut h, false);
        assert_eq!(h["x-frame-options"], "DENY");
        assert_eq!(
            h["content-security-policy"],
            "frame-ancestors 'none'; object-src 'none'; base-uri 'self'"
        );
        assert_eq!(
            (
                h["x-content-type-options"].to_str().unwrap(),
                h["referrer-policy"].to_str().unwrap()
            ),
            ("nosniff", "same-origin")
        );
        assert!(!h.contains_key("strict-transport-security"), "no HSTS over plain HTTP");
        let mut h = http::HeaderMap::new();
        h.insert("x-frame-options", http::HeaderValue::from_static("SAMEORIGIN"));
        super::add_security_headers(&mut h, true);
        assert_eq!(h["strict-transport-security"], "max-age=31536000");
        assert_eq!(h["x-frame-options"], "SAMEORIGIN", "a header set earlier wins");
    }
}
