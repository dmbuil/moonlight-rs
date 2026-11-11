//! Pure functions: raw OpenStack JSON → UI models. No I/O, so every join is
//! unit-tested below with payloads shaped like the real APIs.

//! Pure functions: raw OpenStack JSON → UI models. No I/O, so every join is
//! unit-tested below with payloads shaped like the real APIs.

use crate::model::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

mod admin;
mod alarm;
mod choices;
mod compute;
mod firewall;
#[cfg(test)]
mod fixtures;
mod heat;
mod identity;
mod image;
mod keymgr;
mod network;
mod octavia;
mod quota;
mod volume;

pub use admin::*;
pub use alarm::*;
pub use choices::*;
pub use compute::*;
pub use firewall::*;
pub use heat::*;
pub use identity::*;
pub use image::*;
pub use keymgr::*;
pub use network::*;
pub use octavia::*;
pub use quota::*;
pub use volume::*;

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn s_ne(v: &Value, k: &str) -> Option<String> {
    s(v, k).filter(|x| !x.is_empty())
}

fn b(v: &Value, k: &str) -> bool {
    match v.get(k) {
        Some(Value::Bool(x)) => *x,
        // Cinder returns "true"/"false" strings for `bootable`
        Some(Value::String(x)) => x.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn i(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn strs(v: &Value, k: &str) -> Vec<String> {
    arr(v, k)
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

/// `name` if set, else the ID — what Horizon shows for unnamed resources.
fn name_or_id(v: &Value) -> String {
    s_ne(v, "name").or_else(|| s(v, "id")).unwrap_or_default()
}

fn index_by_id(items: &[Value]) -> HashMap<String, &Value> {
    items.iter().filter_map(|x| Some((s(x, "id")?, x))).collect()
}

/// These IDs sorted, each once, without blanks: the lookups a page asks for.
fn distinct(ids: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = ids.filter(|id| !id.is_empty()).collect();
    out.sort();
    out.dedup();
    out
}

fn names_by_id(items: &[Value]) -> HashMap<String, String> {
    items
        .iter()
        .filter_map(|x| Some((s(x, "id")?, name_or_id(x))))
        .collect()
}

/// Some list endpoints wrap each item (`{"keypair": {...}}`).
fn unwrap<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(v)
}

fn kv_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = v
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// The ID in a Barbican `secret_ref` / `container_ref` URL: its last path
/// segment (Barbican returns refs, not IDs).
pub fn ref_id(r: &str) -> String {
    r.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Barbican pages with `limit`/`offset` and reports `total` (not `marker`): the
/// offset of the next page after one of `got` items at `offset`, if any.
/// Without `total`, the `next` link says whether there is more.
pub fn next_offset(page: &Value, offset: u64, got: usize) -> Option<u64> {
    let next = offset + got as u64;
    if got == 0 {
        return None;
    }
    match page.get("total").and_then(Value::as_u64) {
        Some(total) => (next < total).then_some(next),
        None => page.get("next").filter(|n| !n.is_null()).map(|_| next),
    }
}

/// OpenStack timestamps as RFC 3339 UTC, for the UI's ages. Nova and Cinder
/// send naive UTC with microseconds (`2026-10-30T10:00:00.000000`), Neutron's
/// agents a space instead of `T` (`2026-10-30 10:00:00`); RFC 3339 passes.
pub fn utc_rfc3339(v: &str) -> Option<String> {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(v) {
        return Some(
            t.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        );
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|f| chrono::NaiveDateTime::parse_from_str(v, f).ok())
        .map(|n| n.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Key Manager (roadmap 55)

    #[test]
    fn barbican_refs_end_in_the_id() {
        assert_eq!(ref_id("https://kms:9311/v1/secrets/5e2a"), "5e2a");
        assert_eq!(ref_id("https://kms:9311/v1/containers/c-1/"), "c-1", "trailing slash");
        assert_eq!(ref_id(""), "");
    }

    #[test]
    fn barbican_pages_by_offset_until_total() {
        // Default page size is 10: 23 secrets take three pages.
        let page = json!({"secrets": [], "total": 23, "next": "https://kms/v1/secrets?limit=10&offset=10"});
        assert_eq!(next_offset(&page, 0, 10), Some(10));
        assert_eq!(next_offset(&page, 10, 10), Some(20));
        assert_eq!(next_offset(&page, 20, 3), None, "last page");
        assert_eq!(
            next_offset(&json!({"total": 50}), 0, 0),
            None,
            "an empty page ends the loop"
        );
        assert_eq!(
            next_offset(&json!({"next": "x"}), 0, 10),
            Some(10),
            "no total: follow `next`"
        );
        assert_eq!(next_offset(&json!({"next": null}), 0, 10), None);
    }

    // --- Admin (roadmap 85) ---

    #[test]
    fn timestamps_of_nova_cinder_and_neutron_become_rfc3339_utc() {
        assert_eq!(
            utc_rfc3339("2026-10-30T10:00:00.000000").as_deref(),
            Some("2026-10-30T10:00:00Z"),
            "Nova, Cinder: naive UTC"
        );
        assert_eq!(
            utc_rfc3339("2026-10-30 10:00:00").as_deref(),
            Some("2026-10-30T10:00:00Z"),
            "Neutron agents: a space"
        );
        assert_eq!(
            utc_rfc3339("2026-10-30T12:00:00+02:00").as_deref(),
            Some("2026-10-30T10:00:00Z")
        );
        assert_eq!(utc_rfc3339(""), None);
    }
}
