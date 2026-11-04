//! Turns SDK and transport errors into a typed `AppError` in one place.
//!
//! The SDK's error types are generic and nested (`OpenStackError` →
//! `ApiError<RestError>` → `reqwest::Error`…), but their `Display` output is
//! stable and carries everything the UI needs: the HTTP status, the request ID
//! and, for catalog misses, the service type. Classifying the message keeps
//! this a pure function that can be tested with real-looking strings.

use crate::model::{AppError, ErrorKind};

/// Classifies `err` (any SDK error, displayed) raised while doing `context`.
pub fn classify(context: &str, err: &str) -> AppError {
    let detail = format!("{context}: {err}");
    let lower = err.to_ascii_lowercase();
    let kind = if let Some(status) = http_status(err) {
        match status {
            // Aodh answers a policy refusal with 401 ("You are not authorized to
            // create action: log://", roadmap 79); an expired token says
            // "requires authentication". Only the latter ends the session.
            401 if lower.contains("not authorized to") => ErrorKind::Forbidden,
            401 => ErrorKind::Unauthenticated,
            403 => ErrorKind::Forbidden,
            404 => ErrorKind::NotFound,
            504 => ErrorKind::Timeout,
            s => ErrorKind::OpenStack(s),
        }
    } else if lower.contains("is not configured for interface") || lower.contains("no catalog information") {
        ErrorKind::ServiceUnavailable
    } else if lower.contains("resource cannot be found") {
        ErrorKind::NotFound
    } else if ["timed out", "timeout", "deadline has elapsed", "error sending request", "connection refused", "dns error"]
        .iter()
        .any(|m| lower.contains(m))
    {
        ErrorKind::Timeout
    } else {
        ErrorKind::Internal
    };
    AppError { kind, detail, service: backticked_after(err, "Service `"), request_id: backticked_after(err, "request-id: `") }
}

/// What the browser may see of a classified error (roadmap 50). Without
/// `show_details`, `detail` becomes just the context (`"GET servers"`): the SDK
/// text carries internal endpoint URIs. Kind, service and request ID stay, so
/// the error page and a support ticket still have what they need.
pub fn public(mut e: AppError, context: &str, show_details: bool) -> AppError {
    if !show_details {
        e.detail = context.to_string();
    }
    e
}

/// HTTP status in the SDK's formats: ``status: `404 Not Found` ``,
/// `openstack HTTP error: 401 Unauthorized`, `failed with status 503`.
fn http_status(err: &str) -> Option<u16> {
    ["status: `", "HTTP error: ", "with status "].iter().find_map(|marker| {
        let rest = &err[err.find(marker)? + marker.len()..];
        let code: String = rest.chars().take_while(char::is_ascii_digit).collect();
        code.parse().ok().filter(|c| (400..600).contains(c))
    })
}

/// The non-empty text between `marker` and the next backtick.
fn backticked_after(err: &str, marker: &str) -> Option<String> {
    let rest = &err[err.find(marker)? + marker.len()..];
    let value = &rest[..rest.find('`')?];
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: &str, req: &str) -> String {
        format!(
            "api error: openstack server error:\n\turi: `https://nova.example/v2.1/servers/x`\n\tstatus: `{status}`\n\tmessage: `Instance x could not be found.`\n\trequest-id: `{req}`"
        )
    }

    #[test]
    fn maps_http_statuses_and_keeps_the_request_id() {
        let e = classify("GET servers/x", &api("404 Not Found", "req-abc"));
        assert_eq!(e.kind, ErrorKind::NotFound);
        assert_eq!(e.request_id.as_deref(), Some("req-abc"));
        assert!(e.detail.starts_with("GET servers/x: api error"));

        assert_eq!(classify("c", &api("401 Unauthorized", "")).kind, ErrorKind::Unauthenticated);
        assert_eq!(classify("c", &api("403 Forbidden", "")).kind, ErrorKind::Forbidden);
        assert_eq!(classify("c", &api("500 Internal Server Error", "")).kind, ErrorKind::OpenStack(500));
        assert_eq!(classify("c", &api("504 Gateway Timeout", "")).kind, ErrorKind::Timeout);
        assert_eq!(classify("c", &api("409 Conflict", "")).kind, ErrorKind::OpenStack(409));
        // empty request id → None
        assert_eq!(classify("c", &api("409 Conflict", "")).request_id, None);
    }

    #[test]
    fn aodh_policy_refusals_are_not_expired_sessions() {
        let aodh = "openstack server error:\n\turi: `https://aodh.example/v2/alarms?`\n\tstatus: `401 Unauthorized`\n\tdata: `{\"error_message\":{\"debuginfo\":null,\"faultcode\":\"Client\",\"faultstring\":\"You are not authorized to create action: log://\"}}`\n\trequest-id: `req-1`";
        assert_eq!(classify("POST alarms", aodh).kind, ErrorKind::Forbidden, "a 401 that is a refusal must not sign the user out");
        let keystone = "openstack server error:\n\tstatus: `401 Unauthorized`\n\tdata: `{\"error\": {\"message\": \"The request you have made requires authentication.\", \"code\": 401}}`";
        assert_eq!(classify("GET servers", keystone).kind, ErrorKind::Unauthenticated);
    }

    #[test]
    fn understands_the_other_status_formats() {
        assert_eq!(classify("login", "openstack HTTP error: 401 Unauthorized").kind, ErrorKind::Unauthenticated);
        assert_eq!(
            classify("discovery", "service_catalog error: HTTP request failed with status 503 Service Unavailable: busy").kind,
            ErrorKind::OpenStack(503)
        );
    }

    #[test]
    fn the_browser_gets_no_internal_uris_unless_allowed() {
        let full = classify("GET servers/x", &api("404 Not Found", "req-abc"));
        let hidden = public(full.clone(), "GET servers/x", false);
        assert_eq!(hidden.detail, "GET servers/x");
        assert!(!hidden.detail.contains("nova.example"));
        assert_eq!((hidden.kind, hidden.request_id.as_deref()), (ErrorKind::NotFound, Some("req-abc")), "kind and request ID stay");
        assert_eq!(public(full.clone(), "GET servers/x", true), full, "[ui] show_error_details = true");
    }

    #[test]
    fn catalog_miss_names_the_service() {
        let e = classify(
            "GET networks",
            "api error: service catalog error: Service `network` is not configured for interface Some(\"public\") at region Some(\"es-north-1\")",
        );
        assert_eq!(e.kind, ErrorKind::ServiceUnavailable);
        assert_eq!(e.service.as_deref(), Some("network"));
    }

    #[test]
    fn transport_failures_are_timeouts() {
        for m in [
            "communication with cloud: error sending request for url (https://x/): operation timed out",
            "communication with cloud: error sending request for url (https://x/): client error (Connect): tcp connect error: Connection refused",
            "deadline has elapsed",
        ] {
            assert_eq!(classify("c", m).kind, ErrorKind::Timeout, "{m}");
        }
    }

    #[test]
    fn everything_else_is_internal() {
        let e = classify("GET servers", "could not parse JSON response: expected value at line 1");
        assert_eq!(e.kind, ErrorKind::Internal);
        assert_eq!(e.service, None);
        assert!(e.retryable());
        assert!(!classify("c", &api("404 Not Found", "")).retryable());
        assert!(classify("c", &api("502 Bad Gateway", "")).retryable());
    }
}
