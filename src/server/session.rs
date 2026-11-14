//! The session lives entirely in an encrypted, HttpOnly cookie (AES-GCM via
//! the `cookie` crate's private jar). The server keeps no per-user state.

use chrono::{DateTime, Utc};
use cookie::{Cookie, CookieJar, Key, SameSite};
use http::HeaderValue;
use http::header::{COOKIE, SET_COOKIE};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use super::config::state;
use crate::model::{AppError, ErrorKind};

/// Cookie name with `[session] secure_cookies = false` (plain-HTTP development).
pub const SESSION_COOKIE: &str = "ml_session";
/// Cookie name with `secure_cookies = true`: the `__Host-` prefix makes the
/// browser refuse it unless it is `Secure`, `Path=/` and has no `Domain`, so a
/// sibling subdomain can't plant or overwrite it.
pub const SECURE_SESSION_COOKIE: &str = "__Host-ml_session";

/// The session cookie's name for this deployment.
pub fn cookie_name(secure: bool) -> &'static str {
    if secure { SECURE_SESSION_COOKIE } else { SESSION_COOKIE }
}

/// The signed-in user, as stored in the cookie. `Debug` hides the token.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Unscoped Keystone token: rescoped per project on demand.
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user_id: String,
    pub user: String,
    pub domain: String,
    pub project_id: String,
    pub project_name: String,
    pub region: Option<String>,
    /// When the user signed in: `[session] timeout_minutes` counts from here.
    /// Required: a cookie from before this field no longer decodes
    /// and signs out once; a default of "now" made it never time out.
    pub logged_in_at: DateTime<Utc>,
    /// Keystone's `password_expires_at`, for the expiry warning.
    #[serde(default)]
    pub password_expires_at: Option<DateTime<Utc>>,
    /// Identity › Domains' *Set Domain Context*: `(id, name)` scoping the
    /// identity lists instead of the user's own domain.
    #[serde(default)]
    pub domain_context: Option<(String, String)>,
    /// How the user signed in through WebSSO; `None` for a
    /// password sign-in. Logout then sends the browser to `[websso]
    /// logout_url`, and API Access offers a `v3websso` `clouds.yaml`.
    #[serde(default)]
    pub websso: Option<SsoVia>,
}

/// The identity provider and protocol of a WebSSO sign-in, from the choice the
/// user picked (`[websso] idp_mapping`). Without a mapping Keystone picks the
/// provider itself, so it is unknown here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsoVia {
    pub identity_provider: Option<String>,
    pub protocol: String,
}

// By hand so the token never reaches a log line or panic message, like
// `settings::Session`'s cookie key.
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("token", &"********")
            .field("expires_at", &self.expires_at)
            .field("user_id", &self.user_id)
            .field("user", &self.user)
            .field("domain", &self.domain)
            .field("project_id", &self.project_id)
            .field("project_name", &self.project_name)
            .field("region", &self.region)
            .field("logged_in_at", &self.logged_in_at)
            .field("password_expires_at", &self.password_expires_at)
            .field("domain_context", &self.domain_context)
            .field("websso", &self.websso)
            .finish()
    }
}

impl Session {
    /// The sign-in ends with the token or after `timeout`, whichever comes first.
    pub fn ends_at(&self, timeout: chrono::Duration) -> DateTime<Utc> {
        self.expires_at.min(self.logged_in_at + timeout)
    }

    /// `margin` keeps a request from starting with a dying token.
    pub fn is_expired(&self, now: DateTime<Utc>, timeout: chrono::Duration, margin: chrono::Duration) -> bool {
        self.ends_at(timeout) <= now + margin
    }

    /// Days until the password expires, when that is within `warn_days`.
    pub fn password_warning(&self, now: DateTime<Utc>, warn_days: Option<u32>) -> Option<i64> {
        let days = (self.password_expires_at? - now).num_days();
        (days <= i64::from(warn_days?)).then_some(days.max(0))
    }
}

/// Encrypts the session into a `Set-Cookie` header value.
/// The cookie lives until `ends_at`.
pub fn encode(session: &Session, key: &Key, secure: bool, ends_at: DateTime<Utc>) -> String {
    let json = serde_json::to_string(session).expect("session serialises");
    let max_age = (ends_at - Utc::now()).num_seconds().max(0);
    let mut jar = CookieJar::new();
    jar.private_mut(key).add(
        Cookie::build((cookie_name(secure), json))
            .path("/")
            .http_only(true)
            .secure(secure)
            .same_site(SameSite::Lax)
            .max_age(cookie::time::Duration::seconds(max_age)),
    );
    jar.get(cookie_name(secure)).expect("cookie just added").to_string()
}

/// Browsers keep a cookie only if its name and value total at most 4096 bytes
/// (RFC 6265 §6.1, the limit every major browser enforces), and silently drop
/// a larger one: the user then lands on the login page again. Federated tokens
/// can carry the user's IdP groups and grow past it.
pub const MAX_COOKIE_BYTES: usize = 4096;

/// Whether a `Set-Cookie` value from [`encode`] will be kept by the browser.
pub fn fits_in_a_cookie(set_cookie: &str) -> bool {
    set_cookie.split(';').next().unwrap_or("").len() <= MAX_COOKIE_BYTES
}

/// Decrypts the session from a `Cookie` header value. Tampered or foreign
/// cookies simply yield `None`. Both names are read, the `__Host-` one first,
/// so turning `secure_cookies` on doesn't sign everybody out (the name is part
/// of the encryption's associated data, so each decrypts only under its own).
pub fn decode(cookie_header: &str, key: &Key) -> Option<Session> {
    let mut jar = CookieJar::new();
    for c in Cookie::split_parse(cookie_header.to_owned()).flatten() {
        jar.add_original(c.into_owned());
    }
    let private = jar.private(key);
    [SECURE_SESSION_COOKIE, SESSION_COOKIE].iter().find_map(|name| {
        private
            .get(name)
            .and_then(|plain| serde_json::from_str(plain.value()).ok())
    })
}

/// `Set-Cookie` values that delete the session cookie under both names (a
/// cookie from before `secure_cookies` changed may still be there).
pub fn removal(secure: bool) -> Vec<String> {
    [SECURE_SESSION_COOKIE, SESSION_COOKIE]
        .iter()
        // The browser ignores a `__Host-` cookie without `Secure`, even to delete it.
        .map(|name| (name, secure || *name == SECURE_SESSION_COOKIE))
        .map(|(name, secure)| {
            Cookie::build((*name, ""))
                .path("/")
                .http_only(true)
                .secure(secure)
                .same_site(SameSite::Lax)
                .max_age(cookie::time::Duration::ZERO)
                .build()
                .to_string()
        })
        .collect()
}

// --- Request/response glue ---------------------------------------------------

/// Reads the session of the current request (SSR render or server fn call).
/// A session that logged out (its token in `AppState::revoked`) is no session.
pub fn current() -> Option<Session> {
    from_headers(&use_context::<http::request::Parts>()?.headers)
}

/// [`current`] for a plain axum handler (no Leptos context): the session in the request's
/// cookies, unless it logged out. Expiry is the caller's to check ([`Session::is_expired`]).
pub fn from_headers(headers: &http::HeaderMap) -> Option<Session> {
    let st = state();
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|h| decode(h, &st.cookie_key))
        .filter(|s| !st.revoked.contains_key(&super::os::token_key(&s.token)))
}

/// The session, or a redirect to the login page.
pub fn require() -> Result<Session, AppError> {
    match current() {
        Some(s) if !s.is_expired(Utc::now(), state().config.session_timeout, state().config.token_margin) => Ok(s),
        _ => {
            leptos_axum::redirect("/login");
            // During an SSR render `redirect` only adds the Location header. Only
            // pages get the 302: a server function call (a dialog's `run_action`,
            // a resource load) must get its JSON error and Leptos' redirect header
            // instead; a 302 makes `fetch` follow it and parse the login page
            // ("error deserializing server function results").
            let accept = use_context::<http::request::Parts>().and_then(|p| {
                p.headers
                    .get(http::header::ACCEPT)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string)
            });
            if let Some(resp) = use_context::<leptos_axum::ResponseOptions>().filter(|_| wants_html(accept.as_deref()))
            {
                resp.set_status(http::StatusCode::FOUND);
                tracing::info!(uri = ?use_context::<http::request::Parts>().map(|p| p.uri), reason = why_not_signed_in(), "page load sent to the login page");
            }
            Err(AppError::new(ErrorKind::Unauthenticated, "unauthenticated"))
        }
    }
}

/// Why `require` found no usable session, for the log (never the token): no
/// session cookie at all (the browser didn't keep or send it), one that doesn't
/// decrypt (another key, e.g. after a restart without `[session] cookie_key`),
/// a logged-out token, or an expired sign-in.
fn why_not_signed_in() -> &'static str {
    let Some(parts) = use_context::<http::request::Parts>() else {
        return "no request";
    };
    let st = state();
    let headers: Vec<&str> = parts
        .headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    let named = headers
        .iter()
        .flat_map(|h| Cookie::split_parse(*h).flatten())
        .any(|c| [SECURE_SESSION_COOKIE, SESSION_COOKIE].contains(&c.name()));
    if !named {
        return "no session cookie";
    }
    match headers.iter().find_map(|h| decode(h, &st.cookie_key)) {
        None => "session cookie does not decrypt",
        Some(s) if st.revoked.contains_key(&super::os::token_key(&s.token)) => "token logged out",
        Some(_) => "sign-in expired",
    }
}

/// Whether the request is a page load (`Accept: text/html`, as leptos_axum's
/// `redirect` decides) rather than a server function call.
fn wants_html(accept: Option<&str>) -> bool {
    accept.is_some_and(|a| a.contains("text/html"))
}

/// The client's address from `[log] client_ip_header`, for sign-in and write
/// logs and the per-address sign-in throttle; `-` when unset or absent.
pub fn client_ip() -> String {
    let st = state();
    st.config
        .client_ip_header
        .as_ref()
        .and_then(|h| {
            use_context::<http::request::Parts>().and_then(|p| client_ip_from(&p.headers, h, st.config.trusted_proxies))
        })
        .unwrap_or_else(|| "-".into())
}

/// [`client_ip`] for a plain axum handler, from the request's headers.
pub fn client_ip_of(headers: &http::HeaderMap) -> String {
    let st = state();
    st.config
        .client_ip_header
        .as_ref()
        .and_then(|h| client_ip_from(headers, h, st.config.trusted_proxies))
        .unwrap_or_else(|| "-".into())
}

/// The client's entry in a `X-Forwarded-For`-style list. Each proxy appends the
/// address it saw on the right; anything to the left of what our `trusted`
/// proxies added was sent by the client and can be forged. So the
/// entry `trusted` places from the right, or the leftmost when the list is
/// shorter; `trusted == 0` takes the leftmost.
/// Several header lines count as one list, in order.
pub fn client_ip_from(headers: &http::HeaderMap, name: &http::HeaderName, trusted: u32) -> Option<String> {
    let hops: Vec<&str> = headers
        .get_all(name)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .collect();
    let from_right = usize::try_from(trusted).unwrap_or(usize::MAX);
    let i = if from_right == 0 {
        0
    } else {
        hops.len().saturating_sub(from_right)
    };
    hops.get(i).map(|a| a.to_string())
}

/// Writes `session` into the response as the encrypted cookie, expiring with the session
/// (`[session] timeout_minutes` or the token, whichever comes first).
pub fn store(session: &Session) {
    let st = state();
    set_cookie(&encode(
        session,
        &st.cookie_key,
        st.config.secure_cookies,
        session.ends_at(st.config.session_timeout),
    ));
}

/// Deletes the session cookie (under both names) in the response.
pub fn clear() {
    for value in removal(state().config.secure_cookies) {
        set_cookie(&value);
    }
}

/// Appends a `Set-Cookie` to the server function's response (several can go out at once,
/// AGENTS.md §6); does nothing outside a Leptos request.
fn set_cookie(value: &str) {
    if let (Some(resp), Ok(v)) = (
        use_context::<leptos_axum::ResponseOptions>(),
        HeaderValue::from_str(value),
    ) {
        resp.append_header(SET_COOKIE, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_page_loads_get_the_login_redirect_status() {
        assert!(
            wants_html(Some("text/html,application/xhtml+xml,*/*;q=0.8")),
            "a browser navigation"
        );
        assert!(
            !wants_html(Some("application/json")),
            "a server function call: its JSON error, not the login page"
        );
        assert!(!wants_html(None));
    }

    fn sample() -> Session {
        Session {
            token: "gAAAAAB-secret-token".into(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            user_id: "u1".into(),
            user: "admin2".into(),
            domain: "Default".into(),
            project_id: "p1".into(),
            project_name: "Proyecto A".into(),
            region: Some("es-central-1".into()),
            logged_in_at: Utc::now(),
            password_expires_at: None,
            domain_context: None,
            websso: None,
        }
    }

    fn enc(s: &Session, key: &Key, secure: bool) -> String {
        encode(s, key, secure, s.expires_at)
    }

    fn header_from_set_cookie(set_cookie: &str) -> String {
        set_cookie.split(';').next().unwrap().to_string()
    }

    #[test]
    fn cookies_from_before_websso_decode_as_password_sessions() {
        let mut json = serde_json::to_value(sample()).unwrap();
        json.as_object_mut().unwrap().remove("websso");
        let s: Session = serde_json::from_value(json).unwrap();
        assert_eq!(s.websso, None, "no `websso` field: a password sign-in");
    }

    #[test]
    fn a_session_with_a_huge_token_does_not_fit_in_a_cookie() {
        let key = Key::generate();
        let mut s = sample();
        assert!(fits_in_a_cookie(&enc(&s, &key, true)), "a password token fits");
        s.token = "g".repeat(3000);
        assert!(
            !fits_in_a_cookie(&enc(&s, &key, true)),
            "base64 and AES-GCM grow it past 4096 bytes: the browser would drop it"
        );
    }

    #[test]
    fn roundtrips_through_an_encrypted_cookie() {
        let key = Key::generate();
        let original = sample();
        let set = enc(&original, &key, false);
        assert!(set.contains("HttpOnly"));
        assert!(set.contains("SameSite=Lax"));
        assert!(!set.contains("secret-token"), "token must not travel in clear text");
        let header = format!("other=1; {}", header_from_set_cookie(&set));
        assert_eq!(decode(&header, &key), Some(original));
    }

    #[test]
    fn rejects_a_cookie_from_another_key_or_tampered() {
        let set = enc(&sample(), &Key::generate(), false);
        let header = header_from_set_cookie(&set);
        assert_eq!(decode(&header, &Key::generate()), None);
        let tampered = format!("{header}x");
        assert_eq!(decode(&tampered, &Key::generate()), None);
    }

    #[test]
    fn secure_flag_follows_config() {
        let key = Key::generate();
        assert!(enc(&sample(), &key, true).contains("Secure"));
        assert!(!enc(&sample(), &key, false).contains("Secure"));
    }

    #[test]
    fn secure_cookies_use_the_host_prefix_and_old_names_still_decode() {
        let key = Key::generate();
        let secure = enc(&sample(), &key, true);
        assert!(
            secure.starts_with("__Host-ml_session=") && secure.contains("Path=/") && !secure.contains("Domain"),
            "{secure}"
        );
        let plain = enc(&sample(), &key, false);
        assert!(plain.starts_with("ml_session="));
        for set in [secure, plain] {
            let user = decode(&header_from_set_cookie(&set), &key).map(|s| s.user);
            assert_eq!(user.as_deref(), Some("admin2"), "both names are read: {set}");
        }
        let gone = removal(false);
        assert_eq!(gone.len(), 2, "logout deletes both names");
        assert!(gone.iter().all(|c| c.contains("Max-Age=0")));
        assert!(
            gone.iter()
                .find(|c| c.starts_with("__Host-"))
                .unwrap()
                .contains("Secure"),
            "a __Host- cookie needs Secure even to be deleted"
        );
    }

    #[test]
    fn expiry_has_a_safety_margin_and_a_session_timeout() {
        let (min, sec) = (chrono::Duration::minutes, chrono::Duration::seconds);
        let mut s = sample();
        let now = Utc::now();
        s.logged_in_at = now;
        s.expires_at = now + sec(30);
        assert!(s.is_expired(now, min(60), sec(60)), "inside the margin");
        assert!(!s.is_expired(now, min(60), sec(0)), "no margin");
        s.expires_at = now + min(50);
        assert!(!s.is_expired(now, min(60), sec(60)));
        assert!(!s.is_expired(now + min(20), min(30), sec(0)));
        assert!(
            s.is_expired(now + min(31), min(30), sec(0)),
            "the timeout ends it before the token expires"
        );
        assert_eq!(s.ends_at(min(30)), now + min(30));
    }

    #[test]
    fn old_cookies_without_the_new_fields_still_decode() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v.as_object_mut()
            .unwrap()
            .retain(|k, _| k != "password_expires_at" && k != "domain_context");
        let s: Session = serde_json::from_value(v).unwrap();
        assert!(s.password_expires_at.is_none() && s.domain_context.is_none());
    }

    #[test]
    fn a_cookie_without_logged_in_at_signs_out_instead_of_never_timing_out() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v.as_object_mut().unwrap().remove("logged_in_at");
        assert!(serde_json::from_value::<Session>(v).is_err());
    }

    #[test]
    fn debug_hides_the_token() {
        let s = sample();
        let out = format!("{s:?}");
        assert!(!out.contains(&s.token) && out.contains("********") && out.contains(&s.user));
    }

    #[test]
    fn client_ip_is_the_entry_the_trusted_proxies_added() {
        let name = http::HeaderName::from_static("x-forwarded-for");
        let mut h = http::HeaderMap::new();
        assert_eq!(client_ip_from(&h, &name, 1), None);
        // Forged by the client, then the real client as seen by proxy 1, then proxy 1 as seen by proxy 2.
        h.insert(&name, http::HeaderValue::from_static(" 6.6.6.6, 203.0.113.9 ,10.0.0.1"));
        assert_eq!(
            client_ip_from(&h, &name, 0).as_deref(),
            Some("6.6.6.6"),
            "0: leftmost, as before"
        );
        assert_eq!(
            client_ip_from(&h, &name, 1).as_deref(),
            Some("10.0.0.1"),
            "1: rightmost"
        );
        assert_eq!(
            client_ip_from(&h, &name, 2).as_deref(),
            Some("203.0.113.9"),
            "2: the forged entry is skipped"
        );
        assert_eq!(
            client_ip_from(&h, &name, 5).as_deref(),
            Some("6.6.6.6"),
            "short list: leftmost"
        );
        h.insert(&name, http::HeaderValue::from_static("203.0.113.9"));
        assert_eq!(
            client_ip_from(&h, &name, 1).as_deref(),
            Some("203.0.113.9"),
            "single value (X-Real-IP)"
        );
        h.append(&name, http::HeaderValue::from_static("10.0.0.1"));
        assert_eq!(
            client_ip_from(&h, &name, 2).as_deref(),
            Some("203.0.113.9"),
            "several header lines are one list"
        );
    }

    #[test]
    fn password_warning_only_inside_the_threshold() {
        let now = Utc::now();
        let mut s = sample();
        assert_eq!(s.password_warning(now, Some(7)), None, "no expiry date");
        s.password_expires_at = Some(now + chrono::Duration::days(3) + chrono::Duration::hours(1));
        assert_eq!(s.password_warning(now, Some(7)), Some(3));
        assert_eq!(s.password_warning(now, Some(2)), None);
        assert_eq!(s.password_warning(now, None), None, "warning disabled (default)");
    }
}
