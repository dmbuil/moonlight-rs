//! WebSSO sign-in through Keystone federation (roadmap 80), as Horizon does it
//! with `WEBSSO_ENABLED`: the browser leaves for
//! `{keystone}/v3/auth/OS-FEDERATION/…/websso`, Keystone's Apache does the
//! identity provider's dance (OIDC, SAML), and Keystone posts an unscoped token
//! back to [`CALLBACK`], which signs in with it like a password login.
//!
//! That POST is cross-site, so `same_origin_only` lets it through and the
//! callback guards against login CSRF (an attacker posting their own token)
//! itself: [`start`] sets an encrypted, one-use [`NONCE_COOKIE`] and the
//! callback accepts a token only from a browser that has it. Keystone checks
//! `origin` by exact string, so the nonce can't travel in the URL.
//!
//! The session cookie is set in the answer to that cross-site POST, as Horizon
//! does: browsers keep `SameSite=Lax` cookies from a top-level navigation's
//! response, and send them on the `303` to `/project` (checked on ttcp-dev).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use cookie::{Cookie, CookieJar, Key, SameSite};

/// Moonlight's path that Keystone posts the token to. With `[session]
/// public_url`'s origin in front, it is the `origin` Keystone checks against
/// `[federation] trusted_dashboard`.
pub const CALLBACK: &str = "/auth/websso";

/// The login page's link for a federated choice: `?choice=<id>`.
pub const START: &str = crate::prefs::WEBSSO_START;

/// The nonce cookie. `SameSite=None` so it comes back on Keystone's
/// cross-site POST, which needs `Secure`; the `__Host-` prefix keeps sibling
/// subdomains from planting one. Browsers accept `Secure` cookies on
/// `http://localhost` and `127.0.0.1`, so development works over plain HTTP.
pub const NONCE_COOKIE: &str = "__Host-ml_sso";

/// How long the user has to sign in at the identity provider.
pub const NONCE_SECONDS: i64 = 10 * 60;

/// The nonce's encrypted contents: its expiry and the choice the user picked
/// (Keystone's POST doesn't say which). AES-GCM encryption is randomised, so
/// every cookie value is unique and its hash names it.
#[derive(serde::Serialize, serde::Deserialize)]
struct Nonce {
    expires: i64,
    choice: String,
}

/// Why the callback sent the browser back to the login page: `/login?sso=…`,
/// read by the login page to show the matching message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No valid nonce: not started from this browser, too slow, or replayed.
    NotStarted,
    /// Keystone refused the token or could not be reached.
    Failed,
    /// Signed in, but the user has no project.
    NoProjects,
}

impl Refusal {
    /// The `sso` query value; must match `pages::login`.
    pub fn code(self) -> &'static str {
        match self {
            Refusal::NotStarted => "expired",
            Refusal::Failed => "failed",
            Refusal::NoProjects => "noprojects",
        }
    }
}

/// Validated `[websso]` (see `settings::websso`).
#[derive(Clone, Debug, PartialEq)]
pub struct WebSso {
    /// Keystone as the browser reaches it, without `/v3` or a trailing `/`.
    keystone: url::Url,
    /// `{public origin}{CALLBACK}`, sent as `origin`.
    origin: String,
    /// IDs of the federated choices (every choice but `credentials`).
    protocols: Vec<String>,
    /// Choice ID → `(identity provider, protocol)`.
    idp_mapping: BTreeMap<String, (String, String)>,
    /// `[websso] logout_url`: where logout sends a WebSSO session's browser
    /// (`api::logout`), to end the identity provider's session too.
    pub logout_url: Option<String>,
}

impl WebSso {
    /// `keystone` may end in `/` or `/v3` (Horizon's `OPENSTACK_KEYSTONE_URL`
    /// does); `public_origin` is `scheme://host[:port]`.
    pub fn new(
        keystone: &str,
        public_origin: &str,
        protocols: Vec<String>,
        idp_mapping: BTreeMap<String, (String, String)>,
        logout_url: Option<String>,
    ) -> Result<Self, String> {
        let base = keystone.trim_end_matches('/');
        let base = base.strip_suffix("/v3").unwrap_or(base);
        let keystone = url::Url::parse(base).map_err(|e| format!("{e} (got {keystone:?})"))?;
        if keystone.cannot_be_a_base() || keystone.query().is_some() {
            return Err(format!("not a base URL (got {keystone:?})"));
        }
        Ok(WebSso { keystone, origin: format!("{public_origin}{CALLBACK}"), protocols, idp_mapping, logout_url })
    }

    /// Where to send the browser for `choice`, or `None` when it is not a
    /// federated choice (so the browser's input never shapes the URL beyond
    /// picking one). Keystone's two shapes: `…/OS-FEDERATION/websso/{protocol}`,
    /// or with `idp_mapping`
    /// `…/OS-FEDERATION/identity_providers/{idp}/protocols/{protocol}/websso`.
    pub fn start_url(&self, choice: &str) -> Option<String> {
        if !self.protocols.iter().any(|p| p == choice) {
            return None;
        }
        let mut u = self.keystone.clone();
        {
            // `push` percent-encodes each segment: an IdP ID with `/` or `?`
            // stays one segment.
            let mut path = u.path_segments_mut().ok()?;
            path.pop_if_empty().extend(["v3", "auth", "OS-FEDERATION"]);
            match self.idp_mapping.get(choice) {
                Some((idp, protocol)) => path.extend(["identity_providers", idp, "protocols", protocol, "websso"]),
                None => path.extend(["websso", choice]),
            };
        }
        u.query_pairs_mut().append_pair("origin", &self.origin);
        Some(u.into())
    }

    /// The identity provider and protocol of a federated `choice`: the
    /// mapping's, else no provider and the choice ID as protocol (Keystone's
    /// `…/websso/{protocol}` URL). `None` for any other choice.
    pub fn via(&self, choice: &str) -> Option<super::session::SsoVia> {
        if !self.protocols.iter().any(|p| p == choice) {
            return None;
        }
        Some(match self.idp_mapping.get(choice) {
            Some((idp, protocol)) => super::session::SsoVia { identity_provider: Some(idp.clone()), protocol: protocol.clone() },
            None => super::session::SsoVia { identity_provider: None, protocol: choice.to_string() },
        })
    }

    /// Keystone's origin, the only `Origin` the callback's POST may carry
    /// besides `null`.
    pub fn keystone_origin(&self) -> String {
        self.keystone.origin().ascii_serialization()
    }
}

/// Whether a request is Keystone's token POST, which `same_origin_only` lets
/// through (only with WebSSO on, and only this path).
pub fn is_callback(method: &http::Method, path: &str, enabled: bool) -> bool {
    enabled && method == http::Method::POST && path == CALLBACK
}

/// `Set-Cookie` of a new nonce for `choice`, valid until `now + NONCE_SECONDS`.
pub fn nonce_cookie(key: &Key, now: DateTime<Utc>, choice: &str) -> String {
    let nonce = serde_json::to_string(&Nonce { expires: now.timestamp() + NONCE_SECONDS, choice: choice.into() }).expect("nonce serialises");
    let mut jar = CookieJar::new();
    jar.private_mut(key).add(nonce_attrs(Cookie::build((NONCE_COOKIE, nonce))).max_age(cookie::time::Duration::seconds(NONCE_SECONDS)));
    jar.get(NONCE_COOKIE).expect("cookie just added").to_string()
}

/// `Set-Cookie` that deletes the nonce.
pub fn nonce_removal() -> String {
    nonce_attrs(Cookie::build((NONCE_COOKIE, ""))).max_age(cookie::time::Duration::ZERO).build().to_string()
}

fn nonce_attrs(c: cookie::CookieBuilder<'static>) -> cookie::CookieBuilder<'static> {
    c.path("/").http_only(true).secure(true).same_site(SameSite::None)
}

/// The nonce in `cookie_header` if it decrypts under `key` and has not
/// expired: the hash of the cookie value (the key for the replay check) and
/// the choice it was issued for.
pub fn valid_nonce(cookie_header: &str, key: &Key, now: DateTime<Utc>) -> Option<(String, String)> {
    let raw = Cookie::split_parse(cookie_header.to_owned()).flatten().find(|c| c.name() == NONCE_COOKIE)?.into_owned();
    let mut jar = CookieJar::new();
    jar.add_original(raw.clone());
    let plain = jar.private(key).get(NONCE_COOKIE)?;
    let nonce: Nonce = serde_json::from_str(plain.value()).ok()?;
    (nonce.expires > now.timestamp()).then(|| (super::os::hash_key(&["websso-nonce", raw.value()]), nonce.choice))
}

/// Keystone's auto-submitting form sends `Origin: <keystone>`, or `null` when
/// its page has `Referrer-Policy: no-referrer`; anything else is another site.
/// No header at all: an old browser, left to the nonce.
pub fn origin_allowed(headers: &http::HeaderMap, keystone_origin: &str) -> bool {
    match headers.get(http::header::ORIGIN).and_then(|v| v.to_str().ok()).map(str::trim) {
        None => true,
        Some(o) => o.eq_ignore_ascii_case("null") || o.eq_ignore_ascii_case(keystone_origin),
    }
}

/// `GET /auth/websso/start?choice=<id>`: sets the nonce cookie and answers
/// `303` to Keystone's WebSSO URL for that choice; `404` with WebSSO off,
/// `400` for a choice that is not a configured federation protocol.
pub async fn start(headers: http::HeaderMap, axum::extract::Query(q): axum::extract::Query<BTreeMap<String, String>>) -> axum::response::Response {
    use axum::response::{IntoResponse, Redirect};
    let st = super::config::state();
    let Some(ws) = &st.config.websso else {
        return http::StatusCode::NOT_FOUND.into_response();
    };
    let choice = q.get("choice").map(String::as_str).unwrap_or("");
    match ws.start_url(choice) {
        Some(url) => {
            let client = st.config.client_ip_header.as_ref().and_then(|h| super::session::client_ip_from(&headers, h, st.config.trusted_proxies)).unwrap_or_else(|| "-".into());
            tracing::info!(%choice, %client, "websso start: sent to Keystone");
            ([(http::header::SET_COOKIE, nonce_cookie(&st.cookie_key, Utc::now(), choice))], Redirect::to(&url)).into_response()
        }
        None => (http::StatusCode::BAD_REQUEST, "unknown sign-in choice").into_response(),
    }
}

/// `POST /auth/websso`, form field `token`: Keystone's unscoped token after
/// federation. With a valid, unused nonce and an allowed `Origin`, signs in
/// as `api::login` does (`os::login_with_token`, `os::new_session`) and answers
/// `303` to `/project` with the session cookie ([`signed_in`]); otherwise `303`
/// to `/login?sso=<Refusal::code>`. The nonce is deleted either way. Not
/// throttled: no password reaches Moonlight. The token is never logged.
pub async fn callback(headers: http::HeaderMap, form: Result<axum::Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>) -> axum::response::Response {
    use axum::response::{IntoResponse, Redirect};
    let st = super::config::state();
    let Some(ws) = &st.config.websso else {
        return http::StatusCode::NOT_FOUND.into_response();
    };
    let client = st.config.client_ip_header.as_ref().and_then(|h| super::session::client_ip_from(&headers, h, st.config.trusted_proxies)).unwrap_or_else(|| "-".into());
    let back = |r: Refusal| {
        ([(http::header::SET_COOKIE, nonce_removal())], Redirect::to(&format!("/login?sso={}", r.code()))).into_response()
    };
    let cookies = headers.get_all(http::header::COOKIE).iter().filter_map(|v| v.to_str().ok()).collect::<Vec<_>>().join("; ");
    let nonce = valid_nonce(&cookies, &st.cookie_key, Utc::now());
    // `get_with` is atomic: of two requests with the same nonce, one inserts.
    let fresh = match &nonce {
        Some((n, _)) => {
            let first = std::sync::atomic::AtomicBool::new(false);
            st.websso_used.get_with(n.clone(), async { first.store(true, std::sync::atomic::Ordering::Relaxed) }).await;
            first.into_inner()
        }
        None => false,
    };
    // The choice comes from our own encrypted cookie; a choice since removed
    // from the settings counts as no nonce.
    let via = nonce.as_ref().and_then(|(_, choice)| ws.via(choice));
    let Some(via) = via.filter(|_| fresh && origin_allowed(&headers, &ws.keystone_origin())) else {
        let origin = headers.get(http::header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or("-");
        tracing::warn!(%client, %origin, "websso callback refused: no valid nonce or foreign origin");
        return back(Refusal::NotStarted);
    };
    let Some(token) = form.as_ref().ok().and_then(|f| f.get("token")).map(|t| t.trim()).filter(|t| !t.is_empty()) else {
        tracing::warn!(%client, "websso callback without a token");
        return back(Refusal::Failed);
    };
    let res = match super::os::login_with_token(token).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(%client, "websso sign-in failed: {}", e.detail);
            return back(Refusal::Failed);
        }
    };
    // Keystone always names the user's domain in a token; `-` only if not.
    let domain = res.user_domain.clone().unwrap_or_else(|| "-".into());
    let mut sess = match super::os::new_session(res, domain).await {
        Ok(s) => s,
        Err(e) if e.kind == crate::model::ErrorKind::Forbidden => return back(Refusal::NoProjects),
        Err(e) => {
            tracing::warn!(%client, "websso sign-in failed: {}", e.detail);
            return back(Refusal::Failed);
        }
    };
    sess.websso = Some(via);
    let session = super::session::encode(&sess, &st.cookie_key, st.config.secure_cookies, sess.ends_at(st.config.session_timeout));
    if !super::session::fits_in_a_cookie(&session) {
        tracing::warn!(user = %sess.user, %client, token_bytes = sess.token.len(), cookie_bytes = session.len(), "websso sign-in refused: the session does not fit in a cookie");
        return back(Refusal::Failed);
    }
    tracing::info!(user = %sess.user, project = %sess.project_name, %client, method = "websso", "login");
    signed_in(session)
}

/// `303` to `/project` setting the session cookie and deleting the nonce.
/// `AppendHeaders`: a plain `[(SET_COOKIE, …); 2]` *inserts* each header, so
/// the nonce removal replaced the session cookie and every WebSSO sign-in
/// landed on the login page with no session cookie (seen on ttcp-dev).
fn signed_in(session: String) -> axum::response::Response {
    use axum::response::{AppendHeaders, IntoResponse, Redirect};
    (AppendHeaders([(http::header::SET_COOKIE, session), (http::header::SET_COOKIE, nonce_removal())]), Redirect::to("/project")).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sso(keystone: &str) -> WebSso {
        let mapping = BTreeMap::from([("acme".to_string(), ("acme idp/1".to_string(), "openid".to_string()))]);
        WebSso::new(keystone, "https://cloud.example.com", vec!["oidc".into(), "saml2".into(), "acme".into()], mapping, None).unwrap()
    }

    #[test]
    fn the_start_url_has_keystones_two_shapes_and_an_escaped_origin() {
        let s = sso("https://keystone.example.com:5000");
        assert_eq!(
            s.start_url("oidc").unwrap(),
            "https://keystone.example.com:5000/v3/auth/OS-FEDERATION/websso/oidc?origin=https%3A%2F%2Fcloud.example.com%2Fauth%2Fwebsso"
        );
        assert_eq!(
            s.start_url("acme").unwrap(),
            "https://keystone.example.com:5000/v3/auth/OS-FEDERATION/identity_providers/acme%20idp%2F1/protocols/openid/websso?origin=https%3A%2F%2Fcloud.example.com%2Fauth%2Fwebsso",
            "idp_mapping: per-provider URL, each segment escaped"
        );
    }

    #[test]
    fn a_choice_names_its_identity_provider_and_protocol() {
        use crate::server::session::SsoVia;
        let s = sso("https://k.example.com");
        assert_eq!(s.via("acme"), Some(SsoVia { identity_provider: Some("acme idp/1".into()), protocol: "openid".into() }), "idp_mapping");
        assert_eq!(s.via("saml2"), Some(SsoVia { identity_provider: None, protocol: "saml2".into() }), "Keystone picks the provider");
        assert_eq!(s.via("credentials"), None);
    }

    #[test]
    fn only_configured_federated_choices_get_a_url() {
        let s = sso("https://k.example.com");
        assert_eq!(s.start_url("credentials"), None, "the password form never leaves Moonlight");
        assert_eq!(s.start_url("evil/../x"), None);
        assert_eq!(s.start_url(""), None);
    }

    /// The `Cookie` header a browser sends back for a `Set-Cookie` value.
    fn sent(set_cookie: &str) -> String {
        set_cookie.split(';').next().unwrap().to_string()
    }

    #[test]
    fn the_nonce_cookie_comes_back_cross_site_and_is_valid_ten_minutes() {
        let key = Key::generate();
        let now = Utc::now();
        let set = nonce_cookie(&key, now, "oidc");
        assert!(set.starts_with("__Host-ml_sso="));
        for attr in ["HttpOnly", "SameSite=None", "Secure", "Path=/", "Max-Age=600"] {
            assert!(set.contains(attr), "{attr} in {set}");
        }
        let (n, choice) = valid_nonce(&format!("other=1; {}", sent(&set)), &key, now).expect("valid right away");
        assert_eq!(choice, "oidc", "the choice travels in the nonce: Keystone's POST doesn't say it");
        assert!(valid_nonce(&sent(&set), &key, now + chrono::Duration::seconds(NONCE_SECONDS - 1)).is_some());
        assert_eq!(valid_nonce(&sent(&set), &key, now + chrono::Duration::seconds(NONCE_SECONDS)), None, "expired");
        assert_ne!(valid_nonce(&sent(&nonce_cookie(&key, now, "oidc")), &key, now).unwrap().0, n, "each nonce has its own replay key");
    }

    #[test]
    fn a_missing_forged_or_foreign_key_nonce_is_refused() {
        let key = Key::generate();
        let now = Utc::now();
        assert_eq!(valid_nonce("", &key, now), None, "sign-in not started from this browser");
        assert_eq!(valid_nonce(&sent(&nonce_cookie(&Key::generate(), now, "oidc")), &key, now), None, "another deployment's key");
        assert_eq!(valid_nonce("__Host-ml_sso={\"expires\":99999999999,\"choice\":\"oidc\"}", &key, now), None, "plain text is not encrypted");
        let mut tampered = sent(&nonce_cookie(&key, now, "oidc"));
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert_eq!(valid_nonce(&tampered, &key, now), None);
        let gone = nonce_removal();
        assert!(gone.starts_with("__Host-ml_sso=;") && gone.contains("Max-Age=0") && gone.contains("Secure"), "{gone}");
    }

    #[test]
    fn the_callback_accepts_keystones_origin_or_null_only() {
        let h = |o: Option<&str>| {
            let mut h = http::HeaderMap::new();
            if let Some(o) = o {
                h.insert(http::header::ORIGIN, o.parse().unwrap());
            }
            h
        };
        let k = sso("https://keystone.example.com:5000/v3").keystone_origin();
        assert_eq!(k, "https://keystone.example.com:5000");
        assert!(origin_allowed(&h(Some("https://keystone.example.com:5000")), &k));
        assert!(origin_allowed(&h(Some("null")), &k), "Keystone's page with Referrer-Policy: no-referrer");
        assert!(origin_allowed(&h(None), &k), "no Origin: the nonce decides");
        assert!(!origin_allowed(&h(Some("https://evil.example.com")), &k));
        assert!(!origin_allowed(&h(Some("https://keystone.example.com")), &k), "another port is another origin");
    }

    #[test]
    fn signing_in_sets_the_session_cookie_and_deletes_the_nonce() {
        let r = signed_in("ml_session=abc; Path=/; HttpOnly; SameSite=Lax".into());
        let set: Vec<_> = r.headers().get_all(http::header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
        assert_eq!(set.len(), 2, "both cookies, not the last one only: {set:?}");
        assert!(set[0].starts_with("ml_session=abc"), "the session cookie survives");
        assert!(set[1].starts_with("__Host-ml_sso=;"), "the nonce is deleted");
        assert_eq!((r.status(), &r.headers()[http::header::LOCATION]), (http::StatusCode::SEE_OTHER, &http::HeaderValue::from_static("/project")));
    }

    #[test]
    fn only_the_callback_post_skips_the_same_origin_check() {
        use http::Method;
        assert!(is_callback(&Method::POST, "/auth/websso", true));
        assert!(!is_callback(&Method::POST, "/auth/websso", false), "WebSSO off: refused like any cross-site POST");
        assert!(!is_callback(&Method::POST, "/auth/websso/", true));
        assert!(!is_callback(&Method::POST, "/api/login", true));
        assert!(!is_callback(&Method::PUT, "/auth/websso", true));
        let refusals = [Refusal::NotStarted, Refusal::Failed, Refusal::NoProjects].map(Refusal::code);
        assert_eq!(refusals, ["expired", "failed", "noprojects"], "pages::login reads these");
    }

    #[test]
    fn the_keystone_url_may_end_in_v3_or_a_slash_or_carry_a_path() {
        let want = "https://k.example.com/identity/v3/auth/OS-FEDERATION/websso/saml2";
        for k in ["https://k.example.com/identity", "https://k.example.com/identity/", "https://k.example.com/identity/v3", "https://k.example.com/identity/v3/"] {
            assert!(sso(k).start_url("saml2").unwrap().starts_with(want), "{k}");
        }
        assert!(WebSso::new("not a url", "https://c", vec![], BTreeMap::new(), None).is_err());
    }
}
