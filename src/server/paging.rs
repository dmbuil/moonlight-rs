//! Pure query building for the paged lists (roadmap 88): no I/O. It turns a
//! [`ListQuery`] from the URL into the API's query parameters, refusing anything that
//! is not in the list's filter and sort tables, and cuts the `page_size + 1` rows an
//! API returns into a page and the marker of the next one. The GET itself is
//! `fetch::page_raw`.

use serde_json::Value;

use crate::model::{check_id, AppError, ListQuery, PagedList, SortStyle};

/// Longest filter value accepted; a name or an address is far shorter.
const MAX_VALUE: usize = 255;

/// The filter of `q` as `(key, value)`, trimmed; `None` when there is no value.
/// A value without a known field, or a field the list lacks, is an error: the URL is
/// user input and a typo must not silently list everything.
fn filter_of(list: PagedList, q: &ListQuery) -> Result<Option<(&'static str, &str)>, AppError> {
    let value = q.value.as_deref().map(str::trim).unwrap_or_default();
    let field = q.field.as_deref().map(str::trim).filter(|f| !f.is_empty());
    let def = match field {
        Some(f) => Some(list.filters().iter().find(|d| d.key == f).ok_or_else(|| AppError::invalid(format!("unknown filter {f:?}")))?),
        None => None,
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_VALUE || value.chars().any(char::is_control) {
        return Err(AppError::invalid("the filter value is too long or has control characters"));
    }
    // Without a field the first filter (the name) is the default, as the filter bar.
    let def = def.unwrap_or(&list.filters()[0]);
    Ok(Some((def.param, value)))
}

/// The API's query parameters for one page of `list`: its fixed ones, the filter,
/// the sort, `limit` and `marker`. `limit` is `page_size + 1`: the extra row only
/// says whether a next page exists ([`cut`]).
pub fn params(list: PagedList, q: &ListQuery, limit: u32) -> Result<Vec<(&'static str, String)>, AppError> {
    let mut out: Vec<(&'static str, String)> = list.fixed().iter().map(|(k, v)| (*k, v.to_string())).collect();
    if let Some((param, value)) = filter_of(list, q)? {
        out.push((param, value.to_string()));
    }
    if let Some(key) = q.sort.as_deref().filter(|k| !k.is_empty()) {
        let def = list.sorts().iter().find(|d| d.key == key).ok_or_else(|| AppError::invalid(format!("unknown sort {key:?}")))?;
        let dir = if q.desc { "desc" } else { "asc" };
        match list.sort_style() {
            SortStyle::KeyDir => {
                out.push(("sort_key", def.param.to_string()));
                out.push(("sort_dir", dir.to_string()));
            }
            SortStyle::Colon => out.push(("sort", format!("{}:{dir}", def.param))),
        }
    }
    out.push(("limit", limit.to_string()));
    if let Some(m) = q.marker.as_deref().filter(|m| !m.is_empty()) {
        // The marker comes from the URL and goes into the query: an ID, nothing else.
        check_id(m)?;
        out.push(("marker", m.to_string()));
    }
    Ok(out)
}

/// Cinder's `name~` (*like*) needs volume 3.34 and `with_count` 3.45 (probed on
/// ttcp-dev, 2026-11-03). Given the cloud's volume microversion, adapts `params`
/// for a Cinder list and says which `OpenStack-API-Version` to send: at 3.45 a
/// count is asked for; below 3.34 the name filter is the exact `name`; an unknown
/// version (detection failed) is treated as the oldest, so no request is refused.
pub fn cinder(mut params: Vec<(&'static str, String)>, mv: Option<(u16, u16)>) -> (Vec<(&'static str, String)>, Option<&'static str>) {
    let at = |v: (u16, u16)| mv.is_some_and(|m| m >= v);
    if at((3, 45)) {
        params.push(("with_count", "true".to_string()));
        return (params, Some("volume 3.45"));
    }
    if at((3, 34)) {
        return (params, Some("volume 3.34"));
    }
    for p in &mut params {
        if p.0 == "name~" {
            p.0 = "name";
        }
    }
    (params, None)
}

/// The query of one request of *Count all* (roadmap 88): `q`'s filter (not its
/// sort, which can't change a count), `limit` and `marker`, and on Neutron
/// `fields=id`, so each row is only its ID.
pub fn count_params(list: PagedList, q: &ListQuery, marker: Option<&str>, limit: u32) -> Result<Vec<(&'static str, String)>, AppError> {
    let q = ListQuery { marker: marker.map(str::to_string), sort: None, desc: false, ..q.clone() };
    let mut out = params(list, &q, limit)?;
    if list.is_neutron() {
        out.push(("fields", "id".to_string()));
    }
    Ok(out)
}

/// One step of *Count all*: adds this answer's `rows` to `counted` and returns
/// the marker to go on from, or `None` when done. It stops on an empty answer,
/// not a short one: an API whose own cap is below the `limit` asked for (Nova's
/// `max_limit`, Neutron's `pagination_max_limit`) answers short pages too.
pub fn count_step(counted: &mut u64, rows: &[Value]) -> Option<String> {
    *counted += rows.len() as u64;
    rows.last().and_then(|r| r.get("id")).and_then(Value::as_str).map(str::to_string)
}

/// Keeps the first `page_size` rows of an answer that asked for one more, and the
/// marker of the next page (the last kept row's `id`) when that extra row came.
pub fn cut(mut rows: Vec<Value>, page_size: usize) -> (Vec<Value>, Option<String>) {
    if rows.len() <= page_size {
        return (rows, None);
    }
    rows.truncate(page_size);
    let next = rows.last().and_then(|r| r.get("id")).and_then(Value::as_str).map(str::to_string);
    (rows, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn q(field: &str, value: &str) -> ListQuery {
        ListQuery { field: Some(field.into()), value: Some(value.into()), ..Default::default() }
    }

    fn get<'a>(p: &'a [(&'static str, String)], k: &str) -> Option<&'a str> {
        p.iter().find(|(key, _)| *key == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn instances_first_page_is_all_tenants_with_one_extra_row() {
        let p = params(PagedList::Instances, &ListQuery::default(), 101).unwrap();
        assert_eq!(p, [("all_tenants", "true".to_string()), ("limit", "101".to_string())]);
    }

    #[test]
    fn a_filter_becomes_the_apis_parameter() {
        let p = params(PagedList::Instances, &q("project", " abc123 "), 51).unwrap();
        assert_eq!(get(&p, "project_id"), Some("abc123"), "trimmed, and the neutral key mapped");
        assert_eq!(get(&p, "limit"), Some("51"));
        assert_eq!(get(&params(PagedList::Instances, &q("zone", "nova"), 51).unwrap(), "availability_zone"), Some("nova"));
    }

    #[test]
    fn a_value_without_a_field_filters_by_name_and_an_empty_one_by_nothing() {
        let by_name = ListQuery { value: Some("web".into()), ..Default::default() };
        assert_eq!(get(&params(PagedList::Instances, &by_name, 11).unwrap(), "name"), Some("web"));
        let blank = params(PagedList::Instances, &q("host", "   "), 11).unwrap();
        assert!(get(&blank, "host").is_none(), "an empty value is no filter");
    }

    #[test]
    fn unknown_filters_and_sorts_are_refused_not_dropped() {
        assert!(params(PagedList::Instances, &q("tenant_id", "x"), 11).is_err(), "API names are not accepted, only the neutral keys");
        let bad_sort = ListQuery { sort: Some("password".into()), ..Default::default() };
        assert!(params(PagedList::Instances, &bad_sort, 11).is_err());
    }

    #[test]
    fn values_are_bounded() {
        assert!(params(PagedList::Instances, &q("name", &"a".repeat(256)), 11).is_err());
        assert!(params(PagedList::Instances, &q("name", "a\nb"), 11).is_err());
        assert!(params(PagedList::Instances, &q("name", &"a".repeat(255)), 11).is_ok());
    }

    #[test]
    fn the_marker_must_be_an_id_not_a_path() {
        for bad in ["../x", "a/b", "a?b=c", "a#b", "a%2Fb"] {
            let m = ListQuery { marker: Some(bad.into()), ..Default::default() };
            assert!(params(PagedList::Instances, &m, 11).is_err(), "{bad} must be refused");
        }
        let ok = ListQuery { marker: Some("3f9c-0a".into()), ..Default::default() };
        assert_eq!(get(&params(PagedList::Instances, &ok, 11).unwrap(), "marker"), Some("3f9c-0a"));
    }

    #[test]
    fn nova_sorts_with_sort_key_and_sort_dir() {
        let s = ListQuery { sort: Some("name".into()), desc: true, ..Default::default() };
        let p = params(PagedList::Instances, &s, 11).unwrap();
        assert_eq!((get(&p, "sort_key"), get(&p, "sort_dir")), (Some("display_name"), Some("desc")));
    }

    #[test]
    fn cinder_lists_sort_with_a_colon_and_filter_names_by_like() {
        let s = ListQuery { sort: Some("size".into()), desc: true, field: Some("name".into()), value: Some("db".into()), ..Default::default() };
        let p = params(PagedList::Volumes, &s, 51).unwrap();
        assert_eq!((get(&p, "sort"), get(&p, "name~"), get(&p, "all_tenants")), (Some("size:desc"), Some("db"), Some("true")));
        assert_eq!(get(&params(PagedList::Snapshots, &q("volume", "v1"), 11).unwrap(), "volume_id"), Some("v1"));
    }

    #[test]
    fn neutron_floating_ips_need_no_all_tenants() {
        let p = params(PagedList::FloatingIps, &q("address", "10.0.0.5"), 11).unwrap();
        assert_eq!(p, [("floating_ip_address", "10.0.0.5".to_string()), ("limit", "11".to_string())], "an admin's list is every project's already");
        let by_default = ListQuery { value: Some("10.0.0.5".into()), ..Default::default() };
        assert_eq!(get(&params(PagedList::FloatingIps, &by_default, 11).unwrap(), "floating_ip_address"), Some("10.0.0.5"), "the address is the default field");
        assert!(params(PagedList::FloatingIps, &q("name", "x"), 11).is_err(), "floating IPs have no name");
    }

    #[test]
    fn slice_c_lists_map_their_filters_and_sorts() {
        // Neutron and Glance list every project's to an admin: no all_tenants.
        let p = params(PagedList::Networks, &q("external", "true"), 11).unwrap();
        assert_eq!(p, [("router:external", "true".to_string()), ("limit", "11".to_string())]);
        assert_eq!(get(&params(PagedList::Trunks, &q("port", "p1"), 11).unwrap(), "port_id"), Some("p1"));
        let by_owner = params(PagedList::Images, &q("project", "abc"), 11).unwrap();
        assert_eq!((get(&by_owner, "owner"), get(&by_owner, "all_tenants")), (Some("abc"), None), "Glance's project is the owner");
        let sorted = ListQuery { sort: Some("size".into()), desc: true, ..Default::default() };
        let p = params(PagedList::Images, &sorted, 11).unwrap();
        assert_eq!((get(&p, "sort_key"), get(&p, "sort_dir")), (Some("size"), Some("desc")));
        assert!(params(PagedList::Routers, &sorted, 11).is_err(), "Neutron lists sort by name and status only");
        // Cinder's groups: every project's, colon sort.
        let s = ListQuery { sort: Some("created".into()), ..q("status", "available") };
        for l in [PagedList::VolumeGroups, PagedList::GroupSnapshots] {
            let p = params(l, &s, 11).unwrap();
            assert_eq!((get(&p, "all_tenants"), get(&p, "status"), get(&p, "sort")), (Some("true"), Some("available"), Some("created_at:asc")), "{l:?}");
        }
    }

    #[test]
    fn every_list_names_by_its_first_filter_and_only_cinder_volumes_take_like() {
        use crate::model::FilterKind;
        let all = [PagedList::Instances, PagedList::Volumes, PagedList::Snapshots, PagedList::FloatingIps, PagedList::Networks, PagedList::Routers, PagedList::Trunks, PagedList::Images, PagedList::VolumeGroups, PagedList::GroupSnapshots];
        for l in all {
            let keys: Vec<_> = l.filters().iter().map(|d| d.key).collect();
            let mut unique = keys.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(keys.len(), unique.len(), "{l:?}: a key twice");
            assert!(!l.sorts().is_empty(), "{l:?}");
            let name = l.filters().iter().find(|d| d.key == "name");
            if let Some(d) = name {
                // Only Nova (regex) and Cinder's volumes/snapshots (`name~`) match a part.
                let partial = matches!(l, PagedList::Instances | PagedList::Volumes | PagedList::Snapshots);
                assert_eq!(d.kind == FilterKind::Text, partial, "{l:?}");
            }
        }
    }

    #[test]
    fn cinder_parameters_follow_the_clouds_microversion() {
        let base = params(PagedList::Volumes, &q("name", "db"), 11).unwrap();
        let (p, h) = cinder(base.clone(), Some((3, 70)));
        assert_eq!((h, get(&p, "with_count"), get(&p, "name~")), (Some("volume 3.45"), Some("true"), Some("db")));
        let (p, h) = cinder(base.clone(), Some((3, 40)));
        assert_eq!((h, get(&p, "with_count"), get(&p, "name~")), (Some("volume 3.34"), None, Some("db")), "like, but no count");
        for old in [Some((3, 0)), None] {
            let (p, h) = cinder(base.clone(), old);
            assert_eq!((h, get(&p, "name"), get(&p, "name~")), (None, Some("db"), None), "{old:?}: an exact name, no header");
        }
    }

    #[test]
    fn whole_value_filters_say_so() {
        use crate::model::FilterKind;
        let kind = |l: PagedList, k: &str| l.filters().iter().find(|d| d.key == k).unwrap().kind;
        // Neutron matched no row for a partial address on ttcp-dev: its filters are exact.
        for k in ["address", "fixed", "network", "router"] {
            assert_eq!(kind(PagedList::FloatingIps, k), FilterKind::Exact, "{k}");
        }
        assert_eq!(kind(PagedList::Instances, "name"), FilterKind::Text, "Nova's name is a regex: a substring works");
        assert_eq!(kind(PagedList::Volumes, "name"), FilterKind::Text, "Cinder's name~ is like");
    }

    #[test]
    fn a_count_ignores_the_sort_and_asks_neutron_for_ids_only() {
        let sorted = ListQuery { sort: Some("name".into()), desc: true, marker: Some("old".into()), ..q("status", "ACTIVE") };
        let p = count_params(PagedList::Networks, &sorted, Some("n9"), 1000).unwrap();
        assert_eq!(
            p,
            [("status", "ACTIVE".to_string()), ("limit", "1000".to_string()), ("marker", "n9".to_string()), ("fields", "id".to_string())],
            "the filter stays, the sort and the page's own marker don't"
        );
        let p = count_params(PagedList::Instances, &q("host", "c1"), None, 1000).unwrap();
        assert_eq!((get(&p, "host"), get(&p, "all_tenants"), get(&p, "fields")), (Some("c1"), Some("true"), None));
        assert!(count_params(PagedList::Images, &q("password", "x"), None, 1000).is_err(), "same validation as a page");
        assert!(count_params(PagedList::Images, &ListQuery::default(), Some("a/b"), 1000).is_err());
    }

    #[test]
    fn a_count_goes_on_until_an_empty_answer() {
        let rows = |ids: &[&str]| ids.iter().map(|i| json!({"id": i})).collect::<Vec<_>>();
        let mut n = 0;
        assert_eq!(count_step(&mut n, &rows(&["a", "b"])).as_deref(), Some("b"), "a short page may be the API's own cap");
        assert_eq!(count_step(&mut n, &rows(&["c"])).as_deref(), Some("c"));
        assert_eq!(count_step(&mut n, &[]), None);
        assert_eq!(n, 3);
    }

    #[test]
    fn cut_keeps_a_page_and_names_the_next_marker() {
        let rows = |n: usize| (0..n).map(|i| json!({"id": format!("id{i}")})).collect::<Vec<_>>();
        let (page, next) = cut(rows(3), 3);
        assert_eq!((page.len(), next), (3, None), "exactly a page: no next");
        let (page, next) = cut(rows(4), 3);
        assert_eq!((page.len(), next.as_deref()), (3, Some("id2")), "the extra row only says there is more");
        assert_eq!(cut(rows(0), 3), (vec![], None));
    }
}
