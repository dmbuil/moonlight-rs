//! Goose scenarios: scripted Moonlight users, GET only once signed in.
//!
//! Each user signs in **once** (the login form's server function, which sets the `ml_session`
//! cookie) and then loops its journey with think times. Signing in on every request would mostly
//! measure Keystone's token issuance, and trip `[login] max_failures`. Users get distinct names
//! (`LOADTEST_USER_PREFIX` + index), so against the fake OpenStack each holds its own token and
//! Moonlight's per-token caches behave as with real users.
//!
//! A page is visited the two ways a browser does: a full load (`GET /project/…`, rendered on the
//! server, what a reload or a typed URL costs) and an in-app navigation, where the hydrated page
//! calls the page's server function (`GET /api/list_…`) and nothing else.
//!
//! Server-function URLs end in a hash of the path Moonlight was built from (`server_fn`'s default),
//! the same for every function in `api.rs`: it is read once from the login form's `action`.
//!
//! Two scenarios (`--scenarios` picks one; both run by default, half the users each):
//!
//! * `instancesuser`: Instances only, full loads and in-app navigations, 1:3 (the first baseline).
//! * `projectuser`: a working session: Overview, Instances, an instance's detail, Volumes and
//!   Networks as in-app navigations, plus a full load of one of those pages per round, in turn.
//!   Instance IDs come from the user's own `list_instances` answer, so the same journey runs on
//!   the fake OpenStack and on a real cloud.
//!
//! Environment: `LOADTEST_USERS` (comma-separated accounts, Goose user `i` signs in as entry
//! `i % n`; default: `LOADTEST_USER_PREFIX` + index, `loaduser0`, `loaduser1`…),
//! `LOADTEST_PASSWORD` (default `loadtest`, which the fake OpenStack accepts like any other; for a
//! real cloud, export it in the shell, never in a file), `LOADTEST_DOMAIN` (empty: Moonlight's
//! `[keystone] default_domain`),
//! `LOADTEST_WAIT_MS` (`min,max` think time between steps, default `1000,3000`; `0,0` for
//! throughput runs). Goose's own options (`--host`, `--users`, `--increase-rate`, `--run-time`,
//! `--report-file`…) are on the command line.

use std::sync::OnceLock;
use std::time::Duration;

use goose::metrics::GooseRequestMetric;
use goose::prelude::*;

/// The hash suffix of Moonlight's server-function URLs, read from the login page.
static FN_SUFFIX: OnceLock<String> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    let (min, max) = wait_ms();
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("InstancesUser")
                .set_wait_time(Duration::from_millis(min), Duration::from_millis(max))?
                .register_transaction(transaction!(sign_in).set_on_start().set_name("sign in"))
                .register_transaction(transaction!(instances_page).set_name("instances page (full load)"))
                .register_transaction(
                    transaction!(instances_nav)
                        .set_weight(3)?
                        .set_name("instances (in-app navigation)"),
                ),
        )
        .register_scenario(
            scenario!("ProjectUser")
                .set_wait_time(Duration::from_millis(min), Duration::from_millis(max))?
                .register_transaction(transaction!(sign_in).set_on_start().set_name("sign in"))
                .register_transaction(transaction!(overview_nav).set_name("overview"))
                .register_transaction(transaction!(instances_list).set_weight(2)?.set_name("instances"))
                .register_transaction(transaction!(instance_nav).set_weight(2)?.set_name("instance detail"))
                .register_transaction(transaction!(volumes_nav).set_name("volumes"))
                .register_transaction(transaction!(networks_nav).set_name("networks"))
                .register_transaction(transaction!(full_load).set_name("full page load")),
        )
        .execute()
        .await?;
    Ok(())
}

/// `LOADTEST_WAIT_MS` as `(min, max)` milliseconds.
fn wait_ms() -> (u64, u64) {
    let raw = std::env::var("LOADTEST_WAIT_MS").unwrap_or_else(|_| "1000,3000".into());
    let mut it = raw.split(',').map(|s| s.trim().parse::<u64>().unwrap_or(0));
    let min = it.next().unwrap_or(0);
    (min, it.next().unwrap_or(min).max(min))
}

/// An environment variable, or its default.
fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.into())
}

/// The URL of the server function `name` (`/api/<name><hash>`).
fn api(name: &str) -> String {
    format!("/api/{name}{}", FN_SUFFIX.get().map(String::as_str).unwrap_or(""))
}

/// The account Goose user `index` signs in as: entry `index % n` of the comma-separated
/// `accounts`, else `prefix` + index.
fn account(accounts: &str, prefix: &str, index: usize) -> String {
    let list: Vec<&str> = accounts.split(',').map(str::trim).filter(|a| !a.is_empty()).collect();
    if list.is_empty() {
        format!("{prefix}{index}")
    } else {
        list[index % list.len()].to_string()
    }
}

/// The `/api/login…` path in the login page's form.
fn login_action(html: &str) -> Option<&str> {
    let start = html.find("action=\"/api/login")? + "action=\"".len();
    let len = html[start..].find('"')?;
    Some(&html[start..start + len])
}

/// Marks the request failed with `why`, so it counts as an error in the report.
fn fail(user: &mut GooseUser, req: &mut GooseRequestMetric, why: &str) -> TransactionResult {
    user.set_failure(why, req, None, None)
}

/// Loads the login page (the first page a user sees), then posts the form as the hydrated page
/// does. Moonlight answers with a redirect to `/project` and the session cookie.
async fn sign_in(user: &mut GooseUser) -> TransactionResult {
    let mut page = user.get("/login").await?;
    let html = match page.response {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return fail(user, &mut page.request, "login page did not load"),
    };
    let Some(action) = login_action(&html) else {
        return fail(user, &mut page.request, "no login form in /login");
    };
    let suffix = action.trim_start_matches("/api/login").to_string();
    let _ = FN_SUFFIX.set(suffix);
    let name = account(
        &env("LOADTEST_USERS", ""),
        &env("LOADTEST_USER_PREFIX", "loaduser"),
        user.weighted_users_index,
    );
    let password = env("LOADTEST_PASSWORD", "loadtest");
    let domain = env("LOADTEST_DOMAIN", "");
    let form = [
        ("username", name.as_str()),
        ("password", &password),
        ("domain", &domain),
    ];
    let path = api("login");
    let reqwest = user
        .get_request_builder(&GooseMethod::Post, &path)?
        .header("accept", "application/json")
        // What a browser sends on a same-origin `fetch`: Moonlight refuses cross-origin writes.
        .header("sec-fetch-site", "same-origin")
        .form(&form);
    let req = GooseRequest::builder()
        .method(GooseMethod::Post)
        .path(path.as_str())
        .name("/api/login")
        .set_request_builder(reqwest)
        .build();
    let mut rsp = user.request(req).await?;
    match &rsp.response {
        Ok(r) if r.status().is_success() => Ok(()),
        Ok(r) => {
            let why = format!("sign-in answered {}", r.status());
            fail(user, &mut rsp.request, &why)
        }
        Err(_) => fail(user, &mut rsp.request, "sign-in failed"),
    }
}

/// A full load of Project › Compute › Instances: the server renders the layout and the list.
async fn instances_page(user: &mut GooseUser) -> TransactionResult {
    page(user, "/project/instances").await
}

/// An in-app navigation to Instances: the hydrated page calls `list_instances` with no filter
/// (`q[desc]=false`: the query is a struct, and `serde_qs` needs one of its fields).
async fn instances_nav(user: &mut GooseUser) -> TransactionResult {
    call(user, "list_instances", "?q%5Bdesc%5D=false").await.map(drop)
}

/// The query of a list server function with no filter, sort or marker.
const NO_FILTER: &str = "?q%5Bdesc%5D=false";

/// GETs the server function `name` with `query` (`?…` or empty), names the request after the
/// function, fails it on a non-2xx answer and returns the body.
async fn call(user: &mut GooseUser, name: &str, query: &str) -> Result<String, Box<TransactionError>> {
    let path = format!("{}{query}", api(name));
    let label = format!("/api/{name}");
    let req = GooseRequest::builder().path(path.as_str()).name(label.as_str()).build();
    let mut rsp = user.request(req).await?;
    let status = match &rsp.response {
        Ok(r) => r.status(),
        Err(_) => {
            fail(user, &mut rsp.request, &format!("{name} failed"))?;
            return Ok(String::new());
        }
    };
    let body = match rsp.response {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => String::new(),
    };
    if !status.is_success() {
        fail(user, &mut rsp.request, &format!("{name} answered {status}"))?;
    }
    Ok(body)
}

/// What a `projectuser` remembers between steps.
#[derive(Clone, Default)]
struct Journey {
    /// Instance IDs from the last `list_instances` answer.
    instances: Vec<String>,
    /// Counts the detail visits, to open the instances in turn.
    step: usize,
    /// Counts the full loads, to load the pages in turn.
    loads: usize,
}

/// The `"id"` of every row in a `list_…` answer (`{"data": {"rows": [{"id": …}, …]}}`).
fn row_ids(body: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    v["data"]["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["id"].as_str().map(str::to_string))
        .collect()
}

/// The user's journey state, created on first use.
fn journey(user: &mut GooseUser) -> &mut Journey {
    if user.get_session_data::<Journey>().is_none() {
        user.set_session_data(Journey::default());
    }
    user.get_session_data_mut::<Journey>().expect("set above")
}

/// The next instance to open, in turn; `None` before the list was read or when it is empty.
fn next_instance(user: &mut GooseUser) -> Option<String> {
    let j = journey(user);
    if j.instances.is_empty() {
        return None;
    }
    j.step += 1;
    Some(j.instances[j.step % j.instances.len()].clone())
}

/// Overview: the quota donuts and this month's usage.
async fn overview_nav(user: &mut GooseUser) -> TransactionResult {
    call(user, "overview", "").await?;
    call(user, "get_usage", "?start=&end=").await.map(drop)
}

/// Instances, keeping the IDs for the detail step.
async fn instances_list(user: &mut GooseUser) -> TransactionResult {
    let body = call(user, "list_instances", NO_FILTER).await?;
    let ids = row_ids(&body);
    if !ids.is_empty() {
        journey(user).instances = ids;
    }
    Ok(())
}

/// One instance's detail page (its *Overview* tab), the instances in turn.
async fn instance_nav(user: &mut GooseUser) -> TransactionResult {
    match next_instance(user) {
        Some(id) => call(user, "get_instance", &format!("?id={id}")).await.map(drop),
        None => Ok(()),
    }
}

/// Volumes.
async fn volumes_nav(user: &mut GooseUser) -> TransactionResult {
    call(user, "list_volumes", NO_FILTER).await.map(drop)
}

/// Networks.
async fn networks_nav(user: &mut GooseUser) -> TransactionResult {
    call(user, "list_networks", "").await.map(drop)
}

/// A full load (a reload or a typed URL) of one of the journey's pages, each in turn.
async fn full_load(user: &mut GooseUser) -> TransactionResult {
    let j = journey(user);
    j.loads += 1;
    let n = j.loads;
    let path = match n % 5 {
        0 => "/project".to_string(),
        1 => "/project/instances".to_string(),
        2 => match next_instance(user) {
            Some(id) => format!("/project/instances/{id}"),
            None => "/project/instances".to_string(),
        },
        3 => "/project/volumes".to_string(),
        _ => "/project/networks".to_string(),
    };
    page(user, &path).await
}

/// GETs a page rendered on the server; fails if it redirected to `/login`. Requests are named by
/// page, with an instance's ID replaced by `{id}`.
async fn page(user: &mut GooseUser, path: &str) -> TransactionResult {
    let label = match path.strip_prefix("/project/instances/") {
        Some(_) => "/project/instances/{id}".to_string(),
        None => path.to_string(),
    };
    let req = GooseRequest::builder().path(path).name(label.as_str()).build();
    let mut rsp = user.request(req).await?;
    let signed_out = matches!(&rsp.response, Ok(r) if r.url().path().starts_with("/login"));
    if signed_out {
        return fail(user, &mut rsp.request, "redirected to /login: the session is gone");
    }
    let status = rsp.response.as_ref().map(|r| r.status()).ok();
    if let Ok(r) = rsp.response {
        let _ = r.bytes().await;
    }
    match status {
        Some(s) if !s.is_success() => fail(user, &mut rsp.request, &format!("{path} answered {s}")),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_function_hash_from_the_login_form() {
        let html = r#"<form method="post" action="/api/login16492309848134432" class="login-form">"#;
        assert_eq!(login_action(html), Some("/api/login16492309848134432"));
        assert_eq!(login_action("<form action=\"/elsewhere\">"), None);
    }

    #[test]
    fn users_take_the_listed_accounts_in_turn() {
        assert_eq!(account("", "loaduser", 3), "loaduser3");
        assert_eq!(account("alice, bob", "x", 0), "alice");
        assert_eq!(account("alice, bob", "x", 3), "bob");
    }

    #[test]
    fn reads_row_ids_from_a_list_answer() {
        let body = r#"{"data":{"rows":[{"id":"a","name":"x"},{"id":"b"}],"next":null},"millis":3,"calls":4}"#;
        assert_eq!(row_ids(body), ["a", "b"]);
        assert!(row_ids("not json").is_empty());
    }
}
