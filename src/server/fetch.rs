//! Read-only data loading. Every function issues **GET requests only** and runs
//! independent calls in parallel with `tokio::join!`, then hands the raw JSON to
//! the pure mappers in [`super::mapping`].

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use openstack_sdk::api::{paged, Pagination, QueryAsync};
use openstack_sdk::AsyncOpenStack;
use serde_json::Value;

use super::mapping;
use super::session::Session;
use super::sfe;
use crate::model::*;

type R<T> = Result<T, AppError>;

/// Runs a paginated list endpoint and returns every item.
macro_rules! list_all {
    ($os:expr, $module:path, $what:literal) => {
        list_where!($os, $module, $what, |_b| ())
    };
}

/// Same as `list_all!`, letting the caller set query filters on the builder.
macro_rules! list_where {
    ($os:expr, $module:path, $what:literal, |$b:ident| $set:expr) => {{
        use $module as m;
        async {
            let mut builder = m::Request::builder();
            let $b = &mut builder;
            $set;
            let ep = builder.build().map_err(sfe(concat!("build ", $what)))?;
            let v: Vec<Value> = counted(paged(ep, Pagination::All).query_async($os)).await.map_err(sfe(concat!("GET ", $what)))?;
            Ok::<_, AppError>(v)
        }
    }};
}

macro_rules! get_one {
    ($os:expr, $module:path, $id_setter:ident, $id:expr, $what:literal) => {{
        use $module as m;
        async {
            let ep = m::Request::builder().$id_setter($id).build().map_err(sfe(concat!("build ", $what)))?;
            let v: Value = counted(ep.query_async($os)).await.map_err(sfe(concat!("GET ", $what)))?;
            Ok::<_, AppError>(v)
        }
    }};
}

/// Merges lists, dropping duplicate IDs (first occurrence wins).
fn merge_by_id(lists: Vec<Vec<Value>>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    lists
        .into_iter()
        .flatten()
        .filter(|v| v.get("id").and_then(Value::as_str).is_none_or(|id| seen.insert(id.to_string())))
        .collect()
}

// Call counting (roadmap 65): each page shows how many GETs it took. A counter
// per server-function call (`serving`, set up by `api::scoped`) is bumped by
// every request (`counted`) and read by `timed`, so the counts can't drift from
// the code. A task-local, because `tokio::join!` runs the calls on one task;
// `spawn_counted` carries it into spawned ones. It also holds the key of the
// session's cached lookups (roadmap 66).
#[derive(Default)]
struct Serving {
    calls: AtomicU32,
    /// `os::scoped_key` of the session; `None`: lookups aren't cached.
    scope: Option<String>,
}

tokio::task_local! {
    static SERVING: Arc<Serving>;
}

/// Runs `fut` (one server function's body) with its own call counter, which
/// `timed` reads, and `scope` (`os::scoped_key`) for the lookup cache. Outside
/// it requests aren't counted, `timed` reports 0 and lookups aren't cached.
pub async fn serving<F: Future>(scope: Option<String>, fut: F) -> F::Output {
    serving_counted(scope, fut).await.0
}

/// [`serving`], also returning how many requests `fut` made (for the debug
/// timing of `api::scoped`, roadmap 83).
pub async fn serving_counted<F: Future>(scope: Option<String>, fut: F) -> (F::Output, u32) {
    let serving = Arc::new(Serving { calls: AtomicU32::new(0), scope });
    let out = SERVING.scope(Arc::clone(&serving), fut).await;
    (out, serving.calls.load(Ordering::Relaxed))
}

/// Awaits one request, counting it. Every OpenStack call in this file goes
/// through here.
async fn counted<F: Future>(request: F) -> F::Output {
    let _ = SERVING.try_with(|c| c.calls.fetch_add(1, Ordering::Relaxed));
    request.await
}

/// Spawns `fut` onto `tasks`, counting its requests with the caller's: spawned
/// tasks don't inherit task-locals.
fn spawn_counted<T: Send + 'static>(tasks: &mut tokio::task::JoinSet<T>, fut: impl Future<Output = T> + Send + 'static) {
    let serving = SERVING.try_with(Arc::clone).unwrap_or_default();
    tasks.spawn(SERVING.scope(serving, fut));
}

/// Requests a per-item fan-out keeps in flight: Admin pages ask one thing per
/// provider, flavor or agent, and hundreds of simultaneous GETs (the *Resource
/// Provider* tab on ttcp-dev) got this machine refused by the cloud for minutes.
const FAN_OUT: usize = 6;

/// A [`tokio::task::JoinSet`] that runs at most `limit` tasks at once: `spawn`
/// waits for a slot. Results come back in completion order, as `join_all`'s.
struct Bounded<T> {
    tasks: tokio::task::JoinSet<T>,
    done: Vec<T>,
    limit: usize,
}

impl<T: Send + 'static> Bounded<T> {
    fn new(limit: usize) -> Self {
        Bounded { tasks: tokio::task::JoinSet::new(), done: Vec::new(), limit: limit.max(1) }
    }

    /// Starts `fut` once fewer than `limit` tasks run (counted like `spawn_counted`).
    async fn spawn(&mut self, fut: impl Future<Output = T> + Send + 'static) {
        while self.tasks.len() >= self.limit {
            match self.tasks.join_next().await {
                Some(Ok(r)) => self.done.push(r),
                Some(Err(e)) => std::panic::resume_unwind(e.into_panic()),
                None => break,
            }
        }
        spawn_counted(&mut self.tasks, fut);
    }

    async fn join_all(self) -> Vec<T> {
        let mut out = self.done;
        out.extend(self.tasks.join_all().await);
        out
    }
}

/// `data` with the time since `start` and the requests counted so far.
fn timed<T>(start: Instant, data: T) -> Timed<T> {
    let calls = SERVING.try_with(|c| c.calls.load(Ordering::Relaxed)).unwrap_or(0);
    Timed { data, millis: start.elapsed().as_millis() as u64, calls }
}

// Lookup cache (roadmap 66): the lists a page fetches only to name IDs change
// rarely compared with how often pages load and poll, so they are reused for
// `[cache] lookup_seconds`. Never a page's main list, never dialog choices
// (they must show current states), and emptied for the session after a write.

/// The cached lookups; `forget_lookups` drops these names.
const LOOKUPS: [&str; 4] = ["flavors", "images", "networks", "volumes"];

/// `fetch` (one of [`LOOKUPS`], by `name`) through `AppState::lookups`. Only
/// successes are kept, and only inside `serving` with a scope.
async fn lookup(name: &'static str, fetch: impl Future<Output = R<Vec<Value>>>) -> R<Arc<Vec<Value>>> {
    debug_assert!(LOOKUPS.contains(&name), "{name} is not in LOOKUPS, so writes wouldn't clear it");
    let st = super::config::try_state().filter(|st| !st.config.lookup_ttl.is_zero());
    let key = SERVING.try_with(|c| c.scope.clone()).ok().flatten().map(|scope| format!("{scope}/{name}"));
    match (st, key) {
        (Some(st), Some(key)) => cached(&st.lookups, key, fetch).await,
        _ => fetch.await.map(Arc::new),
    }
}

/// `fetch` through `cache` under `key`: a hit skips it; only successes are kept.
async fn cached(cache: &moka::future::Cache<String, Arc<Vec<Value>>>, key: String, fetch: impl Future<Output = R<Vec<Value>>>) -> R<Arc<Vec<Value>>> {
    if let Some(v) = cache.get(&key).await {
        return Ok(v);
    }
    let v = Arc::new(fetch.await?);
    cache.insert(key, v.clone()).await;
    Ok(v)
}

/// Drops the cached lookups of one session scope (`os::scoped_key`): after a
/// write, at logout.
pub async fn forget_lookups(scope: &str) {
    if let Some(st) = super::config::try_state() {
        for name in LOOKUPS {
            st.lookups.invalidate(&format!("{scope}/{name}")).await;
        }
    }
}

/// [`soft`] for a cached lookup.
fn soft_lookup(r: R<Arc<Vec<Value>>>) -> Arc<Vec<Value>> {
    r.unwrap_or_default()
}

/// Nova microversion requested for server reads: 2.47 embeds the full flavor
/// (incl. extra specs) and returns `locked` (>= 2.9). RHOSO supports up to 2.95.
const COMPUTE_MICROVERSION: &str = "compute 2.47";

fn compute_mv() -> (http::HeaderName, http::HeaderValue) {
    (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(COMPUTE_MICROVERSION))
}

async fn servers(os: &AsyncOpenStack) -> R<Vec<Value>> {
    let (k, v) = compute_mv();
    list_where!(os, openstack_sdk::api::compute::v2::server::list_detailed_21, "servers/detail", |b| b.header(k, v)).await
}
async fn servers_brief(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::compute::v2::server::list_21, "servers").await
}
async fn flavors(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::compute::v2::flavor::list_detailed_20, "flavors/detail").await
}
async fn images_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::image::v2::image::list, "images").await
}
/// Images the project may boot, the way Horizon's launch wizard asks Glance:
/// public, community, shared with it (accepted) and its own. A plain list would
/// also return other projects' private images to admins.
async fn launch_images_raw(os: &AsyncOpenStack, project_id: &str) -> R<Vec<Value>> {
    use openstack_sdk::api::image::v2::image::list as im;
    let (public, community, shared, own) = tokio::join!(
        list_where!(os, im, "images?visibility=public", |b| b.visibility("public")),
        list_where!(os, im, "images?visibility=community", |b| b.visibility("community")),
        list_where!(os, im, "images?visibility=shared", |b| b.visibility("shared").member_status("accepted")),
        list_where!(os, im, "images?owner", |b| b.owner(project_id))
    );
    Ok(merge_by_id(vec![public?, soft(community), soft(shared), own?]))
}
async fn volumes_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::block_storage::v3::volume::list_detailed, "volumes/detail").await
}
/// The networks a project sees, like Horizon: its own, shared ones and external
/// ones. Filters are explicit because Neutron lists every project's networks to
/// admins, which on a large cloud made each lookup download the whole cloud
/// (roadmap 64). Only the project's own list is required.
async fn visible_networks(os: &AsyncOpenStack, project_id: &str) -> R<Vec<Value>> {
    use openstack_sdk::api::network::v2::network::list as net;
    let (own, shared, external) = tokio::join!(
        list_where!(os, net, "networks?tenant_id", |b| b.tenant_id(project_id)),
        list_where!(os, net, "networks?shared", |b| b.shared(true)),
        list_where!(os, net, "networks?router:external", |b| b.router_external(true))
    );
    Ok(merge_by_id(vec![own?, soft(shared), soft(external)]))
}

/// [`visible_networks`] and their subnets, for pages that name both.
struct NetLookups {
    networks: Vec<Value>,
    subnets: Vec<Value>,
}

/// Visible networks and their subnets (roadmap 64): the project's subnets
/// (`tenant_id`) and shared ones (`shared=true`, which Neutron copies from the
/// network) with the networks. For admins a second round fetches, by
/// `network_id`, the subnets of each external network that is neither the
/// project's nor shared (usually the admin project's), so router gateways and
/// floating IP pools are named. Neutron hides those subnets from everyone else,
/// so for members the round would only cost calls: measured on ttcp-dev as
/// `power_user`, three calls returning nothing.
async fn net_lookups(os: &AsyncOpenStack, project_id: &str) -> R<NetLookups> {
    use openstack_sdk::api::network::v2::subnet::list as sub;
    let (ne, own, shared) = tokio::join!(
        visible_networks(os, project_id),
        list_where!(os, sub, "subnets?tenant_id", |b| b.tenant_id(project_id)),
        list_where!(os, sub, "subnets?shared", |b| b.shared(true))
    );
    let networks = ne?;
    let subnets = merge_by_id(vec![own?, soft(shared)]);
    let missing = if super::os::token_is_admin(os) { mapping::networks_without_subnets(&networks, &subnets) } else { Vec::new() };
    // One call per network whose subnets the lists missed: bounded, as an admin
    // can see many (`FAN_OUT`).
    let mut tasks = Bounded::new(FAN_OUT);
    for id in missing {
        let os = os.clone();
        tasks.spawn(async move { soft(list_where!(&os, sub, "subnets?network_id", |b| b.network_id(id.as_str())).await) }).await;
    }
    let subnets = merge_by_id(std::iter::once(subnets).chain(tasks.join_all().await).collect());
    Ok(NetLookups { networks, subnets })
}

/// A failing lookup list (e.g. images) must not break the main table.
fn soft(r: R<Vec<Value>>) -> Vec<Value> {
    r.unwrap_or_default()
}

/// Instances list: servers (2.47), flavors, images and volumes in parallel, the
/// last three only to resolve names (`soft`).
pub async fn instances(os: &AsyncOpenStack) -> R<Timed<Vec<Instance>>> {
    let t = Instant::now();
    let (sv, fl, im, vo) = tokio::join!(servers(os), lookup("flavors", flavors(os)), lookup("images", images_raw(os)), lookup("volumes", volumes_raw(os)));
    Ok(timed(t, mapping::instances(&sv?, &soft_lookup(fl), &soft_lookup(im), &soft_lookup(vo))))
}

/// Instance detail: the server plus its interfaces, actions, volumes and the
/// lookups for names, in parallel.
pub async fn instance(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<InstanceDetail>> {
    use openstack_sdk::api::compute::v2::server::{get_20 as get, instance_action, interface};
    let t = Instant::now();
    let server = async {
        let (k, v) = compute_mv();
        let ep = get::Request::builder().id(id).header(k, v).build().map_err(sfe("build server"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET server"))?;
        Ok::<_, AppError>(v)
    };
    let ifaces = async {
        let ep = interface::list_21::Request::builder()
            .server_id(id)
            .build()
            .map_err(sfe("build os-interface"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET os-interface"))?;
        Ok::<_, AppError>(v.get("interfaceAttachments").and_then(Value::as_array).cloned().unwrap_or_default())
    };
    let actions = async {
        let ep = instance_action::list_21::Request::builder()
            .server_id(id)
            .build()
            .map_err(sfe("build os-instance-actions"))?;
        let v: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET os-instance-actions"))?;
        Ok::<_, AppError>(v)
    };
    // Name, admin state, VNIC type, port security and groups of each interface,
    // which `os-interface` doesn't return (roadmap 44).
    let ports = list_where!(os, openstack_sdk::api::network::v2::port::list, "ports?device_id", |b| b.device_id(id));
    // The group holding the server, by membership: `server_groups` is only on
    // the server from compute 2.71 and server reads use 2.47 (roadmap 52).
    let groups = list_all!(os, openstack_sdk::api::compute::v2::server_group::list_20, "os-server-groups");
    let (sv, fl, im, vo, ne, it, ac, sg, po, gr) = tokio::join!(
        server,
        lookup("flavors", flavors(os)),
        lookup("images", images_raw(os)),
        lookup("volumes", volumes_raw(os)),
        lookup("networks", visible_networks(os, project_id)),
        ifaces,
        actions,
        security_groups_raw(os, project_id),
        ports,
        groups
    );
    let sv = sv?;
    let (fl, im, vo, ne, it, ac, sg, po, gr) =
        (soft_lookup(fl), soft_lookup(im), soft_lookup(vo), soft_lookup(ne), soft(it), soft(ac), soft(sg), soft(po), soft(gr));
    Ok(timed(t, mapping::instance_detail(mapping::DetailInputs {
            server: &sv,
            flavors: &fl,
            images: &im,
            volumes: &vo,
            networks: &ne,
            interfaces: &it,
            ports: &po,
            actions: &ac,
            security_groups: &sg,
            server_groups: &gr,
        }),
    ))
}

/// Project id → name. The user's own projects always resolve; Keystone's full
/// project list only succeeds for admins/readers and is used when allowed.
pub async fn project_names(os: &AsyncOpenStack, mine: &[ProjectRef]) -> std::collections::HashMap<String, String> {
    use openstack_sdk::api::identity::v3::project::list as m;
    let mut names: std::collections::HashMap<String, String> =
        mine.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let all = async {
        let ep = m::Request::builder().build().map_err(sfe("build projects"))?;
        let v: Vec<Value> = counted(paged(ep, Pagination::All).query_async(os)).await.map_err(sfe("GET projects"))?;
        Ok::<_, AppError>(v)
    };
    if let Ok(all) = all.await {
        for p in mapping::projects(&all) {
            names.entry(p.id).or_insert(p.name);
        }
    }
    names
}

/// Images visible to the project, with the owners' project names (`mine`: the
/// user's projects, known without an extra call).
pub async fn images(os: &AsyncOpenStack, project_id: &str, mine: &[ProjectRef]) -> R<Timed<Vec<Image>>> {
    let t = Instant::now();
    let (im, names) = tokio::join!(images_raw(os), project_names(os, mine));
    Ok(timed(t, mapping::images(&im?, project_id, &names)))
}

/// Nova microversion for key pairs: 2.2 adds `type`. Not higher: from 2.35
/// the list pages with `limit`/`marker`, which `paged` doesn't follow here.
const KEYPAIR_MICROVERSION: &str = "compute 2.2";

fn keypair_mv() -> (http::HeaderName, http::HeaderValue) {
    (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(KEYPAIR_MICROVERSION))
}

async fn keypairs_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    let (k, v) = keypair_mv();
    list_where!(os, openstack_sdk::api::compute::v2::keypair::list_20, "os-keypairs", |b| b.header(k, v)).await
}

/// Key pairs (compute 2.2, for `type`).
pub async fn keypairs(os: &AsyncOpenStack) -> R<Timed<Vec<KeyPair>>> {
    let t = Instant::now();
    let raw = keypairs_raw(os).await?;
    Ok(timed(t, mapping::keypairs(&raw)))
}

/// Server groups.
pub async fn server_groups(os: &AsyncOpenStack) -> R<Timed<Vec<ServerGroup>>> {
    let t = Instant::now();
    let raw = list_all!(os, openstack_sdk::api::compute::v2::server_group::list_20, "os-server-groups").await?;
    Ok(timed(t, mapping::server_groups(&raw)))
}

/// Volumes, with the names of the servers they are attached to.
pub async fn volumes(os: &AsyncOpenStack) -> R<Timed<Vec<Volume>>> {
    let t = Instant::now();
    let (vo, sv) = tokio::join!(volumes_raw(os), servers_brief(os));
    Ok(timed(t, mapping::volumes(&vo?, &soft(sv))))
}

/// Volume snapshots, with their volumes' names.
pub async fn snapshots(os: &AsyncOpenStack) -> R<Timed<Vec<Snapshot>>> {
    let t = Instant::now();
    let (sn, vo) = tokio::join!(
        list_all!(os, openstack_sdk::api::block_storage::v3::snapshot::list_detailed, "snapshots/detail"),
        lookup("volumes", volumes_raw(os))
    );
    Ok(timed(t, mapping::snapshots(&sn?, &soft_lookup(vo))))
}

// --- Volume backups (roadmap 5) -----------------------------------------------

/// Volumes › Backups, with their volumes' names (`soft`: a failing lookup only
/// shows IDs).
pub async fn backups(os: &AsyncOpenStack) -> R<Timed<Vec<BackupRow>>> {
    let t = Instant::now();
    let (bk, vo) = tokio::join!(
        list_all!(os, openstack_sdk::api::block_storage::v3::backup::list_detailed, "backups/detail"),
        lookup("volumes", volumes_raw(os))
    );
    Ok(timed(t, mapping::backups(&bk?, &soft_lookup(vo))))
}

/// One backup, with its volume's name.
pub async fn backup(os: &AsyncOpenStack, id: &str) -> R<Timed<BackupRow>> {
    let t = Instant::now();
    let (bk, vo) = tokio::join!(
        get_one!(os, openstack_sdk::api::block_storage::v3::backup::get, id, id, "backup"),
        lookup("volumes", volumes_raw(os))
    );
    Ok(timed(t, mapping::backups(&[bk?], &soft_lookup(vo)).remove(0)))
}

// --- Volume groups (roadmap 5) ------------------------------------------------

/// `OpenStack-API-Version: volume <v>`, overriding the SDK's negotiated one:
/// volumes return `group_id` from 3.13, snapshots `group_snapshot_id` from 3.14.
fn volume_mv(v: &'static str) -> (http::HeaderName, http::HeaderValue) {
    (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(v))
}

async fn volume_groups_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::block_storage::v3::group::list_detailed_313, "groups/detail").await
}
async fn group_snapshots_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::block_storage::v3::group_snapshot::list_detailed_314, "group_snapshots/detail").await
}
/// Group types, only for their names (`soft` at the callers).
async fn group_types_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::block_storage::v3::group_type::list_311, "group_types").await
}
/// Volume types, for names. Not `Pageable` (AGENTS.md §6).
async fn volume_types_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    use openstack_sdk::api::block_storage::v3::r#type::list as m;
    let ep = m::Request::builder().build().map_err(sfe("build volume types"))?;
    counted(ep.query_async(os)).await.map_err(sfe("GET volume types"))
}

/// Volumes › Groups: the groups, plus group types, volume types and group
/// snapshots for names and *Has Snapshots*.
pub async fn volume_groups(os: &AsyncOpenStack) -> R<Timed<Vec<VolumeGroupRow>>> {
    let t = Instant::now();
    let (g, gt, vt, gs) = tokio::join!(volume_groups_raw(os), group_types_raw(os), volume_types_raw(os), group_snapshots_raw(os));
    Ok(timed(t, mapping::volume_groups(&g?, &soft(gt), &soft(vt), &soft(gs))))
}

/// One volume group with its volumes (volume 3.13 for `group_id`) and snapshots.
pub async fn volume_group(os: &AsyncOpenStack, id: &str) -> R<Timed<VolumeGroupDetail>> {
    let t = Instant::now();
    let (k, v) = volume_mv("volume 3.13");
    let (g, gt, vt, gs, vo) = tokio::join!(
        get_one!(os, openstack_sdk::api::block_storage::v3::group::get_313, id, id, "group"),
        group_types_raw(os),
        volume_types_raw(os),
        group_snapshots_raw(os),
        list_where!(os, openstack_sdk::api::block_storage::v3::volume::list_detailed, "volumes/detail", |b| b.header(k, v))
    );
    Ok(timed(t, mapping::volume_group_detail(&g?, &soft(gt), &soft(vt), &soft(gs), &soft(vo))))
}

/// Volumes › Group Snapshots, with their groups' names.
pub async fn group_snapshots(os: &AsyncOpenStack) -> R<Timed<Vec<GroupSnapshotRow>>> {
    let t = Instant::now();
    let (gs, g, gt) = tokio::join!(group_snapshots_raw(os), volume_groups_raw(os), group_types_raw(os));
    Ok(timed(t, mapping::group_snapshots_rows(&gs?, &soft(g), &soft(gt))))
}

/// One group snapshot with its volume snapshots (volume 3.14 for `group_snapshot_id`).
pub async fn group_snapshot(os: &AsyncOpenStack, id: &str) -> R<Timed<GroupSnapshotDetail>> {
    let t = Instant::now();
    let (k, v) = volume_mv("volume 3.14");
    let (gs, g, gt, sn, vo) = tokio::join!(
        get_one!(os, openstack_sdk::api::block_storage::v3::group_snapshot::get_314, id, id, "group snapshot"),
        volume_groups_raw(os),
        group_types_raw(os),
        list_where!(os, openstack_sdk::api::block_storage::v3::snapshot::list_detailed, "snapshots/detail", |b| b.header(k, v)),
        lookup("volumes", volumes_raw(os))
    );
    Ok(timed(t, mapping::group_snapshot_detail(&gs?, &soft(g), &soft(gt), &soft(sn), &soft_lookup(vo))))
}

/// Like Horizon: the project's networks, plus shared and external ones.
/// Filters are explicit because Neutron returns every project's resources to admins.
pub async fn networks(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<NetworkRow>>> {
    let t = Instant::now();
    let n = net_lookups(os, project_id).await?;
    Ok(timed(t, mapping::networks(&n.networks, &n.subnets, project_id)))
}

/// The project's routers (filtered by `tenant_id`: admins see every project's).
pub async fn routers(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<RouterRow>>> {
    let t = Instant::now();
    let (ro, ne) = tokio::join!(
        list_where!(os, openstack_sdk::api::network::v2::router::list, "routers", |b| b.tenant_id(project_id)),
        lookup("networks", visible_networks(os, project_id))
    );
    Ok(timed(t, mapping::routers(&ro?, &soft_lookup(ne))))
}

/// The project's security groups (filtered by `tenant_id`).
pub async fn security_groups(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<SecurityGroupRow>>> {
    let t = Instant::now();
    let raw = list_where!(os, openstack_sdk::api::network::v2::security_group::list, "security-groups", |b| b
        .tenant_id(project_id))
    .await?;
    Ok(timed(t, mapping::security_groups(&raw)))
}

/// The project's floating IPs, with network and server names.
pub async fn floating_ips(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<FloatingIpRow>>> {
    let t = Instant::now();
    let (fi, ne, sv) = tokio::join!(
        list_where!(os, openstack_sdk::api::network::v2::floatingip::list, "floatingips", |b| b.tenant_id(project_id)),
        lookup("networks", visible_networks(os, project_id)),
        servers_brief(os)
    );
    Ok(timed(t, mapping::floating_ips(&fi?, &soft_lookup(ne), &soft(sv))))
}

/// A floating IP and its port forwarding rules (only fetched when the cloud has
/// the extension: `forwarding` is false otherwise).
pub async fn floating_ip_detail(os: &AsyncOpenStack, project_id: &str, id: &str, forwarding: bool) -> R<Timed<FloatingIpDetail>> {
    use openstack_sdk::api::network::v2::{floatingip, port};
    let t = Instant::now();
    let rules = async {
        if !forwarding {
            return Ok(Vec::new());
        }
        list_where!(os, floatingip::port_forwarding::list, "port_forwardings", |b| b.floatingip_id(id)).await
    };
    let (f, ru, ne, sv, po) = tokio::join!(
        get_one!(os, floatingip::get, id, id, "floating IP"),
        rules,
        lookup("networks", visible_networks(os, project_id)),
        servers_brief(os),
        list_where!(os, port::list, "ports?tenant_id", |b| b.tenant_id(project_id))
    );
    Ok(timed(t, mapping::floating_ip_detail(&f?, &ru?, &soft_lookup(ne), &soft(sv), &soft(po))))
}

/// GET of an endpoint the SDK lacks (trunks), through `actions::JsonRequest`.
async fn get_json(os: &AsyncOpenStack, ep: super::actions::JsonRequest, what: &'static str) -> R<Value> {
    debug_assert_eq!(ep.method, http::Method::GET, "fetch.rs is GET-only");
    counted(ep.query_async(os)).await.map_err(sfe(what))
}

/// Trunks of the project (`project_id`: admins see every project's).
pub async fn trunks(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<TrunkRow>>> {
    use super::actions::JsonRequest;
    use openstack_sdk::api::network::v2::port::list as ports;
    use openstack_sdk::types::ServiceType;
    let t = Instant::now();
    let (tr, po) = tokio::join!(
        get_json(os, JsonRequest::get(ServiceType::Network, "trunks").param("project_id", project_id), "GET trunks"),
        list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id))
    );
    let tr = tr?;
    Ok(timed(t, mapping::trunks(tr.get("trunks").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), &soft(po))))
}

/// One trunk of the project, sub-ports resolved.
pub async fn trunk(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<TrunkRow>> {
    use super::actions::JsonRequest;
    use openstack_sdk::api::network::v2::port::list as ports;
    use openstack_sdk::types::ServiceType;
    let t = Instant::now();
    let (tr, po) = tokio::join!(
        get_json(os, JsonRequest::get(ServiceType::Network, format!("trunks/{id}")), "GET trunk"),
        list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id))
    );
    let tr = tr?;
    let one = tr.get("trunk").cloned().unwrap_or(tr);
    Ok(timed(t, mapping::trunks(std::slice::from_ref(&one), &soft(po)).remove(0)))
}

// --- Firewall (FWaaS v2, roadmap 93) --------------------------------------------
// Not in the SDK (`openstack-sdk-network` has no `fwaas` module): JSON GETs.

/// One FWaaS collection the project can use: its own (`project_id`: admins see
/// every project's) and the shared ones, each once. `kind` is `firewall_rules`,
/// `firewall_policies` or `firewall_groups` (the path under `fwaas/` and the key).
async fn fwaas_list(os: &AsyncOpenStack, project_id: &str, kind: &'static str) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    use openstack_sdk::types::ServiceType;
    let path = format!("fwaas/{kind}");
    let (own, shared) = tokio::join!(
        get_json(os, JsonRequest::get(ServiceType::Network, path.clone()).param("project_id", project_id), "GET firewall list"),
        get_json(os, JsonRequest::get(ServiceType::Network, path).param("shared", "true"), "GET shared firewall list")
    );
    let items = |v: Value| v.get(kind).and_then(Value::as_array).cloned().unwrap_or_default();
    // The shared half only adds names: `soft`, so it never breaks the page.
    Ok(merge_by_id(vec![items(own?), soft(shared.map(items))]))
}

/// One FWaaS object (`kind` as in [`fwaas_list`], `key` its singular).
async fn fwaas_one(os: &AsyncOpenStack, kind: &'static str, key: &'static str, id: &str) -> R<Value> {
    use super::actions::JsonRequest;
    use openstack_sdk::types::ServiceType;
    let v = get_json(os, JsonRequest::get(ServiceType::Network, format!("fwaas/{kind}/{id}")), "GET firewall object").await?;
    Ok(v.get(key).cloned().unwrap_or(v))
}

/// The project's router interface and instance ports, routers and servers: what
/// a firewall group's ports are resolved with.
async fn firewall_port_lookups(os: &AsyncOpenStack, project_id: &str) -> (R<Vec<Value>>, Vec<Value>, Vec<Value>) {
    use openstack_sdk::api::network::v2::{port::list as ports, router::list as routers};
    let (po, ro, sv) = tokio::join!(
        list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id)),
        list_where!(os, routers, "routers", |b| b.tenant_id(project_id)),
        servers_brief(os)
    );
    (po, soft(ro), soft(sv))
}

/// Project › Network › Firewall › *Firewall Rules*, with the policies using them.
pub async fn firewall_rules(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<FirewallRuleRow>>> {
    let t = Instant::now();
    let (ru, po) = tokio::join!(fwaas_list(os, project_id, "firewall_rules"), fwaas_list(os, project_id, "firewall_policies"));
    Ok(timed(t, mapping::firewall_rules(&ru?, &soft(po), project_id)))
}

/// *Firewall Policies*, their rules named.
pub async fn firewall_policies(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<FirewallPolicyRow>>> {
    let t = Instant::now();
    let (po, ru) = tokio::join!(fwaas_list(os, project_id, "firewall_policies"), fwaas_list(os, project_id, "firewall_rules"));
    Ok(timed(t, mapping::firewall_policies(&po?, &soft(ru), project_id)))
}

/// *Firewall Groups*, policies and ports named (devices only on the detail page).
pub async fn firewall_groups(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<FirewallGroupRow>>> {
    use openstack_sdk::api::network::v2::port::list as ports;
    let t = Instant::now();
    let (gr, po, pt) = tokio::join!(
        fwaas_list(os, project_id, "firewall_groups"),
        fwaas_list(os, project_id, "firewall_policies"),
        list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id))
    );
    Ok(timed(t, mapping::firewall_groups(&gr?, &soft(po), &soft(pt), &[], &[], project_id)))
}

/// One firewall rule.
pub async fn firewall_rule(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<FirewallRuleRow>> {
    let t = Instant::now();
    let (ru, po) = tokio::join!(fwaas_one(os, "firewall_rules", "firewall_rule", id), fwaas_list(os, project_id, "firewall_policies"));
    Ok(timed(t, mapping::firewall_rules(std::slice::from_ref(&ru?), &soft(po), project_id).remove(0)))
}

/// One firewall policy and its rules in full.
pub async fn firewall_policy(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<FirewallPolicyDetail>> {
    let t = Instant::now();
    let (p, ru, all) = tokio::join!(
        fwaas_one(os, "firewall_policies", "firewall_policy", id),
        fwaas_list(os, project_id, "firewall_rules"),
        fwaas_list(os, project_id, "firewall_policies")
    );
    Ok(timed(t, mapping::firewall_policy_detail(&p?, &soft(ru), &soft(all), project_id)))
}

/// One firewall group, its ports resolved to their router or instance.
pub async fn firewall_group(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<FirewallGroupRow>> {
    let t = Instant::now();
    let (g, po, (pt, ro, sv)) = tokio::join!(
        fwaas_one(os, "firewall_groups", "firewall_group", id),
        fwaas_list(os, project_id, "firewall_policies"),
        firewall_port_lookups(os, project_id)
    );
    Ok(timed(t, mapping::firewall_groups(std::slice::from_ref(&g?), &soft(po), &soft(pt), &ro, &sv, project_id).remove(0)))
}

// --- Orchestration (Heat, roadmap 62) ------------------------------------------

/// Stacks per page (`limit`); the next page starts after the last one's ID.
const HEAT_PAGE: usize = 100;
/// Stops a list that never ends.
const HEAT_MAX_PAGES: u32 = 50;
/// The newest events the *Events* tab shows.
const HEAT_EVENTS: &str = "500";

fn heat_get(path: impl Into<String>) -> super::actions::JsonRequest {
    super::actions::JsonRequest::orchestration(http::Method::GET, path, None)
}

/// Every stack of the project (top level; nested stacks are reached from
/// their parent's resources): `limit`/`marker` pages.
async fn stacks_raw(os: &AsyncOpenStack, filter: Option<(&'static str, &str)>) -> R<Vec<Value>> {
    let (mut out, mut marker, mut pages) = (Vec::new(), None::<String>, 0u32);
    loop {
        let mut ep = heat_get("stacks").param("limit", HEAT_PAGE.to_string());
        if let Some((k, v)) = filter {
            ep = ep.param(k, v);
        }
        if let Some(m) = &marker {
            ep = ep.param("marker", m.clone());
        }
        let page = get_json(os, ep, "GET stacks").await?;
        pages += 1;
        let items = page.get("stacks").and_then(Value::as_array).cloned().unwrap_or_default();
        let last = items.last().and_then(|x| x.get("id")).and_then(Value::as_str).map(str::to_string);
        let full = items.len() >= HEAT_PAGE;
        out.extend(items);
        match last {
            Some(l) if full && pages < HEAT_MAX_PAGES => marker = Some(l),
            _ => return Ok(out),
        }
    }
}

/// Orchestration › Stacks.
pub async fn stacks(os: &AsyncOpenStack) -> R<Timed<Vec<StackRow>>> {
    let t = Instant::now();
    let raw = stacks_raw(os, None).await?;
    Ok(timed(t, mapping::stacks(&raw)))
}

/// One stack with its resources, events and template. Heat's paths take the
/// stack's name and ID (`stacks/{name}/{id}`; `stacks/{id}` alone is a
/// redirect), so the name is looked up first with the list's `id` filter.
pub async fn stack(os: &AsyncOpenStack, id: &str) -> R<Timed<StackDetail>> {
    let t = Instant::now();
    let found = stacks_raw(os, Some(("id", id))).await?;
    let name = found
        .iter()
        .find(|x| x.get("id").and_then(Value::as_str) == Some(id))
        .and_then(|x| x.get("stack_name").and_then(Value::as_str))
        .ok_or_else(|| AppError::new(ErrorKind::NotFound, format!("stack {id} not found")))?
        .to_string();
    check_id(&name)?;
    let base = format!("stacks/{name}/{id}");
    let (stack, resources, events, template) = tokio::join!(
        get_json(os, heat_get(base.clone()), "GET stack"),
        get_json(os, heat_get(format!("{base}/resources")), "GET stack resources"),
        get_json(os, heat_get(format!("{base}/events")).param("sort_dir", "desc").param("sort_keys", "event_time").param("limit", HEAT_EVENTS), "GET stack events"),
        get_json(os, heat_get(format!("{base}/template")), "GET stack template"),
    );
    let list = |v: R<Value>, key: &str| v.map(|v| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default());
    // Resources and events are tabs: a failing one leaves the rest of the page.
    let resources = soft(list(resources, "resources"));
    let events = soft(list(events, "events"));
    Ok(timed(t, mapping::stack_detail(&stack?, &resources, &events, &soft_value(template))))
}

/// Orchestration › Resource Types: every type this Heat offers, with descriptions.
pub async fn resource_types(os: &AsyncOpenStack) -> R<Timed<Vec<ResourceTypeRow>>> {
    let t = Instant::now();
    let v = get_json(os, heat_get("resource_types").param("with_description", "true"), "GET resource types").await?;
    Ok(timed(t, mapping::resource_types(arr_of(&v, "resource_types"))))
}

/// One resource type's schema. Type names (`OS::Nova::Server`) go in the path as they are.
pub async fn resource_type(os: &AsyncOpenStack, name: &str) -> R<Timed<ResourceTypeDetail>> {
    let t = Instant::now();
    check_id(name)?;
    let v = get_json(os, heat_get(format!("resource_types/{name}")), "GET resource type").await?;
    Ok(timed(t, mapping::resource_type_detail(&v)))
}

/// Orchestration › Template Versions.
pub async fn template_versions(os: &AsyncOpenStack) -> R<Timed<Vec<TemplateVersion>>> {
    let t = Instant::now();
    let v = get_json(os, heat_get("template_versions"), "GET template versions").await?;
    Ok(timed(t, mapping::template_versions(arr_of(&v, "template_versions"))))
}

/// The functions one template version supports.
pub async fn template_functions(os: &AsyncOpenStack, version: &str) -> R<Timed<Vec<TemplateFunction>>> {
    let t = Instant::now();
    check_id(version)?;
    let v = get_json(os, heat_get(format!("template_versions/{version}/functions")), "GET template functions").await?;
    Ok(timed(t, mapping::template_functions(arr_of(&v, "template_functions"))))
}

// --- Alarming (Aodh, roadmap 79) -------------------------------------------------

/// Alarms per page (`limit`); the next page starts after the last one's ID.
/// Aodh caps unpaged lists at its `api.default_api_return_limit`, so always page.
const AODH_PAGE: usize = 100;
/// Stops a list that never ends.
const AODH_MAX_PAGES: u32 = 50;
/// The newest history entries the *History* tab shows.
const AODH_HISTORY: &str = "200";

fn aodh_get(path: impl std::fmt::Display) -> super::actions::JsonRequest {
    super::actions::JsonRequest::alarming(http::Method::GET, path, None)
}

/// Monitoring › Alarms: the project's alarms. Admins see every project's, so
/// the list is filtered by `project_id` (Aodh's `q.*` query; for other users
/// Aodh only accepts their own project there).
pub async fn alarms(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<AlarmRow>>> {
    let t = Instant::now();
    let (mut out, mut marker, mut pages) = (Vec::<Value>::new(), None::<String>, 0u32);
    loop {
        let mut ep = aodh_get("alarms")
            .param("q.field", "project_id")
            .param("q.op", "eq")
            .param("q.value", project_id)
            .param("limit", AODH_PAGE.to_string());
        if let Some(m) = &marker {
            ep = ep.param("marker", m.clone());
        }
        let page = get_json(os, ep, "GET alarms").await?;
        pages += 1;
        let items = page.as_array().cloned().unwrap_or_default();
        let last = items.last().and_then(|x| x.get("alarm_id")).and_then(Value::as_str).map(str::to_string);
        let full = items.len() >= AODH_PAGE;
        out.extend(items);
        match last {
            Some(l) if full && pages < AODH_MAX_PAGES => marker = Some(l),
            _ => break,
        }
    }
    Ok(timed(t, mapping::alarms(&out)))
}

/// One alarm with its newest history entries, in parallel. The history is a
/// tab: when it fails, the overview still shows.
pub async fn alarm(os: &AsyncOpenStack, id: &str) -> R<Timed<AlarmDetail>> {
    let t = Instant::now();
    let (alarm, history) = tokio::join!(
        get_json(os, aodh_get(format!("alarms/{id}")), "GET alarm"),
        get_json(os, aodh_get(format!("alarms/{id}/history")).param("limit", AODH_HISTORY), "GET alarm history"),
    );
    let history = soft(history.map(|v| v.as_array().cloned().unwrap_or_default()));
    Ok(timed(t, mapping::alarm_detail(&alarm?, &history)))
}

/// The array under `key`, or none.
fn arr_of<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

// --- Key Manager (Barbican, roadmap 55) ---------------------------------------

/// Barbican's largest page by default (`max_limit_paging`); fewer round trips
/// than its default of 10.
const KM_PAGE: u64 = 100;
/// Stops a list that never ends (a Barbican reporting a wrong `total`).
const KM_MAX_PAGES: u32 = 50;

/// Every item of a Barbican list (`secrets`, `containers`): `limit`/`offset`
/// pages until `total`.
async fn km_all(os: &AsyncOpenStack, key: &'static str, what: &'static str) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    let (mut out, mut offset, mut pages) = (Vec::new(), 0u64, 0u32);
    loop {
        let ep = JsonRequest::key_manager(http::Method::GET, key, None).param("limit", KM_PAGE.to_string()).param("offset", offset.to_string());
        let page = get_json(os, ep, what).await?;
        pages += 1;
        let items = page.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        let next = mapping::next_offset(&page, offset, items.len());
        out.extend(items);
        match next {
            Some(n) if pages < KM_MAX_PAGES => offset = n,
            _ => return Ok(out),
        }
    }
}

/// Key Manager › Secrets: the project's secrets (metadata only).
pub async fn secrets(os: &AsyncOpenStack) -> R<Timed<Vec<SecretRow>>> {
    let t = Instant::now();
    let raw = km_all(os, "secrets", "GET secrets").await?;
    Ok(timed(t, mapping::secrets(&raw)))
}

/// `(Barbican ID, use)` for every certificate ref of the project's
/// Octavia listeners and pools (`mapping::octavia_cert_refs`); empty when
/// `project` is `None` (the `load-balancer` plugin is off) or Octavia fails.
async fn octavia_cert_refs(os: &AsyncOpenStack, project: Option<&str>) -> Vec<(String, OctaviaUse)> {
    use openstack_sdk::api::load_balancer::v2 as lb;
    let Some(project_id) = project else { return vec![] };
    let (ls, po, lbs) = tokio::join!(
        list_where!(os, lb::listener::list, "listeners", |b| b.project_id(project_id)),
        list_where!(os, lb::pool::list, "pools", |b| b.project_id(project_id)),
        list_where!(os, lb::loadbalancer::list, "loadbalancers", |b| b.project_id(project_id)),
    );
    mapping::octavia_cert_refs(&soft(ls), &soft(po), &soft(lbs))
}

/// One secret: metadata, `read` ACL (users named, what uses it) and consumers
/// (the consumers API is newer than the rest: `None` where Barbican lacks it).
/// Never the payload. `lb_project` is the project whose Octavia listeners and
/// pools are checked for it (roadmap 75), `None` when the `load-balancer` plugin
/// is off.
pub async fn secret(os: &AsyncOpenStack, id: &str, payload_allowed: bool, lb_project: Option<&str>) -> R<Timed<SecretDetail>> {
    use super::actions::JsonRequest;
    let get = |path: String, what| get_json(os, JsonRequest::key_manager(http::Method::GET, path, None), what);
    let t = Instant::now();
    let (meta, acl, consumers, containers, octavia) = tokio::join!(
        get(format!("secrets/{id}"), "GET secret"),
        get(format!("secrets/{id}/acl"), "GET secret ACL"),
        get(format!("secrets/{id}/consumers"), "GET secret consumers"),
        // Containers holding it and used by a service (roadmap 75's warning).
        km_all(os, "containers", "GET containers"),
        octavia_cert_refs(os, lb_project),
    );
    let mut meta = meta?;
    // The single-secret GET may omit `secret_ref` on old releases; the ID is known.
    if meta.get("secret_ref").and_then(Value::as_str).is_none_or(str::is_empty) {
        meta["secret_ref"] = Value::String(id.to_string());
    }
    let secret = mapping::secrets(std::slice::from_ref(&meta)).remove(0);
    let consumers = consumers.ok().map(|c| mapping::secret_consumers(c.get("consumers").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])));
    let holders = mapping::containers(&soft(containers), &[]);
    let in_use = mapping::secret_in_use(id, consumers.as_deref().unwrap_or(&[]), &holders);
    let octavia = mapping::octavia_uses(&octavia, id, &holders);
    let acl = km_acl(os, &soft_value(acl), in_use, octavia).await;
    Ok(timed(t, SecretDetail { secret, acl, consumers, payload_allowed }))
}

/// A `GET …/acl` answer as [`KmAcl`], users named where Keystone allows;
/// `can_edit` is set by the server function, which knows the viewer.
async fn km_acl(os: &AsyncOpenStack, raw: &Value, in_use: Vec<String>, octavia: Vec<OctaviaUse>) -> KmAcl {
    let (users, project_access) = mapping::secret_acl(raw);
    let user_names = user_names(os, &users).await;
    KmAcl { users, project_access, user_names, can_edit: false, in_use, octavia }
}

/// At most this many ACL users are looked up by name; the rest show their ID.
const MAX_NAME_LOOKUPS: usize = 20;

/// `(id, name)` of the users Keystone lets the token read (`users/{id}`, in
/// parallel); a 403 (non-admins) or a deleted user just leaves the ID.
async fn user_names(os: &AsyncOpenStack, ids: &[String]) -> Vec<(String, String)> {
    let mut set = tokio::task::JoinSet::new();
    for id in ids.iter().take(MAX_NAME_LOOKUPS).cloned() {
        let os = os.clone();
        spawn_counted(&mut set, async move { identity_me(&os, &id).await.ok().and_then(|u| Some((id, u.get("name")?.as_str()?.to_string()))) });
    }
    let mut out: Vec<(String, String)> = set.join_all().await.into_iter().flatten().collect();
    out.sort_by_key(|(id, _)| ids.iter().position(|x| x == id));
    out
}

/// A secret's payload (`[key_manager] allow_payload`, fetched on click only):
/// asked in its own content type, as text; binary payloads come back base64.
/// Returns (payload, content type, whether it is base64).
pub async fn secret_payload(os: &AsyncOpenStack, id: &str) -> R<(String, String, bool)> {
    use super::actions::JsonRequest;
    use base64::Engine;
    use zeroize::Zeroize;
    let meta = get_json(os, JsonRequest::key_manager(http::Method::GET, format!("secrets/{id}"), None), "GET secret").await?;
    let content_type = mapping::secrets(std::slice::from_ref(&meta)).remove(0).content_type.ok_or_else(|| AppError::invalid("this secret has no payload"))?;
    let mut ep = JsonRequest::key_manager(http::Method::GET, format!("secrets/{id}/payload"), None);
    let mut headers = http::HeaderMap::new();
    headers.insert(http::header::ACCEPT, http::HeaderValue::from_str(&content_type).map_err(|_| AppError::invalid("unusable content type"))?);
    ep.headers = Some(headers);
    let rsp = counted(openstack_sdk::api::raw(ep).query_async(os)).await.map_err(sfe("GET secret payload"))?;
    // Our own copy, wiped once encoded; the HTTP stack's buffers are out of reach.
    let mut bytes = Vec::from(rsp.into_body());
    let out = match std::str::from_utf8(&bytes) {
        Ok(text) if content_type.starts_with("text/") => (text.to_string(), content_type, false),
        _ => (base64::engine::general_purpose::STANDARD.encode(&bytes), content_type, true),
    };
    bytes.zeroize();
    Ok(out)
}

/// Key Manager › Containers, their secrets named.
pub async fn containers(os: &AsyncOpenStack) -> R<Timed<Vec<ContainerRow>>> {
    let t = Instant::now();
    let (c, sec) = tokio::join!(km_all(os, "containers", "GET containers"), km_all(os, "secrets", "GET secrets"));
    let raw = c?;
    let secrets = sec.unwrap_or_default();
    Ok(timed(t, mapping::containers(&raw, &secrets)))
}

/// One container, its secrets named.
/// One container with its ACL; `lb_project` as in [`secret`].
pub async fn container(os: &AsyncOpenStack, id: &str, lb_project: Option<&str>) -> R<Timed<ContainerDetail>> {
    use super::actions::JsonRequest;
    let get = |path: String, what| get_json(os, JsonRequest::key_manager(http::Method::GET, path, None), what);
    let t = Instant::now();
    let (c, sec, acl, octavia) = tokio::join!(
        get(format!("containers/{id}"), "GET container"),
        km_all(os, "secrets", "GET secrets"),
        get(format!("containers/{id}/acl"), "GET container ACL"),
        octavia_cert_refs(os, lb_project),
    );
    let c = c?;
    let secrets = sec.unwrap_or_default();
    let container = mapping::containers(std::slice::from_ref(&c), &secrets).remove(0);
    let in_use = mapping::container_in_use(&container);
    let octavia = mapping::octavia_uses(&octavia, id, &[]);
    let acl = km_acl(os, &soft_value(acl), in_use, octavia).await;
    Ok(timed(t, ContainerDetail { container, acl }))
}

/// The project's QoS policies and the shared ones (admins would see every project's).
async fn qos_policies_raw(os: &AsyncOpenStack, project_id: &str) -> R<Vec<Value>> {
    use openstack_sdk::api::network::v2::qos::policy::list as m;
    let (own, shared) = tokio::join!(
        list_where!(os, m, "qos/policies", |b| b.tenant_id(project_id)),
        list_where!(os, m, "qos/policies?shared", |b| b.shared(true))
    );
    Ok(merge_by_id(vec![own?, soft(shared)]))
}

/// The project's QoS policies and the shared ones.
pub async fn qos_policies(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<QosPolicyRow>>> {
    let t = Instant::now();
    Ok(timed(t, mapping::qos_policies(&qos_policies_raw(os, project_id).await?, project_id)))
}

/// One QoS policy with its rules.
pub async fn qos_policy(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<QosPolicyRow>> {
    let t = Instant::now();
    let p = get_one!(os, openstack_sdk::api::network::v2::qos::policy::get, id, id, "QoS policy").await?;
    Ok(timed(t, mapping::qos_policies(&[p], project_id).remove(0)))
}

/// Octavia lists every project's load balancers to admins: filter by project.
pub async fn load_balancers(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<LoadBalancerRow>>> {
    let t = Instant::now();
    let raw = list_where!(os, openstack_sdk::api::load_balancer::v2::loadbalancer::list, "loadbalancers", |b| b
        .project_id(project_id))
    .await?;
    Ok(timed(t, mapping::load_balancers(&raw)))
}

/// A load balancer with its listeners, pools, members and monitors. Two rounds:
/// the load balancer and its lists, then the members of each pool and the VIP's
/// floating IP, all in parallel. Lookups only used for names are soft.
pub async fn load_balancer_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<LoadBalancerDetail>> {
    use openstack_sdk::api::load_balancer::v2 as lb;
    let t = Instant::now();
    let (l, ls, po, n) = tokio::join!(
        get_one!(os, lb::loadbalancer::get, id, id, "loadbalancer"),
        list_where!(os, lb::listener::list, "listeners", |b| b.load_balancer_id(id)),
        list_where!(os, lb::pool::list, "pools", |b| b.loadbalancer_id(id)),
        net_lookups(os, project_id)
    );
    let (l, po, ls) = (l?, soft(po), soft(ls));
    let n = n.unwrap_or(NetLookups { networks: vec![], subnets: vec![] });
    let mut tasks = tokio::task::JoinSet::new();
    for pool_id in po.iter().filter_map(|p| p.get("id").and_then(Value::as_str)).map(str::to_string) {
        let os = os.clone();
        spawn_counted(&mut tasks, async move {
            let v = list_where!(&os, lb::pool::member::list, "members", |b| b.pool_id(pool_id.as_str())).await;
            (pool_id, soft(v))
        });
    }
    // Each pool's monitor by its ID, not the session project's monitors: an admin
    // can open another project's load balancer (roadmap 54).
    let mut monitor_tasks = tokio::task::JoinSet::new();
    for monitor_id in po.iter().filter_map(|p| p.get("healthmonitor_id").and_then(Value::as_str)).map(str::to_string) {
        let os = os.clone();
        spawn_counted(&mut monitor_tasks, async move { get_one!(&os, lb::healthmonitor::get, id, monitor_id.as_str(), "healthmonitor").await.ok() });
    }
    // L7 policies by listener, then each one's rules (roadmap 54). Not by the
    // session's `project_id`: a policy belongs to its load balancer's project, and
    // an admin can open another project's load balancer (on ttcp-dev the session
    // was scoped to another project after a new sign-in, and the list came back empty).
    let mut l7_tasks = tokio::task::JoinSet::new();
    for listener_id in ls.iter().filter_map(|x| x.get("id").and_then(Value::as_str)).map(str::to_string) {
        let os = os.clone();
        spawn_counted(&mut l7_tasks, async move {
            let policies = soft(list_where!(&os, lb::l7policy::list, "l7policies?listener_id", |b| b.listener_id(listener_id.as_str())).await);
            let mut rule_tasks = tokio::task::JoinSet::new();
            for policy_id in policies.iter().filter_map(|p| p.get("id").and_then(Value::as_str)).map(str::to_string) {
                let os = os.clone();
                spawn_counted(&mut rule_tasks, async move {
                    let v = list_where!(&os, lb::l7policy::rule::list, "l7policies/{id}/rules", |b| b.l7policy_id(policy_id.as_str())).await;
                    (policy_id, soft(v))
                });
            }
            (policies, rule_tasks.join_all().await)
        });
    }
    let vip_port = l.get("vip_port_id").and_then(Value::as_str).unwrap_or_default().to_string();
    let fips = async {
        if vip_port.is_empty() {
            return Ok(Vec::new());
        }
        list_where!(os, openstack_sdk::api::network::v2::floatingip::list, "floatingips?port_id", |b| b.port_id(vip_port.as_str())).await
    };
    let (members, fi, l7, hm) = tokio::join!(tasks.join_all(), fips, l7_tasks.join_all(), monitor_tasks.join_all());
    let hm: Vec<Value> = hm.into_iter().flatten().collect();
    let (mut l7policies, mut l7rules) = (Vec::new(), Vec::new());
    for (policies, rules) in l7 {
        l7policies.extend(policies);
        l7rules.extend(rules);
    }
    let x = mapping::LbInputs {
        listeners: &ls,
        pools: &po,
        members: &members,
        monitors: &hm,
        networks: &n.networks,
        subnets: &n.subnets,
        floating_ips: &soft(fi),
        l7policies: &l7policies,
        l7rules: &l7rules,
    };
    Ok(timed(t, mapping::load_balancer_detail(&l, x)))
}

/// Everything the *Network Topology* panel draws, in one round of parallel GETs.
/// Networks as on the Networks panel (own, shared, external); Neutron lists are
/// filtered by project because admins would otherwise get every project's.
pub async fn topology(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Topology>> {
    use openstack_sdk::api::network::v2::{floatingip, port, router};
    let t = Instant::now();
    let (n, ro, po, sv, fi) = tokio::join!(
        net_lookups(os, project_id),
        list_where!(os, router::list, "routers", |b| b.tenant_id(project_id)),
        list_where!(os, port::list, "ports?tenant_id", |b| b.tenant_id(project_id)),
        servers(os),
        list_where!(os, floatingip::list, "floatingips", |b| b.tenant_id(project_id))
    );
    let (n, ro, po, sv) = (n?, ro?, po?, sv?);
    Ok(timed(t, mapping::topology(mapping::TopologyInputs {
            project_id,
            networks: &n.networks,
            subnets: &n.subnets,
            routers: &ro,
            ports: &po,
            servers: &sv,
            floating_ips: &soft(fi),
        }),
    ))
}

/// Usage per server for `[start, end)` (`YYYY-MM-DDTHH:MM:SS`), GET only.
/// Paged by instance ID (compute >= 2.40); at most 10 pages of 1000.
pub async fn usage(os: &AsyncOpenStack, project_id: &str, start: &str, end: &str) -> R<Timed<Vec<ServerUsage>>> {
    use openstack_sdk::api::compute::v2::simple_tenant_usage::get_21 as m;
    const PAGE: u32 = 1000;
    let t = Instant::now();
    let (mut out, mut marker, mut calls) = (Vec::new(), None::<String>, 0u32);
    loop {
        let (k, v) = compute_mv();
        let mut b = m::Request::builder();
        b.id(project_id).start(start).end(end).limit(PAGE).header(k, v);
        if let Some(mk) = &marker {
            b.marker(mk.as_str());
        }
        let ep = b.build().map_err(sfe("build usage"))?;
        let raw: Value = counted(ep.query_async(os)).await.map_err(sfe("GET usage"))?;
        calls += 1;
        let page = mapping::server_usages(&raw);
        let full = page.len() == PAGE as usize;
        marker = page.last().map(|u| u.id.clone());
        out.extend(page);
        if !full || calls >= 10 {
            break;
        }
    }
    Ok(timed(t, out))
}

/// Admin › Overview (roadmap 85): every project's usage for `[start, end)`,
/// detailed per server as Horizon asks (`detailed=1`), named from Keystone.
/// Paged like [`usage`]: `limit` counts server usages across projects and the
/// marker is the last instance, so a project can span two pages (merged by
/// `mapping::tenant_usages`).
pub async fn admin_usage(os: &AsyncOpenStack, start: &str, end: &str) -> R<Timed<Vec<ProjectUsage>>> {
    use openstack_sdk::api::compute::v2::simple_tenant_usage::list_21 as m;
    const PAGE: u32 = 1000;
    let t = Instant::now();
    let pages = async {
        let (mut out, mut marker, mut calls) = (Vec::<Value>::new(), None::<String>, 0u32);
        loop {
            let (k, v) = compute_mv();
            let mut b = m::Request::builder();
            b.detailed("1").start(start).end(end).limit(PAGE).header(k, v);
            if let Some(mk) = &marker {
                b.marker(mk.as_str());
            }
            let ep = b.build().map_err(sfe("build usage list"))?;
            let page: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET usage list"))?;
            calls += 1;
            let last = page.iter().flat_map(mapping::server_usages).collect::<Vec<_>>();
            let full = last.len() == PAGE as usize;
            marker = last.last().map(|u| u.id.clone());
            out.extend(page);
            if !full || calls >= 10 {
                return Ok::<_, AppError>(out);
            }
        }
    };
    let (raw, names) = tokio::join!(pages, project_names(os, &[]));
    Ok(timed(t, mapping::tenant_usages(&raw?, &names)))
}

/// Overview quotas per service; a failing service keeps the others
/// (`QuotaSection::error`).
pub async fn overview(os: &AsyncOpenStack, project_id: &str) -> R<Timed<Vec<QuotaSection>>> {
    let t = Instant::now();
    let nova = async {
        use openstack_sdk::api::compute::v2::limit::list_21 as m;
        let ep = m::Request::builder().build().map_err(sfe("build compute limits"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET compute limits"))?;
        Ok::<_, AppError>(mapping::compute_quotas(&v))
    };
    let cinder = async {
        use openstack_sdk::api::block_storage::v3::limit::list as m;
        let ep = m::Request::builder().build().map_err(sfe("build volume limits"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET volume limits"))?;
        Ok::<_, AppError>(mapping::volume_quotas(&v))
    };
    let neutron = async {
        use openstack_sdk::api::network::v2::quota::details as m;
        let ep = m::Request::builder().id(project_id).build().map_err(sfe("build network quota"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET network quota"))?;
        Ok::<_, AppError>(mapping::network_quotas(&v))
    };
    let (n, c, q) = tokio::join!(nova, cinder, neutron);
    let section = |group, r: R<Vec<Quota>>| match r {
        Ok(quotas) => QuotaSection { group, quotas, error: None },
        Err(e) => QuotaSection { group, quotas: vec![], error: Some(e.to_string()) },
    };
    Ok(timed(t, vec![
            section(QuotaGroup::Compute, n),
            section(QuotaGroup::Volume, c),
            section(QuotaGroup::Network, q),
        ],
    ))
}

/// Project › Quotas (roadmap 82): each service's whole quota set, in parallel;
/// a failing service keeps the others (`QuotaCard::error`). Octavia only with
/// `load_balancer`, Barbican only with `key_manager` (each: its plugin enabled
/// and the service in the catalog).
/// `session_project` is the token's: Barbican reads usage only for it, and
/// another project's limits another way (`mapping::key_manager_project_rows`).
pub async fn quotas(os: &AsyncOpenStack, project_id: &str, session_project: &str, load_balancer: bool, key_manager: bool) -> R<Timed<Vec<QuotaCard>>> {
    use super::actions::JsonRequest;
    let t = Instant::now();
    let nova = async {
        use openstack_sdk::api::compute::v2::quota_set::details_20 as m;
        let ep = m::Request::builder().id(project_id).build().map_err(sfe("build compute quotas"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET compute quotas"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Compute, &v, &Value::Null))
    };
    let cinder = async {
        use openstack_sdk::api::block_storage::v3::quota_set::get as m;
        let ep = m::Request::builder().project_id(project_id).usage(true).build().map_err(sfe("build volume quotas"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET volume quotas"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Volume, &v, &Value::Null))
    };
    let neutron = async {
        use openstack_sdk::api::network::v2::quota::details as m;
        let ep = m::Request::builder().id(project_id).build().map_err(sfe("build network quota"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET network quota"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Network, &v, &Value::Null))
    };
    let octavia = async {
        use openstack_sdk::api::load_balancer::v2::quota::get as m;
        let ep = m::Request::builder().project_id(project_id).build().map_err(sfe("build load balancer quotas"))?;
        // Octavia reports `null` for a quota left at the default: read those too.
        let defaults = get_json(os, JsonRequest::get(openstack_sdk::types::ServiceType::LoadBalancer, "lbaas/quotas/defaults"), "GET load balancer default quotas");
        let (v, d) = tokio::join!(counted(ep.query_async(os)), defaults);
        let v: Value = v.map_err(sfe("GET load balancer quotas"))?;
        let d = soft_value(d);
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::LoadBalancer, &v, d.get("quota").unwrap_or(&d)))
    };
    let lb = async {
        if load_balancer {
            Some(octavia.await)
        } else {
            None
        }
    };
    let barbican_other = async {
        // 404 = no values set for that project (Barbican's answer, not an error).
        let opt = |r: R<Value>| match r {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        };
        let pq = |p: &str| get_json(os, JsonRequest::key_manager(http::Method::GET, format!("project-quotas/{p}"), None), "GET key manager project quotas");
        let (t, m, q) = tokio::join!(pq(project_id), pq(session_project), get_json(os, JsonRequest::key_manager(http::Method::GET, "quotas", None), "GET key manager quotas"));
        let (t, m) = (opt(t)?, opt(m).ok());
        Ok::<_, AppError>(mapping::key_manager_project_rows(t.as_ref(), m.as_ref().map(Option::as_ref), &soft_value(q)))
    };
    let barbican = async {
        if project_id != session_project {
            return barbican_other.await;
        }
        // `v1/quotas` is the token's project: the session's, as `project_id`.
        let km = |path: &'static str, what| get_json(os, JsonRequest::key_manager(http::Method::GET, path, None).param("limit", "1"), what);
        let quotas = get_json(os, JsonRequest::key_manager(http::Method::GET, "quotas", None), "GET key manager quotas");
        let (q, se, co, or) =
            tokio::join!(quotas, km("secrets", "GET secrets total"), km("containers", "GET containers total"), km("orders", "GET orders total"));
        let (se, co, or) = (soft_value(se), soft_value(co), soft_value(or));
        Ok::<_, AppError>(mapping::key_manager_quota_rows(&q?, &[("secrets", &se), ("containers", &co), ("orders", &or)]))
    };
    let km = async {
        if key_manager {
            Some(barbican.await)
        } else {
            None
        }
    };
    let (n, c, q, l, k) = tokio::join!(nova, cinder, neutron, lb, km);
    let card = |service, r: R<Vec<QuotaRow>>| match r {
        Ok(rows) => QuotaCard { service, project_id: project_id.to_string(), rows, error: None },
        Err(e) => QuotaCard { service, project_id: project_id.to_string(), rows: vec![], error: Some(e.to_string()) },
    };
    let mut out = vec![card(QuotaService::Compute, n), card(QuotaService::Volume, c), card(QuotaService::Network, q)];
    out.extend(l.map(|r| card(QuotaService::LoadBalancer, r)));
    out.extend(k.map(|r| card(QuotaService::KeyManager, r)));
    Ok(timed(t, out))
}

// ---------------------------------------------------------------------------
// Detail views (GET only)
// ---------------------------------------------------------------------------

async fn security_groups_raw(os: &AsyncOpenStack, project_id: &str) -> R<Vec<Value>> {
    list_where!(os, openstack_sdk::api::network::v2::security_group::list, "security-groups", |b| b
        .tenant_id(project_id))
    .await
}
async fn snapshots_raw(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::block_storage::v3::snapshot::list_detailed, "snapshots/detail").await
}

/// Network detail: the network, its subnets and ports.
pub async fn network_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<NetworkDetail>> {
    use openstack_sdk::api::network::v2::port::list as ports;
    let t = Instant::now();
    let (net, sn, po, sv, sg) = tokio::join!(
        get_one!(os, openstack_sdk::api::network::v2::network::get, id, id, "network"),
        list_where!(os, openstack_sdk::api::network::v2::subnet::list, "subnets?network_id", |b| b.network_id(id)),
        list_where!(os, ports, "ports?network_id", |b| b.network_id(id)),
        servers_brief(os),
        security_groups_raw(os, project_id)
    );
    let (net, sn) = (net?, soft(sn));
    let lookups = mapping::PortLookups::new(std::slice::from_ref(&net), &sn, &soft(sv), &soft(sg));
    let ports = lookups.ports(&soft(po));
    Ok(timed(t, mapping::network_detail(&net, &sn, ports, project_id)))
}

/// One subnet, with its network's name.
pub async fn subnet_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<SubnetRow>> {
    let t = Instant::now();
    let (sn, ne) = tokio::join!(
        get_one!(os, openstack_sdk::api::network::v2::subnet::get, id, id, "subnet"),
        lookup("networks", visible_networks(os, project_id))
    );
    Ok(timed(t, mapping::subnet(&sn?, &mapping::network_names(&soft_lookup(ne)))))
}

/// One port, with network, subnet, server and security group names.
pub async fn port_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<PortRow>> {
    let t = Instant::now();
    let (p, n, sv, sg) = tokio::join!(
        get_one!(os, openstack_sdk::api::network::v2::port::get, id, id, "port"),
        net_lookups(os, project_id),
        servers_brief(os),
        security_groups_raw(os, project_id)
    );
    let n = n.unwrap_or(NetLookups { networks: vec![], subnets: vec![] });
    let l = mapping::PortLookups::new(&n.networks, &n.subnets, &soft(sv), &soft(sg));
    Ok(timed(t, l.port(&p?)))
}

/// Router detail: the router, its interfaces and its gateway network's name.
pub async fn router_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<RouterDetail>> {
    use openstack_sdk::api::network::v2::port::list as ports;
    let t = Instant::now();
    let (r, po, n) = tokio::join!(
        get_one!(os, openstack_sdk::api::network::v2::router::get, id, id, "router"),
        list_where!(os, ports, "ports?device_id", |b| b.device_id(id)),
        net_lookups(os, project_id)
    );
    let n = n.unwrap_or(NetLookups { networks: vec![], subnets: vec![] });
    let l = mapping::PortLookups::new(&n.networks, &n.subnets, &[], &[]);
    Ok(timed(t, mapping::router_detail(&r?, &n.networks, l.ports(&soft(po)))))
}

/// Security group detail: the group and its rules, remote groups named.
pub async fn security_group_detail(os: &AsyncOpenStack, project_id: &str, id: &str) -> R<Timed<SecurityGroupDetail>> {
    let t = Instant::now();
    let (g, all) = tokio::join!(
        get_one!(os, openstack_sdk::api::network::v2::security_group::get, id, id, "security group"),
        security_groups_raw(os, project_id)
    );
    Ok(timed(t, mapping::security_group_detail(&g?, &soft(all))))
}

/// Volume detail: the volume, its snapshots, attached servers' names and Cinder's
/// messages (volume API ≥ 3.3, else empty).
pub async fn volume_detail(os: &AsyncOpenStack, id: &str) -> R<Timed<VolumeDetail>> {
    let t = Instant::now();
    let (v, sv, sn, msgs) = tokio::join!(
        get_one!(os, openstack_sdk::api::block_storage::v3::volume::get, id, id, "volume"),
        servers_brief(os),
        snapshots_raw(os),
        // Volume API >= 3.3 (the SDK negotiates it); older clouds just show none.
        async {
            use openstack_sdk::api::block_storage::v3::message::list_33 as m;
            let ep = m::Request::builder().build().map_err(sfe("build messages"))?;
            let v: Vec<Value> = counted(paged(ep, Pagination::All).query_async(os)).await.map_err(sfe("GET messages"))?;
            Ok::<_, AppError>(v)
        }
    );
    let mut d = mapping::volume_detail(&v?, &soft(sv), &soft(sn));
    // Optional data: a failure (e.g. volume API < 3.3) leaves the list empty, but is logged.
    let msgs = msgs.unwrap_or_else(|e| {
        tracing::warn!("volume messages unavailable: {e}");
        Vec::new()
    });
    d.messages = mapping::volume_messages(&msgs, id);
    Ok(timed(t, d))
}

/// One volume snapshot.
pub async fn snapshot_detail(os: &AsyncOpenStack, id: &str) -> R<Timed<Snapshot>> {
    let t = Instant::now();
    let (s, vo) = tokio::join!(
        get_one!(os, openstack_sdk::api::block_storage::v3::snapshot::get, id, id, "snapshot"),
        lookup("volumes", volumes_raw(os))
    );
    Ok(timed(t, mapping::snapshots(&[s?], &soft_lookup(vo)).remove(0)))
}

/// Image detail, with the owner's project name.
pub async fn image_detail(os: &AsyncOpenStack, project_id: &str, mine: &[ProjectRef], id: &str) -> R<Timed<ImageDetail>> {
    let t = Instant::now();
    let (im, names) = tokio::join!(
        get_one!(os, openstack_sdk::api::image::v2::image::get, id, id, "image"),
        project_names(os, mine)
    );
    Ok(timed(t, mapping::image_detail(&im?, project_id, &names)))
}

/// One key pair (compute 2.2, for `type`).
pub async fn keypair_detail(os: &AsyncOpenStack, name: &str) -> R<Timed<KeyPairDetail>> {
    let t = Instant::now();
    use openstack_sdk::api::compute::v2::keypair::get_20 as m;
    let (h, v) = keypair_mv();
    let ep = m::Request::builder().id(name).header(h, v).build().map_err(sfe("build keypair"))?;
    let k: Value = counted(ep.query_async(os)).await.map_err(sfe("GET keypair"))?;
    Ok(timed(t, mapping::keypair_detail(&k)))
}

// ---------------------------------------------------------------------------
// Identity (roadmap 57). Like Horizon, admins see the domain's projects and
// users, others their own; if Keystone refuses the admin list (e.g. a cloud
// that enforces system scope), the user's own is shown instead.
// ---------------------------------------------------------------------------

/// `GET` of a non-pageable list (`response_key` unwraps it).
macro_rules! get_list {
    ($os:expr, $module:path, $what:literal) => {{
        use $module as m;
        async {
            let ep = m::Request::builder().build().map_err(sfe(concat!("build ", $what)))?;
            let v: Vec<Value> = counted(ep.query_async($os)).await.map_err(sfe(concat!("GET ", $what)))?;
            Ok::<_, AppError>(v)
        }
    }};
    ($os:expr, $module:path, $what:literal, |$b:ident| $set:expr) => {{
        use $module as m;
        async {
            let mut $b = m::Request::builder();
            $set;
            let ep = $b.build().map_err(sfe(concat!("build ", $what)))?;
            let v: Vec<Value> = counted(ep.query_async($os)).await.map_err(sfe(concat!("GET ", $what)))?;
            Ok::<_, AppError>(v)
        }
    }};
}

/// The signed-in user (`users/{id}`): their domain, and the *Users* row of a non-admin.
async fn identity_me(os: &AsyncOpenStack, user_id: &str) -> R<Value> {
    use openstack_sdk::api::identity::v3::user::get as m;
    let ep = m::Request::builder().id(user_id).build().map_err(sfe("build user"))?;
    let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET user"))?;
    Ok(unwrapped(v, "user"))
}

/// `{"user": {...}}` → the inner object (some endpoints wrap it).
fn unwrapped(v: Value, key: &str) -> Value {
    match v.get(key) {
        Some(inner) => inner.clone(),
        None => v,
    }
}

/// The user's domain ID and the domain names known: theirs (from the session)
/// plus Keystone's domain list when the token may read it (admins).
async fn identity_domains(os: &AsyncOpenStack, me: &Value, own_name: &str) -> (Option<String>, HashMap<String, String>) {
    let domain_id = me.get("domain_id").and_then(Value::as_str).map(str::to_string);
    let all = soft(list_all!(os, openstack_sdk::api::identity::v3::domain::list, "domains").await);
    let names = mapping::domain_names(&all, domain_id.clone().map(|d| (d, own_name.to_string())));
    (domain_id, names)
}

/// The domain new projects, users and groups go to: the session's domain
/// context, else the signed-in user's domain (`users/{me}`), like Horizon's
/// `get_domain_id_for_operation`.
pub async fn operation_domain(os: &AsyncOpenStack, s: &Session) -> R<String> {
    Ok(operation_domain_ref(os, s).await?.0)
}

/// The same with its name: the context's, else the user's domain (the session
/// keeps the name the user signed in with).
pub async fn operation_domain_ref(os: &AsyncOpenStack, s: &Session) -> R<(String, String)> {
    if let Some(d) = &s.domain_context {
        return Ok(d.clone());
    }
    let me = identity_me(os, &s.user_id).await?;
    let id = me.get("domain_id").and_then(Value::as_str).map(str::to_string).ok_or_else(|| AppError::internal("the user has no domain_id"))?;
    Ok((id, s.domain.clone()))
}

/// A project's name from Keystone (`None` if it can't be read: show the ID).
pub async fn project_name(os: &AsyncOpenStack, project_id: &str) -> Option<String> {
    let ep = openstack_sdk::api::identity::v3::project::get::Request::builder().id(project_id).build().ok()?;
    let v: Value = counted(ep.query_async(os)).await.ok()?;
    unwrapped(v, "project").get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string)
}

/// The domain of `project_id` (`None` if it can't be read: lists stay unfiltered).
async fn project_domain(os: &AsyncOpenStack, project_id: &str) -> Option<String> {
    let ep = openstack_sdk::api::identity::v3::project::get::Request::builder().id(project_id).build().ok()?;
    let v: Value = counted(ep.query_async(os)).await.ok()?;
    unwrapped(v, "project").get("domain_id").and_then(Value::as_str).map(str::to_string)
}

/// The domain the identity lists are scoped to: the session's domain context
/// (*Set Domain Context*, multi-domain clouds) when set, else the user's.
fn list_domain(s: &Session, own: Option<String>) -> Option<String> {
    s.domain_context.as_ref().map(|(id, _)| id.clone()).or(own)
}

/// Identity › Domains (multi-domain clouds; admins).
pub async fn identity_domain_list(os: &AsyncOpenStack, s: &Session) -> R<Timed<Vec<IdentityDomain>>> {
    let t = Instant::now();
    let raw = list_all!(os, openstack_sdk::api::identity::v3::domain::list, "domains").await?;
    let context = s.domain_context.as_ref().map(|(id, _)| id.as_str());
    Ok(timed(t, mapping::identity_domains(&raw, context)))
}

/// One domain's `(id, name)`, to set it as the domain context.
pub async fn identity_domain_ref(os: &AsyncOpenStack, id: &str) -> R<(String, String)> {
    use openstack_sdk::api::identity::v3::domain::get as m;
    let ep = m::Request::builder().id(id).build().map_err(sfe("build domain"))?;
    let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET domain"))?;
    let v = unwrapped(v, "domain");
    let name = v.get("name").and_then(Value::as_str).unwrap_or(id).to_string();
    Ok((id.to_string(), name))
}

fn forbidden(e: &AppError) -> bool {
    e.kind == ErrorKind::Forbidden
}

// --- Admin: every project's resources and cloud-wide objects (roadmap 85) ------
// Only for admins (`api::require_admin`). Lists take `all_tenants`, or no
// project filter for Neutron and Glance, the opposite of the Project panels.

/// Every project's servers (compute 2.47, as the Instances panel).
async fn all_servers(os: &AsyncOpenStack) -> R<Vec<Value>> {
    let (k, v) = compute_mv();
    list_where!(os, openstack_sdk::api::compute::v2::server::list_detailed_21, "servers/detail?all_tenants", |b| b.header(k, v).all_tenants("true")).await
}
async fn all_servers_brief(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_where!(os, openstack_sdk::api::compute::v2::server::list_21, "servers?all_tenants", |b| b.all_tenants("true")).await
}
async fn all_volumes(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_where!(os, openstack_sdk::api::block_storage::v3::volume::list_detailed, "volumes/detail?all_tenants", |b| b.all_tenants(true)).await
}
async fn all_networks(os: &AsyncOpenStack) -> R<Vec<Value>> {
    list_all!(os, openstack_sdk::api::network::v2::network::list, "networks (all)").await
}

/// Compute 2.53 for hypervisors: UUID IDs, and still the usage fields (gone
/// from 2.88), which Horizon's table shows.
const HYPERVISOR_MICROVERSION: &str = "compute 2.53";
/// Compute 2.61 for flavors: `extra_specs` in the list (Horizon's *Metadata*).
const FLAVOR_MICROVERSION: &str = "compute 2.61";
/// Volume 3.58: groups and group snapshots carry `project_id`.
const GROUP_PROJECT_MICROVERSION: &str = "volume 3.58";

/// Admin › Compute › Hypervisors: hypervisors and the `nova-compute` services.
/// `placement`: the catalog has it, so the page offers the *Resource Provider*
/// tab, which loads on its own ([`admin_resource_providers`]).
pub async fn admin_hypervisors(os: &AsyncOpenStack, placement: bool) -> R<Timed<Hypervisors>> {
    let t = Instant::now();
    let h = (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(HYPERVISOR_MICROVERSION));
    let (hv, sv) = tokio::join!(
        list_where!(os, openstack_sdk::api::compute::v2::hypervisor::list_detailed_21, "os-hypervisors/detail", |b| b.header(h.0.clone(), h.1.clone())),
        // 2.53 as well: service UUIDs, the target of *Enable* / *Disable Service*.
        get_list!(os, openstack_sdk::api::compute::v2::service::list_20, "compute services", |b| b.binary("nova-compute").header(h.0.clone(), h.1.clone()))
    );
    Ok(timed(t, Hypervisors { hypervisors: mapping::hypervisors(&hv?), hosts: sv.map(|v| mapping::host_services(&v)).into(), placement }))
}

/// A Placement GET at microversion 1.14: without the header Placement answers
/// as 1.0, which has no `…/traits` (1.6; every call failed on ttcp-dev) and no
/// `parent_provider_uuid` / `root_provider_uuid` in the list (1.14).
fn placement_get(path: impl Into<String>) -> super::actions::JsonRequest {
    let mut r = super::actions::JsonRequest::get(openstack_sdk::types::ServiceType::Placement, path);
    let mut h = http::HeaderMap::new();
    h.insert(http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static("placement 1.14"));
    r.headers = Some(h);
    r
}

/// Admin › Hypervisors' *Resource Provider* tab, when opened: Placement's
/// providers with their inventories and usages. Raw GETs (the SDK's `placement`
/// feature is off), at the endpoint's root, where Placement serves its API; two
/// per provider, plus the traits that may name a GPU's vGPU type (each host's
/// first GPU, its other GPUs only when that one has custom traits), [`FAN_OUT`] at
/// a time (all at once got ttcp-dev to refuse this machine for minutes). A
/// provider whose details fail shows `-` for them. Ordered as a tree
/// ([`provider_tree`]): GPUs under their compute node.
pub async fn admin_resource_providers(os: &AsyncOpenStack) -> R<Timed<Vec<ResourceProviderRow>>> {
    let t = Instant::now();
    let list = get_json(os, placement_get("resource_providers"), "GET resource_providers").await?;
    let providers = list.get("resource_providers").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut tasks = Bounded::new(FAN_OUT);
    for (i, p) in providers.iter().enumerate() {
        let (os, uuid) = (os.clone(), p.get("uuid").and_then(Value::as_str).unwrap_or_default().to_string());
        tasks
            .spawn(async move {
                let (inv, usage) = tokio::join!(
                    get_json(&os, placement_get(format!("resource_providers/{uuid}/inventories")), "GET provider inventories"),
                    get_json(&os, placement_get(format!("resource_providers/{uuid}/usages")), "GET provider usages")
                );
                (i, inv.unwrap_or(Value::Null), usage.unwrap_or(Value::Null))
            })
            .await;
    }
    let mut details = tasks.join_all().await;
    details.sort_by_key(|(i, ..)| *i);
    let mut rows: Vec<_> = details.iter().map(|(i, inv, usage)| mapping::resource_provider(&providers[*i], inv, usage)).collect();
    // The vGPU type is a `CUSTOM_*` trait, when the cloud sets one (ttcp-dev
    // doesn't: one call per GPU, 199, brought nothing). Ask each host's first GPU;
    // only when it has custom traits, the host's other GPUs.
    let roots: Vec<Option<String>> = providers.iter().map(|p| p.get("root_provider_uuid").and_then(Value::as_str).map(str::to_string)).collect();
    let gpus: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].vgpus.is_some()).collect();
    let traits_of = |idx: Vec<usize>| {
        let uuids: Vec<(usize, String)> = idx.into_iter().map(|i| (i, rows[i].uuid.clone())).collect();
        async move {
            let mut tasks = Bounded::new(FAN_OUT);
            for (i, uuid) in uuids {
                let os = os.clone();
                tasks
                    .spawn(async move {
                        let v = get_json(&os, placement_get(format!("resource_providers/{uuid}/traits")), "GET provider traits").await;
                        (i, v.map(|v| mapping::custom_traits(&v)).unwrap_or_default())
                    })
                    .await;
            }
            tasks.join_all().await
        }
    };
    let mut first_of_root: Vec<usize> = Vec::new();
    for &i in &gpus {
        if !first_of_root.iter().any(|&f| roots[f] == roots[i]) {
            first_of_root.push(i);
        }
    }
    let probed = traits_of(first_of_root.clone()).await;
    let typed_roots: Vec<Option<String>> = probed.iter().filter(|(_, tr)| !tr.is_empty()).map(|(i, _)| roots[*i].clone()).collect();
    let rest: Vec<usize> = gpus.iter().copied().filter(|i| !first_of_root.contains(i) && typed_roots.contains(&roots[*i])).collect();
    let more = traits_of(rest).await;
    for (i, tr) in probed.into_iter().chain(more) {
        rows[i].custom_traits = tr;
    }
    Ok(timed(t, provider_tree(rows)))
}

/// Admin › Compute › Host Aggregates, and the availability zones.
pub async fn admin_aggregates(os: &AsyncOpenStack) -> R<Timed<Aggregates>> {
    let t = Instant::now();
    let (ag, az) = tokio::join!(
        get_list!(os, openstack_sdk::api::compute::v2::aggregate::list_21, "os-aggregates"),
        get_list!(os, openstack_sdk::api::compute::v2::availability_zone::list_detail_21, "os-availability-zone/detail")
    );
    Ok(timed(t, Aggregates { aggregates: mapping::aggregates(&ag?), zones: az.map(|v| mapping::zones(&v)).into() }))
}

/// Admin › Compute › Flavors: public and private (`is_public=None`).
pub async fn admin_flavors(os: &AsyncOpenStack) -> R<Timed<Vec<FlavorRow>>> {
    let t = Instant::now();
    let h = (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(FLAVOR_MICROVERSION));
    let (raw, names) = tokio::join!(
        list_where!(os, openstack_sdk::api::compute::v2::flavor::list_detailed_20, "flavors/detail?is_public=None", |b| b.header(h.0, h.1).is_public("None")),
        project_names(os, &[])
    );
    let mut rows = mapping::flavor_rows(&raw?);
    // Who may use each private flavor: one `os-flavor-access` per private flavor
    // (Nova has no bulk call), in parallel; a failure leaves that flavor's list empty.
    let mut tasks = Bounded::new(FAN_OUT);
    for (i, f) in rows.iter().enumerate().filter(|(_, f)| !f.public) {
        let (os, id) = (os.clone(), f.id.clone());
        tasks.spawn(async move {
            use openstack_sdk::api::compute::v2::flavor::flavor_access::list_21 as m;
            let raw: R<Vec<Value>> = async {
                let ep = m::Request::builder().flavor_id(id).build().map_err(sfe("build flavor access"))?;
                counted(ep.query_async(&os)).await.map_err(sfe("GET flavor access"))
            }
            .await;
            (i, raw)
        }).await;
    }
    for (i, raw) in tasks.join_all().await {
        if let Ok(raw) = raw {
            rows[i].access = mapping::flavor_access(&raw, &names);
        }
    }
    Ok(timed(t, rows))
}

/// Admin › Compute › Instances: every project's servers, with project and host.
pub async fn admin_instances(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<Instance>>>> {
    let t = Instant::now();
    let (sv, im, vo, names) = tokio::join!(all_servers(os), images_raw(os), all_volumes(os), project_names(os, &[]));
    let sv = sv?;
    let rows = mapping::instances(&sv, &[], &soft(im), &soft(vo));
    Ok(timed(t, mapping::owned(rows, |i| &i.id, &sv, "tenant_id", Some("OS-EXT-SRV-ATTR:host"), &names)))
}

/// One page's raw rows, the next page's marker (`None` on the last page) and,
/// from Cinder at volume 3.45, the total of rows matching the filter.
struct RawPage {
    rows: Vec<Value>,
    next: Option<String>,
    count: Option<u64>,
}

/// One page of a paged list (roadmap 88): a single GET with `limit = page_size + 1`
/// and the filter, sort and marker of `q` (`paging::params` validates them), cut
/// into the page's raw rows and the next page's marker. Raw requests, not the SDK's
/// list builders: they lack some filters (Cinder's) and paging is one request, not
/// `Pagination::All`. `cinder_mv` is the cloud's Cinder microversion, which decides
/// a Cinder list's name filter and count (`paging::cinder`). `project`: the
/// session project for a Project panel (slice E), `None` for an Admin one
/// (`paging::params_in`).
async fn page_raw(os: &AsyncOpenStack, list: PagedList, project: Option<&str>, q: &ListQuery, page_size: u32, cinder_mv: Option<(u16, u16)>) -> R<RawPage> {
    let params = super::paging::params_in(list, project, q, page_size + 1)?;
    let (req, key, what) = list_request(list, params, cinder_mv, false);
    let body = get_json(os, req, what).await?;
    let count = body.get("count").and_then(Value::as_u64);
    let rows = body.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let (rows, next) = super::paging::cut(rows, page_size as usize);
    Ok(RawPage { rows, next, count })
}

/// The GET of one request of a paged list with these query `params`, the key of
/// its rows in the answer, and what to call it in errors. `brief` asks for the
/// lightest list the API has (Nova `servers`, Cinder without `/detail`), for
/// *Count all*. Cinder volumes and snapshots adapt to `cinder_mv`
/// (`paging::cinder`); groups always send volume 3.58, as the whole lists.
fn list_request(list: PagedList, mut params: Vec<(&'static str, String)>, cinder_mv: Option<(u16, u16)>, brief: bool) -> (super::actions::JsonRequest, &'static str, &'static str) {
    use super::actions::JsonRequest;
    use openstack_sdk::types::ServiceType;
    let pick = |detail: &'static str, light: &'static str| if brief { light } else { detail };
    let (service, path, key) = match list {
        PagedList::Instances => (ServiceType::Compute, pick("servers/detail", "servers"), "servers"),
        PagedList::Volumes => (ServiceType::BlockStorage, pick("volumes/detail", "volumes"), "volumes"),
        PagedList::Snapshots => (ServiceType::BlockStorage, pick("snapshots/detail", "snapshots"), "snapshots"),
        PagedList::FloatingIps => (ServiceType::Network, "floatingips", "floatingips"),
        PagedList::Networks => (ServiceType::Network, "networks", "networks"),
        PagedList::Routers => (ServiceType::Network, "routers", "routers"),
        PagedList::Trunks => (ServiceType::Network, "trunks", "trunks"),
        PagedList::Images => (ServiceType::Image, "images", "images"),
        PagedList::VolumeGroups => (ServiceType::BlockStorage, pick("groups/detail", "groups"), "groups"),
        PagedList::GroupSnapshots => (ServiceType::BlockStorage, pick("group_snapshots/detail", "group_snapshots"), "group_snapshots"),
    };
    let header = if list.is_cinder() {
        let (p, h) = super::paging::cinder(params, cinder_mv);
        params = p;
        h.map(volume_mv)
    } else if list == PagedList::Instances {
        Some(compute_mv())
    } else if matches!(list, PagedList::VolumeGroups | PagedList::GroupSnapshots) {
        // `project_id` on each row, as the whole lists ask.
        Some(volume_mv(GROUP_PROJECT_MICROVERSION))
    } else {
        None
    };
    let mut req = JsonRequest::get(service, path);
    for (k, v) in params {
        req = req.param(k, v);
    }
    if let Some((name, value)) = header {
        let mut h = http::HeaderMap::new();
        h.insert(name, value);
        req.headers = Some(h);
    }
    (req, key, path)
}

/// Rows *Count all* asks for per request: the APIs' usual cap (Nova `max_limit`,
/// Glance `api_limit_max`).
const COUNT_PAGE: u32 = 1000;

/// Most requests one *Count all* sends; past it the count reads "at least"
/// (`RowCount::more`): 100 000 rows.
pub const MAX_COUNT_PAGES: u32 = 100;

/// Time *Count all* keeps asking for: the server function as a whole has
/// `os::REQUEST_TIMEOUT` (20 s), and an "at least N" is more use than a timeout.
const COUNT_BUDGET: std::time::Duration = std::time::Duration::from_secs(15);

/// *Count all* for a paged list (roadmap 88): the rows matching `q`'s filter in
/// `project` (`None`: every project's, an Admin list), asked only when the user
/// clicks, never before the table shows. One
/// request when Cinder gives `count` (volume 3.45), else the lightest list
/// (`list_request` with `brief`, `fields=id` on Neutron) walked
/// [`COUNT_PAGE`] rows at a time until an empty answer, at most
/// [`MAX_COUNT_PAGES`] requests or [`COUNT_BUDGET`]. As costly as the whole list
/// was before paging.
pub async fn count_rows(os: &AsyncOpenStack, list: PagedList, project: Option<&str>, q: &ListQuery, cinder_mv: Option<(u16, u16)>) -> R<Timed<RowCount>> {
    let t = Instant::now();
    let mut counted = 0;
    let mut marker: Option<String> = None;
    for _ in 0..MAX_COUNT_PAGES {
        if t.elapsed() > COUNT_BUDGET {
            break;
        }
        let params = super::paging::count_params(list, project, q, marker.as_deref(), COUNT_PAGE)?;
        let (req, key, what) = list_request(list, params, cinder_mv, true);
        let body = get_json(os, req, what).await?;
        if let Some(n) = body.get("count").and_then(Value::as_u64) {
            return Ok(timed(t, RowCount { count: n, more: false }));
        }
        let rows = body.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        match super::paging::count_step(&mut counted, rows) {
            Some(m) => marker = Some(m),
            None => return Ok(timed(t, RowCount { count: counted, more: false })),
        }
    }
    Ok(timed(t, RowCount { count: counted, more: true }))
}

/// Glance's images with these IDs (`images?id=in:a,b`), 50 per request: a page
/// names the images of its rows without listing every image of the cloud.
async fn images_by_ids(os: &AsyncOpenStack, ids: Vec<String>) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    let mut tasks = Bounded::new(FAN_OUT);
    for chunk in ids.chunks(50) {
        let (os, chunk) = (os.clone(), chunk.to_vec());
        tasks
            .spawn(async move {
                // Glance answers 25 rows unless told otherwise.
                let req = JsonRequest::get(openstack_sdk::types::ServiceType::Image, "images").param("id", format!("in:{}", chunk.join(","))).param("limit", chunk.len().to_string());
                let body = get_json(&os, req, "GET images by ID").await?;
                Ok::<_, AppError>(body.get("images").and_then(Value::as_array).cloned().unwrap_or_default())
            })
            .await;
    }
    Ok(tasks.join_all().await.into_iter().collect::<R<Vec<_>>>()?.into_iter().flatten().collect())
}

/// These volumes (`volumes/{id}`), [`FAN_OUT`] at a time; one that can't be read
/// is left out, its row shows what Nova knows.
async fn volumes_by_ids(os: &AsyncOpenStack, ids: Vec<String>) -> R<Vec<Value>> {
    by_ids(os, NameKind::Volumes, ids).await
}

/// These servers or volumes, one `GET servers/{id}` / `volumes/{id}` each,
/// [`FAN_OUT`] at a time (neither API filters a list by several IDs, roadmap 88).
/// An ID that isn't an ID, or a resource that can't be read (deleted, 404), is
/// left out.
async fn by_ids(os: &AsyncOpenStack, kind: NameKind, ids: Vec<String>) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    use openstack_sdk::types::ServiceType;
    let mut tasks = Bounded::new(FAN_OUT);
    for id in ids.into_iter().filter(|id| check_id(id).is_ok()) {
        let os = os.clone();
        tasks
            .spawn(async move {
                let (req, key, what) = match kind {
                    NameKind::Volumes => (JsonRequest::get(ServiceType::BlockStorage, format!("volumes/{id}")), "volume", "GET volume"),
                    NameKind::Servers => (JsonRequest::get(ServiceType::Compute, format!("servers/{id}")), "server", "GET server"),
                };
                get_json(&os, req, what).await.ok().map(|v| unwrapped(v, key))
            })
            .await;
    }
    Ok(tasks.join_all().await.into_iter().flatten().collect())
}

/// Neutron networks by ID (`networks?id=a&id=b`), only their names.
async fn networks_by_ids(os: &AsyncOpenStack, ids: Vec<String>) -> R<Vec<Value>> {
    neutron_where(os, "networks", "id", ids, &["id", "name"]).await
}

/// Neutron's `path` rows whose `filter` is any of `values` (a repeated filter
/// is *any of*), one GET per 50 values so the URL stays short, with only
/// `fields`. Values that aren't IDs are dropped; none at all asks nothing.
async fn neutron_where(os: &AsyncOpenStack, path: &'static str, filter: &'static str, values: Vec<String>, fields: &[&'static str]) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    let values: Vec<String> = values.into_iter().filter(|v| check_id(v).is_ok()).collect();
    let mut out = Vec::new();
    for chunk in values.chunks(50) {
        let mut req = JsonRequest::get(openstack_sdk::types::ServiceType::Network, path);
        for f in fields {
            req = req.param("fields", *f);
        }
        for v in chunk {
            req = req.param(filter, v.clone());
        }
        let body = get_json(os, req, path).await?;
        out.extend(body.get(path).and_then(Value::as_array).cloned().unwrap_or_default());
    }
    Ok(out)
}

/// Admin › Compute › Instances, one page (roadmap 88): the cost depends on the
/// page, not the cloud. Besides the servers page and the project names, it reads
/// only the images the page's servers were built from: no cloud-wide
/// `volumes/detail?all_tenants` or image list. Boot-from-volume rows name their
/// image later, through [`boot_images`].
pub async fn admin_instances_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<Instance>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Instances, None, q, page_size, None), project_names(os, &[]));
    let RawPage { rows: sv, next, .. } = page?;
    let im = images_by_ids(os, mapping::image_ids(&sv)).await;
    let rows = mapping::instances_page(&sv, &soft(im));
    let rows = mapping::owned(rows, |i| &i.id, &sv, "tenant_id", Some("OS-EXT-SRV-ATTR:host"), &names);
    Ok(timed(t, Page { rows, next, count: None, paged: true, page_size }))
}

/// Most volumes one [`boot_images`] call reads. The page sends its IDs in
/// chunks of `model::BOOT_IMAGE_CHUNK` (the IDs travel in a GET's query string);
/// the cap only refuses a hand-made request for more.
pub const MAX_BOOT_VOLUMES: usize = 100;

/// The images of the servers booted from these volumes, `(server ID, image name)`,
/// for a paged Instances list's second call (roadmap 88). Cinder keeps only the
/// last `id=` of a list query (probed on ttcp-dev, 2026-11-03), so it is one
/// `volumes/{id}` per volume, [`FAN_OUT`] at a time.
pub async fn boot_images(os: &AsyncOpenStack, ids: Vec<String>) -> R<Timed<Vec<(String, String)>>> {
    let t = Instant::now();
    if ids.len() > MAX_BOOT_VOLUMES {
        return Err(AppError::invalid(format!("at most {MAX_BOOT_VOLUMES} volumes at once")));
    }
    let vo = volumes_by_ids(os, ids).await?;
    Ok(timed(t, mapping::boot_images(&vo)))
}

/// Most IDs one [`late_names`] call reads; the page sends `model::BOOT_IMAGE_CHUNK`.
pub const MAX_LATE_NAMES: usize = 100;

/// The names (and, for volumes, hosts) of the servers or volumes a paged page
/// points at, read after the table shows (roadmap 88).
pub async fn late_names(os: &AsyncOpenStack, kind: NameKind, ids: Vec<String>) -> R<Timed<Vec<LateName>>> {
    let t = Instant::now();
    if ids.len() > MAX_LATE_NAMES {
        return Err(AppError::invalid(format!("at most {MAX_LATE_NAMES} names at once")));
    }
    let raw = by_ids(os, kind, ids).await?;
    Ok(timed(t, raw.iter().filter_map(mapping::late_name).collect()))
}

/// Admin › Volume › Volumes, one page (roadmap 88): the volumes page and the
/// project names. Attached servers are named afterwards ([`late_names`]), not
/// from every server of the cloud. `volume_mv`: the cloud's Cinder microversion,
/// for the name filter and the total.
pub async fn admin_volumes_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32, volume_mv: Option<(u16, u16)>) -> R<Timed<Page<Owned<Volume>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Volumes, None, q, page_size, volume_mv), project_names(os, &[]));
    let RawPage { rows: vo, next, count } = page?;
    let rows = mapping::owned(mapping::volumes_page(&vo, &[]), |v| &v.id, &vo, "os-vol-tenant-attr:tenant_id", Some("os-vol-host-attr:host"), &names);
    Ok(timed(t, Page { rows, next, count, paged: true, page_size }))
}

/// Admin › Volume › Snapshots, one page (roadmap 88). Each snapshot's volume, and
/// with it the host column, is named afterwards ([`late_names`]), not from every
/// project's volumes.
pub async fn admin_snapshots_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32, volume_mv: Option<(u16, u16)>) -> R<Timed<Page<Owned<Snapshot>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Snapshots, None, q, page_size, volume_mv), project_names(os, &[]));
    let RawPage { rows: sn, next, count } = page?;
    let rows = mapping::owned(mapping::snapshots_page(&sn, &[]), |s| &s.id, &sn, "os-extended-snapshot-attributes:project_id", None, &names);
    Ok(timed(t, Page { rows, next, count, paged: true, page_size }))
}

/// Admin › Network › Floating IPs, one page (roadmap 88): the page, its pools in
/// one `networks?id=…` GET and the project names. Instances are named afterwards
/// ([`late_names`]), not from every server of the cloud.
pub async fn admin_floating_ips_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<FloatingIpRow>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::FloatingIps, None, q, page_size, None), project_names(os, &[]));
    let RawPage { rows: fi, next, .. } = page?;
    let ne = networks_by_ids(os, mapping::pool_ids(&fi)).await;
    let rows = mapping::owned(mapping::floating_ips_page(&fi, &soft(ne), &[]), |f| &f.id, &fi, "project_id", None, &names);
    Ok(timed(t, Page { rows, next, count: None, paged: true, page_size }))
}

/// Admin › Network › Networks, one page (roadmap 88): the page, its subnets
/// (`subnets?network_id=…`, not every subnet of the cloud) and the project names.
/// The DHCP-agent counts stay one call per agent, as [`admin_networks`].
pub async fn admin_networks_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32, dhcp: bool) -> R<Timed<Page<Owned<NetworkRow>>>> {
    let t = Instant::now();
    let (page, names, agents) = tokio::join!(
        page_raw(os, PagedList::Networks, None, q, page_size, None),
        project_names(os, &[]),
        async { if dhcp { dhcp_agent_counts(os).await.ok().flatten() } else { None } }
    );
    let RawPage { rows: ne, next, .. } = page?;
    let sub = neutron_where(os, "subnets", "network_id", mapping::row_ids(&ne), &["id", "name", "cidr"]).await;
    let mut rows = mapping::networks_page(&ne, &soft(sub), "");
    if let Some(counts) = &agents {
        for r in &mut rows {
            r.dhcp_agents = Some(counts.get(&r.id).copied().unwrap_or(0));
        }
    }
    Ok(timed(t, Page { rows: mapping::owned(rows, |n| &n.id, &ne, "project_id", None, &names), next, count: None, paged: true, page_size }))
}

/// Admin › Network › Routers, one page (roadmap 88): the page, its external
/// networks by ID and the project names, not every network of the cloud.
pub async fn admin_routers_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<RouterRow>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Routers, None, q, page_size, None), project_names(os, &[]));
    let RawPage { rows: ro, next, .. } = page?;
    let ne = networks_by_ids(os, mapping::gateway_ids(&ro)).await;
    let rows = mapping::owned(mapping::routers_page(&ro, &soft(ne)), |r| &r.id, &ro, "project_id", None, &names);
    Ok(timed(t, Page { rows, next, count: None, paged: true, page_size }))
}

/// Admin › Network › Trunks, one page (roadmap 88): the page, its parent and
/// sub-ports by ID (`ports?id=…`, only names) and the project names, instead of
/// every port of the cloud.
pub async fn admin_trunks_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<TrunkRow>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Trunks, None, q, page_size, None), project_names(os, &[]));
    let RawPage { rows: tr, next, .. } = page?;
    let po = neutron_where(os, "ports", "id", mapping::trunk_port_ids(&tr), &["id", "name"]).await;
    let rows = mapping::owned(mapping::trunks_page(&tr, &soft(po)), |r| &r.id, &tr, "project_id", None, &names);
    Ok(timed(t, Page { rows, next, count: None, paged: true, page_size }))
}

/// Admin › Compute › Images, one page (roadmap 88): the page and the project names.
pub async fn admin_images_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<Image>>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Images, None, q, page_size, None), project_names(os, &[]));
    let RawPage { rows: im, next, .. } = page?;
    let rows = mapping::owned(mapping::images_page(&im, "", &names), |i| &i.id, &im, "owner", None, &names);
    Ok(timed(t, Page { rows, next, count: None, paged: true, page_size }))
}

/// Admin › Volume › Groups, one page (roadmap 88). Group and volume types stay
/// whole lists (a cloud has a handful), and so do the group snapshots that say
/// whether a group has any: Cinder filters them by one `group_id` only.
pub async fn admin_volume_groups_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<VolumeGroupRow>>>> {
    use openstack_sdk::api::block_storage::v3::group_snapshot;
    let t = Instant::now();
    let (page, gt, vt, gs, names) = tokio::join!(
        page_raw(os, PagedList::VolumeGroups, None, q, page_size, None),
        group_types_raw(os),
        volume_types_raw(os),
        list_where!(os, group_snapshot::list_detailed_314, "group_snapshots/detail?all_tenants", |b| b.all_tenants(true)),
        project_names(os, &[])
    );
    let RawPage { rows: g, next, .. } = page?;
    let rows = mapping::volume_groups_page(&g, &soft(gt), &soft(vt), &soft(gs));
    Ok(timed(t, Page { rows: mapping::owned(rows, |r| &r.id, &g, "project_id", None, &names), next, count: None, paged: true, page_size }))
}

/// Admin › Volume › Group Snapshots, one page (roadmap 88): the page, the groups
/// it names (`groups/{id}`, [`FAN_OUT`] at a time: Cinder has no *any of*
/// filter), the group types and the project names.
pub async fn admin_group_snapshots_page(os: &AsyncOpenStack, q: &ListQuery, page_size: u32) -> R<Timed<Page<Owned<GroupSnapshotRow>>>> {
    use super::actions::JsonRequest;
    let t = Instant::now();
    let (page, gt, names) = tokio::join!(page_raw(os, PagedList::GroupSnapshots, None, q, page_size, None), group_types_raw(os), project_names(os, &[]));
    let RawPage { rows: gs, next, .. } = page?;
    let mut tasks = Bounded::new(FAN_OUT);
    for id in mapping::group_ids(&gs).into_iter().filter(|id| check_id(id).is_ok()) {
        let os = os.clone();
        tasks
            .spawn(async move {
                let req = JsonRequest::get(openstack_sdk::types::ServiceType::BlockStorage, format!("groups/{id}"));
                get_json(&os, req, "GET group").await.ok().map(|v| unwrapped(v, "group"))
            })
            .await;
    }
    let groups: Vec<Value> = tasks.join_all().await.into_iter().flatten().collect();
    let rows = mapping::group_snapshots_page(&gs, &groups, &soft(gt));
    Ok(timed(t, Page { rows: mapping::owned(rows, |r| &r.id, &gs, "project_id", None, &names), next, count: None, paged: true, page_size }))
}

// --- Paged Project panels (roadmap 88, slice E: `[ui] paged_lists = "all"`) ---------
//
// The Admin pages' pattern for the session project: one GET of `page_size + 1`
// rows (`page_raw` with the project, so `paging::params_in` scopes it), lookups
// bounded by the page, and the names of servers and volumes a row points at read
// after the table shows (`late_names`). Project › Networks stays whole: it merges
// three Neutron lists (own, shared, external), which one marker can't page.

/// Project › Compute › Instances, one page: the servers and the images they were
/// built from (`images?id=in:…`), instead of every image and volume the project
/// has. Boot-from-volume rows name their image afterwards ([`boot_images`]).
pub async fn instances_page(os: &AsyncOpenStack, project_id: &str, q: &ListQuery, page_size: u32) -> R<Timed<Page<Instance>>> {
    let t = Instant::now();
    let RawPage { rows: sv, next, .. } = page_raw(os, PagedList::Instances, Some(project_id), q, page_size, None).await?;
    let im = images_by_ids(os, mapping::image_ids(&sv)).await;
    Ok(timed(t, Page { rows: mapping::instances_page(&sv, &soft(im)), next, count: None, paged: true, page_size }))
}

/// Project › Volumes › Volumes, one page, with Cinder's total at volume 3.45. The
/// attached servers are named afterwards ([`late_names`]), not from every server
/// of the project.
pub async fn volumes_page(os: &AsyncOpenStack, project_id: &str, q: &ListQuery, page_size: u32, volume_mv: Option<(u16, u16)>) -> R<Timed<Page<Volume>>> {
    let t = Instant::now();
    let RawPage { rows: vo, next, count } = page_raw(os, PagedList::Volumes, Some(project_id), q, page_size, volume_mv).await?;
    Ok(timed(t, Page { rows: mapping::volumes_page(&vo, &[]), next, count, paged: true, page_size }))
}

/// Project › Volumes › Snapshots, one page; each snapshot's volume is named
/// afterwards ([`late_names`]).
pub async fn snapshots_page(os: &AsyncOpenStack, project_id: &str, q: &ListQuery, page_size: u32, volume_mv: Option<(u16, u16)>) -> R<Timed<Page<Snapshot>>> {
    let t = Instant::now();
    let RawPage { rows: sn, next, count } = page_raw(os, PagedList::Snapshots, Some(project_id), q, page_size, volume_mv).await?;
    Ok(timed(t, Page { rows: mapping::snapshots_page(&sn, &[]), next, count, paged: true, page_size }))
}

/// Project › Network › Floating IPs, one page: the project's (`project_id`), their
/// pools in one `networks?id=…` GET; instances are named afterwards ([`late_names`]).
pub async fn floating_ips_page(os: &AsyncOpenStack, project_id: &str, q: &ListQuery, page_size: u32) -> R<Timed<Page<FloatingIpRow>>> {
    let t = Instant::now();
    let RawPage { rows: fi, next, .. } = page_raw(os, PagedList::FloatingIps, Some(project_id), q, page_size, None).await?;
    let ne = networks_by_ids(os, mapping::pool_ids(&fi)).await;
    Ok(timed(t, Page { rows: mapping::floating_ips_page(&fi, &soft(ne), &[]), next, count: None, paged: true, page_size }))
}

/// Project › Network › Routers, one page: the project's, and their external
/// networks by ID.
pub async fn routers_page(os: &AsyncOpenStack, project_id: &str, q: &ListQuery, page_size: u32) -> R<Timed<Page<RouterRow>>> {
    let t = Instant::now();
    let RawPage { rows: ro, next, .. } = page_raw(os, PagedList::Routers, Some(project_id), q, page_size, None).await?;
    let ne = networks_by_ids(os, mapping::gateway_ids(&ro)).await;
    Ok(timed(t, Page { rows: mapping::routers_page(&ro, &soft(ne)), next, count: None, paged: true, page_size }))
}

/// Project › Compute › Images, one page of the images the token may see (the
/// whole list's Glance query), with the owners' names (`mine`: the user's
/// projects). In Glance's order, not the whole list's "own first, by name".
pub async fn images_page(os: &AsyncOpenStack, project_id: &str, mine: &[ProjectRef], q: &ListQuery, page_size: u32) -> R<Timed<Page<Image>>> {
    let t = Instant::now();
    let (page, names) = tokio::join!(page_raw(os, PagedList::Images, Some(project_id), q, page_size, None), project_names(os, mine));
    let RawPage { rows: im, next, .. } = page?;
    Ok(timed(t, Page { rows: mapping::images_page(&im, project_id, &names), next, count: None, paged: true, page_size }))
}

/// Admin › Compute › Images: every image Glance shows an admin, with its owner.
pub async fn admin_images(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<Image>>>> {
    let t = Instant::now();
    let (im, names) = tokio::join!(images_raw(os), project_names(os, &[]));
    let im = im?;
    let rows = mapping::images(&im, "", &names);
    Ok(timed(t, mapping::owned(rows, |i| &i.id, &im, "owner", None, &names)))
}

/// Admin › Volume › Volumes: every project's volumes, with project and host.
pub async fn admin_volumes(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<Volume>>>> {
    let t = Instant::now();
    let (vo, sv, names) = tokio::join!(all_volumes(os), all_servers_brief(os), project_names(os, &[]));
    let vo = vo?;
    let rows = mapping::volumes(&vo, &soft(sv));
    Ok(timed(t, mapping::owned(rows, |v| &v.id, &vo, "os-vol-tenant-attr:tenant_id", Some("os-vol-host-attr:host"), &names)))
}

/// Admin › Volume › Snapshots: every project's, the host being their volume's.
pub async fn admin_snapshots(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<Snapshot>>>> {
    let t = Instant::now();
    let (sn, vo, names) = tokio::join!(
        list_where!(os, openstack_sdk::api::block_storage::v3::snapshot::list_detailed, "snapshots/detail?all_tenants", |b| b.all_tenants(true)),
        all_volumes(os),
        project_names(os, &[])
    );
    let (sn, vo) = (sn?, soft(vo));
    let rows = mapping::snapshots(&sn, &vo);
    let hosts: HashMap<String, String> = vo.iter().filter_map(|v| Some((v.get("id")?.as_str()?.to_string(), v.get("os-vol-host-attr:host")?.as_str()?.to_string()))).collect();
    let mut out = mapping::owned(rows, |s| &s.id, &sn, "os-extended-snapshot-attributes:project_id", None, &names);
    for o in &mut out {
        o.host = hosts.get(&o.row.volume_id).cloned();
    }
    Ok(timed(t, out))
}

/// Admin › Volume › Groups: every project's (volume 3.58 for `project_id`).
pub async fn admin_volume_groups(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<VolumeGroupRow>>>> {
    use openstack_sdk::api::block_storage::v3::{group, group_snapshot};
    let t = Instant::now();
    let (k, v) = volume_mv(GROUP_PROJECT_MICROVERSION);
    let (g, gt, vt, gs, names) = tokio::join!(
        list_where!(os, group::list_detailed_313, "groups/detail?all_tenants", |b| b.header(k, v).all_tenants(true)),
        group_types_raw(os),
        volume_types_raw(os),
        list_where!(os, group_snapshot::list_detailed_314, "group_snapshots/detail?all_tenants", |b| b.all_tenants(true)),
        project_names(os, &[])
    );
    let g = g?;
    let rows = mapping::volume_groups(&g, &soft(gt), &soft(vt), &soft(gs));
    Ok(timed(t, mapping::owned(rows, |r| &r.id, &g, "project_id", None, &names)))
}

/// Admin › Volume › Group Snapshots: every project's (volume 3.58).
pub async fn admin_group_snapshots(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<GroupSnapshotRow>>>> {
    use openstack_sdk::api::block_storage::v3::{group, group_snapshot};
    let t = Instant::now();
    let (k, v) = volume_mv(GROUP_PROJECT_MICROVERSION);
    let (gs, g, gt, names) = tokio::join!(
        list_where!(os, group_snapshot::list_detailed_314, "group_snapshots/detail?all_tenants", |b| b.header(k, v).all_tenants(true)),
        list_where!(os, group::list_detailed_313, "groups/detail?all_tenants", |b| b.all_tenants(true)),
        group_types_raw(os),
        project_names(os, &[])
    );
    let gs = gs?;
    let rows = mapping::group_snapshots_rows(&gs, &soft(g), &soft(gt));
    Ok(timed(t, mapping::owned(rows, |r| &r.id, &gs, "project_id", None, &names)))
}

/// Admin › Volume › Volume Types and QoS Specs. Whether a type is encrypted
/// takes one GET per type (`types/{id}/encryption`, `{}` when not), as Horizon.
pub async fn admin_volume_types(os: &AsyncOpenStack) -> R<Timed<VolumeTypes>> {
    use super::actions::JsonRequest;
    let t = Instant::now();
    let (ty, qos) = tokio::join!(
        volume_types_raw(os),
        list_all!(os, openstack_sdk::api::block_storage::v3::qos_spec::list, "qos-specs")
    );
    let ty = ty?;
    let mut tasks = Bounded::new(FAN_OUT);
    for id in ty.iter().filter_map(|v| v.get("id").and_then(Value::as_str)) {
        let (os, id) = (os.clone(), id.to_string());
        tasks.spawn(async move {
            let e = get_json(&os, JsonRequest::get(openstack_sdk::types::ServiceType::BlockStorage, format!("types/{id}/encryption")), "GET volume type encryption").await;
            e.ok().as_ref().and_then(mapping::encryption_spec).map(|spec| (id, spec))
        }).await;
    }
    let encryption: Vec<(String, EncryptionSpec)> = tasks.join_all().await.into_iter().flatten().collect();
    let qos_raw = qos.as_ref().map(Vec::as_slice).unwrap_or(&[]).to_vec();
    Ok(timed(t, VolumeTypes { types: mapping::volume_type_rows(&ty, &encryption, &qos_raw), qos: qos.map(|v| mapping::qos_specs(&v)).into() }))
}

/// Admin › Volume › Group Types.
pub async fn admin_group_types(os: &AsyncOpenStack) -> R<Timed<Vec<GroupTypeRow>>> {
    let t = Instant::now();
    let raw = group_types_raw(os).await?;
    Ok(timed(t, mapping::group_type_rows(&raw)))
}

/// Admin › Network › Networks: every network, with its subnets and project.
/// With `dhcp` (Neutron `dhcp_agent_scheduler`), the DHCP agents hosting each
/// network: Horizon asks `networks/{id}/dhcp-agents` once per network (112 calls
/// on ttcp-dev); here the DHCP agents are listed and each one's
/// `agents/{id}/dhcp-networks` asked, one call per agent. A failure leaves the
/// column at `-` rather than failing the page.
pub async fn admin_networks(os: &AsyncOpenStack, dhcp: bool) -> R<Timed<Vec<Owned<NetworkRow>>>> {
    let t = Instant::now();
    let (ne, sub, names, agents) = tokio::join!(
        all_networks(os),
        list_all!(os, openstack_sdk::api::network::v2::subnet::list, "subnets (all)"),
        project_names(os, &[]),
        async { if dhcp { dhcp_agent_counts(os).await.ok().flatten() } else { None } }
    );
    let ne = ne?;
    let mut rows = mapping::networks(&ne, &soft(sub), "");
    if let Some(counts) = &agents {
        for r in &mut rows {
            r.dhcp_agents = Some(counts.get(&r.id).copied().unwrap_or(0));
        }
    }
    Ok(timed(t, mapping::owned(rows, |n| &n.id, &ne, "project_id", None, &names)))
}

/// Networks per DHCP agent, counted by network ID (see [`admin_networks`]).
async fn dhcp_agent_counts(os: &AsyncOpenStack) -> R<Option<std::collections::HashMap<String, usize>>> {
    let raw = list_where!(os, openstack_sdk::api::network::v2::agent::list, "DHCP agents", |b| b.agent_type("DHCP agent")).await?;
    // OVN serves DHCP itself and has no DHCP agent, yet may still advertise
    // `dhcp_agent_scheduler`: no counts then, and the page drops the column
    // rather than show 0 on every network (roadmap 89).
    if raw.is_empty() {
        return Ok(None);
    }
    let mut tasks = Bounded::new(FAN_OUT);
    for id in raw.iter().filter_map(|a| a.get("id").and_then(Value::as_str)) {
        let (os, id) = (os.clone(), id.to_string());
        tasks.spawn(async move {
            use openstack_sdk::api::network::v2::agent::dhcp_network::list as m;
            let ep = m::Request::builder().agent_id(id).build().map_err(sfe("build agent dhcp-networks"))?;
            let v: Value = counted(ep.query_async(&os)).await.map_err(sfe("GET agent dhcp-networks"))?;
            Ok::<_, AppError>(v)
        }).await;
    }
    let per_agent = tasks.join_all().await.into_iter().collect::<R<Vec<Value>>>()?;
    Ok(Some(mapping::dhcp_agent_counts(&per_agent)))
}

/// Admin › Network › Routers: every project's.
pub async fn admin_routers(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<RouterRow>>>> {
    let t = Instant::now();
    let (ro, ne, names) = tokio::join!(list_all!(os, openstack_sdk::api::network::v2::router::list, "routers (all)"), all_networks(os), project_names(os, &[]));
    let ro = ro?;
    let rows = mapping::routers(&ro, &soft(ne));
    Ok(timed(t, mapping::owned(rows, |r| &r.id, &ro, "project_id", None, &names)))
}

/// Admin › Network › Floating IPs: every project's.
pub async fn admin_floating_ips(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<FloatingIpRow>>>> {
    let t = Instant::now();
    let (fi, ne, sv, names) = tokio::join!(
        list_all!(os, openstack_sdk::api::network::v2::floatingip::list, "floatingips (all)"),
        all_networks(os),
        all_servers_brief(os),
        project_names(os, &[])
    );
    let fi = fi?;
    let rows = mapping::floating_ips(&fi, &soft(ne), &soft(sv));
    Ok(timed(t, mapping::owned(rows, |f| &f.id, &fi, "project_id", None, &names)))
}

/// Admin › Network › Trunks: every project's; parent ports named from the
/// ports list (`soft`: IDs when it fails), asked only when there are trunks.
pub async fn admin_trunks(os: &AsyncOpenStack) -> R<Timed<Vec<Owned<TrunkRow>>>> {
    use super::actions::JsonRequest;
    let t = Instant::now();
    let (tr, names) = tokio::join!(get_json(os, JsonRequest::get(openstack_sdk::types::ServiceType::Network, "trunks"), "GET trunks (all)"), project_names(os, &[]));
    let tr = tr?;
    let raw = tr.get("trunks").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    // Every project's ports only name parent ports: skipped when there are no
    // trunks (ttcp-dev: 2.1 s with the port list, slower than Horizon).
    let po = if raw.is_empty() { Ok(Vec::new()) } else { list_all!(os, openstack_sdk::api::network::v2::port::list, "ports (all)").await };
    let rows = mapping::trunks(raw, &soft(po));
    Ok(timed(t, mapping::owned(rows, |r| &r.id, raw, "project_id", None, &names)))
}

/// Admin › Network › RBAC Policies, networks and projects named.
pub async fn admin_rbac_policies(os: &AsyncOpenStack) -> R<Timed<Vec<RbacPolicyRow>>> {
    let t = Instant::now();
    let (rb, ne, names) = tokio::join!(list_all!(os, openstack_sdk::api::network::v2::rbac_policy::list, "rbac-policies"), all_networks(os), project_names(os, &[]));
    let rb = rb?;
    // QoS policies and security groups only when some policy shares one (most
    // policies are networks'); unfiltered, so every project's.
    let uses = |ty: &str| rb.iter().any(|r| r.get("object_type").and_then(Value::as_str) == Some(ty));
    let (qos, sg) = tokio::join!(
        async { if uses("qos_policy") { soft(list_all!(os, openstack_sdk::api::network::v2::qos::policy::list, "qos/policies (all)").await) } else { vec![] } },
        async { if uses("security_group") { soft(list_all!(os, openstack_sdk::api::network::v2::security_group::list, "security-groups (all)").await) } else { vec![] } }
    );
    let ne = soft(ne);
    Ok(timed(t, mapping::rbac_policies(&rb, &[("network", &ne[..]), ("qos_policy", &qos[..]), ("security_group", &sg[..])], &names)))
}

/// Admin › System Information (roadmap 85): the catalog plus Nova's and
/// Cinder's `os-services` and Neutron's agents, in parallel, each failing on its
/// own. A list whose service (or, for agents, the `agent` extension) `caps`
/// says is missing is `None` and not asked for.
pub async fn system_info(os: &AsyncOpenStack, caps: &Capabilities) -> R<Timed<SystemInfo>> {
    let t = Instant::now();
    let has = |r: Requires| r.met(caps);
    let nova = async {
        if !has(Requires::Service("compute")) {
            return None;
        }
        let raw = get_list!(os, openstack_sdk::api::compute::v2::service::list_20, "compute services").await;
        Some(raw.map(|v| mapping::host_services(&v)).into())
    };
    let cinder = async {
        if !has(Requires::Service("block-storage")) {
            return None;
        }
        // No `response_key` in the SDK: the body is `{"services": [...]}`.
        let raw = async {
            use openstack_sdk::api::block_storage::v3::service::list as m;
            let ep = m::Request::builder().build().map_err(sfe("build volume services"))?;
            let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET volume services"))?;
            Ok::<_, AppError>(v)
        };
        Some(raw.await.map(|v| mapping::host_services(v.get("services").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]))).into())
    };
    let neutron = async {
        if !has(Requires::All(&[Requires::Service("network"), needs::NETWORK_AGENTS])) {
            return None;
        }
        let raw = list_all!(os, openstack_sdk::api::network::v2::agent::list, "network agents").await;
        Some(raw.map(|v| mapping::network_agents(&v)).into())
    };
    let (compute, volume, agents) = tokio::join!(nova, cinder, neutron);
    Ok(timed(t, SystemInfo { services: super::os::catalog_services(os), compute, volume, agents }))
}

/// Compute microversion for the `default` quota class (see `default_quotas`).
const QUOTA_CLASS_MICROVERSION: &str = "compute 2.50";

/// Runs `f` only when `on` (a service the catalog has).
async fn when<T>(on: bool, f: impl Future<Output = T>) -> Option<T> {
    if on { Some(f.await) } else { None }
}

/// Admin › System › Defaults (roadmap 85): the default quotas new projects get,
/// per service, in parallel, each failing on its own. Nova and Cinder keep them
/// in the `default` quota class; Neutron answers per project
/// (`quotas/{project}/default`), the same for any project. A service the
/// catalog lacks is left out.
pub async fn default_quotas(os: &AsyncOpenStack, project_id: &str, caps: &Capabilities) -> R<Timed<Vec<QuotaCard>>> {
    let t = Instant::now();
    let has = |ty| Requires::Service(ty).met(caps);
    let nova = async {
        use openstack_sdk::api::compute::v2::quota_class_set::get_21 as m;
        // 2.50 adds `server_groups` and `server_group_members` (Horizon's ten
        // rows; at 2.1 ttcp-dev showed eight) and drops the network proxies.
        // Not 2.57, which drops the injected-file quotas Horizon still lists.
        let h = (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(QUOTA_CLASS_MICROVERSION));
        let ep = m::Request::builder().id("default").header(h.0, h.1).build().map_err(sfe("build default compute quotas"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET default compute quotas"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Compute, &v, &Value::Null))
    };
    let cinder = async {
        use openstack_sdk::api::block_storage::v3::quota_class_set::get as m;
        let ep = m::Request::builder().name("default").build().map_err(sfe("build default volume quotas"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET default volume quotas"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Volume, &v, &Value::Null))
    };
    let neutron = async {
        use openstack_sdk::api::network::v2::quota::defaults as m;
        let ep = m::Request::builder().id(project_id).build().map_err(sfe("build default network quotas"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET default network quotas"))?;
        Ok::<_, AppError>(mapping::quota_rows(QuotaService::Network, &v, &Value::Null))
    };
    let (n, c, q) = tokio::join!(when(has("compute"), nova), when(has("block-storage"), cinder), when(has("network"), neutron));
    let card = |service, r: R<Vec<QuotaRow>>| match r {
        Ok(rows) => QuotaCard { service, project_id: project_id.to_string(), rows, error: None },
        Err(e) => QuotaCard { service, project_id: project_id.to_string(), rows: vec![], error: Some(e.to_string()) },
    };
    let out = [(QuotaService::Compute, n), (QuotaService::Volume, c), (QuotaService::Network, q)]
        .into_iter()
        .filter_map(|(service, r)| r.map(|r| card(service, r)))
        .collect();
    Ok(timed(t, out))
}

/// Glance's namespaces as raw JSON, optionally only those associated with `resource_type`.
/// The SDK's list takes no paging parameters, so a raw GET follows Glance's `next` with
/// `marker`, at most `METADEF_PAGES` pages.
async fn metadef_pages(os: &AsyncOpenStack, resource_type: Option<&str>) -> R<Vec<Value>> {
    use super::actions::JsonRequest;
    const METADEF_PAGES: usize = 20;
    let mut all: Vec<Value> = Vec::new();
    let mut marker: Option<String> = None;
    for _ in 0..METADEF_PAGES {
        let mut req = JsonRequest::get(openstack_sdk::types::ServiceType::Image, "metadefs/namespaces").param("limit", "100");
        if let Some(rt) = resource_type {
            req = req.param("resource_types", rt);
        }
        if let Some(m) = &marker {
            req = req.param("marker", m.clone());
        }
        let page = get_json(os, req, "GET metadata definitions").await?;
        let items = page.get("namespaces").and_then(Value::as_array).cloned().unwrap_or_default();
        marker = items.last().and_then(|n| n.get("namespace")).and_then(Value::as_str).map(str::to_string);
        all.extend(items);
        if page.get("next").is_none() || marker.is_none() {
            break;
        }
    }
    Ok(all)
}

/// Admin › System › Metadata Definitions (roadmap 85): every namespace Glance
/// shows the token (admins: all).
pub async fn metadef_namespaces(os: &AsyncOpenStack) -> R<Timed<Vec<MetadefNamespace>>> {
    let t = Instant::now();
    let all = metadef_pages(os, None).await?;
    Ok(timed(t, mapping::metadef_namespaces(&all)))
}

/// The catalog's suggestions for *Update Metadata* (roadmap 25): the properties of every
/// namespace associated with `resource_type`. One GET lists the namespaces, then one per
/// namespace for its properties, [`FAN_OUT`] at a time (AGENTS.md §6). A namespace whose
/// details fail is skipped, so one broken namespace doesn't empty the picker.
pub async fn metadata_options(os: &AsyncOpenStack, resource_type: &str) -> R<Timed<Vec<MetadataOption>>> {
    use super::actions::JsonRequest;
    let t = Instant::now();
    let list = metadef_pages(os, Some(resource_type)).await?;
    let mut tasks = Bounded::new(FAN_OUT);
    for name in list.iter().filter_map(|n| n.get("namespace").and_then(Value::as_str)) {
        // The name goes into the path unencoded: skip anything that isn't a plain
        // namespace (`OS::Compute::Watchdog`), as `check_id` does for IDs (roadmap 48).
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '-')) {
            continue;
        }
        let (os, name, rt) = (os.clone(), name.to_string(), resource_type.to_string());
        tasks
            .spawn(async move {
                let req = JsonRequest::get(openstack_sdk::types::ServiceType::Image, format!("metadefs/namespaces/{name}")).param("resource_type", rt.clone());
                get_json(&os, req, "GET metadata definition").await.map(|ns| mapping::metadef_options(&ns)).unwrap_or_default()
            })
            .await;
    }
    let mut out: Vec<MetadataOption> = tasks.join_all().await.into_iter().flatten().collect();
    out.sort_by_key(|a| (a.namespace.to_lowercase(), a.title.to_lowercase()));
    Ok(timed(t, out))
}

/// Identity › Projects: the domain's projects for admins, the user's own otherwise.
pub async fn identity_projects(os: &AsyncOpenStack, s: &Session, admin: bool) -> R<Timed<Vec<IdentityProject>>> {
    let t = Instant::now();
    let me = identity_me(os, &s.user_id).await?;
    let (own, domains) = identity_domains(os, &me, &s.domain).await;
    let domain_id = list_domain(s, own);
    let own = || get_list!(os, openstack_sdk::api::identity::v3::user::project::list, "users/{id}/projects", |b| b.user_id(s.user_id.as_str()));
    let raw = if admin {
        let all = list_where!(os, openstack_sdk::api::identity::v3::project::list, "projects", |b| if let Some(d) = &domain_id {
            b.domain_id(d.as_str());
        })
        .await;
        match all {
            Err(e) if forbidden(&e) => own().await?,
            r => r?,
        }
    } else {
        own().await?
    };
    let mine: Vec<String> = if admin {
        soft(own().await).iter().filter_map(|p| p.get("id").and_then(Value::as_str).map(str::to_string)).collect()
    } else {
        raw.iter().filter_map(|p| p.get("id").and_then(Value::as_str).map(str::to_string)).collect()
    };
    Ok(timed(t, mapping::identity_projects(&raw, &domains, &s.project_id, &mine)))
}

/// Identity › Users: the domain's users for admins, the user alone otherwise.
pub async fn identity_users(os: &AsyncOpenStack, s: &Session, admin: bool) -> R<Timed<Vec<IdentityUser>>> {
    let t = Instant::now();
    let me = identity_me(os, &s.user_id).await?;
    let (own, domains) = identity_domains(os, &me, &s.domain).await;
    let domain_id = list_domain(s, own);
    if admin {
        let all = list_where!(os, openstack_sdk::api::identity::v3::user::list, "users", |b| if let Some(d) = &domain_id {
            b.domain_id(d.as_str());
        })
        .await;
        match all {
            Err(e) if forbidden(&e) => {}
            r => return Ok(timed(t, mapping::identity_users(&r?, &domains))),
        }
    }
    Ok(timed(t, mapping::identity_users(&[me], &domains)))
}

/// Identity › Users › a user (Keystone lets non-admins read only themselves).
pub async fn identity_user(os: &AsyncOpenStack, s: &Session, mine: &[ProjectRef], admin: bool, id: &str) -> R<Timed<IdentityUserDetail>> {
    use openstack_sdk::api::identity::v3 as idv3;
    let t = Instant::now();
    let (user, me, projects) = tokio::join!(identity_me(os, id), identity_me(os, &s.user_id), project_names(os, mine));
    let (_, domains) = identity_domains(os, &soft_value(me), &s.domain).await;
    let mut d = mapping::identity_user_detail(&user?, &domains, &projects);
    // Admins also see Horizon's Role assignments and Groups tabs.
    if admin {
        let (a, g) = tokio::join!(
            get_list!(os, idv3::role_assignment::list, "role_assignments?user.id", |b| b.user_id(id).include_names("true")),
            get_list!(os, idv3::user::group::list, "users/{id}/groups", |b| b.user_id(id))
        );
        if let (Ok(a), Ok(g)) = (a, g) {
            d.assignments = Some(mapping::role_assignment_rows(&a));
            d.groups = Some(g.iter().filter_map(|x| Some(Ref { id: x.get("id")?.as_str()?.to_string(), name: x.get("name").and_then(Value::as_str).unwrap_or_default().to_string() })).collect());
        }
    }
    Ok(timed(t, d))
}

fn soft_value(r: R<Value>) -> Value {
    r.unwrap_or(Value::Null)
}

/// Identity › Projects › a project. Admins also get its members (role
/// assignments, then each group's users in parallel), like Horizon's tabs.
pub async fn identity_project(os: &AsyncOpenStack, s: &Session, admin: bool, id: &str) -> R<Timed<IdentityProjectDetail>> {
    use openstack_sdk::api::identity::v3 as idv3;
    let t = Instant::now();
    let project = async {
        let ep = idv3::project::get::Request::builder().id(id).build().map_err(sfe("build project"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET project"))?;
        Ok::<_, AppError>(unwrapped(v, "project"))
    };
    let (p, me) = tokio::join!(project, identity_me(os, &s.user_id));
    let p = p?;
    let (_, domains) = identity_domains(os, &soft_value(me), &s.domain).await;
    let (users, groups) = if admin {
        let assignments = get_list!(os, idv3::role_assignment::list, "role_assignments", |b| b.scope_project_id(id).include_names("true")).await;
        match assignments {
            // Keystone may still refuse (system-scoped policies): no tabs then.
            Err(e) if forbidden(&e) => (None, None),
            a => {
                let a = a?;
                let group_ids: Vec<String> = a.iter().filter_map(|x| x.get("group")?.get("id")?.as_str().map(str::to_string)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let mut tasks = tokio::task::JoinSet::new();
                for gid in group_ids {
                    let os = os.clone();
                    spawn_counted(&mut tasks, async move {
                        let v = get_list!(&os, idv3::group::user::list, "groups/{id}/users", |b| b.group_id(gid.as_str())).await;
                        (gid, soft(v))
                    });
                }
                let group_users = tasks.join_all().await;
                let (u, g) = mapping::project_members(&a, &group_users);
                (Some(u), Some(g))
            }
        }
    } else {
        (None, None)
    };
    let mut rows = mapping::identity_projects(std::slice::from_ref(&p), &domains, &s.project_id, &[]);
    let project = rows.pop().ok_or_else(|| AppError::internal("project without an id"))?;
    Ok(timed(t, IdentityProjectDetail { project, domain_id: p.get("domain_id").and_then(Value::as_str).map(str::to_string), users, groups }))
}

/// Identity › Groups (admins; Keystone refuses others).
pub async fn identity_groups(os: &AsyncOpenStack, s: &Session) -> R<Timed<Vec<IdentityGroup>>> {
    let t = Instant::now();
    let me = identity_me(os, &s.user_id).await?;
    let (own, domains) = identity_domains(os, &me, &s.domain).await;
    let domain_id = list_domain(s, own);
    let raw = list_where!(os, openstack_sdk::api::identity::v3::group::list, "groups", |b| if let Some(d) = &domain_id {
        b.domain_id(d.as_str());
    })
    .await?;
    Ok(timed(t, mapping::identity_groups(&raw, &domains)))
}

/// Identity › Roles (admins), with the roles each implies.
pub async fn identity_roles(os: &AsyncOpenStack) -> R<Timed<Vec<IdentityRole>>> {
    let t = Instant::now();
    let (raw, inf) = tokio::join!(
        get_list!(os, openstack_sdk::api::identity::v3::role::list, "roles"),
        get_list!(os, openstack_sdk::api::identity::v3::role_inference::list, "role_inferences")
    );
    Ok(timed(t, mapping::identity_roles(&raw?, &soft(inf))))
}

/// Identity › Groups › a group and its members.
pub async fn identity_group(os: &AsyncOpenStack, s: &Session, id: &str) -> R<Timed<IdentityGroupDetail>> {
    use openstack_sdk::api::identity::v3 as idv3;
    let t = Instant::now();
    let group = async {
        let ep = idv3::group::get::Request::builder().id(id).build().map_err(sfe("build group"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET group"))?;
        Ok::<_, AppError>(v)
    };
    let (g, users, me) = tokio::join!(group, get_list!(os, idv3::group::user::list, "groups/{id}/users", |b| b.group_id(id)), identity_me(os, &s.user_id));
    let (_, domains) = identity_domains(os, &soft_value(me), &s.domain).await;
    Ok(timed(t, mapping::identity_group_detail(&g?, &users?, &domains)))
}

/// Identity › Application Credentials › one of the user's.
pub async fn app_credential(os: &AsyncOpenStack, s: &Session, id: &str) -> R<Timed<AppCredentialDetail>> {
    use openstack_sdk::api::identity::v3::user::application_credential::get as m;
    let t = Instant::now();
    let ep = m::Request::builder().user_id(s.user_id.as_str()).id(id).build().map_err(sfe("build application credential"))?;
    let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET application credential"))?;
    Ok(timed(t, mapping::app_credential_detail(&v)))
}

/// Identity › Application Credentials: the user's own.
pub async fn app_credentials(os: &AsyncOpenStack, s: &Session) -> R<Timed<Vec<AppCredential>>> {
    let t = Instant::now();
    let raw = get_list!(os, openstack_sdk::api::identity::v3::user::application_credential::list, "application_credentials", |b| b
        .user_id(s.user_id.as_str()))
    .await?;
    Ok(timed(t, mapping::app_credentials(&raw)))
}

/// Compute microversion for one server group: 2.64 returns `policy` and
/// `rules` (`max_server_per_host`) instead of the `policies` list.
const SERVER_GROUP_MICROVERSION: &str = "compute 2.64";

/// One server group, and the servers (for member names and status; `soft`, so
/// a failing list leaves the IDs).
pub async fn server_group_detail(os: &AsyncOpenStack, id: &str) -> R<Timed<ServerGroupDetail>> {
    let t = Instant::now();
    use openstack_sdk::api::compute::v2::server_group::get_21 as m;
    let group = async {
        let h = (http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(SERVER_GROUP_MICROVERSION));
        let ep = m::Request::builder().id(id).header(h.0, h.1).build().map_err(sfe("build server group"))?;
        let v: Value = counted(ep.query_async(os)).await.map_err(sfe("GET server group"))?;
        Ok::<_, AppError>(v)
    };
    let (g, sv) = tokio::join!(group, servers(os));
    Ok(timed(t, mapping::server_group_detail(&g?, &soft(sv))))
}

// ---------------------------------------------------------------------------
// Choices for forms (GET only)
// ---------------------------------------------------------------------------

/// The options of one dialog field set; GET only, one or a few calls each.
pub async fn choices(os: &AsyncOpenStack, project_id: &str, set: ChoiceSet) -> R<Vec<Choice>> {
    use mapping::{choices as ch, label_name};
    Ok(match set {
        ChoiceSet::Flavors => {
            let l = &super::config::state().config.prefs.launch;
            mapping::flavor_choices(&flavors(os).await?, l.flavor_sort, l.flavor_sort_reverse)
        }
        ChoiceSet::LaunchImages => mapping::image_choices(&launch_images_raw(os, project_id).await?, false),
        ChoiceSet::InstanceSnapshots => mapping::image_choices(&launch_images_raw(os, project_id).await?, true),
        ChoiceSet::FreePorts => {
            use openstack_sdk::api::network::v2::port::list as ports;
            mapping::free_port_choices(&list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id)).await?)
        }
        ChoiceSet::Images => {
            let im = images_raw(os).await?;
            let active: Vec<Value> =
                im.into_iter().filter(|i| i.get("status").and_then(Value::as_str) == Some("active")).collect();
            ch(&active, "id", label_name)
        }
        ChoiceSet::Networks => {
            use openstack_sdk::api::network::v2::{network::list as net, subnet::list as sub};
            // Own and shared networks (not external: instances don't plug into
            // them), and the subnets of those, filtered the same way (roadmap 64).
            let (own, shared, own_sn, shared_sn) = tokio::join!(
                list_where!(os, net, "networks?tenant_id", |b| b.tenant_id(project_id)),
                list_where!(os, net, "networks?shared", |b| b.shared(true)),
                list_where!(os, sub, "subnets?tenant_id", |b| b.tenant_id(project_id)),
                list_where!(os, sub, "subnets?shared", |b| b.shared(true))
            );
            let (nets, subs) = (merge_by_id(vec![own?, soft(shared)]), merge_by_id(vec![soft(own_sn), soft(shared_sn)]));
            mapping::network_choices(&nets, &subs)
        }
        ChoiceSet::ExternalNetworks => {
            use openstack_sdk::api::network::v2::network::list as net;
            ch(&list_where!(os, net, "networks?router:external", |b| b.router_external(true)).await?, "id", label_name)
        }
        ChoiceSet::Subnets => {
            use openstack_sdk::api::network::v2::subnet::list as sub;
            let (sn, ne) = tokio::join!(
                list_where!(os, sub, "subnets?tenant_id", |b| b.tenant_id(project_id)),
                visible_networks(os, project_id)
            );
            let names = mapping::network_names(&soft(ne));
            ch(&sn?, "id", |x| mapping::label_subnet(x, &names))
        }
        ChoiceSet::KeyPairs => {
            mapping::keypair_choices(&keypairs_raw(os).await?)
        }
        // Nova takes security groups by name in the launch request.
        ChoiceSet::SecurityGroups => mapping::security_group_choices(&security_groups_raw(os, project_id).await?, "name"),
        ChoiceSet::SecurityGroupIds => mapping::security_group_choices(&security_groups_raw(os, project_id).await?, "id"),
        ChoiceSet::ServerGroups => mapping::server_group_choices(
            &list_all!(os, openstack_sdk::api::compute::v2::server_group::list_20, "os-server-groups").await?,
        ),
        ChoiceSet::ComputeZones => {
            use openstack_sdk::api::compute::v2::availability_zone::list_21 as m;
            let ep = m::Request::builder().build().map_err(sfe("build compute AZs"))?;
            let v: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET compute AZs"))?;
            zones(&v)
        }
        // Admin › *Create RBAC Policy* (roadmap 85): every project's, named with
        // their project (names repeat across projects); `*` first, as Horizon.
        ChoiceSet::RbacTargets | ChoiceSet::AllProjects => {
            let names = project_names(os, &[]).await;
            let mut out: Vec<Choice> = names.iter().map(|(id, n)| Choice::new(id.as_str(), n.as_str())).collect();
            out.sort_by_key(|c| c.label.to_lowercase());
            if set == ChoiceSet::RbacTargets {
                out.insert(0, Choice::new("*", "*"));
            }
            out
        }
        ChoiceSet::AllNetworks => {
            let (raw, names) = tokio::join!(all_networks(os), project_names(os, &[]));
            mapping::owned_choices(&raw?, &names)
        }
        ChoiceSet::AllQosPolicies => {
            let (raw, names) = tokio::join!(list_all!(os, openstack_sdk::api::network::v2::qos::policy::list, "qos/policies (all)"), project_names(os, &[]));
            mapping::owned_choices(&raw?, &names)
        }
        ChoiceSet::NetworkZones => mapping::zone_choices(
            &list_where!(os, openstack_sdk::api::network::v2::availability_zone::list, "network AZs", |b| b.resource("network").state("available")).await?,
            "network",
        ),
        // Unfiltered: ttcp-dev's Neutron answers 400 to `resource=router` (while
        // advertising `router_availability_zone`); the mapper filters instead.
        ChoiceSet::ComputeHosts | ChoiceSet::AllComputeHosts => mapping::compute_host_choices(
            &get_list!(os, openstack_sdk::api::compute::v2::service::list_20, "compute services", |b| b.binary("nova-compute")).await?,
            set == ChoiceSet::ComputeHosts,
        ),
        ChoiceSet::RouterZones => mapping::zone_choices(&list_all!(os, openstack_sdk::api::network::v2::availability_zone::list, "router AZs").await?, "router"),
        ChoiceSet::QosSpecs => ch(&list_all!(os, openstack_sdk::api::block_storage::v3::qos_spec::list, "qos-specs").await?, "id", label_name),
        // One call each, every project's: on ttcp-dev about as large as Admin ›
        // Trunks' port list (2 s). Filtered to the owner and the floating IP's
        // pool in the dialog. Routers (every project's: Neutron accepts any
        // router between the subnet and the pool) only with the topology
        // check, the external networks only without it (roadmap 89).
        ChoiceSet::AllAssociablePorts => {
            let topology = super::config::state().config.prefs.network.fip_topology_check;
            let routers = async {
                if !topology {
                    return Ok(Vec::new());
                }
                list_all!(os, openstack_sdk::api::network::v2::router::list, "routers (all)").await
            };
            let pools = async {
                if topology {
                    return Ok(Vec::new());
                }
                list_where!(os, openstack_sdk::api::network::v2::network::list, "networks?router:external", |b| b.router_external(true)).await
            };
            let (po, sv, fi, ro, pl) = tokio::join!(
                list_all!(os, openstack_sdk::api::network::v2::port::list, "ports (all)"),
                all_servers_brief(os),
                list_all!(os, openstack_sdk::api::network::v2::floatingip::list, "floatingips (all)"),
                routers,
                pools
            );
            let (po, names) = (po?, mapping::server_names(&soft(sv)));
            if topology {
                let reach = mapping::reachable_subnets_by_pool(&po, &ro?);
                mapping::owned_port_choices(&po, &soft(fi), &names, mapping::PoolReach::Subnets(&reach))
            } else {
                let pools: Vec<String> = pl?.iter().filter_map(|n| n.get("id").and_then(Value::as_str).map(str::to_string)).collect();
                mapping::owned_port_choices(&po, &soft(fi), &names, mapping::PoolReach::Every(&pools))
            }
        }
        ChoiceSet::MetadefResourceTypes => {
            use super::actions::JsonRequest;
            let v = get_json(os, JsonRequest::get(openstack_sdk::types::ServiceType::Image, "metadefs/resource_types"), "GET metadata resource types").await?;
            mapping::metadef_resource_type_choices(&v)
        }
        ChoiceSet::CinderPools => {
            use openstack_sdk::api::block_storage::v3::scheduler_stat::pool::list as m;
            let ep = m::Request::builder().build().map_err(sfe("build volume pools"))?;
            let v: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET volume pools"))?;
            mapping::pool_choices(&v)
        }
        ChoiceSet::VolumeZones => {
            use openstack_sdk::api::block_storage::v3::availability_zone::list as m;
            let ep = m::Request::builder().build().map_err(sfe("build volume AZs"))?;
            let v: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET volume AZs"))?;
            zones(&v)
        }
        ChoiceSet::VolumeTypes => {
            use openstack_sdk::api::block_storage::v3::r#type::list as m;
            let ep = m::Request::builder().build().map_err(sfe("build volume types"))?;
            let v: Vec<Value> = counted(ep.query_async(os)).await.map_err(sfe("GET volume types"))?;
            ch(&v, "name", label_name)
        }
        ChoiceSet::AvailableVolumes => mapping::volume_choices(&volumes_raw(os).await?),
        ChoiceSet::VolumeSnapshots => mapping::snapshot_choices(&snapshots_raw(os).await?),
        ChoiceSet::Servers => ch(&servers_brief(os).await?, "id", label_name),
        ChoiceSet::AssociablePorts | ChoiceSet::InstancePorts => {
            use openstack_sdk::api::network::v2::port::list as ports;
            let associate = set == ChoiceSet::AssociablePorts;
            // `[network] fip_topology_check`: only ports a floating IP can reach.
            let topology = associate && super::config::state().config.prefs.network.fip_topology_check;
            let routers = async {
                if !topology {
                    return Ok(Vec::new());
                }
                list_where!(os, openstack_sdk::api::network::v2::router::list, "routers", |b| b.tenant_id(project_id)).await
            };
            let (po, sv, fi, ro) = tokio::join!(
                list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id)),
                servers_brief(os),
                list_where!(os, openstack_sdk::api::network::v2::floatingip::list, "floatingips", |b| b
                    .tenant_id(project_id)),
                routers
            );
            let (po, names) = (po?, mapping::server_names(&soft(sv)));
            let reachable = topology.then(|| mapping::externally_reachable_networks(&po, &soft(ro)));
            mapping::instance_port_choices(&po, &soft(fi), &names, associate, reachable.as_ref())
        }
        ChoiceSet::FirewallRules => mapping::firewall_rule_choices(&fwaas_list(os, project_id, "firewall_rules").await?),
        ChoiceSet::FirewallPolicies => mapping::firewall_policy_choices(&fwaas_list(os, project_id, "firewall_policies").await?),
        ChoiceSet::FirewallPorts => {
            let ((pt, ro, sv), gr) = tokio::join!(firewall_port_lookups(os, project_id), fwaas_list(os, project_id, "firewall_groups"));
            mapping::firewall_port_choices(&pt?, &gr?, &ro, &sv)
        }
        ChoiceSet::QosPolicies => mapping::qos_policy_choices(&qos_policies_raw(os, project_id).await?),
        ChoiceSet::LbMemberTargets => {
            use openstack_sdk::api::network::v2::port::list as ports;
            let (po, sv, n) = tokio::join!(
                list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id)),
                servers_brief(os),
                net_lookups(os, project_id)
            );
            let subnets = n.map(|n| n.subnets).unwrap_or_default();
            mapping::lb_member_choices(&po?, &mapping::server_names(&soft(sv)), &subnets)
        }
        ChoiceSet::IdentityRoles => ch(&get_list!(os, openstack_sdk::api::identity::v3::role::list, "roles").await?, "id", label_name),
        // Like Horizon's member pickers: the domain's users, groups and projects
        // (the current project's domain), not every domain's (service users).
        ChoiceSet::IdentityUsers => {
            let d = project_domain(os, project_id).await;
            ch(&list_where!(os, openstack_sdk::api::identity::v3::user::list, "users?domain_id", |b| if let Some(d) = &d { b.domain_id(d.as_str()); }).await?, "id", label_name)
        }
        ChoiceSet::IdentityGroups => {
            let d = project_domain(os, project_id).await;
            ch(&list_where!(os, openstack_sdk::api::identity::v3::group::list, "groups?domain_id", |b| if let Some(d) = &d { b.domain_id(d.as_str()); }).await?, "id", label_name)
        }
        ChoiceSet::IdentityProjects => {
            let d = project_domain(os, project_id).await;
            ch(&list_where!(os, openstack_sdk::api::identity::v3::project::list, "projects?domain_id", |b| if let Some(d) = &d { b.domain_id(d.as_str()); }).await?, "id", label_name)
        }
        // Needs the session: answered by `api::get_choices`.
        ChoiceSet::OperationDomain => vec![],
        ChoiceSet::GroupTypes => ch(&group_types_raw(os).await?, "id", label_name),
        ChoiceSet::VolumeTypeIds => ch(&volume_types_raw(os).await?, "id", label_name),
        ChoiceSet::GroupVolumes => {
            let (k, v) = volume_mv("volume 3.13");
            let (g, vo, vt) = tokio::join!(
                volume_groups_raw(os),
                list_where!(os, openstack_sdk::api::block_storage::v3::volume::list_detailed, "volumes/detail", |b| b.header(k, v)),
                volume_types_raw(os)
            );
            mapping::group_volume_choices(&g?, &vo?, &soft(vt))
        }
        ChoiceSet::Secrets => mapping::secret_choices(&km_all(os, "secrets", "GET secrets").await?),
        ChoiceSet::TokenRoles => super::os::token_role_names(os).into_iter().map(|r| Choice::new(r.clone(), r)).collect(),
        ChoiceSet::LbMemberSubnets => {
            let n = net_lookups(os, project_id).await?;
            mapping::lb_member_subnet_choices(&n.networks, &n.subnets)
        }
        ChoiceSet::LbCertificates => {
            let (c, sec) = tokio::join!(km_all(os, "containers", "GET containers"), km_all(os, "secrets", "GET secrets"));
            mapping::lb_certificate_choices(&c?, &sec?)
        }
        // The project's subnets and shared networks' (`shared=true`, which Neutron
        // copies onto their subnets), like Horizon's VIP list (roadmap 54).
        ChoiceSet::LbVipSubnets => {
            use openstack_sdk::api::network::v2::subnet::list as sub;
            let (own, shared, ne) = tokio::join!(
                list_where!(os, sub, "subnets?tenant_id", |b| b.tenant_id(project_id)),
                list_where!(os, sub, "subnets?shared", |b| b.shared(true)),
                visible_networks(os, project_id)
            );
            let names = mapping::network_names(&soft(ne));
            ch(&merge_by_id(vec![own?, soft(shared)]), "id", |x| mapping::label_subnet(x, &names))
        }
        ChoiceSet::LbAvailabilityZones => mapping::lb_az_choices(&list_all!(os, openstack_sdk::api::load_balancer::v2::availability_zone::list, "lbaas/availabilityzones").await?),
        ChoiceSet::LbFlavors => mapping::lb_flavor_choices(&list_all!(os, openstack_sdk::api::load_balancer::v2::flavor::list, "lbaas/flavors").await?),
        ChoiceSet::ForwardablePorts => {
            use openstack_sdk::api::network::v2::port::list as ports;
            let (po, sv) = tokio::join!(list_where!(os, ports, "ports?tenant_id", |b| b.tenant_id(project_id)), servers_brief(os));
            mapping::forwardable_port_choices(&po?, &mapping::server_names(&soft(sv)))
        }
        ChoiceSet::FreeFloatingIps => {
            use openstack_sdk::api::network::v2::floatingip::list as fips;
            mapping::free_floating_ip_choices(&list_where!(os, fips, "floatingips", |b| b.tenant_id(project_id)).await?)
        }
    })
}

/// Available zones from Nova (`zoneName`) or Cinder (`zoneName`) listings.
fn zones(v: &[Value]) -> Vec<Choice> {
    let mut out: Vec<Choice> = v
        .iter()
        .filter(|z| z.get("zoneState").and_then(|s| s.get("available")).and_then(Value::as_bool).unwrap_or(true))
        .filter_map(|z| z.get("zoneName").and_then(Value::as_str))
        .filter(|n| *n != "internal")
        .map(|n| Choice::new(n, n))
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Roadmap 85: hundreds of simultaneous Placement GETs got ttcp-dev to refuse
    /// this machine for minutes; a fan-out never has more than `limit` in flight.
    #[tokio::test]
    async fn bounded_fan_out_caps_requests_in_flight() {
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        let (now, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let mut tasks = Bounded::new(3);
        for i in 0..20 {
            let (now, peak) = (now.clone(), peak.clone());
            tasks
                .spawn(async move {
                    peak.fetch_max(now.fetch_add(1, SeqCst) + 1, SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    now.fetch_sub(1, SeqCst);
                    i
                })
                .await;
        }
        let mut out = tasks.join_all().await;
        out.sort();
        assert_eq!(out, (0..20).collect::<Vec<_>>(), "every result, those finished while waiting for a slot too");
        assert_eq!(peak.load(SeqCst), 3, "never more than the limit at once");
    }

    /// Counts come from the requests, joined or spawned, not from hand-typed
    /// numbers (roadmap 65); outside `serving` nothing is counted.
    #[tokio::test]
    async fn timed_counts_every_request_of_the_call() {
        let t = Instant::now();
        assert_eq!(timed(t, ()).calls, 0, "no counter: 0");
        let n = serving(None, async {
            tokio::join!(counted(async {}), counted(async {}));
            let mut tasks = tokio::task::JoinSet::new();
            for _ in 0..3 {
                spawn_counted(&mut tasks, counted(async {}));
            }
            tasks.join_all().await;
            timed(t, ()).calls
        })
        .await;
        assert_eq!(n, 5, "two joined, three spawned");
        let fresh = serving(None, async { timed(t, ()).calls }).await;
        assert_eq!(fresh, 0, "each server function starts at 0");
    }

    /// Roadmap 66: a hit costs no request; failures and other keys still fetch.
    #[tokio::test]
    async fn cached_lookups_skip_the_request_until_they_expire() {
        let cache = moka::future::Cache::builder().time_to_live(std::time::Duration::from_millis(200)).build();
        let calls = serving(None, async {
            let ok = || counted(async { Ok(vec![json!({"id": "f1"})]) });
            let failing = || counted(async { Err::<Vec<Value>, _>(AppError::internal("down")) });
            assert!(cached(&cache, "s/flavors".into(), failing()).await.is_err());
            assert_eq!(cached(&cache, "s/flavors".into(), ok()).await.unwrap().len(), 1, "a failure is not kept");
            cached(&cache, "s/flavors".into(), ok()).await.unwrap();
            cached(&cache, "other/flavors".into(), ok()).await.unwrap();
            timed(Instant::now(), ()).calls
        })
        .await;
        assert_eq!(calls, 3, "failure, first fetch and the other scope; the hit is free");
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(cache.get("s/flavors").await.is_none(), "gone after its time to live");
    }

    #[tokio::test]
    async fn without_a_scope_lookups_are_not_cached() {
        let n = serving(None, async {
            for _ in 0..2 {
                lookup("flavors", counted(async { Ok(vec![]) })).await.unwrap();
            }
            timed(Instant::now(), ()).calls
        })
        .await;
        assert_eq!(n, 2);
    }

    #[test]
    fn merge_keeps_first_occurrence_and_items_without_id() {
        let out = merge_by_id(vec![
            vec![json!({"id": "a", "src": 1}), json!({"id": "b"})],
            vec![json!({"id": "a", "src": 2}), json!({"name": "no-id"})],
        ]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0]["src"], 1);
    }
}
