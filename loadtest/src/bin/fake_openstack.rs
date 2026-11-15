//! A fake OpenStack for load testing Moonlight: Keystone, Nova, Neutron, Cinder and Glance, just
//! enough for the scenarios' pages, answering at once or after `--delay-ms`.
//!
//! GET only, like the scenarios: every write answers `405`, except Keystone's `POST auth/tokens`
//! (signing in and rescoping are reads for Moonlight). Any password is accepted, and every sign-in
//! gets a distinct token, so N users exercise Moonlight's per-token caches the way N real users
//! would. Payloads are generated (`--servers`, `--volumes`…) in the shape of the real APIs, like
//! the fixtures of Moonlight's mapping tests. An unknown path answers `404` and is logged, so a
//! page that starts calling a new API shows up in this log.
//!
//! One listener serves every service under its own prefix (`/identity`, `/compute`, `/network`,
//! `/volume`, `/image`); the catalog points back at it.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use clap::Parser;
use serde_json::{Value, json};

/// Command line: where to listen, how big the project is and how slow the cloud.
#[derive(Parser, Debug, Clone)]
#[command(about = "A fake OpenStack for Moonlight's load tests (GET only)")]
struct Args {
    /// Address to listen on.
    #[arg(long, default_value = "127.0.0.1:5000")]
    listen: SocketAddr,
    /// Servers in the project.
    #[arg(long, default_value_t = 50)]
    servers: usize,
    /// Volumes in the project (the first ones are attached to the first servers).
    #[arg(long, default_value_t = 50)]
    volumes: usize,
    /// Images visible to the project.
    #[arg(long, default_value_t = 20)]
    images: usize,
    /// Flavors.
    #[arg(long, default_value_t = 10)]
    flavors: usize,
    /// Networks of the project (each with one subnet), plus one external network.
    #[arg(long, default_value_t = 5)]
    networks: usize,
    /// Delay before every answer, in milliseconds: emulates a slow cloud.
    #[arg(long, default_value_t = 0)]
    delay_ms: u64,
    /// Random extra delay, up to this many milliseconds, on top of `--delay-ms`.
    #[arg(long, default_value_t = 0)]
    jitter_ms: u64,
}

/// What every request shares: the arguments, the issued tokens and a request counter.
struct Stub {
    args: Args,
    /// The base URL the catalog and version documents point at.
    base: String,
    /// Token → user name, for rescoping (`v3token`) and to name the user in the answer.
    tokens: Mutex<HashMap<String, String>>,
    /// Sequence for token IDs, and the jitter's seed.
    seq: AtomicU64,
    /// Requests answered, by `METHOD /path-shape`, printed on Ctrl-C.
    counts: Mutex<HashMap<String, u64>>,
}

/// The project every user is a member of.
const PROJECT_ID: &str = "0f5e1d8a6c2b4e3f9a7d1c0b2e4f6a8d";
/// Its name.
const PROJECT_NAME: &str = "loadtest";
/// The only region.
const REGION: &str = "RegionOne";

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let base = format!("http://{}", args.listen);
    let stub = Arc::new(Stub {
        base,
        tokens: Mutex::new(HashMap::new()),
        seq: AtomicU64::new(1),
        counts: Mutex::new(HashMap::new()),
        args: args.clone(),
    });
    let app = Router::new().fallback(handle).with_state(stub.clone());
    let listener = tokio::net::TcpListener::bind(args.listen).await.expect("bind");
    println!(
        "fake OpenStack on {} ({} servers, {} volumes, {} images, {} flavors, {} networks; delay {} ms + up to {} ms)",
        stub.base, args.servers, args.volumes, args.images, args.flavors, args.networks, args.delay_ms, args.jitter_ms
    );
    println!("Keystone URL for Moonlight: {}/identity", stub.base);
    let summary = stub.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            let counts = summary.counts.lock().unwrap();
            let mut rows: Vec<_> = counts.iter().collect();
            rows.sort_by(|a, b| b.1.cmp(a.1));
            println!("\nrequests answered:");
            for (k, n) in rows {
                println!("{n:>9}  {k}");
            }
        })
        .await
        .expect("serve");
}

/// Every request: count it, wait the configured delay, then route it by service prefix.
async fn handle(
    State(stub): State<Arc<Stub>>,
    method: Method,
    uri: Uri,
    Query(q): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    let path = uri.path().trim_end_matches('/').to_string();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let delay = stub.args.delay_ms + jitter(&stub);
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    let shape = shape_of(&parts);
    *stub
        .counts
        .lock()
        .unwrap()
        .entry(format!("{method} /{shape}"))
        .or_default() += 1;
    if method != Method::GET && !(method == Method::POST && parts.ends_with(&["auth", "tokens"])) {
        return (StatusCode::METHOD_NOT_ALLOWED, "the fake OpenStack is read-only").into_response();
    }
    let answer = match parts.split_first() {
        Some((&"identity", rest)) => identity(&stub, &method, rest, &body),
        Some((&"compute", rest)) => compute(&stub, rest, &q),
        Some((&"network", rest)) => network(&stub, rest, &q),
        Some((&"volume", rest)) => volume(&stub, rest, &q),
        Some((&"image", rest)) => image(&stub, rest, &q),
        _ => None,
    };
    answer.unwrap_or_else(|| {
        eprintln!("404 {method} {uri}");
        (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"error": {"message": "not in the fake OpenStack"}})),
        )
            .into_response()
    })
}

/// A path with IDs replaced by `{id}`, so the counters group one endpoint.
fn shape_of(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| {
            if p.len() >= 20 && p.contains('-') || p.len() == 32 {
                "{id}"
            } else {
                p
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A pseudo-random extra delay in `0..=jitter_ms` (splitmix64 over the request sequence).
fn jitter(stub: &Stub) -> u64 {
    if stub.args.jitter_ms == 0 {
        return 0;
    }
    let mut z = stub
        .seq
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) % (stub.args.jitter_ms + 1)
}

/// `200` with a JSON body.
fn ok(v: Value) -> Option<Response> {
    Some(axum::Json(v).into_response())
}

// ---------------------------------------------------------------- Keystone

/// Keystone v3: version discovery, `POST auth/tokens` (password or token, scoped or not) and the
/// project lists.
fn identity(stub: &Stub, method: &Method, rest: &[&str], body: &[u8]) -> Option<Response> {
    let v3 = json!({
        "id": "v3.14", "status": "stable", "updated": "2020-04-07T00:00:00Z",
        "links": [{"rel": "self", "href": format!("{}/identity/v3/", stub.base)}],
        "media-types": [{"base": "application/json", "type": "application/vnd.openstack.identity-v3+json"}]
    });
    match rest {
        [] => ok(json!({"versions": {"values": [v3]}})),
        ["v3"] => ok(json!({"version": v3})),
        ["v3", "auth", "tokens"] if method == Method::POST => token(stub, body),
        ["v3", "auth", "projects"] => ok(json!({"projects": [project()], "links": {"self": null, "next": null}})),
        ["v3", "users", _, "projects"] => ok(json!({"projects": [project()], "links": {"self": null, "next": null}})),
        _ => None,
    }
}

/// The project every user belongs to, as Keystone lists it.
fn project() -> Value {
    json!({"id": PROJECT_ID, "name": PROJECT_NAME, "domain_id": "default", "enabled": true,
           "description": "", "is_domain": false, "parent_id": "default", "tags": []})
}

/// `POST /v3/auth/tokens`: a new token for a password (any password) or for a token this stub
/// issued; scoped (with roles and a catalog) when the request asks for the project.
fn token(stub: &Stub, body: &[u8]) -> Option<Response> {
    let req: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let identity = &req["auth"]["identity"];
    let user = if let Some(name) = identity["password"]["user"]["name"].as_str() {
        name.to_string()
    } else if let Some(t) = identity["token"]["id"].as_str() {
        match stub.tokens.lock().unwrap().get(t) {
            Some(u) => u.clone(),
            None => {
                return Some(
                    (
                        StatusCode::UNAUTHORIZED,
                        axum::Json(json!({"error": {"code": 401, "message": "unknown token"}})),
                    )
                        .into_response(),
                );
            }
        }
    } else {
        return Some((StatusCode::BAD_REQUEST, "no identity").into_response());
    };
    let scoped = req["auth"]["scope"]["project"].is_object();
    let n = stub.seq.fetch_add(1, Ordering::Relaxed);
    let id = format!("gAAAAAloadtest{n:016x}");
    stub.tokens.lock().unwrap().insert(id.clone(), user.clone());
    let methods = if identity["password"].is_object() {
        "password"
    } else {
        "token"
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut tok = json!({
        "methods": [methods],
        "user": {"id": user_id(&user), "name": user, "domain": {"id": "default", "name": "Default"},
                 "password_expires_at": null},
        "audit_ids": [format!("audit{n}")],
        "issued_at": iso(now),
        "expires_at": iso(now + 3600),
    });
    if scoped {
        tok["project"] =
            json!({"id": PROJECT_ID, "name": PROJECT_NAME, "domain": {"id": "default", "name": "Default"}});
        tok["roles"] = json!([{"id": "r-member", "name": "member"}, {"id": "r-reader", "name": "reader"}]);
        tok["catalog"] = catalog(&stub.base);
        tok["is_domain"] = json!(false);
    }
    let mut rsp = (StatusCode::CREATED, axum::Json(json!({"token": tok}))).into_response();
    rsp.headers_mut()
        .insert("x-subject-token", HeaderValue::from_str(&id).unwrap());
    Some(rsp)
}

/// A stable user ID for a name.
fn user_id(name: &str) -> String {
    let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    format!("{h:016x}{:016x}", h.rotate_left(17))
}

/// A Unix time as Keystone writes it (`2026-11-15T10:00:00.000000Z`).
fn iso(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000000Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The service catalog: every service on this stub, public interface, one region.
fn catalog(base: &str) -> Value {
    let svc = |ty: &str, name: &str, path: &str| {
        json!({"type": ty, "name": name, "id": format!("svc-{name}"), "endpoints": [
            {"id": format!("ep-{name}"), "interface": "public", "region": REGION, "region_id": REGION,
             "url": format!("{base}/{path}")}
        ]})
    };
    json!([
        svc("identity", "keystone", "identity"),
        svc("compute", "nova", "compute"),
        svc("network", "neutron", "network"),
        svc("block-storage", "cinder", "volume"),
        svc("image", "glance", "image"),
    ])
}

// ---------------------------------------------------------------- Listing helpers

/// One page of `items` after `marker`, at most `limit` long (the APIs' marker pagination), and the
/// marker of the next page when more remain.
fn page(items: Vec<Value>, q: &HashMap<String, String>) -> (Vec<Value>, Option<String>) {
    let start = q
        .get("marker")
        .and_then(|m| items.iter().position(|v| v["id"].as_str() == Some(m)).map(|i| i + 1))
        .unwrap_or(0);
    let limit = q.get("limit").and_then(|l| l.parse().ok()).unwrap_or(usize::MAX);
    let rest = items.len().saturating_sub(start);
    let page: Vec<Value> = items.into_iter().skip(start).take(limit).collect();
    let next = (rest > page.len()).then(|| page.last().and_then(|v| v["id"].as_str()).unwrap_or("").to_string());
    (page, next)
}

/// A paged list wrapped as Nova, Neutron and Cinder do: `{key: […], key_links: [next]}`.
fn listed(key: &str, url: &str, items: Vec<Value>, q: &HashMap<String, String>) -> Option<Response> {
    let (rows, next) = page(items, q);
    let mut v = json!({ key: rows });
    if let Some(m) = next {
        let limit = q.get("limit").map(String::as_str).unwrap_or("1000");
        v[format!("{key}_links")] = json!([{"rel": "next", "href": format!("{url}?limit={limit}&marker={m}")}]);
    }
    ok(v)
}

/// The `n`th ID of a kind, UUID-shaped (`kind` is a 4-digit hex tag).
fn uid(kind: u16, n: usize) -> String {
    format!("{kind:04x}0000-0000-4000-8000-{n:012x}")
}

/// A version document entry.
fn version(id: &str, href: String, extra: Value) -> Value {
    let mut v = json!({"id": id, "status": "CURRENT", "updated": "2024-01-01T00:00:00Z",
                       "links": [{"rel": "self", "href": href}]});
    if let (Some(v), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        v.extend(e.clone());
    }
    v
}

// ---------------------------------------------------------------- Nova

/// Nova v2.1 (microversions up to 2.95): servers, flavors, limits and an instance's tabs.
fn compute(stub: &Stub, rest: &[&str], q: &HashMap<String, String>) -> Option<Response> {
    let a = &stub.args;
    let root = format!("{}/compute/v2.1", stub.base);
    let v21 = version(
        "v2.1",
        format!("{root}/"),
        json!({"version": "2.95", "min_version": "2.1"}),
    );
    match rest {
        [] => ok(json!({"versions": [v21]})),
        ["v2.1"] => ok(json!({"version": v21})),
        ["v2.1", "servers", "detail"] => listed("servers", &format!("{root}/servers/detail"), servers(a), q),
        ["v2.1", "servers"] => listed(
            "servers",
            &format!("{root}/servers"),
            servers(a)
                .into_iter()
                .map(|s| json!({"id": s["id"], "name": s["name"], "links": []}))
                .collect(),
            q,
        ),
        ["v2.1", "servers", id] => servers(a)
            .into_iter()
            .find(|s| s["id"].as_str() == Some(id))
            .and_then(|s| ok(json!({"server": s}))),
        ["v2.1", "servers", _, "os-interface"] => ok(json!({"interfaceAttachments": []})),
        ["v2.1", "servers", _, "os-instance-actions"] => ok(json!({"instanceActions": []})),
        ["v2.1", "flavors", "detail"] => listed("flavors", &format!("{root}/flavors/detail"), flavors(a), q),
        ["v2.1", "flavors"] => listed("flavors", &format!("{root}/flavors"), flavors(a), q),
        ["v2.1", "os-keypairs"] => ok(json!({"keypairs": []})),
        ["v2.1", "os-server-groups"] => ok(json!({"server_groups": []})),
        ["v2.1", "os-availability-zone"] => {
            ok(json!({"availabilityZoneInfo": [{"zoneName": "nova", "zoneState": {"available": true}, "hosts": null}]}))
        }
        ["v2.1", "limits"] => ok(json!({"limits": {"rate": [], "absolute": {
            "maxTotalInstances": 100, "totalInstancesUsed": a.servers,
            "maxTotalCores": 400, "totalCoresUsed": a.servers * 2,
            "maxTotalRAMSize": 819_200, "totalRAMUsed": a.servers * 4096,
            "maxTotalKeypairs": 100, "maxServerGroups": 10, "totalServerGroupsUsed": 0,
            "maxServerMeta": 128, "maxImageMeta": 128, "maxPersonality": 5, "maxPersonalitySize": 10240,
            "maxSecurityGroups": -1, "maxSecurityGroupRules": -1, "maxTotalFloatingIps": -1,
            "totalFloatingIpsUsed": 0, "totalSecurityGroupsUsed": 0, "maxServerGroupMembers": 10
        }}})),
        ["v2.1", "os-simple-tenant-usage", project] => {
            // Paged by instance (`marker` = the last `instance_id`); `id` lets `page` follow it.
            let usages = servers(a)
                .into_iter()
                .map(|s| {
                    json!({"id": s["id"], "instance_id": s["id"], "name": s["name"], "vcpus": 2, "memory_mb": 4096,
                           "local_gb": 40, "hours": 360.5, "uptime": 1_297_800, "state": s["OS-EXT-STS:vm_state"],
                           "flavor": "m1.f0", "tenant_id": project, "started_at": "2026-09-14T10:23:00.000000",
                           "ended_at": null})
                })
                .collect();
            let (rows, _) = page(usages, q);
            ok(json!({"tenant_usage": {"tenant_id": project, "server_usages": rows,
                "total_hours": 360.5 * a.servers as f64, "total_vcpus_usage": 721.0 * a.servers as f64,
                "total_memory_mb_usage": 1_476_608.0 * a.servers as f64, "total_local_gb_usage": 14_420.0 * a.servers as f64,
                "start": "2026-11-01T00:00:00.000000", "stop": "2026-11-16T00:00:00.000000"}}))
        }
        _ => None,
    }
}

/// The project's servers, shaped like `GET /servers/detail` at microversion 2.95.
fn servers(a: &Args) -> Vec<Value> {
    (0..a.servers)
        .map(|i| {
            let net = i % a.networks.max(1);
            let mut volumes = Vec::new();
            if i < a.volumes {
                volumes.push(json!({"id": uid(0xb01, i), "delete_on_termination": false}));
            }
            json!({
                "id": uid(0x5e5, i),
                "name": format!("vm-{i:05}"),
                "status": if i % 10 == 9 { "SHUTOFF" } else { "ACTIVE" },
                "tenant_id": PROJECT_ID,
                "user_id": "u-owner",
                "created": "2026-09-14T10:22:00Z",
                "updated": "2026-10-01T08:00:00Z",
                "key_name": "ops-2026",
                "locked": false,
                "image": if i % 2 == 0 { json!({"id": uid(0x1a6, i % a.images.max(1))}) } else { json!("") },
                "flavor": {"original_name": format!("m1.f{}", i % a.flavors.max(1)), "vcpus": 2, "ram": 4096,
                           "disk": 40, "ephemeral": 0, "swap": 0, "extra_specs": {}},
                "OS-EXT-AZ:availability_zone": "nova",
                "OS-EXT-STS:power_state": if i % 10 == 9 { 4 } else { 1 },
                "OS-EXT-STS:task_state": null,
                "OS-EXT-STS:vm_state": if i % 10 == 9 { "stopped" } else { "active" },
                "OS-SRV-USG:launched_at": "2026-09-14T10:23:00.000000",
                "addresses": {
                    format!("net-{net}"): [
                        {"addr": format!("10.{net}.{}.{}", i / 250, i % 250 + 2), "version": 4,
                         "OS-EXT-IPS:type": "fixed", "OS-EXT-IPS-MAC:mac_addr": format!("fa:16:3e:00:{:02x}:{:02x}", i / 256 % 256, i % 256)}
                    ]
                },
                "security_groups": [{"name": "default"}],
                "os-extended-volumes:volumes_attached": volumes,
                "metadata": {"role": "load", "index": i.to_string()},
                "tags": [],
                "description": null,
                "hostId": "a1b2c3",
                "accessIPv4": "", "accessIPv6": "",
                "config_drive": "",
                "progress": 0,
                "links": [],
            })
        })
        .collect()
}

/// The flavors, shaped like `GET /flavors/detail`.
fn flavors(a: &Args) -> Vec<Value> {
    (0..a.flavors)
        .map(|i| {
            json!({"id": format!("f{i}"), "name": format!("m1.f{i}"), "vcpus": 1 << (i % 5), "ram": 1024 << (i % 6),
                   "disk": 20 + 10 * i, "OS-FLV-EXT-DATA:ephemeral": 0, "swap": "", "rxtx_factor": 1.0,
                   "os-flavor-access:is_public": true, "OS-FLV-DISABLED:disabled": false, "description": null,
                   "extra_specs": {}, "links": []})
        })
        .collect()
}

// ---------------------------------------------------------------- Neutron

/// Neutron v2.0: extensions, networks, subnets, ports, floating IPs, routers, security groups.
fn network(stub: &Stub, rest: &[&str], q: &HashMap<String, String>) -> Option<Response> {
    let a = &stub.args;
    let root = format!("{}/network/v2.0", stub.base);
    let v2 = version("v2.0", format!("{root}/"), json!({}));
    let nets = networks(a);
    // Neutron filters by query parameters; the scenarios only ask for the project's own,
    // shared and external networks.
    let filtered = |list: Vec<Value>| -> Vec<Value> {
        list.into_iter()
            .filter(|n| {
                q.iter().all(|(k, v)| match k.as_str() {
                    "tenant_id" | "project_id" => n["tenant_id"].as_str() == Some(v),
                    "shared" => n["shared"].as_bool().map(|b| b.to_string()).as_deref() == Some(v),
                    "router:external" => n["router:external"].as_bool().map(|b| b.to_string()).as_deref() == Some(v),
                    "device_id" => n["device_id"].as_str() == Some(v),
                    _ => true,
                })
            })
            .collect()
    };
    match rest {
        [] => ok(json!({"versions": [v2]})),
        ["v2.0"] => ok(json!({"version": v2})),
        ["v2.0", "extensions"] => ok(json!({"extensions": EXTENSIONS
            .iter()
            .map(|x| json!({"alias": x, "name": x, "description": "", "updated": "2024-01-01T00:00:00Z", "links": []}))
            .collect::<Vec<_>>()})),
        ["v2.0", "networks"] => listed("networks", &format!("{root}/networks"), filtered(nets), q),
        ["v2.0", "networks", id] => nets
            .into_iter()
            .find(|n| n["id"].as_str() == Some(id))
            .and_then(|n| ok(json!({"network": n}))),
        ["v2.0", "subnets"] => listed("subnets", &format!("{root}/subnets"), filtered(subnets(a)), q),
        ["v2.0", "ports"] => listed("ports", &format!("{root}/ports"), filtered(ports(a)), q),
        ["v2.0", "floatingips"] => listed("floatingips", &format!("{root}/floatingips"), Vec::new(), q),
        ["v2.0", "routers"] => listed("routers", &format!("{root}/routers"), Vec::new(), q),
        ["v2.0", "security-groups"] => listed(
            "security_groups",
            &format!("{root}/security-groups"),
            vec![
                json!({"id": uid(0x5ec, 0), "name": "default", "description": "Default security group",
                        "tenant_id": PROJECT_ID, "project_id": PROJECT_ID, "stateful": true, "security_group_rules": []}),
            ],
            q,
        ),
        ["v2.0", "quotas", _, "details"] => {
            let q = |used: usize, limit: i64| json!({"used": used, "limit": limit, "reserved": 0});
            ok(
                json!({"quota": {"network": q(a.networks, 100), "subnet": q(a.networks, 100),
                "port": q(a.servers, 500), "router": q(0, 10), "floatingip": q(0, 50),
                "security_group": q(1, 10), "security_group_rule": q(4, 100), "rbac_policy": q(0, -1)}}),
            )
        }
        ["v2.0", "quotas", _] => ok(
            json!({"quota": {"network": 100, "subnet": 100, "port": 500, "router": 10,
            "floatingip": 50, "security_group": 10, "security_group_rule": 100, "rbac_policy": 10}}),
        ),
        _ => None,
    }
}

/// The Neutron extensions the stub claims, so no extension-gated page or action is hidden.
const EXTENSIONS: &[&str] = &[
    "router",
    "external-net",
    "extraroute",
    "security-group",
    "quotas",
    "port-security",
    "allowed-address-pairs",
    "binding",
    "agent",
    "availability_zone",
    "dns-integration",
    "qos",
    "standard-attr-tag",
    "subnet_allocation",
    "net-mtu",
    "trunk",
    "l3-ha",
    "dvr",
];

/// The project's networks plus one shared external network.
fn networks(a: &Args) -> Vec<Value> {
    let mut v: Vec<Value> = (0..a.networks)
        .map(|i| {
            json!({"id": uid(0x0e7, i), "name": format!("net-{i}"), "tenant_id": PROJECT_ID, "project_id": PROJECT_ID,
                   "status": "ACTIVE", "admin_state_up": true, "shared": false, "router:external": false,
                   "subnets": [uid(0x5b7, i)], "mtu": 1442, "availability_zones": ["nova"],
                   "provider:network_type": "geneve", "port_security_enabled": true, "tags": []})
        })
        .collect();
    v.push(
        json!({"id": uid(0x0e7, 9999), "name": "public", "tenant_id": "admin-project", "project_id": "admin-project",
                  "status": "ACTIVE", "admin_state_up": true, "shared": false, "router:external": true,
                  "subnets": [], "mtu": 1500, "availability_zones": ["nova"], "tags": []}),
    );
    v
}

/// One subnet per project network.
fn subnets(a: &Args) -> Vec<Value> {
    (0..a.networks)
        .map(|i| {
            json!({"id": uid(0x5b7, i), "name": format!("subnet-{i}"), "network_id": uid(0x0e7, i),
                   "tenant_id": PROJECT_ID, "project_id": PROJECT_ID, "ip_version": 4, "cidr": format!("10.{i}.0.0/16"),
                   "gateway_ip": format!("10.{i}.0.1"), "enable_dhcp": true, "allocation_pools": [],
                   "dns_nameservers": [], "host_routes": []})
        })
        .collect()
}

/// One port per server, on its server's network.
fn ports(a: &Args) -> Vec<Value> {
    (0..a.servers)
        .map(|i| {
            let net = i % a.networks.max(1);
            json!({"id": uid(0x907, i), "name": "", "network_id": uid(0x0e7, net), "tenant_id": PROJECT_ID,
                   "project_id": PROJECT_ID, "device_id": uid(0x5e5, i), "device_owner": "compute:nova",
                   "status": "ACTIVE", "admin_state_up": true, "mac_address": format!("fa:16:3e:00:{:02x}:{:02x}", i / 256 % 256, i % 256),
                   "fixed_ips": [{"subnet_id": uid(0x5b7, net), "ip_address": format!("10.{net}.{}.{}", i / 250, i % 250 + 2)}],
                   "security_groups": [uid(0x5ec, 0)], "port_security_enabled": true, "binding:vnic_type": "normal"})
        })
        .collect()
}

// ---------------------------------------------------------------- Cinder

/// Cinder v3 (microversions up to 3.70): volumes, snapshots, limits.
fn volume(stub: &Stub, rest: &[&str], q: &HashMap<String, String>) -> Option<Response> {
    let a = &stub.args;
    let root = format!("{}/volume/v3", stub.base);
    let v3 = version(
        "v3.0",
        format!("{root}/"),
        json!({"version": "3.70", "min_version": "3.0"}),
    );
    match rest {
        [] => ok(json!({"versions": [v3]})),
        ["v3"] => ok(json!({"version": v3})),
        ["v3", "volumes", "detail"] | ["v3", _, "volumes", "detail"] => {
            listed("volumes", &format!("{root}/volumes/detail"), volumes(a), q)
        }
        ["v3", "snapshots", "detail"] | ["v3", _, "snapshots", "detail"] => {
            listed("snapshots", &format!("{root}/snapshots/detail"), Vec::new(), q)
        }
        ["v3", "limits"] | ["v3", _, "limits"] => ok(json!({"limits": {"rate": [], "absolute": {
            "maxTotalVolumes": 100, "totalVolumesUsed": a.volumes, "maxTotalVolumeGigabytes": 10_000,
            "totalGigabytesUsed": a.volumes * 20, "maxTotalSnapshots": 100, "totalSnapshotsUsed": 0,
            "maxTotalBackups": 10, "totalBackupsUsed": 0, "maxTotalBackupGigabytes": 1000, "totalBackupGigabytesUsed": 0
        }}})),
        _ => None,
    }
}

/// The project's volumes, shaped like `GET /volumes/detail`; the first ones are attached.
fn volumes(a: &Args) -> Vec<Value> {
    (0..a.volumes)
        .map(|i| {
            let attachments = if i < a.servers {
                json!([{"server_id": uid(0x5e5, i), "attachment_id": uid(0xa77, i), "device": "/dev/vda",
                        "volume_id": uid(0xb01, i), "id": uid(0xb01, i), "host_name": null}])
            } else {
                json!([])
            };
            json!({"id": uid(0xb01, i), "name": format!("vol-{i:05}"), "status": if i < a.servers { "in-use" } else { "available" },
                   "size": 20, "volume_type": "standard", "availability_zone": "nova", "bootable": "true",
                   "encrypted": false, "multiattach": false, "attachments": attachments,
                   "os-vol-tenant-attr:tenant_id": PROJECT_ID, "created_at": "2026-09-14T10:22:00.000000",
                   "updated_at": null, "description": null, "metadata": {}, "snapshot_id": null, "source_volid": null,
                   "volume_image_metadata": {"image_name": "rhel-9.4"}, "links": []})
        })
        .collect()
}

// ---------------------------------------------------------------- Glance

/// Glance v2: images (with `next` links, as Glance pages) and the import methods.
fn image(stub: &Stub, rest: &[&str], q: &HashMap<String, String>) -> Option<Response> {
    let a = &stub.args;
    let v2 = version("v2.16", format!("{}/image/v2/", stub.base), json!({}));
    match rest {
        [] => ok(json!({"versions": [v2]})),
        ["v2", "images"] => {
            let (rows, next) = page(images(a), q);
            let mut v = json!({"images": rows, "first": "/v2/images", "schema": "/v2/schemas/images"});
            if let Some(m) = next {
                let limit = q.get("limit").map(String::as_str).unwrap_or("25");
                v["next"] = json!(format!("/v2/images?limit={limit}&marker={m}"));
            }
            ok(v)
        }
        ["v2", "images", id] => images(a)
            .into_iter()
            .find(|i| i["id"].as_str() == Some(id))
            .and_then(ok),
        ["v2", "info", "import"] => ok(json!({"import-methods": {"description": "Import methods available.",
            "type": "array", "value": ["glance-direct", "web-download"]}})),
        _ => None,
    }
}

/// Images visible to the project, shaped like `GET /v2/images`.
fn images(a: &Args) -> Vec<Value> {
    (0..a.images)
        .map(|i| {
            json!({"id": uid(0x1a6, i), "name": format!("image-{i:03}"), "status": "active",
                   "visibility": if i % 3 == 0 { "public" } else { "private" }, "owner": PROJECT_ID,
                   "disk_format": "qcow2", "container_format": "bare", "size": 1_073_741_824_u64 + i as u64,
                   "min_disk": 10, "min_ram": 0, "protected": false, "os_hidden": false, "tags": [],
                   "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
                   "self": format!("/v2/images/{}", uid(0x1a6, i)), "file": format!("/v2/images/{}/file", uid(0x1a6, i)),
                   "schema": "/v2/schemas/image"})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_follow_the_marker_and_stop_at_the_end() {
        let items: Vec<Value> = (0..5).map(|i| json!({"id": format!("i{i}")})).collect();
        let q = |m: Option<&str>| {
            let mut q = HashMap::from([("limit".to_string(), "2".to_string())]);
            if let Some(m) = m {
                q.insert("marker".into(), m.into());
            }
            q
        };
        let (p, next) = page(items.clone(), &q(None));
        assert_eq!((p.len(), next.as_deref()), (2, Some("i1")));
        let (p, next) = page(items.clone(), &q(Some("i3")));
        assert_eq!((p.len(), next), (1, None), "the last page has no next marker");
    }

    #[test]
    fn keystone_times_are_utc_iso() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(iso(1_763_200_000), "2025-11-15T09:46:40.000000Z");
    }

    #[test]
    fn counters_group_ids() {
        assert_eq!(
            shape_of(&["compute", "v2.1", "servers", &uid(0x5e5, 3)]),
            "compute/v2.1/servers/{id}"
        );
    }
}
