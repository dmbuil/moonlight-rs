//! Mutating operations. Every [`OsAction`] is translated into one or more typed
//! `openstack_sdk` endpoints and handed to an [`Exec`], which according to the
//! configured [`WriteMode`]:
//!
//! * `Off`     → rejects the action before building anything;
//! * `DryRun`  → builds each request (method, path, JSON body) and returns it
//!   **without sending anything**;
//! * `On`      → sends the requests.
//!
//! Because dry-run never touches the network, the tests below exercise every
//! action exactly as production would build it.

//! Mutating operations. Every [`OsAction`] is translated into one or more typed
//! `openstack_sdk` endpoints and handed to an [`Exec`], which according to the
//! configured [`WriteMode`]:
//!
//! * `Off`     → rejects the action before building anything;
//! * `DryRun`  → builds each request (method, path, JSON body) and returns it
//!   **without sending anything**;
//! * `On`      → sends the requests.
//!
//! Because dry-run never touches the network, the tests below exercise every
//! action exactly as production would build it.

use super::sfe;
use crate::model::*;
use openstack_sdk::AsyncOpenStack;
use openstack_sdk::api::{QueryAsync, RestEndpoint, ignore};
use serde_json::{Value, json};
use std::borrow::Cow;

type R<T> = Result<T, AppError>;

/// Runs requests under `[writes] mode`: rejects them when `off`, only records a
/// preview in `dry-run`, sends them when `on`. Every write goes through one (AGENTS.md §1).
pub struct Exec<'a> {
    mode: WriteMode,
    os: Option<&'a AsyncOpenStack>,
    requests: Vec<RequestPreview>,
    secret: Option<Secret>,
    results: Vec<ItemResult>,
    /// The ID `create` returned last.
    created: Option<String>,
}

/// What a request will look like on the wire (relative to the service endpoint).
pub fn preview<E: RestEndpoint>(ep: &E) -> R<RequestPreview> {
    let body = ep.body().map_err(sfe("serialise request body"))?.map(|(_, bytes)| {
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(mut v) => {
                mask_secrets(&mut v);
                serde_json::to_string_pretty(&v).unwrap_or_default()
            }
            Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
        }
    });
    Ok(RequestPreview {
        service: ep.service_type().to_string(),
        method: ep.method().to_string(),
        path: with_query(ep),
        body,
    })
}

/// A JSON request the SDK cannot build correctly (port forwarding: its builder
/// types ports as `f32` and ranges as numbers). Still one `RestEndpoint`, so it
/// goes through `Exec` (dry-run, preview) like every typed one.
pub struct JsonRequest {
    pub method: http::Method,
    pub path: String,
    pub service: openstack_sdk::types::ServiceType,
    pub body: Option<Value>,
    pub query: Vec<(&'static str, String)>,
    /// Extra headers (Barbican's payload `Accept`); they replace the SDK's
    /// default `Accept: application/json`.
    pub headers: Option<http::HeaderMap>,
}

impl JsonRequest {
    /// A request with any method, path (relative to the service endpoint) and JSON body.
    pub fn new(
        method: http::Method,
        service: openstack_sdk::types::ServiceType,
        path: impl Into<String>,
        body: Option<Value>,
    ) -> Self {
        JsonRequest {
            method,
            path: path.into(),
            service,
            body,
            query: Vec::new(),
            headers: None,
        }
    }
    /// A Barbican request (roadmap 55). The SDK has no Key Manager API, and
    /// catalogs list Barbican unversioned (`https://…:9311`), so paths carry
    /// `v1/` themselves; discovery is not run for it.
    pub fn key_manager(method: http::Method, path: impl std::fmt::Display, body: Option<Value>) -> Self {
        Self::new(
            method,
            openstack_sdk::types::ServiceType::Other("key-manager".into()),
            format!("v1/{path}"),
            body,
        )
    }
    /// A Heat request (roadmap 62). The SDK has no Orchestration API. Heat's
    /// catalog URL is versioned and project-scoped (`…/v1/<project_id>`), and
    /// the SDK appends relative paths after it, so paths are plain `stacks`.
    pub fn orchestration(method: http::Method, path: impl Into<String>, body: Option<Value>) -> Self {
        Self::new(
            method,
            openstack_sdk::types::ServiceType::Other("orchestration".into()),
            path,
            body,
        )
    }
    /// An Aodh request (roadmap 79). The SDK has no Alarming API, and catalogs
    /// list Aodh unversioned (`https://…:8042`), so paths carry `v2/` themselves;
    /// discovery is not run for it.
    pub fn alarming(method: http::Method, path: impl std::fmt::Display, body: Option<Value>) -> Self {
        Self::new(
            method,
            openstack_sdk::types::ServiceType::Other("alarming".into()),
            format!("v2/{path}"),
            body,
        )
    }
    /// A GET (read paths in `fetch.rs` use it too; GET only there).
    pub fn get(service: openstack_sdk::types::ServiceType, path: impl Into<String>) -> Self {
        Self::new(http::Method::GET, service, path, None)
    }
    /// Adds a query parameter.
    pub fn param(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.query.push((key, value.into()));
        self
    }
}

/// Bodies can carry secrets (a Barbican payload, a password): wiped when the
/// request is dropped. The SDK's serialised copy is out of our reach.
impl Drop for JsonRequest {
    fn drop(&mut self) {
        if let Some(b) = self.body.as_mut() {
            wipe_secrets(b);
        }
    }
}

impl RestEndpoint for JsonRequest {
    fn parameters(&self) -> openstack_sdk::api::QueryParams<'_> {
        let mut p = openstack_sdk::api::QueryParams::default();
        for (k, v) in &self.query {
            p.push(*k, v.as_str());
        }
        p
    }
    fn method(&self) -> http::Method {
        self.method.clone()
    }
    fn request_headers(&self) -> Option<&http::HeaderMap> {
        self.headers.as_ref()
    }
    fn endpoint(&self) -> Cow<'static, str> {
        self.path.clone().into()
    }
    fn service_type(&self) -> openstack_sdk::types::ServiceType {
        self.service.clone()
    }
    fn body(&self) -> Result<Option<(&'static str, Vec<u8>)>, openstack_sdk::api::BodyError> {
        Ok(self.body.as_ref().map(|b| {
            (
                "application/json",
                serde_json::to_vec(b).expect("JSON value serialises"),
            )
        }))
    }
}

/// An SDK enum from its wire name (`"HTTP"`, `"ROUND_ROBIN"`…): every Octavia
/// module has its own copy of these enums, all deserialisable from the API's strings.
fn enum_of<T: serde::de::DeserializeOwned>(v: &str, what: &str) -> R<T> {
    serde_json::from_value(Value::String(v.to_string())).map_err(|_| AppError::invalid(format!("unknown {what}: {v}")))
}

fn octavia(method: http::Method, path: String, body: Value) -> JsonRequest {
    JsonRequest::new(
        method,
        openstack_sdk::types::ServiceType::LoadBalancer,
        path,
        Some(body),
    )
}

/// Keys whose values never appear in a preview (they reach the browser and logs);
/// `secret` is an application credential's (roadmap 57), `payload` a Barbican
/// secret's (roadmap 55).
const SECRET_KEYS: [&str; 4] = ["adminPass", "password", "secret", "payload"];

/// Calls `f` on every string value under one of [`SECRET_KEYS`], at any depth.
fn each_secret(v: &mut Value, f: &mut impl FnMut(&mut String)) {
    match v {
        Value::Object(m) => m.iter_mut().for_each(|(k, x)| match x {
            Value::String(s) if SECRET_KEYS.contains(&k.as_str()) => f(s),
            _ => each_secret(x, f),
        }),
        Value::Array(a) => a.iter_mut().for_each(|x| each_secret(x, f)),
        _ => {}
    }
}

/// Masks the values under [`SECRET_KEYS`] in request previews.
fn mask_secrets(v: &mut Value) {
    each_secret(v, &mut |s| *s = "********".into());
}

/// Overwrites the values under [`SECRET_KEYS`] with zeros (`zeroize`).
fn wipe_secrets(v: &mut Value) {
    each_secret(v, &mut |s| zeroize::Zeroize::zeroize(s));
}

/// `/{endpoint}` plus its query string, so previews show e.g. `?cascade=true`.
fn with_query<E: RestEndpoint>(ep: &E) -> String {
    let mut url = url::Url::parse("http://preview/").expect("static URL");
    url.set_path(&ep.endpoint());
    ep.parameters().add_to_url(&mut url);
    match url.query() {
        Some(q) if !q.is_empty() => format!("{}?{q}", url.path()),
        _ => url.path().to_string(),
    }
}

fn build_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::invalid(format!("invalid request: {e}"))
}

impl<'a> Exec<'a> {
    /// An executor for `mode`; `os` is only needed (and only used) when `mode` is `on`.
    pub fn new(mode: WriteMode, os: Option<&'a AsyncOpenStack>) -> Self {
        Self {
            mode,
            os,
            requests: Vec::new(),
            secret: None,
            results: Vec::new(),
            created: None,
        }
    }

    /// The mode it runs in.
    pub fn mode(&self) -> WriteMode {
        self.mode
    }

    /// The request previews, in order (for the console calls).
    pub fn requests(self) -> Vec<RequestPreview> {
        self.requests
    }

    /// Everything `run_action` returns: previews, a one-time secret, batch results.
    pub fn outcome(self) -> ActionOutcome {
        ActionOutcome {
            mode: self.mode,
            requests: self.requests,
            secret: self.secret,
            results: self.results,
            created: self.created,
        }
    }

    /// Masks the values of these parameters (hidden, `NoEcho`) in the last
    /// request's preview: their names are the template's, not [`SECRET_KEYS`].
    pub(super) fn hide_params(&mut self, names: &[String]) {
        let Some(body) = self.requests.last_mut().and_then(|r| r.body.as_mut()) else {
            return;
        };
        let Ok(mut v) = serde_json::from_str::<Value>(body) else {
            return;
        };
        if let Some(p) = v.get_mut("parameters").and_then(Value::as_object_mut) {
            for n in names {
                if let Some(x) = p.get_mut(n) {
                    *x = json!("********");
                }
            }
        }
        *body = serde_json::to_string_pretty(&v).unwrap_or_default();
    }

    fn client(&self) -> R<&'a AsyncOpenStack> {
        self.os.ok_or_else(|| AppError::internal("no OpenStack connection"))
    }

    /// Records the upload that follows *Create Image* › *File* (roadmap 45): a streamed `PUT`
    /// the browser makes (or Moonlight forwards), outside the SDK, so it is only previewed
    /// here. The preview lets dry-run show what would happen, as for every other write.
    fn note_upload(&mut self, image_id: &str, bytes: u64) {
        self.requests.push(RequestPreview {
            service: "image".into(),
            method: "PUT".into(),
            path: format!("/images/{image_id}/file"),
            body: Some(format!("application/octet-stream, {bytes} bytes (streamed, not shown)")),
        });
    }

    /// Sends (or previews) a request whose response we do not need.
    async fn send<E>(&mut self, ep: E) -> R<()>
    where
        E: RestEndpoint + Send + Sync,
    {
        self.requests.push(preview(&ep)?);
        match self.mode {
            WriteMode::Off => Err(AppError::invalid("writes are disabled")),
            WriteMode::DryRun => Ok(()),
            WriteMode::On => ignore(ep)
                .query_async(self.client()?)
                .await
                .map_err(sfe("OpenStack rejected the request")),
        }
    }

    /// Sends (or previews) a request and returns its JSON response (`None` in dry-run).
    pub(super) async fn call<E>(&mut self, ep: E) -> R<Option<Value>>
    where
        E: RestEndpoint + Send + Sync,
    {
        self.requests.push(preview(&ep)?);
        match self.mode {
            WriteMode::Off => Err(AppError::invalid("writes are disabled")),
            WriteMode::DryRun => Ok(None),
            WriteMode::On => ep
                .query_async(self.client()?)
                .await
                .map(Some)
                .map_err(sfe("OpenStack rejected the request")),
        }
    }

    /// Sends (or previews) a create request and returns the new resource ID.
    /// In dry-run the ID is a readable placeholder, e.g. `<new network id>`.
    async fn create<E>(&mut self, ep: E, what: &str) -> R<String>
    where
        E: RestEndpoint + Send + Sync,
    {
        self.requests.push(preview(&ep)?);
        match self.mode {
            WriteMode::Off => Err(AppError::invalid("writes are disabled")),
            WriteMode::DryRun => Ok(format!("<new {what} id>")),
            WriteMode::On => {
                let v: Value = ep
                    .query_async(self.client()?)
                    .await
                    .map_err(sfe("OpenStack rejected the request"))?;
                let id =
                    created_id(&v).ok_or_else(|| AppError::internal(format!("{what} created but no id returned")))?;
                self.created = Some(id.clone());
                Ok(id)
            }
        }
    }
}

/// The new resource's ID in a create response. Barbican answers with a ref URL,
/// not an ID (roadmap 55); Heat with `{"stack": {"id", "links"}}` (roadmap 62);
/// Aodh with the alarm, whose ID is `alarm_id` (roadmap 79: without it, a
/// created alarm was reported as a failure).
fn created_id(v: &Value) -> Option<String> {
    v.get("id")
        .or_else(|| v.get("stack")?.get("id"))
        .or_else(|| v.get("alarm_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            ["secret_ref", "container_ref"]
                .iter()
                .find_map(|k| v.get(*k)?.as_str())
                .map(super::mapping::ref_id)
        })
}

fn checked_metadata(pairs: &[(String, String)]) -> R<Vec<(String, String)>> {
    validate_metadata(pairs).map_err(AppError::invalid)
}

fn opt(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// A key that becomes a path segment (flavor extra specs): non-empty, no `/`.
fn path_key(key: &str) -> R<&str> {
    let k = non_empty(key, "key")?;
    if k.contains(['/', '?', '#']) {
        return Err(AppError::invalid(format!("invalid key: {k}")));
    }
    Ok(k)
}

/// A count that must be at least 1 (VCPUs, RAM), as Nova's `int32`.
fn positive(n: i64, field: &str) -> R<i32> {
    i32::try_from(n)
        .ok()
        .filter(|n| *n >= 1)
        .ok_or_else(|| AppError::invalid(format!("{field} must be at least 1")))
}

fn non_empty<'s>(s: &'s str, field: &str) -> R<&'s str> {
    let t = s.trim();
    if t.is_empty() {
        Err(AppError::invalid(format!("{field} is required")))
    } else {
        Ok(t)
    }
}

mod admin;
mod admin_volume;
mod alarm;
mod block_storage;
mod compute;
mod firewall;
mod heat;
mod identity;
mod image;
mod keymgr;
mod l7;
mod network;
mod octavia;
mod qos;
mod router;
#[cfg(test)]
mod testing;

use admin::*;
use compute::*;
pub use heat::*;
pub use identity::*;
pub use keymgr::*;
use network::*;

/// Runs one action. `Off` mode is rejected up-front.
pub async fn apply(x: &mut Exec<'_>, action: &OsAction) -> R<()> {
    if x.mode == WriteMode::Off {
        return Err(AppError::invalid("writes are disabled on this Moonlight instance"));
    }
    use OsAction::*;
    match action {
        QuotaUpdate {
            service,
            project_id,
            values,
        } => x.send(quota_update(*service, project_id, values)?).await?,
        DefaultQuotaUpdate { service, values } => x.send(default_quota_update(*service, values)?).await?,
        MetadefNamespaceImport {
            definition,
            public,
            protected,
        } => admin::metadef_namespace_import(x, definition, public, protected).await?,
        MetadefNamespaceUpdate {
            id,
            display_name,
            description,
            public,
            protected,
        } => admin::metadef_namespace_update(x, id, display_name, description, public, protected).await?,
        MetadefNamespaceDelete { id } => admin::metadef_namespace_delete(x, id).await?,
        MetadefNamespaceSetTypes {
            id,
            add_type_ids,
            remove_type_ids,
            prefixes,
        } => admin::metadef_namespace_set_types(x, id, add_type_ids, remove_type_ids, prefixes).await?,
        ServerLaunch(spec) => launch(x, spec).await?,
        ServerUpdate { id, name } => compute::server_update(x, id, name).await?,
        ServerStart { id } => compute::server_start(x, id).await?,
        ServerStop { id } => compute::server_stop(x, id).await?,
        ServerReboot { id, kind } => compute::server_reboot(x, id, kind).await?,
        ServerPause { id } => compute::server_pause(x, id).await?,
        ServerUnpause { id } => compute::server_unpause(x, id).await?,
        ServerSuspend { id } => compute::server_suspend(x, id).await?,
        ServerResume { id } => compute::server_resume(x, id).await?,
        ServerLock { id } => compute::server_lock(x, id).await?,
        ServerUnlock { id } => compute::server_unlock(x, id).await?,
        ServerSnapshot { id, name } => compute::server_snapshot(x, id, name).await?,
        ServerDelete { id } => compute::server_delete(x, id).await?,
        ServerSetMetadata { id, metadata } => compute::server_set_metadata(x, id, metadata).await?,
        ServerSetSecurityGroups { id, add, remove } => compute::server_set_security_groups(x, id, add, remove).await?,
        ServerAttachVolume { server_id, volume_id } => compute::server_attach_volume(x, server_id, volume_id).await?,
        ServerDetachVolume { server_id, volume_id } => compute::server_detach_volume(x, server_id, volume_id).await?,
        ServerResize { id, flavor_id } => compute::server_resize(x, id, flavor_id).await?,
        ServerConfirmResize { id } => compute::server_confirm_resize(x, id).await?,
        ServerRevertResize { id } => compute::server_revert_resize(x, id).await?,
        ServerRebuild { id, image_id } => compute::server_rebuild(x, id, image_id).await?,
        ServerShelve { id } => compute::server_shelve(x, id).await?,
        ServerUnshelve { id, availability_zone } => match opt(availability_zone) {
            Some(az) => {
                use openstack_sdk::api::compute::v2::server::unshelve_277 as m;
                let body = m::UnshelveBuilder::default()
                    .availability_zone(az)
                    .build()
                    .map_err(build_err)?;
                x.send(
                    m::Request::builder()
                        .id(id.as_str())
                        .unshelve(body)
                        .build()
                        .map_err(build_err)?,
                )
                .await?;
            }
            None => {
                use openstack_sdk::api::compute::v2::server::unshelve_21 as m;
                x.send(
                    m::Request::builder()
                        .id(id.as_str())
                        .unshelve(Value::Null)
                        .build()
                        .map_err(build_err)?,
                )
                .await?;
            }
        },
        ServerRescue { id, image_id } => compute::server_rescue(x, id, image_id).await?,
        ServerUnrescue { id } => compute::server_unrescue(x, id).await?,
        ServerAttachInterface {
            id,
            network_id,
            port_id,
            fixed_ip,
        } => compute::server_attach_interface(x, id, network_id, port_id, fixed_ip).await?,
        ServerDetachInterface { id, port_id } => compute::server_detach_interface(x, id, port_id).await?,
        ImageUpdate { id, changes } => image::image_update(x, id, changes).await?,
        ImageSetProperties { id, set, remove } => image::image_set_properties(x, id, set, remove).await?,
        ImageDelete { id } => image::image_delete(x, id).await?,
        ImageCreate(spec) => image::image_create(x, spec).await?,
        KeyPairImport {
            name,
            public_key,
            key_type,
        } => compute::key_pair_import(x, name, public_key, key_type).await?,
        KeyPairGenerate { name } => compute::key_pair_generate(x, name).await?,
        KeyPairDelete { name } => compute::key_pair_delete(x, name).await?,
        ServerGroupCreate { name, policy } => compute::server_group_create(x, name, policy).await?,
        ServerGroupDelete { id } => compute::server_group_delete(x, id).await?,
        VolumeCreate {
            name,
            description,
            size_gb,
            volume_type,
            availability_zone,
            source,
            source_id,
        } => {
            block_storage::volume_create(
                x,
                name,
                description,
                size_gb,
                volume_type,
                availability_zone,
                source,
                source_id,
            )
            .await?
        }
        VolumeUpdate { id, name, description } => block_storage::volume_update(x, id, name, description).await?,
        VolumeExtend { id, new_size_gb } => block_storage::volume_extend(x, id, new_size_gb).await?,
        VolumeDelete { id } => block_storage::volume_delete(x, id).await?,
        VolumeSetMetadata { id, metadata } => block_storage::volume_set_metadata(x, id, metadata).await?,
        VolumeRetype { id, new_type, migrate } => block_storage::volume_retype(x, id, new_type, migrate).await?,
        VolumeUploadImage {
            id,
            image_name,
            disk_format,
            force,
            visibility,
        } => block_storage::volume_upload_image(x, id, image_name, disk_format, force, visibility).await?,
        VolumeTransferCreate { volume_id, name } => block_storage::volume_transfer_create(x, volume_id, name).await?,
        VolumeTransferAccept { transfer_id, auth_key } => {
            block_storage::volume_transfer_accept(x, transfer_id, auth_key).await?
        }
        VolumeSnapshotCreate {
            volume_id,
            name,
            description,
            force,
        } => block_storage::volume_snapshot_create(x, volume_id, name, description, force).await?,
        SnapshotUpdate { id, name, description } => block_storage::snapshot_update(x, id, name, description).await?,
        SnapshotDelete { id } => block_storage::snapshot_delete(x, id).await?,
        SnapshotSetMetadata { id, metadata } => block_storage::snapshot_set_metadata(x, id, metadata).await?,
        VolumeGroupDelete { id, delete_volumes } => block_storage::delete_volume_group(x, id, delete_volumes).await?,
        VolumeGroupCreate {
            name,
            description,
            group_type_id,
            volume_type_ids,
            availability_zone,
        } => {
            block_storage::volume_group_create(x, name, description, group_type_id, volume_type_ids, availability_zone)
                .await?
        }
        VolumeGroupCreateFrom {
            name,
            description,
            source_group_id,
            group_snapshot_id,
        } => block_storage::volume_group_create_from(x, name, description, source_group_id, group_snapshot_id).await?,
        VolumeGroupUpdate { id, name, description } => {
            block_storage::volume_group_update(x, id, name, description).await?
        }
        VolumeGroupSetVolumes {
            id,
            add_volume_ids,
            remove_volume_ids,
        } => block_storage::volume_group_set_volumes(x, id, add_volume_ids, remove_volume_ids).await?,
        GroupSnapshotCreate {
            group_id,
            name,
            description,
        } => block_storage::group_snapshot_create(x, group_id, name, description).await?,
        GroupSnapshotDelete { id } => block_storage::group_snapshot_delete(x, id).await?,
        BackupCreate {
            volume_id,
            name,
            description,
            incremental,
            force,
            snapshot_id,
        } => block_storage::backup_create(x, volume_id, name, description, incremental, force, snapshot_id).await?,
        BackupUpdate { id, name, description } => block_storage::backup_update(x, id, name, description).await?,
        BackupRestore { id, volume_id, name } => block_storage::backup_restore(x, id, volume_id, name).await?,
        BackupDelete { id } => block_storage::backup_delete(x, id).await?,
        NetworkCreate { name, admin_up, subnet } => network::network_create(x, name, admin_up, subnet).await?,
        AdminNetworkCreate(spec) => network::admin_network_create(x, spec).await?,
        AdminNetworkUpdate {
            id,
            name,
            admin_up,
            shared,
            external,
        } => network::admin_network_update(x, id, name, admin_up, shared, external).await?,
        FlavorCreate(spec) => admin::flavor_create(x, spec).await?,
        FlavorReplace { spec, extra_specs } => admin::flavor_replace(x, spec, extra_specs).await?,
        FlavorDelete { id } => admin::flavor_delete(x, id).await?,
        FlavorSetExtraSpecs { id, set, unset } => admin::flavor_set_extra_specs(x, id, set, unset).await?,
        FlavorSetAccess { id, add, remove } => admin::flavor_set_access(x, id, add, remove).await?,
        AggregateCreate { name, zone, hosts } => admin::aggregate_create(x, name, zone, hosts).await?,
        AggregateUpdate { id, name, zone } => admin::aggregate_update(x, id, name, zone).await?,
        AggregateSetHosts { id, add, remove } => admin::aggregate_set_hosts(x, id, add, remove).await?,
        AggregateSetMetadata { id, set, unset } => admin::aggregate_set_metadata(x, id, set, unset).await?,
        AggregateDelete { id } => admin::aggregate_delete(x, id).await?,
        ComputeServiceSet { id, enabled, reason } => admin::compute_service_set(x, id, enabled, reason).await?,
        ServerMigrate { id, host } => admin::server_migrate(x, id, host).await?,
        ServerLiveMigrate {
            id,
            host,
            block_migration,
        } => admin::server_live_migrate(x, id, host, block_migration).await?,
        VolumeResetStatus { id, status } => admin_volume::volume_reset_status(x, id, status).await?,
        VolumeMigrate {
            id,
            host,
            force_host_copy,
        } => admin_volume::volume_migrate(x, id, host, force_host_copy).await?,
        VolumeManage(spec) => admin_volume::volume_manage(x, spec).await?,
        VolumeUnmanage { id } => admin_volume::volume_unmanage(x, id).await?,
        VolumeTypeCreate {
            name,
            description,
            public,
        } => admin_volume::volume_type_create(x, name, description, public).await?,
        VolumeTypeUpdate {
            id,
            name,
            description,
            public,
        } => admin_volume::volume_type_update(x, id, name, description, public).await?,
        VolumeTypeDelete { id } => admin_volume::volume_type_delete(x, id).await?,
        VolumeTypeSetExtraSpecs { id, set, unset } => {
            admin_volume::volume_type_set_extra_specs(x, id, set, unset).await?
        }
        VolumeTypeSetQos {
            id,
            qos_id,
            current_qos_id,
        } => admin_volume::volume_type_set_qos(x, id, qos_id, current_qos_id).await?,
        VolumeTypeEncryptionSet { id, spec, update } => {
            admin_volume::volume_type_encryption_set(x, id, spec, update).await?
        }
        VolumeTypeEncryptionDelete { id } => admin_volume::volume_type_encryption_delete(x, id).await?,
        QosSpecCreate { name, consumer, specs } => admin_volume::qos_spec_create(x, name, consumer, specs).await?,
        QosSpecSetConsumer { id, consumer } => admin_volume::qos_spec_set_consumer(x, id, consumer).await?,
        QosSpecSetSpecs { id, set, unset } => admin_volume::qos_spec_set_specs(x, id, set, unset).await?,
        QosSpecDelete { id } => admin_volume::qos_spec_delete(x, id).await?,
        GroupTypeCreate {
            name,
            description,
            public,
        } => admin_volume::group_type_create(x, name, description, public).await?,
        GroupTypeUpdate {
            id,
            name,
            description,
            public,
        } => admin_volume::group_type_update(x, id, name, description, public).await?,
        GroupTypeDelete { id } => admin_volume::group_type_delete(x, id).await?,
        GroupTypeSetSpecs { id, set, unset } => admin_volume::group_type_set_specs(x, id, set, unset).await?,
        SnapshotResetStatus { id, status } => admin_volume::snapshot_reset_status(x, id, status).await?,
        RbacPolicyCreate {
            kind,
            object_id,
            target_project,
        } => network::rbac_policy_create(x, kind, object_id, target_project).await?,
        RbacPolicyUpdate { id, target_project } => network::rbac_policy_update(x, id, target_project).await?,
        RbacPolicyDelete { id } => network::rbac_policy_delete(x, id).await?,
        NetworkUpdate { id, name, admin_up } => network::network_update(x, id, name, admin_up).await?,
        NetworkDelete { id } => network::network_delete(x, id).await?,
        SubnetCreate { network_id, spec } => create_subnet(x, network_id, spec).await?,
        SubnetUpdate {
            id,
            name,
            enable_dhcp,
            dns_nameservers,
        } => network::subnet_update(x, id, name, enable_dhcp, dns_nameservers).await?,
        SubnetDelete { id } => network::subnet_delete(x, id).await?,
        PortCreate {
            network_id,
            name,
            admin_up,
            fixed_ip,
        } => network::port_create(x, network_id, name, admin_up, fixed_ip).await?,
        PortUpdate {
            id,
            name,
            admin_up,
            vnic_type,
            port_security,
            security_group_ids,
        } => network::port_update(x, id, name, admin_up, vnic_type, port_security, security_group_ids).await?,
        PortSetSecurityGroups { id, security_group_ids } => {
            network::port_set_security_groups(x, id, security_group_ids).await?
        }
        PortDelete { id } => network::port_delete(x, id).await?,
        RouterCreate {
            name,
            admin_up,
            external_network_id,
            qos_policy_id,
        } => router::router_create(x, name, admin_up, external_network_id, qos_policy_id).await?,
        AdminRouterCreate(spec) => router::admin_router_create(x, spec).await?,
        RouterUpdate {
            id,
            name,
            admin_up,
            snat,
            distributed,
            ha,
        } => router::router_update(x, id, name, admin_up, snat, distributed, ha).await?,
        RouterSetGateway { id, network_id } => router::router_set_gateway(x, id, network_id).await?,
        RouterClearGateway { id } => router::router_clear_gateway(x, id).await?,
        RouterAddInterface { id, subnet_id } => router::router_add_interface(x, id, subnet_id).await?,
        RouterRemoveInterface { id, port_id } => router::router_remove_interface(x, id, port_id).await?,
        RouterAddRoute {
            id,
            destination,
            nexthop,
        } => router::router_add_route(x, id, destination, nexthop).await?,
        RouterRemoveRoute {
            id,
            destination,
            nexthop,
        } => router::router_remove_route(x, id, destination, nexthop).await?,
        RouterSetRoutes { id, routes } => router::router_set_routes(x, id, routes).await?,
        RouterDelete { id } => router::router_delete(x, id).await?,
        // The SDK has no trunk endpoints (only their tags): JSON requests.
        TrunkCreate {
            name,
            description,
            port_id,
            admin_up,
            subports,
        } => network::trunk_create(x, name, description, port_id, admin_up, subports).await?,
        TrunkUpdate {
            id,
            name,
            description,
            admin_up,
        } => network::trunk_update(x, id, name, description, admin_up).await?,
        TrunkDelete { id } => network::trunk_delete(x, id).await?,
        TrunkAddSubports { id, subports } => network::trunk_add_subports(x, id, subports).await?,
        TrunkRemoveSubports { id, port_ids } => network::trunk_remove_subports(x, id, port_ids).await?,
        // Not in the SDK (`openstack-sdk-network` has no `fwaas` module): JSON requests.
        FirewallRuleCreate(spec) => firewall::firewall_rule_create(x, spec).await?,
        FirewallRuleUpdate { id, spec } => firewall::firewall_rule_update(x, id, spec).await?,
        FirewallRuleDelete { id } => firewall::firewall_rule_delete(x, id).await?,
        FirewallPolicyCreate {
            name,
            description,
            rule_ids,
            audited,
        } => firewall::firewall_policy_create(x, name, description, rule_ids, audited).await?,
        FirewallPolicyUpdate {
            id,
            name,
            description,
            audited,
        } => firewall::firewall_policy_update(x, id, name, description, audited).await?,
        FirewallPolicyDelete { id } => firewall::firewall_policy_delete(x, id).await?,
        FirewallPolicyInsertRule {
            id,
            rule_id,
            before_id,
            after_id,
        } => firewall::firewall_policy_insert_rule(x, id, rule_id, before_id, after_id).await?,
        FirewallPolicyRemoveRule { id, rule_id } => firewall::firewall_policy_remove_rule(x, id, rule_id).await?,
        FirewallGroupCreate {
            name,
            description,
            ingress_policy_id,
            egress_policy_id,
            port_ids,
            admin_up,
        } => {
            firewall::firewall_group_create(
                x,
                name,
                description,
                ingress_policy_id,
                egress_policy_id,
                port_ids,
                admin_up,
            )
            .await?
        }
        FirewallGroupUpdate {
            id,
            name,
            description,
            ingress_policy_id,
            egress_policy_id,
            admin_up,
        } => {
            firewall::firewall_group_update(x, id, name, description, ingress_policy_id, egress_policy_id, admin_up)
                .await?
        }
        FirewallGroupSetPorts { id, port_ids } => firewall::firewall_group_set_ports(x, id, port_ids).await?,
        FirewallGroupDelete { id } => firewall::firewall_group_delete(x, id).await?,
        QosPolicyCreate {
            name,
            description,
            shared,
        } => qos::qos_policy_create(x, name, description, shared).await?,
        QosPolicyUpdate {
            id,
            name,
            description,
            shared,
        } => qos::qos_policy_update(x, id, name, description, shared).await?,
        QosPolicyDelete { id } => qos::qos_policy_delete(x, id).await?,
        // Rules: one JSON path for the four types (`…/<type>_rules`), the body
        // built and checked by `QosRuleSpec::body`.
        QosRuleCreate { policy_id, spec } => qos::qos_rule_create(x, policy_id, spec).await?,
        QosRuleUpdate { policy_id, id, spec } => qos::qos_rule_update(x, policy_id, id, spec).await?,
        QosRuleDelete { policy_id, kind, id } => qos::qos_rule_delete(x, policy_id, kind, id).await?,
        SecretCreate(spec) => keymgr::secret_create(x, spec).await?,
        SecretDelete { id } => keymgr::secret_delete(x, id).await?,
        ContainerCreate {
            name,
            container_type,
            secrets,
        } => keymgr::container_create(x, name, container_type, secrets).await?,
        ContainerDelete { id } => keymgr::container_delete(x, id).await?,
        SecretAclSet {
            id,
            project_access,
            users,
        } => keymgr::secret_acl_set(x, id, project_access, users).await?,
        SecretAclDelete { id } => keymgr::secret_acl_delete(x, id).await?,
        ContainerAclSet {
            id,
            project_access,
            users,
        } => keymgr::container_acl_set(x, id, project_access, users).await?,
        ContainerAclDelete { id } => keymgr::container_acl_delete(x, id).await?,
        StackCheck { name, id } => heat::stack_check(x, name, id).await?,
        StackSuspend { name, id } => heat::stack_suspend(x, name, id).await?,
        StackResume { name, id } => heat::stack_resume(x, name, id).await?,
        StackDelete { name, id } => heat::stack_delete(x, name, id).await?,
        StackCreate(spec) => heat::stack_create(x, spec).await?,
        StackUpdate { id, spec } => heat::stack_update(x, id, spec).await?,
        // The body is built and checked by `AlarmSpec::body`, as in the dialog.
        AlarmCreate(spec) => alarm::alarm_create(x, spec).await?,
        AlarmUpdate { id, spec } => alarm::alarm_update(x, id, spec).await?,
        PortForwardingCreate(spec) => network::port_forwarding_create(x, spec).await?,
        PortForwardingUpdate { id, spec } => network::port_forwarding_update(x, id, spec).await?,
        PortForwardingDelete { floating_ip_id, id } => network::port_forwarding_delete(x, floating_ip_id, id).await?,
        LoadBalancerCreate(spec) => octavia::load_balancer_create(x, spec).await?,
        LoadBalancerUpdate {
            id,
            name,
            description,
            admin_up,
        } => octavia::load_balancer_update(x, id, name, description, admin_up).await?,
        ListenerCreate {
            loadbalancer_id,
            spec,
            default_pool_id,
        } => octavia::listener_create(x, loadbalancer_id, spec, default_pool_id).await?,
        // JSON: a cleared default pool must be sent as `null`, which the SDK cannot express.
        // Cleared options likewise: `null` timeouts, `{}` headers and `[]` CIDRs are
        // Octavia's "back to the default" (roadmap 54).
        ListenerUpdate {
            id,
            name,
            description,
            connection_limit,
            default_pool_id,
            admin_up,
            protocol,
            options,
            tls,
        } => {
            octavia::listener_update(
                x,
                id,
                name,
                description,
                connection_limit,
                default_pool_id,
                admin_up,
                protocol,
                options,
                tls,
            )
            .await?
        }
        ListenerDelete { id } => octavia::listener_delete(x, id).await?,
        PoolCreate {
            loadbalancer_id,
            listener_id,
            spec,
        } => octavia::pool_create(x, loadbalancer_id, listener_id, spec).await?,
        // JSON: removing persistence means `"session_persistence": null`.
        PoolUpdate {
            id,
            name,
            description,
            algorithm,
            session_persistence,
            cookie_name,
            admin_up,
            protocol,
            tls,
        } => {
            octavia::pool_update(
                x,
                id,
                name,
                description,
                algorithm,
                session_persistence,
                cookie_name,
                admin_up,
                protocol,
                tls,
            )
            .await?
        }
        PoolDelete { id } => octavia::pool_delete(x, id).await?,
        MemberCreate { pool_id, spec } => octavia::member_create(x, pool_id, spec).await?,
        MemberUpdate {
            pool_id,
            id,
            name,
            weight,
            backup,
            admin_up,
            monitor_address,
            monitor_port,
        } => {
            octavia::member_update(
                x,
                pool_id,
                id,
                name,
                weight,
                backup,
                admin_up,
                monitor_address,
                monitor_port,
            )
            .await?
        }
        AppCredentialCreate(spec) => identity::app_credential_create(x, spec).await?,
        ProjectCreate {
            name,
            description,
            enabled,
            domain_id,
        } => identity::project_create(x, name, description, enabled, domain_id).await?,
        ProjectUpdate {
            id,
            name,
            description,
            enabled,
        } => identity::project_update(x, id, name, description, enabled).await?,
        ProjectDelete { id } => identity::project_delete(x, id).await?,
        ProjectRoleGrant {
            project_id,
            user_id,
            group_id,
            role_id,
        }
        | ProjectRoleRevoke {
            project_id,
            user_id,
            group_id,
            role_id,
        } => identity::project_role_grant_or_revoke(x, project_id, user_id, group_id, role_id, action).await?,
        UserCreate(spec) => identity::user_create(x, spec).await?,
        UserUpdate {
            id,
            name,
            email,
            description,
            project_id,
        } => identity::user_update(x, id, name, email, description, project_id).await?,
        UserSetEnabled { id, enabled } => identity::user_set_enabled(x, id, enabled).await?,
        UserSetPassword { id, password } => identity::user_set_password(x, id, password).await?,
        UserDelete { id } => identity::user_delete(x, id).await?,
        GroupCreate {
            name,
            description,
            domain_id,
        } => identity::group_create(x, name, description, domain_id).await?,
        GroupUpdate { id, name, description } => identity::group_update(x, id, name, description).await?,
        GroupDelete { id } => identity::group_delete(x, id).await?,
        GroupAddUser { group_id, user_id } => identity::group_add_user(x, group_id, user_id).await?,
        GroupRemoveUser { group_id, user_id } => identity::group_remove_user(x, group_id, user_id).await?,
        RoleCreate { name } => identity::role_create(x, name).await?,
        RoleUpdate { id, name } => identity::role_update(x, id, name).await?,
        RoleDelete { id } => identity::role_delete(x, id).await?,
        AppCredentialDelete { user_id, id } => identity::app_credential_delete(x, user_id, id).await?,
        PoolSetMembers { pool_id, members } => octavia::pool_set_members(x, pool_id, members).await?,
        MemberDelete { pool_id, id } => octavia::member_delete(x, pool_id, id).await?,
        MonitorCreate { pool_id, spec } => octavia::monitor_create(x, pool_id, spec).await?,
        MonitorUpdate { id, spec, admin_up } => octavia::monitor_update(x, id, spec, admin_up).await?,
        MonitorDelete { id } => octavia::monitor_delete(x, id).await?,
        L7PolicyCreate { listener_id, spec } => l7::l7_policy_create(x, listener_id, spec).await?,
        L7PolicyUpdate { id, spec } => l7::l7_policy_update(x, id, spec).await?,
        L7PolicyDelete { id } => l7::l7_policy_delete(x, id).await?,
        L7RuleCreate { policy_id, spec } => l7::l7_rule_create(x, policy_id, spec).await?,
        L7RuleUpdate { policy_id, id, spec } => l7::l7_rule_update(x, policy_id, id, spec).await?,
        L7RuleDelete { policy_id, id } => l7::l7_rule_delete(x, policy_id, id).await?,
        LoadBalancerDelete { id, cascade } => octavia::load_balancer_delete(x, id, cascade).await?,
        SecurityGroupCreate { name, description } => network::security_group_create(x, name, description).await?,
        SecurityGroupUpdate { id, name, description } => {
            network::security_group_update(x, id, name, description).await?
        }
        SecurityGroupDelete { id } => network::security_group_delete(x, id).await?,
        RuleCreate(spec) => create_rule(x, spec).await?,
        RuleDelete { id } => network::rule_delete(x, id).await?,
        FloatingIpAllocate {
            network_id,
            description,
            port_id,
            qos_policy_id,
        } => network::floating_ip_allocate(x, network_id, description, port_id, qos_policy_id).await?,
        AdminFloatingIpAllocate {
            network_id,
            project_id,
            address,
            description,
        } => network::admin_floating_ip_allocate(x, network_id, project_id, address, description).await?,
        FloatingIpAssociate { id, port_id } => network::floating_ip_associate(x, id, port_id).await?,
        FloatingIpDisassociate { id } => network::floating_ip_disassociate(x, id).await?,
        FloatingIpRelease { id } => network::floating_ip_release(x, id).await?,
        Batch(items) => batch(x, items).await?,
    }
    Ok(())
}

/// Keystone revocation of the session's token on logout (roadmap 49):
/// `DELETE /v3/auth/tokens` with the token in `X-Subject-Token`. A write, so it
/// goes through `Exec` (sent only with `[writes] mode = on`) although it isn't
/// an `OsAction`: the browser must never be able to name a token. The preview
/// has no headers, so the token never shows up in it.
pub async fn revoke_token(x: &mut Exec<'_>, token: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::auth::token::delete as m;
    let mut subject =
        http::HeaderValue::from_str(token).map_err(|_| AppError::internal("token is not a valid header value"))?;
    subject.set_sensitive(true);
    x.send(
        m::Request::builder()
            .header(http::HeaderName::from_static("x-subject-token"), subject)
            .build()
            .map_err(build_err)?,
    )
    .await
}

/// Runs each item, recording its result instead of stopping at the first
/// failure, so one undeletable resource doesn't block the rest.
async fn batch(x: &mut Exec<'_>, items: &[(String, OsAction)]) -> R<()> {
    if items.is_empty() {
        return Err(AppError::invalid("nothing selected"));
    }
    if items.len() > MAX_BATCH {
        return Err(AppError::invalid(format!("at most {MAX_BATCH} items at once")));
    }
    if items.iter().any(|(_, a)| matches!(a, OsAction::Batch(_))) {
        return Err(AppError::invalid("batches cannot be nested"));
    }
    for (label, action) in items {
        // Boxed: `apply` is recursive through here.
        let r = Box::pin(apply(x, action)).await;
        x.results.push(ItemResult {
            label: label.clone(),
            error: r.err().map(|e| e.to_string()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    /// Roadmap 67: `assert_paths_checked` covers the actions the tests dry-run,
    /// so every variant must appear in the `mod tests` of one of this
    /// directory's files (roadmap 106 split them by service; every `.rs` file is
    /// read, so a new one needs no list either). The variant list comes from
    /// serde's error for an unknown variant: a new variant needs no list to be
    /// updated.
    #[test]
    fn every_action_has_a_dry_run_test() {
        let err = serde_json::from_value::<OsAction>(serde_json::json!({"NoSuchAction": null}))
            .unwrap_err()
            .to_string();
        let names: Vec<&str> = err
            .split("expected one of ")
            .nth(1)
            .expect(&err)
            .split('`')
            .skip(1)
            .step_by(2)
            .collect();
        assert!(names.len() > 100, "variants read from serde: {err}");
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/server/actions");
        let sources: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect();
        assert!(sources.len() > 10, "files read from {dir}");
        let tests: Vec<&str> = sources
            .iter()
            .filter_map(|src| src.find("\nmod tests").map(|i| &src[i..]))
            .collect();
        let used = |n: &str| {
            tests.iter().any(|tests| {
                tests
                    .match_indices(&format!("OsAction::{n}"))
                    .any(|(i, m)| !tests[i + m.len()..].starts_with(|c: char| c.is_alphanumeric()))
            })
        };
        let missing: Vec<&str> = names.into_iter().filter(|n| !used(n)).collect();
        assert!(
            missing.is_empty(),
            "OsAction variants without a dry-run test: {missing:?}"
        );
    }

    #[tokio::test]
    async fn logout_revocation_is_a_keystone_delete_that_never_shows_the_token() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        revoke_token(&mut x, "gAAAAAB-secret").await.unwrap();
        let r = x.requests();
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("DELETE", "/auth/tokens", "identity")
        );
        assert!(
            !format!("{r:?}").contains("gAAAAAB-secret"),
            "the token is only in a header, never in the preview"
        );
        let mut off = Exec::new(WriteMode::Off, None);
        assert!(revoke_token(&mut off, "t").await.is_err(), "writes off: nothing sent");
    }

    #[tokio::test]
    async fn batch_builds_one_request_per_item_in_order() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &deletes(&["a", "b", "c"])).await.unwrap();
        let out = x.outcome();
        let paths: Vec<_> = out
            .requests
            .iter()
            .map(|r| (r.method.as_str(), r.path.as_str()))
            .collect();
        assert_eq!(
            paths,
            [
                ("DELETE", "/volumes/a"),
                ("DELETE", "/volumes/b"),
                ("DELETE", "/volumes/c")
            ]
        );
        let labels: Vec<_> = out.results.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["vol a", "vol b", "vol c"]);
        assert!(out.results.iter().all(|r| r.error.is_none()));
    }

    #[tokio::test]
    async fn batch_rejects_empty_nested_oversized_and_off() {
        let nested = OsAction::Batch(vec![("x".into(), deletes(&["a"]))]);
        let too_many: Vec<String> = (0..=MAX_BATCH).map(|i| i.to_string()).collect();
        let too_many = deletes(&too_many.iter().map(String::as_str).collect::<Vec<_>>());
        for a in [OsAction::Batch(vec![]), nested, too_many] {
            let mut x = Exec::new(WriteMode::DryRun, None);
            assert!(apply(&mut x, &a).await.is_err());
            assert!(x.outcome().requests.is_empty());
        }
        let mut x = Exec::new(WriteMode::Off, None);
        assert!(apply(&mut x, &deletes(&["a"])).await.is_err());
        assert!(x.outcome().requests.is_empty());
    }

    #[test]
    fn secret_values_are_wiped() {
        let mut v = json!({"secret": {"payload": "s3cr3t", "name": "keep"}, "auth": [{"password": "pw"}]});
        wipe_secrets(&mut v);
        assert_eq!(
            v,
            json!({"secret": {"payload": "", "name": "keep"}, "auth": [{"password": ""}]})
        );
    }

    #[test]
    fn created_ids_of_every_create_response() {
        assert_eq!(created_id(&json!({"id": "n1"})).as_deref(), Some("n1"));
        assert_eq!(
            created_id(&json!({"stack": {"id": "s1", "links": []}})).as_deref(),
            Some("s1")
        );
        assert_eq!(
            created_id(&json!({"secret_ref": "https://kms/v1/secrets/k1"})).as_deref(),
            Some("k1")
        );
        assert_eq!(
            created_id(&json!({"alarm_id": "a1", "name": "cpu"})).as_deref(),
            Some("a1"),
            "Aodh's create returns the alarm"
        );
        assert_eq!(created_id(&json!({"name": "x"})), None);
    }
}
