//! The instance *Log* and *Console* tabs (roadmap 31). Both are Nova POSTs
//! that read without changing the server, so they are not in `fetch.rs`
//! (GET only) but go through [`Exec`] like writes: sent only with
//! `[writes] mode = on`, previewed in dry-run, rejected when off. On top of
//! that `[console] enabled` must be set. These are the only two such calls.

use openstack_sdk::api::compute::v2::server::{os_get_console_output, os_get_vncconsole_21, remote_console::create_26};
use openstack_sdk::AsyncOpenStack;
use serde_json::Value;

use super::actions::Exec;
use crate::model::*;
use crate::prefs::{ConsolePrefs, ConsoleType};

type R<T> = Result<T, AppError>;

fn build_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::invalid(format!("invalid request: {e}"))
}

fn enabled(prefs: &ConsolePrefs) -> R<()> {
    if prefs.enabled {
        Ok(())
    } else {
        Err(AppError::invalid("the instance log and console are disabled ([console] enabled)"))
    }
}

/// `{"os-getConsoleOutput": {"length": N}}`; no `length` for the whole log.
pub fn log_request(id: &str, lines: Option<u32>) -> R<os_get_console_output::Request<'_>> {
    let mut body = os_get_console_output::OsGetConsoleOutputBuilder::default();
    if let Some(n) = lines {
        body.length(i32::try_from(n.max(1)).unwrap_or(i32::MAX));
    }
    os_get_console_output::Request::builder().id(id).os_get_console_output(body.build().map_err(build_err)?).build().map_err(build_err)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Protocol {
    Vnc,
    Spice,
}

fn remote_request(id: &str, p: Protocol) -> R<create_26::Request<'_>> {
    let (protocol, kind) = match p {
        Protocol::Vnc => (create_26::Protocol::Vnc, create_26::Type::Novnc),
        Protocol::Spice => (create_26::Protocol::Spice, create_26::Type::SpiceHtml5),
    };
    let body = create_26::RemoteConsoleBuilder::default().protocol(protocol)._type(kind).build().map_err(build_err)?;
    create_26::Request::builder().server_id(id).remote_console(body).build().map_err(build_err)
}

/// Before Nova 2.6 only the VNC action exists.
fn legacy_vnc_request(id: &str) -> R<os_get_vncconsole_21::Request<'_>> {
    let body = os_get_vncconsole_21::OsGetVncconsoleBuilder::default()._type(os_get_vncconsole_21::Type::Novnc).build().map_err(build_err)?;
    os_get_vncconsole_21::Request::builder().id(id).os_get_vncconsole(body).build().map_err(build_err)
}

/// The protocols to try, in order.
fn protocols(kind: ConsoleType) -> &'static [Protocol] {
    match kind {
        ConsoleType::Auto => &[Protocol::Vnc, Protocol::Spice],
        ConsoleType::Vnc => &[Protocol::Vnc],
        ConsoleType::Spice => &[Protocol::Spice],
    }
}

/// `{"output": "…"}`.
pub fn log_text(v: &Value) -> String {
    v.get("output").and_then(Value::as_str).unwrap_or_default().to_string()
}

/// `{protocol, type, url}`, as `remote-consoles` returns it (response key
/// already stripped) or wrapped in `console` by `os-getVNCConsole`.
pub fn remote_console(v: &Value) -> R<RemoteConsole> {
    let c = v.get("console").unwrap_or(v);
    let field = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let url = field("url");
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::internal("Nova returned no console URL"));
    }
    Ok(RemoteConsole { protocol: field("protocol"), kind: field("type"), url })
}

pub async fn log(mode: WriteMode, prefs: &ConsolePrefs, os: Option<&AsyncOpenStack>, id: &str, lines: Option<u32>) -> R<ConsoleReply<String>> {
    enabled(prefs)?;
    let mut x = Exec::new(mode, os);
    let data = x.call(log_request(id, lines)?).await?.map(|v| log_text(&v));
    Ok(ConsoleReply { mode, requests: x.requests(), data })
}

/// `remote_consoles`: the cloud has Nova 2.6 (`needs::REMOTE_CONSOLE`).
pub async fn remote(mode: WriteMode, prefs: &ConsolePrefs, os: Option<&AsyncOpenStack>, id: &str, remote_consoles: bool) -> R<ConsoleReply<RemoteConsole>> {
    enabled(prefs)?;
    let mut x = Exec::new(mode, os);
    let reply = if remote_consoles {
        let mut first_err = None;
        let mut reply = None;
        for p in protocols(prefs.kind) {
            match x.call(remote_request(id, *p)?).await {
                Ok(v) => {
                    reply = Some(v);
                    break;
                }
                // The cloud may offer SPICE but not VNC: try the next one.
                Err(e) => first_err = first_err.or(Some(e)),
            }
        }
        match (reply, first_err) {
            (Some(v), _) => v,
            (None, Some(e)) => return Err(e),
            (None, None) => None,
        }
    } else {
        x.call(legacy_vnc_request(id)?).await?
    };
    let data = reply.as_ref().map(remote_console).transpose()?;
    Ok(ConsoleReply { mode: x.mode(), requests: x.requests(), data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn on() -> ConsolePrefs {
        ConsolePrefs { enabled: true, ..ConsolePrefs::default() }
    }

    fn body(r: &RequestPreview) -> Value {
        serde_json::from_str(r.body.as_deref().expect("body")).unwrap()
    }

    #[tokio::test]
    async fn log_asks_for_the_last_lines_or_everything() {
        let r = log(WriteMode::DryRun, &on(), None, "vm1", Some(35)).await.unwrap();
        assert_eq!(r.data, None, "dry-run sends nothing");
        assert_eq!((r.requests[0].method.as_str(), r.requests[0].path.as_str()), ("POST", "/servers/vm1/action"));
        assert_eq!(body(&r.requests[0]), json!({"os-getConsoleOutput": {"length": 35}}));
        let r = log(WriteMode::DryRun, &on(), None, "vm1", None).await.unwrap();
        assert_eq!(body(&r.requests[0]), json!({"os-getConsoleOutput": {}}));
    }

    #[tokio::test]
    async fn remote_console_previews_vnc_first() {
        let r = remote(WriteMode::DryRun, &on(), None, "vm1", true).await.unwrap();
        assert_eq!(r.requests.len(), 1, "dry-run stops at the first protocol");
        assert_eq!((r.requests[0].method.as_str(), r.requests[0].path.as_str()), ("POST", "/servers/vm1/remote-consoles"));
        assert_eq!(body(&r.requests[0]), json!({"remote_console": {"protocol": "vnc", "type": "novnc"}}));
        let spice = ConsolePrefs { kind: ConsoleType::Spice, ..on() };
        let r = remote(WriteMode::DryRun, &spice, None, "vm1", true).await.unwrap();
        assert_eq!(body(&r.requests[0]), json!({"remote_console": {"protocol": "spice", "type": "spice-html5"}}));
    }

    #[tokio::test]
    async fn old_clouds_get_the_vnc_action() {
        let r = remote(WriteMode::DryRun, &on(), None, "vm1", false).await.unwrap();
        assert_eq!(r.requests[0].path, "/servers/vm1/action");
        assert_eq!(body(&r.requests[0]), json!({"os-getVNCConsole": {"type": "novnc"}}));
    }

    #[tokio::test]
    async fn disabled_or_writes_off_sends_nothing() {
        assert!(log(WriteMode::On, &ConsolePrefs::default(), None, "vm1", Some(35)).await.is_err(), "[console] enabled is off by default");
        assert!(remote(WriteMode::On, &ConsolePrefs::default(), None, "vm1", true).await.is_err());
        assert!(log(WriteMode::Off, &on(), None, "vm1", Some(35)).await.is_err());
    }

    #[test]
    fn responses_are_read() {
        assert_eq!(log_text(&json!({"output": "login: "})), "login: ");
        assert_eq!(log_text(&json!({})), "");
        let new = remote_console(&json!({"protocol": "vnc", "type": "novnc", "url": "https://vnc.example/vnc_auto.html?token=t"})).unwrap();
        assert_eq!((new.protocol.as_str(), new.kind.as_str()), ("vnc", "novnc"));
        let old = remote_console(&json!({"console": {"type": "novnc", "url": "http://vnc.example/?token=t"}})).unwrap();
        assert_eq!(old.url, "http://vnc.example/?token=t");
        assert!(remote_console(&json!({"url": "javascript:alert(1)"})).is_err(), "only http(s) URLs are framed");
    }
}
