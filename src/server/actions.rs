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

use std::borrow::Cow;

use openstack_sdk::api::{ignore, QueryAsync, RestEndpoint};
use openstack_sdk::AsyncOpenStack;
use serde_json::{json, Value};

use super::sfe;
use crate::model::*;

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
    let body = ep
        .body()
        .map_err(sfe("serialise request body"))?
        .map(|(_, bytes)| match serde_json::from_slice::<Value>(&bytes) {
            Ok(mut v) => {
                mask_secrets(&mut v);
                serde_json::to_string_pretty(&v).unwrap_or_default()
            }
            Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
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
    pub fn new(method: http::Method, service: openstack_sdk::types::ServiceType, path: impl Into<String>, body: Option<Value>) -> Self {
        JsonRequest { method, path: path.into(), service, body, query: Vec::new(), headers: None }
    }
    /// A Barbican request (roadmap 55). The SDK has no Key Manager API, and
    /// catalogs list Barbican unversioned (`https://…:9311`), so paths carry
    /// `v1/` themselves; discovery is not run for it.
    pub fn key_manager(method: http::Method, path: impl std::fmt::Display, body: Option<Value>) -> Self {
        Self::new(method, openstack_sdk::types::ServiceType::Other("key-manager".into()), format!("v1/{path}"), body)
    }
    /// A Heat request (roadmap 62). The SDK has no Orchestration API. Heat's
    /// catalog URL is versioned and project-scoped (`…/v1/<project_id>`), and
    /// the SDK appends relative paths after it, so paths are plain `stacks`.
    pub fn orchestration(method: http::Method, path: impl Into<String>, body: Option<Value>) -> Self {
        Self::new(method, openstack_sdk::types::ServiceType::Other("orchestration".into()), path, body)
    }
    /// An Aodh request (roadmap 79). The SDK has no Alarming API, and catalogs
    /// list Aodh unversioned (`https://…:8042`), so paths carry `v2/` themselves;
    /// discovery is not run for it.
    pub fn alarming(method: http::Method, path: impl std::fmt::Display, body: Option<Value>) -> Self {
        Self::new(method, openstack_sdk::types::ServiceType::Other("alarming".into()), format!("v2/{path}"), body)
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
        Ok(self.body.as_ref().map(|b| ("application/json", serde_json::to_vec(b).expect("JSON value serialises"))))
    }
}

/// An SDK enum from its wire name (`"HTTP"`, `"ROUND_ROBIN"`…): every Octavia
/// module has its own copy of these enums, all deserialisable from the API's strings.
fn enum_of<T: serde::de::DeserializeOwned>(v: &str, what: &str) -> R<T> {
    serde_json::from_value(Value::String(v.to_string())).map_err(|_| AppError::invalid(format!("unknown {what}: {v}")))
}

/// Fills an SDK member builder (the load balancer create body, `member::create`
/// and the batch `member::replace` each have their own type) from a spec.
macro_rules! member_fields {
    ($b:expr, $m:expr) => {{
        let (b, m) = (&mut $b, $m);
        b.address(m.address.trim()).protocol_port(i32::from(m.port)).backup(m.backup).admin_state_up(m.admin_up);
        if let Some(v) = opt(&m.name) {
            b.name(v);
        }
        if let Some(v) = opt(&m.subnet_id) {
            b.subnet_id(v);
        }
        if let Some(w) = m.weight {
            b.weight(i32::from(w));
        }
        if let Some(v) = opt(&m.monitor_address) {
            b.monitor_address(v);
        }
        if let Some(p) = m.monitor_port {
            b.monitor_port(i32::from(p));
        }
    }};
}

/// Fills an SDK listener builder (the load balancer create body and
/// `listener::create` have their own types) with the *Advanced* options
/// (roadmap 54): only what differs from Octavia's defaults is sent.
macro_rules! listener_options {
    ($b:expr, $o:expr) => {{
        let (b, o): (_, &LbListenerOptions) = (&mut $b, $o);
        if let Some(t) = o.timeout_client_data {
            b.timeout_client_data(t as i32);
        }
        if let Some(t) = o.timeout_member_connect {
            b.timeout_member_connect(t as i32);
        }
        if let Some(t) = o.timeout_member_data {
            b.timeout_member_data(t as i32);
        }
        if let Some(t) = o.timeout_tcp_inspect {
            b.timeout_tcp_inspect(t as i32);
        }
        if !o.insert_headers.is_empty() {
            b.insert_headers(o.insert_headers.iter().map(|h| (h.as_str(), "true")));
        }
        if let Some(c) = o.allowed_cidrs.as_ref().filter(|c| !c.is_empty()) {
            b.allowed_cidrs(c.iter().map(|x| Cow::from(x.trim().to_string())).collect::<Vec<_>>());
        }
        if let Some(c) = o.ciphers() {
            b.tls_ciphers(c);
        }
    }};
}

/// A `TERMINATED_HTTPS` listener's certificates and, when off, its admin state
/// (roadmap 54), on either SDK listener builder.
macro_rules! listener_certificates {
    ($b:expr, $l:expr) => {{
        let (b, l): (_, &LbListenerSpec) = (&mut $b, $l);
        if let Some(r) = opt(&l.default_tls_container_ref) {
            b.default_tls_container_ref(r);
        }
        if !l.admin_up {
            b.admin_state_up(false);
        }
        let sni: Vec<Cow<str>> = l.sni_container_refs.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).map(|r| Cow::from(r.to_string())).collect();
        if !sni.is_empty() {
            b.sni_container_refs(sni);
        }
    }};
}

/// Pool re-encryption (roadmap 54): `tls_enabled` only when on, so older
/// Octavias (< 2.8) never see the key; ciphers only with it. Also the admin
/// state, when off.
macro_rules! pool_tls {
    ($b:expr, $p:expr) => {{
        let (b, p): (_, &LbPoolSpec) = (&mut $b, $p);
        if !p.admin_up {
            b.admin_state_up(false);
        }
        if p.tls_enabled {
            b.tls_enabled(true);
            if let Some(c) = opt(&p.tls_ciphers) {
                b.tls_ciphers(c);
            }
        }
    }};
}

/// *Edit Quotas* (roadmap 82): the changed limits of one service. JSON, not
/// the typed `quota_set::set_*` builders: their fields are fixed per release,
/// and only `values` (each a key of `service.keys()`) is sent. Nova gets no
/// microversion, so the keys 2.57 dropped (`injected_files`…) are accepted.
fn quota_update(service: QuotaService, project_id: &str, values: &std::collections::BTreeMap<String, i64>) -> R<JsonRequest> {
    use openstack_sdk::types::ServiceType;
    let project = non_empty(project_id, "project")?;
    if values.is_empty() {
        return Err(AppError::invalid("no quota changed"));
    }
    if let Some((k, v)) = values.iter().find(|(k, v)| !service.keys().contains(&k.as_str()) || **v < -1) {
        return Err(AppError::invalid(format!("quota {k} = {v} is not allowed")));
    }
    let (ty, path, wrap) = match service {
        QuotaService::Compute => (ServiceType::Compute, format!("os-quota-sets/{project}"), "quota_set"),
        QuotaService::Volume => (ServiceType::BlockStorage, format!("os-quota-sets/{project}"), "quota_set"),
        QuotaService::Network => (ServiceType::Network, format!("quotas/{project}"), "quota"),
        QuotaService::LoadBalancer => (ServiceType::LoadBalancer, format!("lbaas/quotas/{project}"), "quota"),
        // Barbican updates only the keys sent; the others keep their value,
        // or the cloud's default when the project had none configured.
        QuotaService::KeyManager => {
            return Ok(JsonRequest::key_manager(http::Method::PUT, format!("project-quotas/{project}"), Some(json!({ "project_quotas": values }))));
        }
    };
    Ok(JsonRequest::new(http::Method::PUT, ty, path, Some(json!({ wrap: values }))))
}

/// *Update Defaults*: the changed limits of the `default` quota class, keys
/// checked as in [`quota_update`]. Only Nova and Cinder have quota classes.
fn default_quota_update(service: QuotaService, values: &std::collections::BTreeMap<String, i64>) -> R<JsonRequest> {
    use openstack_sdk::types::ServiceType;
    let ty = match service {
        QuotaService::Compute => ServiceType::Compute,
        QuotaService::Volume => ServiceType::BlockStorage,
        other => return Err(AppError::invalid(format!("{other:?} has no default quota class"))),
    };
    if values.is_empty() {
        return Err(AppError::invalid("no quota changed"));
    }
    if let Some((k, v)) = values.iter().find(|(k, v)| !service.keys().contains(&k.as_str()) || **v < -1) {
        return Err(AppError::invalid(format!("quota {k} = {v} is not allowed")));
    }
    Ok(JsonRequest::new(http::Method::PUT, ty, "os-quota-class-sets/default", Some(json!({ "quota_class_set": values }))))
}

/// A Glance metadata definitions request. Raw: the SDK's namespace builders
/// have fixed fields, so an imported definition's properties and objects
/// would be lost.
fn metadef(method: http::Method, path: String, body: Option<Value>) -> JsonRequest {
    JsonRequest::new(method, openstack_sdk::types::ServiceType::Image, path, body)
}

/// *Import Namespace*'s body: `definition` must be a JSON object with a
/// `namespace` (a valid path segment, as it names the namespace afterwards);
/// the form's *Public* and *Protected* replace whatever it says.
fn metadef_definition(definition: &str, public: bool, protected: bool) -> R<Value> {
    let mut v: Value = serde_json::from_str(definition).map_err(|e| AppError::invalid(format!("the definition is not valid JSON: {e}")))?;
    let Some(obj) = v.as_object_mut() else { return Err(AppError::invalid("the definition must be a JSON object")) };
    let ns = obj.get("namespace").and_then(Value::as_str).unwrap_or("").to_string();
    crate::model::check_id(&ns).map_err(|_| AppError::invalid("the definition needs a \"namespace\" name"))?;
    obj.insert("visibility".into(), json!(if public { "public" } else { "private" }));
    obj.insert("protected".into(), json!(protected));
    Ok(v)
}

/// *Delete Group*. A JSON request because the SDK's `group::delete_313` posts
/// to `groups/action`, without the group's ID. Cinder needs volume 3.13 for
/// group actions, and a JSON request negotiates no microversion: set it here.
fn volume_group_delete(id: &str, delete_volumes: bool) -> R<JsonRequest> {
    let mut r = JsonRequest::new(
        http::Method::POST,
        openstack_sdk::types::ServiceType::BlockStorage,
        format!("groups/{}/action", non_empty(id, "group")?),
        Some(json!({"delete": {"delete-volumes": delete_volumes}})),
    );
    let mut h = http::HeaderMap::new();
    h.insert("openstack-api-version", http::HeaderValue::from_static("volume 3.13"));
    r.headers = Some(h);
    Ok(r)
}

fn octavia(method: http::Method, path: String, body: Value) -> JsonRequest {
    JsonRequest::new(method, openstack_sdk::types::ServiceType::LoadBalancer, path, Some(body))
}

/// `{"port_forwarding": {...}}`: single ports as integers, ranges as `"a:b"`
/// under the `*_range` keys (port-ranges extension).
fn port_forwarding_body(s: &PortForwardingSpec) -> R<Value> {
    let (internal, external) = s.ports().map_err(AppError::invalid)?;
    let mut pf = serde_json::Map::new();
    pf.insert("protocol".into(), json!(s.protocol));
    pf.insert("internal_port_id".into(), json!(non_empty(&s.internal_port_id, "internal port")?));
    pf.insert("internal_ip_address".into(), json!(s.internal_ip.trim()));
    for (key, p) in [("internal_port", internal), ("external_port", external)] {
        match p {
            PortSpec::One(n) => pf.insert(key.into(), json!(n)),
            PortSpec::Range(..) => pf.insert(format!("{key}_range"), json!(p.to_string())),
        };
    }
    if let Some(d) = opt(&s.description) {
        pf.insert("description".into(), json!(d));
    }
    Ok(json!({ "port_forwarding": pf }))
}

/// Access rules typed as Horizon accepts them: a JSON or YAML list of
/// `{service, method, path}` (YAML parses JSON too). Methods are upper-cased.
pub fn parse_access_rules(text: &str) -> Result<Vec<AccessRule>, String> {
    use yaml_rust2::{Yaml, YamlLoader};
    let docs = YamlLoader::load_from_str(text).map_err(|e| format!("access rules are not valid JSON or YAML: {e}"))?;
    let Some(Yaml::Array(items)) = docs.into_iter().next() else {
        return Err("access rules must be a list of rules".into());
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let get = |k: &str| item[k].as_str().map(str::trim).unwrap_or_default().to_string();
            if !matches!(item, Yaml::Hash(_)) {
                return Err(format!("access rule {}: expected service, method and path", i + 1));
            }
            let rule = AccessRule { service: get("service"), method: get("method").to_ascii_uppercase(), path: get("path") };
            rule.check().map_err(|e| format!("rule {}: {e}", i + 1))?;
            Ok(rule)
        })
        .collect()
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
        Self { mode, os, requests: Vec::new(), secret: None, results: Vec::new(), created: None }
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
        ActionOutcome { mode: self.mode, requests: self.requests, secret: self.secret, results: self.results, created: self.created }
    }

    /// Masks the values of these parameters (hidden, `NoEcho`) in the last
    /// request's preview: their names are the template's, not [`SECRET_KEYS`].
    pub(super) fn hide_params(&mut self, names: &[String]) {
        let Some(body) = self.requests.last_mut().and_then(|r| r.body.as_mut()) else { return };
        let Ok(mut v) = serde_json::from_str::<Value>(body) else { return };
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
            WriteMode::On => ep.query_async(self.client()?).await.map(Some).map_err(sfe("OpenStack rejected the request")),
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
                let id = created_id(&v).ok_or_else(|| AppError::internal(format!("{what} created but no id returned")))?;
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
        .or_else(|| ["secret_ref", "container_ref"].iter().find_map(|k| v.get(*k)?.as_str()).map(super::mapping::ref_id))
}

/// Barbican's secret types (`POST /v1/secrets` `secret_type`).
pub const SECRET_TYPES: [&str; 6] = ["symmetric", "public", "private", "passphrase", "certificate", "opaque"];

/// `POST /v1/secrets`. Barbican refuses a content type without a payload, so
/// both go together; `application/octet-stream` payloads are base64
/// (`payload_content_encoding`), checked here so a typo fails before sending.
/// `stacks/{name}/{id}`: Heat needs both (`stacks/{id}` alone is a redirect).
fn stack_path(name: &str, id: &str) -> R<String> {
    Ok(format!("stacks/{}/{}", non_empty(name, "stack name")?, non_empty(id, "stack")?))
}

/// The template part of a Heat body (`template` or `template_url`, `environment`),
/// shared with `validate` and `preview` (`server/heat.rs`).
pub fn template_body(src: &TemplateSource) -> R<serde_json::Map<String, Value>> {
    let mut m = serde_json::Map::new();
    match src.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) if u.starts_with("https://") || u.starts_with("http://") => {
            m.insert("template_url".into(), json!(u));
        }
        Some(_) => return Err(AppError::invalid("the template URL must start with http:// or https://")),
        None if src.template.trim().is_empty() => return Err(AppError::invalid("a template is required")),
        // Heat accepts the text as a string and parses YAML or JSON itself.
        None => {
            m.insert("template".into(), json!(src.template));
        }
    }
    let env = src.environment.as_deref().filter(|e| !e.trim().is_empty());
    if src.template.len() + env.map_or(0, str::len) > MAX_TEMPLATE_BYTES {
        return Err(AppError::invalid(format!("the template and environment exceed {} KiB", MAX_TEMPLATE_BYTES / 1024)));
    }
    if let Some(e) = env {
        m.insert("environment".into(), json!(e));
    }
    Ok(m)
}

/// `POST stacks` (`create`, with `stack_name`) or `PATCH stacks/{name}/{id}`.
pub fn stack_body(spec: &StackSpec, create: bool) -> R<Value> {
    if !valid_stack_name(&spec.name) {
        return Err(AppError::invalid("a stack name starts with a letter and has only letters, digits, _, - and ."));
    }
    let mut m = template_body(&spec.source)?;
    if create {
        m.insert("stack_name".into(), json!(spec.name));
    }
    let params: serde_json::Map<String, Value> = spec.parameters.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    m.insert("parameters".into(), Value::Object(params));
    if let Some(t) = spec.timeout_mins {
        if t == 0 {
            return Err(AppError::invalid("the timeout must be at least one minute"));
        }
        m.insert("timeout_mins".into(), json!(t));
    }
    m.insert("disable_rollback".into(), json!(spec.disable_rollback));
    Ok(Value::Object(m))
}

/// A stack action (`check`, `suspend`, `resume`): `POST …/actions` with `{action: null}`.
fn stack_action(name: &str, id: &str, action: &str) -> R<JsonRequest> {
    let body = serde_json::json!({ action: null });
    Ok(JsonRequest::orchestration(http::Method::POST, format!("{}/actions", stack_path(name, id)?), Some(body)))
}

/// `PUT …/acl` replacing the whole `read` ACL (roadmap 75). User IDs go in the
/// body, but are checked like IDs: one per entry, no separators.
fn acl_put(path: &str, project_access: bool, users: &[String]) -> R<JsonRequest> {
    for u in users {
        check_id(u)?;
        if u.contains(',') || u.contains(char::is_whitespace) {
            return Err(AppError::invalid(format!("not a valid user ID: {u:?}")));
        }
    }
    let body = json!({ "read": { "project-access": project_access, "users": users } });
    Ok(JsonRequest::key_manager(http::Method::PUT, path, Some(body)))
}

fn secret_body(s: &SecretSpec) -> R<Value> {
    use base64::Engine;
    if !SECRET_TYPES.contains(&s.secret_type.as_str()) {
        return Err(AppError::invalid(format!("unknown secret type {}", s.secret_type)));
    }
    let mut b = serde_json::Map::new();
    b.insert("secret_type".into(), json!(s.secret_type));
    if let Some(n) = opt(&s.name) {
        b.insert("name".into(), json!(n));
    }
    if let Some(p) = s.payload.as_ref().filter(|p| !p.0.is_empty()) {
        b.insert("payload".into(), json!(p.0));
        b.insert("payload_content_type".into(), json!(s.payload_content_type));
        match s.payload_content_type.as_str() {
            "text/plain" => {}
            "application/octet-stream" => {
                let clean: String = p.0.split_whitespace().collect();
                let mut decoded = base64::engine::general_purpose::STANDARD.decode(&clean).map_err(|_| AppError::invalid("the payload is not valid base64"))?;
                zeroize::Zeroize::zeroize(&mut decoded);
                b.insert("payload".into(), json!(clean));
                b.insert("payload_content_encoding".into(), json!("base64"));
            }
            other => return Err(AppError::invalid(format!("unsupported payload content type {other}"))),
        }
    }
    if let Some(a) = opt(&s.algorithm) {
        b.insert("algorithm".into(), json!(a));
    }
    match s.bit_length {
        Some(n) if n <= 0 => return Err(AppError::invalid("the bit length must be positive")),
        Some(n) => {
            b.insert("bit_length".into(), json!(n));
        }
        None => {}
    }
    if let Some(m) = opt(&s.mode) {
        b.insert("mode".into(), json!(m));
    }
    if let Some(e) = opt(&s.expiration) {
        b.insert("expiration".into(), json!(e));
    }
    Ok(Value::Object(b))
}

/// The roles each container type takes (`None`: any, for `generic`), and the
/// ones it requires. Barbican's `certificate` is what Octavia's
/// `TERMINATED_HTTPS` listeners read.
pub fn container_roles(container_type: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match container_type {
        "certificate" => Some((&["certificate", "private_key", "private_key_passphrase", "intermediates"], &["certificate"])),
        "rsa" => Some((&["public_key", "private_key", "private_key_passphrase"], &["public_key", "private_key"])),
        _ => None,
    }
}

/// `POST /v1/containers`: secrets by role, as refs. Each ref must be an
/// http(s) URL ending in a safe ID (roadmap 48); roles of a typed container are
/// checked and each used once.
fn container_body(name: Option<&str>, container_type: &str, secrets: &[(String, String)]) -> R<Value> {
    if !["generic", "certificate", "rsa"].contains(&container_type) {
        return Err(AppError::invalid(format!("unknown container type {container_type}")));
    }
    let mut refs = Vec::new();
    for (role, r) in secrets {
        let role = non_empty(role, "role")?;
        // `…/secrets/{id}` and nothing after the ID.
        let id = r.trim_end_matches('/').rsplit_once("/secrets/").map(|(_, id)| id).filter(|id| !id.contains('/'));
        match id {
            Some(id) if r.starts_with("https://") || r.starts_with("http://") => check_id(id)?,
            _ => return Err(AppError::invalid(format!("not a secret reference: {r}"))),
        }
        refs.push(json!({"name": role, "secret_ref": r}));
    }
    if let Some((allowed, required)) = container_roles(container_type) {
        let roles: Vec<&str> = secrets.iter().map(|(r, _)| r.trim()).collect();
        if let Some(r) = roles.iter().find(|r| !allowed.contains(r)) {
            return Err(AppError::invalid(format!("a {container_type} container has no {r} role")));
        }
        if let Some(r) = required.iter().find(|r| !roles.contains(r)) {
            return Err(AppError::invalid(format!("a {container_type} container needs a {r}")));
        }
        if (1..roles.len()).any(|i| roles[..i].contains(&roles[i])) {
            return Err(AppError::invalid("each role takes one secret"));
        }
    }
    let mut b = json!({"type": container_type, "secret_refs": refs});
    if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
        b["name"] = json!(n);
    }
    Ok(b)
}

/// Glance JSON-Patch for [`ImageChanges`]: `replace` for core fields, `add`
/// (adds or replaces) and `remove` for custom properties. Nothing changed is
/// an error, so no empty PATCH is sent.
fn image_patch(c: &ImageChanges) -> R<Vec<Value>> {
    let mut ops = Vec::new();
    let mut replace = |path: &str, value: Value| ops.push(json!({"op": "replace", "path": path, "value": value}));
    if let Some(n) = &c.name {
        replace("/name", json!(non_empty(n, "name")?));
    }
    if let Some(v) = &c.visibility {
        if !["public", "private", "shared", "community"].contains(&v.as_str()) {
            return Err(AppError::invalid("invalid visibility"));
        }
        replace("/visibility", json!(v));
    }
    if let Some(p) = c.protected {
        replace("/protected", json!(p));
    }
    if let Some(d) = c.min_disk_gb {
        replace("/min_disk", json!(d));
    }
    if let Some(r) = c.min_ram_mb {
        replace("/min_ram", json!(r));
    }
    if let Some(f) = &c.disk_format {
        replace("/disk_format", json!(non_empty(f, "disk format")?));
    }
    let set = checked_metadata(&c.set)?;
    for k in c.remove.iter().map(|k| k.trim()) {
        if k.is_empty() || set.iter().any(|(s, _)| s == k) {
            return Err(AppError::invalid(format!("cannot both set and remove {k:?}")));
        }
        ops.push(json!({"op": "remove", "path": format!("/{}", pointer_escape(k)?)}));
    }
    for (k, v) in &set {
        ops.push(json!({"op": "add", "path": format!("/{}", pointer_escape(k)?), "value": v}));
    }
    if ops.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    Ok(ops)
}

/// A specs edit (`set` pairs, `unset` keys): refused when empty, `set`
/// checked like metadata.
fn spec_changes(set: &[(String, String)], unset: &[String]) -> R<Vec<(String, String)>> {
    if set.is_empty() && unset.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    checked_metadata(set)
}

/// `status` if it is one of `allowed` (Horizon's choices): Cinder stores any
/// string `os-reset_status` is given, so a typo would strand the resource.
fn known_status<'s>(status: &'s str, allowed: &[&str]) -> R<&'s str> {
    let s = status.trim();
    if allowed.contains(&s) { Ok(s) } else { Err(AppError::invalid(format!("unknown status {s}"))) }
}

fn checked_metadata(pairs: &[(String, String)]) -> R<Vec<(String, String)>> {
    validate_metadata(pairs).map_err(AppError::invalid)
}

/// A custom image property as a JSON-Pointer segment (RFC 6901); Glance's own
/// fields can't be touched through the metadata editor.
fn pointer_escape(key: &str) -> R<String> {
    if super::mapping::is_image_core_key(key) {
        return Err(AppError::invalid(format!("{key} is not a custom property")));
    }
    Ok(key.replace('~', "~0").replace('/', "~1"))
}

/// One of the SDK's image enums (disk or container format, visibility) from its Glance name.
fn image_enum<T: serde::de::DeserializeOwned>(what: &str, v: &str) -> R<T> {
    serde_json::from_value(json!(v.trim())).map_err(|_| AppError::invalid(format!("unknown {what} {v:?}")))
}

/// The `uri` of a `web-download` import: `http(s)` only (Glance's default policy refuses
/// other schemes and private addresses anyway), without whitespace.
fn image_source_url(url: &str) -> R<&str> {
    let u = url.trim();
    let host = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or("");
    if host.is_empty() || host.starts_with('/') || u.chars().any(char::is_whitespace) {
        return Err(AppError::invalid("the image URL must be http:// or https://"));
    }
    Ok(u)
}

fn opt(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// A Neutron FWaaS v2 request (`fwaas/{path}`, roadmap 93).
fn fwaas(method: http::Method, path: String, body: Option<Value>) -> JsonRequest {
    JsonRequest::new(method, openstack_sdk::types::ServiceType::Network, format!("fwaas/{path}"), body)
}

/// A Nova `servers/{id}/action` request, optionally at a microversion
/// (`"compute 2.30"`).
fn nova_action(id: &str, body: Value, microversion: Option<&'static str>) -> R<JsonRequest> {
    let mut r = JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::Compute, format!("servers/{}/action", non_empty(id, "instance")?), Some(body));
    if let Some(mv) = microversion {
        let mut h = http::HeaderMap::new();
        // Headers replace the SDK's defaults, so `Accept` is set again.
        h.insert(http::header::ACCEPT, http::HeaderValue::from_static("application/json"));
        h.insert(http::HeaderName::from_static("openstack-api-version"), http::HeaderValue::from_static(mv));
        r.headers = Some(h);
    }
    Ok(r)
}

/// `addTenantAccess` / `removeTenantAccess` of a flavor, one action per project.
async fn flavor_access(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::flavor::{add_tenant_access as a, remove_tenant_access as r};
    for p in add {
        let body = a::AddTenantAccessBuilder::default().tenant(non_empty(p, "project")?).build().map_err(build_err)?;
        x.send(a::Request::builder().id(id).add_tenant_access(body).build().map_err(build_err)?).await?;
    }
    for p in remove {
        let body = r::RemoveTenantAccessBuilder::default().tenant(non_empty(p, "project")?).build().map_err(build_err)?;
        x.send(r::Request::builder().id(id).remove_tenant_access(body).build().map_err(build_err)?).await?;
    }
    Ok(())
}

/// `add_host` / `remove_host` of an aggregate, one action per host.
async fn aggregate_hosts(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::{add_host as a, remove_host as r};
    for h in add {
        let body = a::AddHostBuilder::default().host(non_empty(h, "host")?).build().map_err(build_err)?;
        x.send(a::Request::builder().id(id).add_host(body).build().map_err(build_err)?).await?;
    }
    for h in remove {
        let body = r::RemoveHostBuilder::default().host(non_empty(h, "host")?).build().map_err(build_err)?;
        x.send(r::Request::builder().id(id).remove_host(body).build().map_err(build_err)?).await?;
    }
    Ok(())
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
    i32::try_from(n).ok().filter(|n| *n >= 1).ok_or_else(|| AppError::invalid(format!("{field} must be at least 1")))
}

/// A size that may be 0 (disks, swap), as Nova's `int32`.
fn at_least_zero(n: i64, field: &str) -> R<i32> {
    i32::try_from(n).ok().filter(|n| *n >= 0).ok_or_else(|| AppError::invalid(format!("{field} can't be negative")))
}

fn non_empty<'s>(s: &'s str, field: &str) -> R<&'s str> {
    let t = s.trim();
    if t.is_empty() {
        Err(AppError::invalid(format!("{field} is required")))
    } else {
        Ok(t)
    }
}

/// Runs one action. `Off` mode is rejected up-front.
pub async fn apply(x: &mut Exec<'_>, action: &OsAction) -> R<()> {
    if x.mode == WriteMode::Off {
        return Err(AppError::invalid("writes are disabled on this Moonlight instance"));
    }
    use OsAction::*;
    match action {
        QuotaUpdate { service, project_id, values } => x.send(quota_update(*service, project_id, values)?).await?,
        DefaultQuotaUpdate { service, values } => x.send(default_quota_update(*service, values)?).await?,
        MetadefNamespaceImport { definition, public, protected } => {
            let body = metadef_definition(definition, *public, *protected)?;
            x.send(metadef(http::Method::POST, "metadefs/namespaces".into(), Some(body))).await?;
        }
        MetadefNamespaceUpdate { id, display_name, description, public, protected } => {
            let mut body = json!({"namespace": id, "visibility": if *public { "public" } else { "private" }, "protected": protected});
            if let Some(n) = opt(display_name) {
                body["display_name"] = json!(n);
            }
            if let Some(d) = opt(description) {
                body["description"] = json!(d);
            }
            x.send(metadef(http::Method::PUT, format!("metadefs/namespaces/{id}"), Some(body))).await?;
        }
        MetadefNamespaceDelete { id } => x.send(metadef(http::Method::DELETE, format!("metadefs/namespaces/{id}"), None)).await?,
        MetadefNamespaceSetTypes { id, add_type_ids, remove_type_ids } => {
            if add_type_ids.is_empty() && remove_type_ids.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            for name in add_type_ids {
                x.send(metadef(http::Method::POST, format!("metadefs/namespaces/{id}/resource_types"), Some(json!({ "name": name })))).await?;
            }
            for name in remove_type_ids {
                x.send(metadef(http::Method::DELETE, format!("metadefs/namespaces/{id}/resource_types/{name}"), None)).await?;
            }
        }
        // ---------------------------------------------------------------- Compute
        ServerLaunch(spec) => launch(x, spec).await?,
        ServerUpdate { id, name } => {
            use openstack_sdk::api::compute::v2::server::set_21 as m;
            let body = m::ServerBuilder::default().name(non_empty(name, "name")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).server(body).build().map_err(build_err)?).await?;
        }
        ServerStart { id } => {
            use openstack_sdk::api::compute::v2::server::os_start as m;
            x.send(m::Request::builder().id(id.as_str()).os_start(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerStop { id } => {
            use openstack_sdk::api::compute::v2::server::os_stop as m;
            x.send(m::Request::builder().id(id.as_str()).os_stop(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerReboot { id, kind } => {
            use openstack_sdk::api::compute::v2::server::reboot as m;
            let t = match kind {
                RebootKind::Soft => m::Type::Soft,
                RebootKind::Hard => m::Type::Hard,
            };
            let body = m::RebootBuilder::default()._type(t).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).reboot(body).build().map_err(build_err)?).await?;
        }
        ServerPause { id } => {
            use openstack_sdk::api::compute::v2::server::pause as m;
            x.send(m::Request::builder().id(id.as_str()).pause(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerUnpause { id } => {
            use openstack_sdk::api::compute::v2::server::unpause as m;
            x.send(m::Request::builder().id(id.as_str()).unpause(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerSuspend { id } => {
            use openstack_sdk::api::compute::v2::server::suspend as m;
            x.send(m::Request::builder().id(id.as_str()).suspend(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerResume { id } => {
            use openstack_sdk::api::compute::v2::server::resume as m;
            x.send(m::Request::builder().id(id.as_str()).resume(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerLock { id } => {
            use openstack_sdk::api::compute::v2::server::lock_21 as m;
            x.send(m::Request::builder().id(id.as_str()).lock(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerUnlock { id } => {
            use openstack_sdk::api::compute::v2::server::unlock_21 as m;
            x.send(m::Request::builder().id(id.as_str()).unlock(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerSnapshot { id, name } => {
            use openstack_sdk::api::compute::v2::server::create_image_21 as m;
            let body = m::CreateImageBuilder::default().name(non_empty(name, "name")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).create_image(body).build().map_err(build_err)?).await?;
        }
        ServerDelete { id } => {
            use openstack_sdk::api::compute::v2::server::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        ServerSetMetadata { id, metadata } => {
            use openstack_sdk::api::compute::v2::server::metadata::replace as m;
            let md = checked_metadata(metadata)?;
            x.send(m::Request::builder().server_id(id.as_str()).metadata(md.into_iter()).build().map_err(build_err)?).await?;
        }
        ServerSetSecurityGroups { id, add, remove } => {
            use openstack_sdk::api::compute::v2::server::{add_security_group as a, remove_security_group as r};
            if add.is_empty() && remove.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            // One Nova action per group, each recorded like a batch item, so a
            // failure (e.g. a port with port security off) names its group.
            for name in add {
                let body = a::AddSecurityGroupBuilder::default().name(non_empty(name, "security group")?).build().map_err(build_err)?;
                let res = x.send(a::Request::builder().id(id.as_str()).add_security_group(body).build().map_err(build_err)?).await;
                x.results.push(ItemResult { label: format!("+ {name}"), error: res.err().map(|e| e.to_string()) });
            }
            for name in remove {
                let body = r::RemoveSecurityGroupBuilder::default().name(non_empty(name, "security group")?).build().map_err(build_err)?;
                let res = x.send(r::Request::builder().id(id.as_str()).remove_security_group(body).build().map_err(build_err)?).await;
                x.results.push(ItemResult { label: format!("- {name}"), error: res.err().map(|e| e.to_string()) });
            }
        }
        ServerAttachVolume { server_id, volume_id } => {
            use openstack_sdk::api::compute::v2::server::volume_attachment::create_20 as m;
            let body = m::VolumeAttachmentBuilder::default().volume_id(volume_id.as_str()).build().map_err(build_err)?;
            x.send(
                m::Request::builder()
                    .server_id(server_id.as_str())
                    .volume_attachment(body)
                    .build()
                    .map_err(build_err)?,
            )
            .await?;
        }
        ServerDetachVolume { server_id, volume_id } => {
            use openstack_sdk::api::compute::v2::server::volume_attachment::delete as m;
            x.send(
                m::Request::builder()
                    .server_id(server_id.as_str())
                    .id(volume_id.as_str())
                    .build()
                    .map_err(build_err)?,
            )
            .await?;
        }
        ServerResize { id, flavor_id } => {
            use openstack_sdk::api::compute::v2::server::resize as m;
            let body = m::ResizeBuilder::default().flavor_ref(non_empty(flavor_id, "flavor")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).resize(body).build().map_err(build_err)?).await?;
        }
        ServerConfirmResize { id } => {
            use openstack_sdk::api::compute::v2::server::confirm_resize as m;
            x.send(m::Request::builder().id(id.as_str()).confirm_resize(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerRevertResize { id } => {
            use openstack_sdk::api::compute::v2::server::revert_resize as m;
            x.send(m::Request::builder().id(id.as_str()).revert_resize(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerRebuild { id, image_id } => {
            use openstack_sdk::api::compute::v2::server::rebuild_294 as m;
            let body = m::RebuildBuilder::default().image_ref(non_empty(image_id, "image")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).rebuild(body).build().map_err(build_err)?).await?;
        }
        ServerShelve { id } => {
            use openstack_sdk::api::compute::v2::server::shelve as m;
            x.send(m::Request::builder().id(id.as_str()).shelve(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerUnshelve { id, availability_zone } => match opt(availability_zone) {
            Some(az) => {
                use openstack_sdk::api::compute::v2::server::unshelve_277 as m;
                let body = m::UnshelveBuilder::default().availability_zone(az).build().map_err(build_err)?;
                x.send(m::Request::builder().id(id.as_str()).unshelve(body).build().map_err(build_err)?).await?;
            }
            None => {
                use openstack_sdk::api::compute::v2::server::unshelve_21 as m;
                x.send(m::Request::builder().id(id.as_str()).unshelve(Value::Null).build().map_err(build_err)?).await?;
            }
        },
        ServerRescue { id, image_id } => {
            use openstack_sdk::api::compute::v2::server::rescue as m;
            let mut b = m::RescueBuilder::default();
            if let Some(img) = opt(image_id) {
                b.rescue_image_ref(img);
            }
            let body = b.build().map_err(build_err)?;
            let resp = x.call(m::Request::builder().id(id.as_str()).rescue(body).build().map_err(build_err)?).await?;
            // Nova generates the password; shown once (like a generated key).
            if let Some(pass) = resp.as_ref().and_then(|v| v.get("adminPass")).and_then(Value::as_str) {
                x.secret = Some(Secret { file_name: format!("rescue-{id}.txt"), content: pass.to_string(), kind: SecretKind::Password, fields: vec![], extra: vec![] });
            }
        }
        ServerUnrescue { id } => {
            use openstack_sdk::api::compute::v2::server::unrescue as m;
            x.send(m::Request::builder().id(id.as_str()).unrescue(Value::Null).build().map_err(build_err)?).await?;
        }
        ServerAttachInterface { id, network_id, port_id, fixed_ip } => {
            use openstack_sdk::api::compute::v2::server::interface::create_20 as m;
            let mut b = m::InterfaceAttachmentBuilder::default();
            match (opt(network_id), opt(port_id)) {
                (Some(net), None) => {
                    b.net_id(net);
                    if let Some(ip) = opt(fixed_ip) {
                        b.fixed_ips(vec![m::FixedIpsBuilder::default().ip_address(ip).build().map_err(build_err)?]);
                    }
                }
                (None, Some(port)) => {
                    b.port_id(port);
                }
                _ => return Err(AppError::invalid("choose either a network or a port")),
            }
            let body = b.build().map_err(build_err)?;
            x.send(m::Request::builder().server_id(id.as_str()).interface_attachment(body).build().map_err(build_err)?).await?;
        }
        ServerDetachInterface { id, port_id } => {
            use openstack_sdk::api::compute::v2::server::interface::delete as m;
            x.send(m::Request::builder().server_id(id.as_str()).id(port_id.as_str()).build().map_err(build_err)?).await?;
        }

        // ----------------------------------------------------------------- Images
        ImageUpdate { id, changes } => {
            use openstack_sdk::api::image::v2::image::patch as m;
            let patch: json_patch::Patch = serde_json::from_value(Value::Array(image_patch(changes)?)).map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).patch(patch).build().map_err(build_err)?).await?;
        }
        ImageSetProperties { id, set, remove } => {
            use openstack_sdk::api::image::v2::image::patch as m;
            let changes = ImageChanges { set: set.clone(), remove: remove.clone(), ..Default::default() };
            let patch: json_patch::Patch = serde_json::from_value(Value::Array(image_patch(&changes)?)).map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).patch(patch).build().map_err(build_err)?).await?;
        }
        ImageDelete { id } => {
            use openstack_sdk::api::image::v2::image::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        ImageCreate(spec) => {
            use openstack_sdk::api::image::v2::image::{create as m, import::create as imp};
            let url = spec.url.as_deref().map(image_source_url).transpose()?;
            if spec.upload_bytes.is_some() && (url.is_some() || spec.upload_bytes == Some(0)) {
                return Err(AppError::invalid("an image comes from a URL or from a non-empty file, not both"));
            }
            let disk: m::DiskFormat = image_enum("disk format", &spec.disk_format)?;
            // Horizon: AMI-style images keep their format as container, the rest are `bare`.
            let container_name = match spec.container_format.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
                Some(c) => c,
                None if matches!(spec.disk_format.trim(), "ami" | "aki" | "ari") => spec.disk_format.trim(),
                None => "bare",
            };
            let container: m::ContainerFormat = image_enum("container format", container_name)?;
            let visibility: m::Visibility = image_enum("visibility", &spec.visibility)?;
            let props = checked_metadata(&spec.properties)?;
            if let Some((k, _)) = props.iter().find(|(k, _)| super::mapping::is_image_core_key(k)) {
                return Err(AppError::invalid(format!("{k} is not a custom property")));
            }
            let mut b = m::Request::builder();
            b.name(Some(Cow::from(non_empty(&spec.name, "name")?.to_string())))
                .disk_format(disk)
                .container_format(container)
                .visibility(visibility)
                .protected(spec.protected)
                .properties(props.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            if let Some(d) = spec.min_disk_gb {
                b.min_disk(d as i32);
            }
            if let Some(r) = spec.min_ram_mb {
                b.min_ram(r as i32);
            }
            let image = x.create(b.build().map_err(build_err)?, "image").await?;
            if let Some(bytes) = spec.upload_bytes {
                x.note_upload(&image, bytes);
            }
            if let Some(url) = url {
                let method = imp::MethodBuilder::default().name("web-download").uri(url).build().map_err(build_err)?;
                x.send(imp::Request::builder().image_id(image).method(method).build().map_err(build_err)?).await?;
            }
        }

        // ------------------------------------------------ Key pairs, server groups
        KeyPairImport { name, public_key, key_type } => {
            use openstack_sdk::api::compute::v2::keypair::create_210 as m;
            let public_key = non_empty(public_key, "public key")?;
            crate::model::check_public_key(key_type, public_key).map_err(AppError::invalid)?;
            let body = m::KeypairBuilder::default()
                .name(non_empty(name, "name")?)
                .public_key(public_key)
                ._type(if key_type == "x509" { m::Type::X509 } else { m::Type::Ssh })
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().keypair(body).build().map_err(build_err)?).await?;
        }
        KeyPairGenerate { name } => {
            use openstack_sdk::api::compute::v2::keypair::create_210 as m;
            use ssh_key::{rand_core::OsRng, Algorithm, LineEnding, PrivateKey};
            let name = non_empty(name, "name")?;
            let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).map_err(sfe("generate key"))?;
            let public_key = key.public_key().to_openssh().map_err(sfe("encode public key"))?;
            let body = m::KeypairBuilder::default()
                .name(name)
                .public_key(public_key.as_str())
                ._type(m::Type::Ssh)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().keypair(body).build().map_err(build_err)?).await?;
            // Only once Nova accepted the public key, and never in dry-run.
            if x.mode == WriteMode::On {
                let pem = key.to_openssh(LineEnding::LF).map_err(sfe("encode private key"))?;
                x.secret = Some(Secret { file_name: format!("{name}.pem"), content: pem.to_string(), kind: SecretKind::PrivateKey, fields: vec![], extra: vec![] });
            }
        }
        KeyPairDelete { name } => {
            use openstack_sdk::api::compute::v2::keypair::delete_20 as m;
            x.send(m::Request::builder().id(name.as_str()).build().map_err(build_err)?).await?;
        }
        ServerGroupCreate { name, policy } => {
            use openstack_sdk::api::compute::v2::server_group::create_264 as m;
            let p = match policy.as_str() {
                "affinity" => m::Policy::Affinity,
                "anti-affinity" => m::Policy::AntiAffinity,
                "soft-affinity" => m::Policy::SoftAffinity,
                "soft-anti-affinity" => m::Policy::SoftAntiAffinity,
                _ => return Err(AppError::invalid("invalid policy")),
            };
            let body = m::ServerGroupBuilder::default()
                .name(non_empty(name, "name")?)
                .policy(p)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().server_group(body).build().map_err(build_err)?).await?;
        }
        ServerGroupDelete { id } => {
            use openstack_sdk::api::compute::v2::server_group::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // ------------------------------------------------------ Volumes, snapshots
        VolumeCreate { name, description, size_gb, volume_type, availability_zone, source, source_id } => {
            use openstack_sdk::api::block_storage::v3::volume::create_30 as m;
            if *size_gb == 0 {
                return Err(AppError::invalid("size must be at least 1 GiB"));
            }
            let mut b = m::VolumeBuilder::default();
            b.name(Some(Cow::from(name.trim().to_string()))).size(Some(*size_gb as i32));
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d.to_string())));
            }
            if let Some(t) = opt(volume_type) {
                b.volume_type(Some(Cow::from(t.to_string())));
            }
            if let Some(z) = opt(availability_zone) {
                b.availability_zone(Some(Cow::from(z.to_string())));
            }
            let src = opt(source_id).map(|s| Cow::from(s.to_string()));
            match (source, src) {
                (VolumeSource::Empty, _) => {}
                (VolumeSource::Image, Some(s)) => {
                    b.image_ref(Some(s));
                }
                (VolumeSource::Snapshot, Some(s)) => {
                    b.snapshot_id(Some(s));
                }
                (VolumeSource::Volume, Some(s)) => {
                    b.source_volid(Some(s));
                }
                (_, None) => return Err(AppError::invalid("volume source is required")),
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().volume(body).build().map_err(build_err)?, "volume").await?;
        }
        VolumeUpdate { id, name, description } => {
            use openstack_sdk::api::block_storage::v3::volume::set_30 as m;
            let body = m::VolumeBuilder::default()
                .name(Some(Cow::from(name.trim().to_string())))
                .description(Some(Cow::from(opt(description).unwrap_or("").to_string())))
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).volume(body).build().map_err(build_err)?).await?;
        }
        VolumeExtend { id, new_size_gb } => {
            use openstack_sdk::api::block_storage::v3::volume::os_extend as m;
            let body = m::OsExtendBuilder::default().new_size(*new_size_gb as i32).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_extend(body).build().map_err(build_err)?).await?;
        }
        VolumeDelete { id } => {
            use openstack_sdk::api::block_storage::v3::volume::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        VolumeSetMetadata { id, metadata } => {
            use openstack_sdk::api::block_storage::v3::volume::metadata::replace as m;
            let md = checked_metadata(metadata)?;
            x.send(m::Request::builder().volume_id(id.as_str()).metadata(md.into_iter()).build().map_err(build_err)?).await?;
        }
        VolumeRetype { id, new_type, migrate } => {
            use openstack_sdk::api::block_storage::v3::volume::os_retype as m;
            let policy = if *migrate { m::MigrationPolicy::OnDemand } else { m::MigrationPolicy::Never };
            let body = m::OsRetypeBuilder::default()
                .new_type(non_empty(new_type, "volume type")?)
                .migration_policy(policy)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_retype(body).build().map_err(build_err)?).await?;
        }
        VolumeUploadImage { id, image_name, disk_format, force, visibility } => {
            use openstack_sdk::api::block_storage::v3::volume::os_volume_upload_image_31 as m;
            let format = match disk_format.as_str() {
                "raw" => m::DiskFormat::Raw,
                "qcow2" => m::DiskFormat::Qcow2,
                "vmdk" => m::DiskFormat::Vmdk,
                "vdi" => m::DiskFormat::Vdi,
                "vhd" => m::DiskFormat::Vhd,
                "vhdx" => m::DiskFormat::Vhdx,
                "ploop" => m::DiskFormat::Ploop,
                other => return Err(AppError::invalid(format!("unknown disk format {other}"))),
            };
            let mut b = m::OsVolumeUploadImageBuilder::default();
            b.image_name(non_empty(image_name, "image name")?)
                .disk_format(format)
                .container_format(Some(Cow::from("bare")))
                .force(*force);
            match opt(visibility) {
                None => {}
                Some("private") => {
                    b.visibility(m::Visibility::Private);
                }
                Some("shared") => {
                    b.visibility(m::Visibility::Shared);
                }
                Some("community") => {
                    b.visibility(m::Visibility::Community);
                }
                Some("public") => {
                    b.visibility(m::Visibility::Public);
                }
                Some(other) => return Err(AppError::invalid(format!("unknown visibility {other}"))),
            }
            let body = b.build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_volume_upload_image(body).build().map_err(build_err)?).await?;
        }
        VolumeTransferCreate { volume_id, name } => {
            use openstack_sdk::api::block_storage::v3::os_volume_transfer::create as m;
            let mut b = m::TransferBuilder::default();
            b.volume_id(volume_id.as_str());
            if let Some(n) = opt(name) {
                b.name(Some(Cow::from(n.to_string())));
            }
            let body = b.build().map_err(build_err)?;
            let resp = x.call(m::Request::builder().transfer(body).build().map_err(build_err)?).await?;
            // The receiving project needs both; Cinder never shows the key again.
            if let Some(v) = resp {
                let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                x.secret = Some(Secret {
                    file_name: format!("transfer-{volume_id}.txt"),
                    content: format!("Transfer ID: {}\nAuthorization key: {}\n", get("id"), get("auth_key")),
                    kind: SecretKind::Transfer,
                    fields: vec![],
                    extra: vec![],
                });
            }
        }
        VolumeTransferAccept { transfer_id, auth_key } => {
            use openstack_sdk::api::block_storage::v3::os_volume_transfer::accept as m;
            let body = m::AcceptBuilder::default().auth_key(non_empty(auth_key, "authorization key")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(transfer_id, "transfer ID")?).accept(body).build().map_err(build_err)?).await?;
        }
        VolumeSnapshotCreate { volume_id, name, description, force } => {
            use openstack_sdk::api::block_storage::v3::snapshot::create as m;
            let mut b = m::SnapshotBuilder::default();
            b.volume_id(volume_id.as_str()).name(Some(Cow::from(non_empty(name, "name")?.to_string())));
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d.to_string())));
            }
            if *force {
                b.force(true);
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().snapshot(body).build().map_err(build_err)?, "snapshot").await?;
        }
        SnapshotUpdate { id, name, description } => {
            use openstack_sdk::api::block_storage::v3::snapshot::set as m;
            let body = m::SnapshotBuilder::default()
                .name(Some(Cow::from(name.trim().to_string())))
                .description(Some(Cow::from(opt(description).unwrap_or("").to_string())))
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).snapshot(body).build().map_err(build_err)?).await?;
        }
        SnapshotDelete { id } => {
            use openstack_sdk::api::block_storage::v3::snapshot::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        SnapshotSetMetadata { id, metadata } => {
            use openstack_sdk::api::block_storage::v3::snapshot::metadata::replace as m;
            let md = checked_metadata(metadata)?;
            x.send(m::Request::builder().snapshot_id(id.as_str()).metadata(md.into_iter()).build().map_err(build_err)?).await?;
        }
        VolumeGroupDelete { id, delete_volumes } => {
            x.send(volume_group_delete(id, *delete_volumes)?).await?;
        }
        VolumeGroupCreate { name, description, group_type_id, volume_type_ids, availability_zone } => {
            use openstack_sdk::api::block_storage::v3::group::create_313 as m;
            let types: Vec<Cow<str>> = volume_type_ids.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).map(Cow::from).collect();
            if types.is_empty() {
                return Err(AppError::invalid("a group needs at least one volume type"));
            }
            let mut b = m::GroupBuilder::default();
            b.name(Some(Cow::from(non_empty(name, "name")?))).group_type(non_empty(group_type_id, "group type")?).volume_types(types);
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            if let Some(z) = opt(availability_zone) {
                b.availability_zone(Some(Cow::from(z)));
            }
            x.create(m::Request::builder().group(b.build().map_err(build_err)?).build().map_err(build_err)?, "group").await?;
        }
        VolumeGroupCreateFrom { name, description, source_group_id, group_snapshot_id } => {
            // Cinder answers `{"group": {...}}` here (no `response_key` in the
            // SDK), so the new ID isn't read back: `send`, not `create`.
            use openstack_sdk::api::block_storage::v3::group::create_from_src_314 as m;
            let mut b = m::CreateFromSrcBuilder::default();
            b.name(Some(Cow::from(non_empty(name, "name")?)));
            match (opt(source_group_id), opt(group_snapshot_id)) {
                (Some(g), None) => b.source_group_id(g),
                (None, Some(s)) => b.group_snapshot_id(s),
                _ => return Err(AppError::invalid("a group is created from either a group or a group snapshot")),
            };
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            x.send(m::Request::builder().create_from_src(b.build().map_err(build_err)?).build().map_err(build_err)?).await?;
        }
        VolumeGroupUpdate { id, name, description } => {
            use openstack_sdk::api::block_storage::v3::group::set_313 as m;
            // An empty description is sent as `""` so clearing it works.
            let body = m::GroupBuilder::default()
                .name(Some(Cow::from(non_empty(name, "name")?)))
                .description(Some(Cow::from(opt(description).unwrap_or(""))))
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(id, "group")?).group(body).build().map_err(build_err)?).await?;
        }
        VolumeGroupSetVolumes { id, add_volume_ids, remove_volume_ids } => {
            use openstack_sdk::api::block_storage::v3::group::set_313 as m;
            // Cinder takes both lists as one comma-separated string.
            let list = |ids: &[String]| Some(ids.iter().map(|i| i.trim()).filter(|i| !i.is_empty()).collect::<Vec<_>>().join(",")).filter(|s| !s.is_empty());
            let (add, remove) = (list(add_volume_ids), list(remove_volume_ids));
            if add.is_none() && remove.is_none() {
                return Err(AppError::invalid("no volumes to add or remove"));
            }
            let mut b = m::GroupBuilder::default();
            if let Some(a) = add {
                b.add_volumes(Some(Cow::from(a)));
            }
            if let Some(r) = remove {
                b.remove_volumes(Some(Cow::from(r)));
            }
            x.send(m::Request::builder().id(non_empty(id, "group")?).group(b.build().map_err(build_err)?).build().map_err(build_err)?).await?;
        }
        GroupSnapshotCreate { group_id, name, description } => {
            use openstack_sdk::api::block_storage::v3::group_snapshot::create_314 as m;
            let mut b = m::GroupSnapshotBuilder::default();
            b.group_id(non_empty(group_id, "group")?).name(Some(Cow::from(non_empty(name, "name")?)));
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            x.create(m::Request::builder().group_snapshot(b.build().map_err(build_err)?).build().map_err(build_err)?, "group snapshot").await?;
        }
        GroupSnapshotDelete { id } => {
            use openstack_sdk::api::block_storage::v3::group_snapshot::delete_314 as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // ------------------------------------------------------ Volume backups (roadmap 5)
        BackupCreate { volume_id, name, description, incremental, force, snapshot_id } => {
            use openstack_sdk::api::block_storage::v3::backup::create_351 as m;
            let mut b = m::BackupBuilder::default();
            b.volume_id(non_empty(volume_id, "volume")?).name(Some(Cow::from(non_empty(name, "name")?)));
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            // Only sent when set: Cinder rejects `incremental` on a first backup
            // of the volume, and `force` is only needed for attached volumes.
            if *incremental {
                b.incremental(true);
            }
            if *force {
                b.force(true);
            }
            if let Some(sn) = opt(snapshot_id) {
                b.snapshot_id(Some(Cow::from(sn)));
            }
            x.create(m::Request::builder().backup(b.build().map_err(build_err)?).build().map_err(build_err)?, "backup").await?;
        }
        BackupUpdate { id, name, description } => {
            use openstack_sdk::api::block_storage::v3::backup::set_39 as m;
            // An empty description is sent as `""` so clearing it works.
            let body = m::BackupBuilder::default()
                .name(Some(Cow::from(non_empty(name, "name")?)))
                .description(Some(Cow::from(opt(description).unwrap_or(""))))
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(id, "backup")?).backup(body).build().map_err(build_err)?).await?;
        }
        BackupRestore { id, volume_id, name } => {
            use openstack_sdk::api::block_storage::v3::backup::restore::create as m;
            let mut b = m::RestoreBuilder::default();
            // `volume_id` restores over that volume; else Cinder makes a new one (`name`).
            if let Some(v) = opt(volume_id) {
                b.volume_id(Some(Cow::from(v)));
            } else if let Some(n) = opt(name) {
                b.name(Some(Cow::from(n)));
            }
            x.send(m::Request::builder().id(non_empty(id, "backup")?).restore(b.build().map_err(build_err)?).build().map_err(build_err)?).await?;
        }
        BackupDelete { id } => {
            use openstack_sdk::api::block_storage::v3::backup::delete as m;
            x.send(m::Request::builder().id(non_empty(id, "backup")?).build().map_err(build_err)?).await?;
        }

        // ------------------------------------------------- Networks, subnets, ports
        NetworkCreate { name, admin_up, subnet } => {
            use openstack_sdk::api::network::v2::network::create as m;
            let body = m::NetworkBuilder::default()
                .name(name.trim())
                .admin_state_up(*admin_up)
                .build()
                .map_err(build_err)?;
            let net_id = x.create(m::Request::builder().network(body).build().map_err(build_err)?, "network").await?;
            if let Some(spec) = subnet {
                create_subnet(x, &net_id, spec).await?;
            }
        }
        AdminNetworkCreate(spec) => {
            use openstack_sdk::api::network::v2::network::create as m;
            let mut b = m::NetworkBuilder::default();
            b.name(spec.name.trim())
                .admin_state_up(spec.admin_up)
                .shared(spec.shared)
                .router_external(spec.external)
                // `tenant_id` is the SDK's name for the owner (Neutron takes both).
                .tenant_id(spec.project_id.as_str());
            if let Some(mtu) = spec.mtu {
                b.mtu(mtu);
            }
            if !spec.az_hints.is_empty() {
                b.availability_zone_hints(spec.az_hints.iter().map(|z| Cow::from(z.as_str())).collect::<Vec<_>>());
            }
            if let Some(p) = &spec.provider {
                // The server's own `[network]` settings, whatever the dialog sent
                // (the defaults in unit tests, which run without `config::init`).
                let net = super::config::try_state().map(|st| st.config.prefs.network.clone()).unwrap_or_default();
                p.validate(&net.provider_types()).map_err(AppError::invalid)?;
                b.provider_network_type(p.network_type.as_str());
                if let Some(phys) = &p.physical_network {
                    b.provider_physical_network(phys.trim());
                }
                if let Some(seg) = p.segmentation_id {
                    b.provider_segmentation_id(seg.to_string());
                }
            }
            let net_id = x.create(m::Request::builder().network(b.build().map_err(build_err)?).build().map_err(build_err)?, "network").await?;
            if let Some(s) = &spec.subnet {
                create_subnet(x, &net_id, s).await?;
            }
        }
        AdminNetworkUpdate { id, name, admin_up, shared, external } => {
            use openstack_sdk::api::network::v2::network::set as m;
            let body = m::NetworkBuilder::default()
                .name(name.trim())
                .admin_state_up(*admin_up)
                .shared(*shared)
                .router_external(*external)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).network(body).build().map_err(build_err)?).await?;
        }
        // ---------------------------------------------------------- Admin › Compute
        FlavorCreate(spec) => {
            use openstack_sdk::api::compute::v2::flavor::create_21 as m;
            let (vcpus, ram, disk) = (positive(spec.vcpus, "VCPUs")?, positive(spec.ram_mb, "RAM")?, at_least_zero(spec.disk_gb, "root disk")?);
            let mut b = m::FlavorBuilder::default();
            b.name(non_empty(&spec.name, "name")?)
                .vcpus(vcpus)
                .ram(ram)
                .disk(disk)
                .os_flv_ext_data_ephemeral(at_least_zero(spec.ephemeral_gb, "ephemeral disk")?)
                .swap(at_least_zero(spec.swap_mb, "swap disk")?)
                // Horizon: a flavor given to projects is private.
                .os_flavor_access_is_public(spec.projects.is_empty());
            if let Some(id) = opt(&spec.flavor_id).filter(|i| !i.eq_ignore_ascii_case("auto")) {
                b.id(Some(Cow::from(id.to_string())));
            }
            if let Some(f) = spec.rxtx_factor {
                if f.is_nan() || f <= 0.0 {
                    return Err(AppError::invalid("RX/TX factor must be positive"));
                }
                b.rxtx_factor(f.to_string());
            }
            let id = x.create(m::Request::builder().flavor(b.build().map_err(build_err)?).build().map_err(build_err)?, "flavor").await?;
            flavor_access(x, &id, &spec.projects, &[]).await?;
        }
        FlavorDelete { id } => {
            use openstack_sdk::api::compute::v2::flavor::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        FlavorSetExtraSpecs { id, set, unset } => {
            use openstack_sdk::api::compute::v2::flavor::extra_spec::{create as c, delete as d};
            if set.is_empty() && unset.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            if !set.is_empty() {
                let specs = checked_metadata(set)?;
                x.send(c::Request::builder().flavor_id(id.as_str()).extra_specs(specs.into_iter()).build().map_err(build_err)?).await?;
            }
            for key in unset {
                // The key is a path segment (`os-extra_specs/hw:cpu_policy`).
                x.send(d::Request::builder().flavor_id(id.as_str()).id(path_key(key)?).build().map_err(build_err)?).await?;
            }
        }
        FlavorSetAccess { id, add, remove } => {
            if add.is_empty() && remove.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            flavor_access(x, id, add, remove).await?;
        }
        AggregateCreate { name, zone, hosts } => {
            use openstack_sdk::api::compute::v2::aggregate::create_21 as m;
            let mut b = m::AggregateBuilder::default();
            b.name(non_empty(name, "name")?);
            if let Some(z) = opt(zone) {
                b.availability_zone(Some(Cow::from(z.to_string())));
            }
            let id = x.create(m::Request::builder().aggregate(b.build().map_err(build_err)?).build().map_err(build_err)?, "aggregate").await?;
            aggregate_hosts(x, &id, hosts, &[]).await?;
        }
        AggregateUpdate { id, name, zone } => {
            use openstack_sdk::api::compute::v2::aggregate::set_21 as m;
            let mut b = m::AggregateBuilder::default();
            b.name(non_empty(name, "name")?);
            // An empty zone is sent as `null`, which removes it (Horizon's edit form).
            b.availability_zone(opt(zone).map(|z| Cow::from(z.to_string())));
            x.send(m::Request::builder().id(id.as_str()).aggregate(b.build().map_err(build_err)?).build().map_err(build_err)?).await?;
        }
        AggregateSetHosts { id, add, remove } => {
            if add.is_empty() && remove.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            aggregate_hosts(x, id, add, remove).await?;
        }
        AggregateSetMetadata { id, set, unset } => {
            use openstack_sdk::api::compute::v2::aggregate::set_metadata as m;
            if set.is_empty() && unset.is_empty() {
                return Err(AppError::invalid("nothing changed"));
            }
            let set = checked_metadata(set)?;
            // Nova removes a key whose value is `null`.
            let md = set.into_iter().map(|(k, v)| (k, Some(Cow::from(v)))).chain(unset.iter().map(|k| (k.clone(), None)));
            let body = m::SetMetadataBuilder::default().metadata(md).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).set_metadata(body).build().map_err(build_err)?).await?;
        }
        AggregateDelete { id } => {
            use openstack_sdk::api::compute::v2::aggregate::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        ComputeServiceSet { id, enabled, reason } => {
            use openstack_sdk::api::compute::v2::service::set_253 as m;
            let mut b = m::Request::builder();
            b.id(id.as_str()).status(if *enabled { m::Status::Enabled } else { m::Status::Disabled });
            if let Some(r) = opt(reason).filter(|_| !enabled) {
                b.disabled_reason(r.to_string());
            }
            x.send(b.build().map_err(build_err)?).await?;
        }
        ServerMigrate { id, host } => {
            // Raw: the SDK's `migrate_256` always sends 2.56, which older clouds
            // refuse even without a host; `{"migrate": null}` works from 2.1.
            let r = match opt(host) {
                Some(h) => nova_action(id, json!({"migrate": {"host": h}}), Some("compute 2.56"))?,
                None => nova_action(id, json!({"migrate": null}), None)?,
            };
            x.send(r).await?;
        }
        ServerLiveMigrate { id, host, block_migration } => {
            // Raw: the SDK types `block_migration` as a bool, so it can't send
            // Nova's `"auto"` (2.25: block migration only without shared storage).
            let block = block_migration.map_or(json!("auto"), Value::Bool);
            let body = json!({"os-migrateLive": {"host": opt(host), "block_migration": block}});
            x.send(nova_action(id, body, Some("compute 2.30"))?).await?;
        }
        VolumeResetStatus { id, status } => {
            use openstack_sdk::api::block_storage::v3::volume::os_reset_status as m;
            let status = known_status(status, crate::model::VOLUME_STATUSES)?;
            let body = m::OsResetStatusBuilder::default().status(Some(Cow::from(status))).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_reset_status(body).build().map_err(build_err)?).await?;
        }
        VolumeMigrate { id, host, force_host_copy } => {
            use openstack_sdk::api::block_storage::v3::volume::os_migrate_volume_30 as m;
            let body = m::OsMigrateVolumeBuilder::default()
                .host(non_empty(host, "destination host")?)
                .force_host_copy(*force_host_copy)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_migrate_volume(body).build().map_err(build_err)?).await?;
        }
        VolumeManage(spec) => {
            use openstack_sdk::api::block_storage::v3::volume_manage::create_30 as m;
            let key = if spec.by_id { "source-id" } else { "source-name" };
            let mut b = m::VolumeBuilder::default();
            b._ref(json!({ key: non_empty(&spec.identifier, "identifier")? }))
                .host(Some(Cow::from(non_empty(&spec.host, "host")?.to_string())))
                .bootable(spec.bootable);
            // Empty fields are left out: Cinder names it after nothing, uses its
            // default type and the back end's zone.
            let some = |v: &Option<String>| opt(v).map(|v| Some(Cow::from(v.to_string())));
            if let Some(v) = some(&spec.name) {
                b.name(v);
            }
            if let Some(v) = some(&spec.description) {
                b.description(v);
            }
            if let Some(v) = some(&spec.volume_type) {
                b.volume_type(v);
            }
            if let Some(v) = some(&spec.availability_zone) {
                b.availability_zone(v);
            }
            let md = checked_metadata(&spec.metadata)?;
            if !md.is_empty() {
                b.metadata(md.into_iter());
            }
            x.create(m::Request::builder().volume(b.build().map_err(build_err)?).build().map_err(build_err)?, "volume").await?;
        }
        VolumeUnmanage { id } => {
            use openstack_sdk::api::block_storage::v3::volume::os_unmanage as m;
            x.send(m::Request::builder().id(id.as_str()).os_unmanage(Value::Null).build().map_err(build_err)?).await?;
        }
        VolumeTypeCreate { name, description, public } => {
            let mut t = json!({"name": non_empty(name, "name")?, "os-volume-type-access:is_public": public});
            if let Some(d) = opt(description) {
                t["description"] = json!(d);
            }
            x.create(JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::BlockStorage, "types", Some(json!({ "volume_type": t }))), "volume type").await?;
        }
        VolumeTypeUpdate { id, name, description, public } => {
            use openstack_sdk::api::block_storage::v3::r#type::set as m;
            let body = m::VolumeTypeBuilder::default()
                .name(Some(Cow::from(non_empty(name, "name")?)))
                .description(Some(Cow::from(opt(description).unwrap_or(""))))
                .is_public(*public)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).volume_type(body).build().map_err(build_err)?).await?;
        }
        VolumeTypeDelete { id } => {
            use openstack_sdk::api::block_storage::v3::r#type::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        VolumeTypeSetExtraSpecs { id, set, unset } => {
            use openstack_sdk::api::block_storage::v3::r#type::extra_spec::{create as c, delete as d};
            let set = spec_changes(set, unset)?;
            if !set.is_empty() {
                x.send(c::Request::builder().type_id(id.as_str()).extra_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v))))).build().map_err(build_err)?).await?;
            }
            for key in unset {
                x.send(d::Request::builder().type_id(id.as_str()).id(path_key(key)?).build().map_err(build_err)?).await?;
            }
        }
        VolumeTypeSetQos { id, qos_id, current_qos_id } => {
            let (new, old) = (opt(qos_id), opt(current_qos_id));
            if new == old {
                return Err(AppError::invalid("nothing changed"));
            }
            // Raw: the SDK's associate/disassociate lack `vol_type_id`. They
            // are GETs that change state (Cinder's API), hence sent as writes.
            let qos_call = |qos: &str, verb: &str| {
                let mut r = JsonRequest::new(http::Method::GET, openstack_sdk::types::ServiceType::BlockStorage, format!("qos-specs/{qos}/{verb}"), None);
                r.query.push(("vol_type_id", id.clone()));
                r
            };
            // `qos_id` / `current_qos_id` are path segments: `check_ids` vets them.
            if let Some(q) = old {
                x.send(qos_call(q, "disassociate")).await?;
            }
            if let Some(q) = new {
                x.send(qos_call(q, "associate")).await?;
            }
        }
        VolumeTypeEncryptionSet { id, spec, update } => {
            if !crate::model::CONTROL_LOCATIONS.contains(&spec.control_location.as_str()) {
                return Err(AppError::invalid(format!("unknown control location {}", spec.control_location)));
            }
            let mut e = json!({"provider": non_empty(&spec.provider, "provider")?, "control_location": spec.control_location});
            if let Some(c) = opt(&spec.cipher) {
                e["cipher"] = json!(c);
            }
            if let Some(k) = spec.key_size {
                e["key_size"] = json!(k);
            }
            // Raw: the SDK's create also requires `control_location` as an enum
            // and its update needs an encryption ID Cinder ignores (cinderclient
            // sends `provider`).
            let (method, path) = if *update { (http::Method::PUT, format!("types/{id}/encryption/provider")) } else { (http::Method::POST, format!("types/{id}/encryption")) };
            x.send(JsonRequest::new(method, openstack_sdk::types::ServiceType::BlockStorage, path, Some(json!({ "encryption": e })))).await?;
        }
        VolumeTypeEncryptionDelete { id } => {
            x.send(JsonRequest::new(http::Method::DELETE, openstack_sdk::types::ServiceType::BlockStorage, format!("types/{id}/encryption/provider"), None)).await?;
        }
        QosSpecCreate { name, consumer, specs } => {
            use openstack_sdk::api::block_storage::v3::qos_spec::create as m;
            let consumer = known_status(consumer, crate::model::QOS_CONSUMERS)?;
            let specs = checked_metadata(specs)?;
            let mut b = m::QosSpecsBuilder::default();
            b.name(non_empty(name, "name")?).properties(std::iter::once(("consumer".to_string(), json!(consumer))).chain(specs.into_iter().map(|(k, v)| (k, json!(v)))));
            x.create(m::Request::builder().qos_specs(b.build().map_err(build_err)?).build().map_err(build_err)?, "QoS spec").await?;
        }
        QosSpecSetConsumer { id, consumer } => {
            use openstack_sdk::api::block_storage::v3::qos_spec::set as m;
            let consumer = known_status(consumer, crate::model::QOS_CONSUMERS)?;
            x.send(m::Request::builder().id(id.as_str()).qos_specs(std::iter::once(("consumer", Some(Cow::from(consumer))))).build().map_err(build_err)?).await?;
        }
        QosSpecSetSpecs { id, set, unset } => {
            use openstack_sdk::api::block_storage::v3::qos_spec::{delete_keys as d, set as m};
            let set = spec_changes(set, unset)?;
            if !set.is_empty() {
                x.send(m::Request::builder().id(id.as_str()).qos_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v))))).build().map_err(build_err)?).await?;
            }
            if !unset.is_empty() {
                x.send(d::Request::builder().id(id.as_str()).keys(unset.iter().map(|k| Cow::from(k.as_str())).collect::<Vec<_>>()).build().map_err(build_err)?).await?;
            }
        }
        QosSpecDelete { id } => {
            use openstack_sdk::api::block_storage::v3::qos_spec::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        GroupTypeCreate { name, description, public } => {
            use openstack_sdk::api::block_storage::v3::group_type::create_311 as m;
            let mut b = m::GroupTypeBuilder::default();
            b.name(non_empty(name, "name")?).is_public(*public);
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            x.create(m::Request::builder().group_type(b.build().map_err(build_err)?).build().map_err(build_err)?, "group type").await?;
        }
        GroupTypeUpdate { id, name, description, public } => {
            use openstack_sdk::api::block_storage::v3::group_type::set_311 as m;
            let body = m::GroupTypeBuilder::default()
                .name(Some(Cow::from(non_empty(name, "name")?)))
                .description(Some(Cow::from(opt(description).unwrap_or(""))))
                .is_public(*public)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).group_type(body).build().map_err(build_err)?).await?;
        }
        GroupTypeDelete { id } => {
            use openstack_sdk::api::block_storage::v3::group_type::delete_311 as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        GroupTypeSetSpecs { id, set, unset } => {
            use openstack_sdk::api::block_storage::v3::group_type::group_spec::{create_311 as c, delete_311 as d};
            let set = spec_changes(set, unset)?;
            if !set.is_empty() {
                x.send(c::Request::builder().group_type_id(id.as_str()).group_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v))))).build().map_err(build_err)?).await?;
            }
            for key in unset {
                x.send(d::Request::builder().group_type_id(id.as_str()).id(path_key(key)?).build().map_err(build_err)?).await?;
            }
        }
        SnapshotResetStatus { id, status } => {
            use openstack_sdk::api::block_storage::v3::snapshot::os_reset_status as m;
            let status = known_status(status, crate::model::SNAPSHOT_STATUSES)?;
            let body = m::OsResetStatusBuilder::default().status(status).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).os_reset_status(body).build().map_err(build_err)?).await?;
        }
        RbacPolicyCreate { kind, object_id, target_project } => {
            use openstack_sdk::api::network::v2::rbac_policy::create as m;
            let (object_type, action) = crate::model::rbac_kind(kind).map_err(AppError::invalid)?;
            let body = m::RbacPolicyBuilder::default()
                .object_type(object_type)
                .action(action)
                .object_id(non_empty(object_id, "object")?)
                .target_tenant(non_empty(target_project, "target project")?)
                .build()
                .map_err(build_err)?;
            x.create(m::Request::builder().rbac_policy(body).build().map_err(build_err)?, "RBAC policy").await?;
        }
        RbacPolicyUpdate { id, target_project } => {
            use openstack_sdk::api::network::v2::rbac_policy::set as m;
            let body = m::RbacPolicyBuilder::default().target_tenant(non_empty(target_project, "target project")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).rbac_policy(body).build().map_err(build_err)?).await?;
        }
        RbacPolicyDelete { id } => {
            use openstack_sdk::api::network::v2::rbac_policy::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        NetworkUpdate { id, name, admin_up } => {
            use openstack_sdk::api::network::v2::network::set as m;
            let body = m::NetworkBuilder::default()
                .name(name.trim())
                .admin_state_up(*admin_up)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).network(body).build().map_err(build_err)?).await?;
        }
        NetworkDelete { id } => {
            use openstack_sdk::api::network::v2::network::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        SubnetCreate { network_id, spec } => create_subnet(x, network_id, spec).await?,
        SubnetUpdate { id, name, enable_dhcp, dns_nameservers } => {
            use openstack_sdk::api::network::v2::subnet::set as m;
            let dns: Vec<Cow<str>> = dns_nameservers.iter().map(|d| Cow::from(d.trim().to_string())).collect();
            let body = m::SubnetBuilder::default()
                .name(name.trim())
                .enable_dhcp(*enable_dhcp)
                .dns_nameservers(dns)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).subnet(body).build().map_err(build_err)?).await?;
        }
        SubnetDelete { id } => {
            use openstack_sdk::api::network::v2::subnet::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        PortCreate { network_id, name, admin_up, fixed_ip } => {
            use openstack_sdk::api::network::v2::port::create as m;
            let mut b = m::PortBuilder::default();
            b.network_id(network_id.as_str()).name(name.trim()).admin_state_up(*admin_up);
            if let Some(ip) = opt(fixed_ip) {
                let f = m::FixedIpsBuilder::default().ip_address(ip.to_string()).build().map_err(build_err)?;
                b.fixed_ips(vec![f]);
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().port(body).build().map_err(build_err)?, "port").await?;
        }
        PortUpdate { id, name, admin_up, vnic_type, port_security, security_group_ids } => {
            use openstack_sdk::api::network::v2::port::set as m;
            let mut b = m::PortBuilder::default();
            if let Some(n) = name {
                b.name(n.trim());
            }
            if let Some(up) = admin_up {
                b.admin_state_up(*up);
            }
            if let Some(v) = vnic_type {
                // The SDK's enum has the API's spellings as serde names.
                let v: m::BindingVnicType = serde_json::from_value(Value::String(v.clone()))
                    .map_err(|_| AppError::invalid(format!("unknown VNIC type: {v}")))?;
                b.binding_vnic_type(v);
            }
            if let Some(on) = port_security {
                b.port_security_enabled(*on);
            }
            if let Some(groups) = security_group_ids {
                b.security_groups(groups.iter().map(|g| Cow::from(g.as_str())).collect::<Vec<_>>());
            }
            let body = b.build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).port(body).build().map_err(build_err)?).await?;
        }
        PortSetSecurityGroups { id, security_group_ids } => {
            use openstack_sdk::api::network::v2::port::set as m;
            let groups: Vec<Cow<str>> = security_group_ids.iter().map(|g| Cow::from(g.as_str())).collect();
            let body = m::PortBuilder::default().security_groups(groups).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).port(body).build().map_err(build_err)?).await?;
        }
        PortDelete { id } => {
            use openstack_sdk::api::network::v2::port::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // ---------------------------------------------------------------- Routers
        RouterCreate { name, admin_up, external_network_id, qos_policy_id } => {
            use openstack_sdk::api::network::v2::router::create as m;
            let mut b = m::RouterBuilder::default();
            b.name(name.trim()).admin_state_up(*admin_up);
            if let Some(net) = opt(external_network_id) {
                let mut gw = m::ExternalGatewayInfoBuilder::default();
                gw.network_id(net.to_string());
                if let Some(q) = opt(qos_policy_id) {
                    gw.qos_policy_id(Some(Cow::from(q.to_string())));
                }
                b.external_gateway_info(Some(gw.build().map_err(build_err)?));
            } else if opt(qos_policy_id).is_some() {
                return Err(AppError::invalid("a gateway QoS policy needs an external network"));
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().router(body).build().map_err(build_err)?, "router").await?;
        }
        AdminRouterCreate(spec) => {
            use openstack_sdk::api::network::v2::router::create as m;
            let mut b = m::RouterBuilder::default();
            b.name(spec.name.trim()).admin_state_up(spec.admin_up).tenant_id(non_empty(&spec.project_id, "project")?);
            if let Some(net) = opt(&spec.external_network_id) {
                let mut gw = m::ExternalGatewayInfoBuilder::default();
                gw.network_id(net.to_string());
                if let Some(snat) = spec.enable_snat {
                    gw.enable_snat(snat);
                }
                b.external_gateway_info(Some(gw.build().map_err(build_err)?));
            } else if spec.enable_snat.is_some() {
                return Err(AppError::invalid("SNAT needs an external network"));
            }
            if !spec.az_hints.is_empty() {
                b.availability_zone_hints(spec.az_hints.iter().map(|z| Cow::from(z.as_str())).collect::<Vec<_>>());
            }
            if let Some(d) = spec.distributed {
                b.distributed(Some(d));
            }
            if let Some(ha) = spec.ha {
                b.ha(Some(ha));
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().router(body).build().map_err(build_err)?, "router").await?;
        }
        RouterUpdate { id, name, admin_up, snat } => {
            use openstack_sdk::api::network::v2::router::set as m;
            let mut b = m::RouterBuilder::default();
            b.name(name.trim()).admin_state_up(*admin_up);
            if let Some((network_id, on)) = snat {
                let gw = m::ExternalGatewayInfoBuilder::default()
                    .network_id(network_id.as_str())
                    .enable_snat(*on)
                    .build()
                    .map_err(build_err)?;
                b.external_gateway_info(Some(gw));
            }
            let body = b.build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterSetGateway { id, network_id } => {
            use openstack_sdk::api::network::v2::router::set as m;
            let gw = m::ExternalGatewayInfoBuilder::default().network_id(network_id.as_str()).build().map_err(build_err)?;
            let body = m::RouterBuilder::default().external_gateway_info(Some(gw)).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterClearGateway { id } => {
            use openstack_sdk::api::network::v2::router::set as m;
            let body = m::RouterBuilder::default()
                .external_gateway_info(None::<m::ExternalGatewayInfo>)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterAddInterface { id, subnet_id } => {
            use openstack_sdk::api::network::v2::router::add_router_interface as m;
            x.send(m::Request::builder().id(id.as_str()).subnet_id(subnet_id.as_str()).build().map_err(build_err)?)
                .await?;
        }
        RouterRemoveInterface { id, port_id } => {
            use openstack_sdk::api::network::v2::router::remove_router_interface as m;
            x.send(m::Request::builder().id(id.as_str()).port_id(port_id.as_str()).build().map_err(build_err)?)
                .await?;
        }
        RouterAddRoute { id, destination, nexthop } => {
            use openstack_sdk::api::network::v2::router::add_extraroutes as m;
            let (d, h) = validate_route(destination, nexthop).map_err(AppError::invalid)?;
            let route = m::RoutesBuilder::default().destination(d).nexthop(h).build().map_err(build_err)?;
            let body = m::RouterBuilder::default().routes(vec![route]).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterRemoveRoute { id, destination, nexthop } => {
            use openstack_sdk::api::network::v2::router::remove_extraroutes as m;
            let route = m::RoutesBuilder::default()
                .destination(destination.as_str())
                .nexthop(nexthop.as_str())
                .build()
                .map_err(build_err)?;
            let body = m::RouterBuilder::default().routes(vec![route]).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterSetRoutes { id, routes } => {
            use openstack_sdk::api::network::v2::router::set as m;
            let list = routes
                .iter()
                .map(|(d, n)| m::RoutesBuilder::default().destination(d.as_str()).nexthop(n.as_str()).build().map_err(build_err))
                .collect::<R<Vec<_>>>()?;
            let body = m::RouterBuilder::default().routes(list).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).router(body).build().map_err(build_err)?).await?;
        }
        RouterDelete { id } => {
            use openstack_sdk::api::network::v2::router::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // -------------------------------------------------------- Trunks
        // The SDK has no trunk endpoints (only their tags): JSON requests.
        TrunkCreate { name, description, port_id, admin_up, subports } => {
            let sub_ports = subports.iter().map(SubportSpec::body).collect::<Result<Vec<_>, _>>().map_err(AppError::invalid)?;
            let mut t = serde_json::Map::new();
            t.insert("name".into(), json!(name.trim()));
            if let Some(d) = opt(description) {
                t.insert("description".into(), json!(d));
            }
            t.insert("port_id".into(), json!(non_empty(port_id, "parent port")?));
            t.insert("admin_state_up".into(), json!(admin_up));
            if !sub_ports.is_empty() {
                t.insert("sub_ports".into(), json!(sub_ports));
            }
            x.create(JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::Network, "trunks", Some(json!({ "trunk": t }))), "trunk").await?;
        }
        TrunkUpdate { id, name, description, admin_up } => {
            let body = json!({"trunk": {"name": name.trim(), "description": description.trim(), "admin_state_up": admin_up}});
            x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Network, format!("trunks/{}", non_empty(id, "trunk")?), Some(body))).await?;
        }
        TrunkDelete { id } => {
            x.send(JsonRequest::new(http::Method::DELETE, openstack_sdk::types::ServiceType::Network, format!("trunks/{}", non_empty(id, "trunk")?), None)).await?;
        }
        TrunkAddSubports { id, subports } => {
            if subports.is_empty() {
                return Err(AppError::invalid("no sub-ports to add"));
            }
            let sub_ports = subports.iter().map(SubportSpec::body).collect::<Result<Vec<_>, _>>().map_err(AppError::invalid)?;
            let path = format!("trunks/{}/add_subports", non_empty(id, "trunk")?);
            x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Network, path, Some(json!({ "sub_ports": sub_ports })))).await?;
        }
        TrunkRemoveSubports { id, port_ids } => {
            let sub_ports: Vec<Value> = port_ids.iter().map(|p| non_empty(p, "sub-port").map(|p| json!({ "port_id": p }))).collect::<R<_>>()?;
            if sub_ports.is_empty() {
                return Err(AppError::invalid("no sub-ports to remove"));
            }
            let path = format!("trunks/{}/remove_subports", non_empty(id, "trunk")?);
            x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Network, path, Some(json!({ "sub_ports": sub_ports })))).await?;
        }

        // -------------------------------------------------------- Firewall (FWaaS v2, roadmap 93)
        // Not in the SDK (`openstack-sdk-network` has no `fwaas` module): JSON requests.
        FirewallRuleCreate(spec) => {
            x.create(fwaas(http::Method::POST, "firewall_rules".into(), Some(spec.body().map_err(AppError::invalid)?)), "firewall rule").await?;
        }
        FirewallRuleUpdate { id, spec } => {
            let path = format!("firewall_rules/{}", non_empty(id, "firewall rule")?);
            x.send(fwaas(http::Method::PUT, path, Some(spec.body().map_err(AppError::invalid)?))).await?;
        }
        FirewallRuleDelete { id } => {
            x.send(fwaas(http::Method::DELETE, format!("firewall_rules/{}", non_empty(id, "firewall rule")?), None)).await?;
        }
        FirewallPolicyCreate { name, description, rule_ids, audited } => {
            let rules = rule_ids.iter().map(|r| non_empty(r, "firewall rule")).collect::<R<Vec<_>>>()?;
            let mut p = serde_json::Map::new();
            p.insert("name".into(), json!(non_empty(name, "name")?));
            p.insert("description".into(), json!(description.trim()));
            p.insert("firewall_rules".into(), json!(rules));
            p.insert("audited".into(), json!(audited));
            x.create(fwaas(http::Method::POST, "firewall_policies".into(), Some(json!({ "firewall_policy": p }))), "firewall policy").await?;
        }
        FirewallPolicyUpdate { id, name, description, audited } => {
            let mut p = serde_json::Map::new();
            p.insert("name".into(), json!(non_empty(name, "name")?));
            p.insert("description".into(), json!(description.trim()));
            p.insert("audited".into(), json!(audited));
            let path = format!("firewall_policies/{}", non_empty(id, "firewall policy")?);
            x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_policy": p })))).await?;
        }
        FirewallPolicyDelete { id } => {
            x.send(fwaas(http::Method::DELETE, format!("firewall_policies/{}", non_empty(id, "firewall policy")?), None)).await?;
        }
        FirewallPolicyInsertRule { id, rule_id, before_id, after_id } => {
            // Neutron puts the rule first when neither is given; both is ambiguous.
            if opt(before_id).is_some() && opt(after_id).is_some() {
                return Err(AppError::invalid("insert the rule either before or after another one, not both"));
            }
            let mut b = serde_json::Map::new();
            b.insert("firewall_rule_id".into(), json!(non_empty(rule_id, "firewall rule")?));
            if let Some(r) = opt(before_id) {
                b.insert("insert_before".into(), json!(r));
            }
            if let Some(r) = opt(after_id) {
                b.insert("insert_after".into(), json!(r));
            }
            let path = format!("firewall_policies/{}/insert_rule", non_empty(id, "firewall policy")?);
            x.send(fwaas(http::Method::PUT, path, Some(Value::Object(b)))).await?;
        }
        FirewallPolicyRemoveRule { id, rule_id } => {
            let path = format!("firewall_policies/{}/remove_rule", non_empty(id, "firewall policy")?);
            x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_rule_id": non_empty(rule_id, "firewall rule")? })))).await?;
        }
        FirewallGroupCreate { name, description, ingress_policy_id, egress_policy_id, port_ids, admin_up } => {
            let ports = port_ids.iter().map(|p| non_empty(p, "port")).collect::<R<Vec<_>>>()?;
            let mut g = serde_json::Map::new();
            g.insert("name".into(), json!(name.trim()));
            g.insert("description".into(), json!(description.trim()));
            if let Some(p) = opt(ingress_policy_id) {
                g.insert("ingress_firewall_policy_id".into(), json!(p));
            }
            if let Some(p) = opt(egress_policy_id) {
                g.insert("egress_firewall_policy_id".into(), json!(p));
            }
            g.insert("ports".into(), json!(ports));
            g.insert("admin_state_up".into(), json!(admin_up));
            x.create(fwaas(http::Method::POST, "firewall_groups".into(), Some(json!({ "firewall_group": g }))), "firewall group").await?;
        }
        FirewallGroupUpdate { id, name, description, ingress_policy_id, egress_policy_id, admin_up } => {
            // `null` policies remove the filtering in that direction.
            let mut g = serde_json::Map::new();
            g.insert("name".into(), json!(name.trim()));
            g.insert("description".into(), json!(description.trim()));
            g.insert("ingress_firewall_policy_id".into(), json!(opt(ingress_policy_id)));
            g.insert("egress_firewall_policy_id".into(), json!(opt(egress_policy_id)));
            g.insert("admin_state_up".into(), json!(admin_up));
            let path = format!("firewall_groups/{}", non_empty(id, "firewall group")?);
            x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_group": g })))).await?;
        }
        FirewallGroupSetPorts { id, port_ids } => {
            // Neutron replaces the whole list: an empty one leaves the group INACTIVE.
            let ports = port_ids.iter().map(|p| non_empty(p, "port")).collect::<R<Vec<_>>>()?;
            let path = format!("firewall_groups/{}", non_empty(id, "firewall group")?);
            x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_group": { "ports": ports } })))).await?;
        }
        FirewallGroupDelete { id } => {
            x.send(fwaas(http::Method::DELETE, format!("firewall_groups/{}", non_empty(id, "firewall group")?), None)).await?;
        }

        // -------------------------------------------------------- Network QoS
        QosPolicyCreate { name, description, shared } => {
            use openstack_sdk::api::network::v2::qos::policy::create as m;
            let mut b = m::PolicyBuilder::default();
            b.name(non_empty(name, "name")?).shared(*shared);
            if let Some(d) = opt(description) {
                b.description(d);
            }
            x.create(m::Request::builder().policy(b.build().map_err(build_err)?).build().map_err(build_err)?, "QoS policy").await?;
        }
        QosPolicyUpdate { id, name, description, shared } => {
            use openstack_sdk::api::network::v2::qos::policy::set as m;
            let body = m::PolicyBuilder::default().name(non_empty(name, "name")?).description(description.trim()).shared(*shared).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).policy(body).build().map_err(build_err)?).await?;
        }
        QosPolicyDelete { id } => {
            use openstack_sdk::api::network::v2::qos::policy::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        // Rules: one JSON path for the four types (`…/<type>_rules`), the body
        // built and checked by `QosRuleSpec::body`.
        QosRuleCreate { policy_id, spec } => {
            let body = spec.body().map_err(AppError::invalid)?;
            let path = format!("qos/policies/{}/{}_rules", non_empty(policy_id, "policy")?, spec.kind);
            let ep = JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::Network, path, Some(body));
            x.create(ep, "QoS rule").await?;
        }
        QosRuleUpdate { policy_id, id, spec } => {
            let body = spec.body().map_err(AppError::invalid)?;
            let path = format!("qos/policies/{}/{}_rules/{}", non_empty(policy_id, "policy")?, spec.kind, non_empty(id, "rule")?);
            x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Network, path, Some(body))).await?;
        }
        QosRuleDelete { policy_id, kind, id } => {
            if !QOS_RULE_TYPES.contains(&kind.as_str()) {
                return Err(AppError::invalid(format!("unknown rule type {kind}")));
            }
            let path = format!("qos/policies/{}/{kind}_rules/{}", non_empty(policy_id, "policy")?, non_empty(id, "rule")?);
            x.send(JsonRequest::new(http::Method::DELETE, openstack_sdk::types::ServiceType::Network, path, None)).await?;
        }

        // ---------------------------------------------- Key Manager (roadmap 55)
        SecretCreate(spec) => {
            x.create(JsonRequest::key_manager(http::Method::POST, "secrets", Some(secret_body(spec)?)), "secret").await?;
        }
        SecretDelete { id } => {
            x.send(JsonRequest::key_manager(http::Method::DELETE, format!("secrets/{}", non_empty(id, "secret")?), None)).await?;
        }
        ContainerCreate { name, container_type, secrets } => {
            let body = container_body(name.as_deref(), container_type, secrets)?;
            x.create(JsonRequest::key_manager(http::Method::POST, "containers", Some(body)), "container").await?;
        }
        ContainerDelete { id } => {
            x.send(JsonRequest::key_manager(http::Method::DELETE, format!("containers/{}", non_empty(id, "container")?), None)).await?;
        }
        SecretAclSet { id, project_access, users } => {
            x.send(acl_put(&format!("secrets/{}/acl", non_empty(id, "secret")?), *project_access, users)?).await?;
        }
        SecretAclDelete { id } => {
            x.send(JsonRequest::key_manager(http::Method::DELETE, format!("secrets/{}/acl", non_empty(id, "secret")?), None)).await?;
        }
        ContainerAclSet { id, project_access, users } => {
            x.send(acl_put(&format!("containers/{}/acl", non_empty(id, "container")?), *project_access, users)?).await?;
        }
        ContainerAclDelete { id } => {
            x.send(JsonRequest::key_manager(http::Method::DELETE, format!("containers/{}/acl", non_empty(id, "container")?), None)).await?;
        }

        // ------------------------------------------- Orchestration (roadmap 62)
        StackCheck { name, id } => {
            x.send(stack_action(name, id, "check")?).await?;
        }
        StackSuspend { name, id } => {
            x.send(stack_action(name, id, "suspend")?).await?;
        }
        StackResume { name, id } => {
            x.send(stack_action(name, id, "resume")?).await?;
        }
        StackDelete { name, id } => {
            x.send(JsonRequest::orchestration(http::Method::DELETE, stack_path(name, id)?, None)).await?;
        }
        StackCreate(spec) => {
            x.create(JsonRequest::orchestration(http::Method::POST, "stacks", Some(stack_body(spec, true)?)), "stack").await?;
            x.hide_params(&spec.hidden);
        }
        StackUpdate { id, spec } => {
            x.send(JsonRequest::orchestration(http::Method::PATCH, stack_path(&spec.name, id)?, Some(stack_body(spec, false)?))).await?;
            x.hide_params(&spec.hidden);
        }

        // -------------------------------------------------------- Alarming (Aodh, roadmap 79)
        // The body is built and checked by `AlarmSpec::body`, as in the dialog.
        AlarmCreate(spec) => {
            let body = spec.body().map_err(AppError::invalid)?;
            x.create(JsonRequest::alarming(http::Method::POST, "alarms", Some(body)), "alarm").await?;
        }
        AlarmUpdate { id, spec } => {
            let body = spec.body().map_err(AppError::invalid)?;
            x.send(JsonRequest::alarming(http::Method::PUT, format!("alarms/{}", non_empty(id, "alarm")?), Some(body))).await?;
        }

        // -------------------------------------------------------- Port forwarding
        PortForwardingCreate(spec) => {
            let path = format!("floatingips/{}/port_forwardings", non_empty(&spec.floating_ip_id, "floating IP")?);
            let body = port_forwarding_body(spec)?;
            x.create(JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::Network, path, Some(body)), "port forwarding")
                .await?;
        }
        PortForwardingUpdate { id, spec } => {
            let path = format!("floatingips/{}/port_forwardings/{}", non_empty(&spec.floating_ip_id, "floating IP")?, non_empty(id, "rule")?);
            let body = port_forwarding_body(spec)?;
            x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Network, path, Some(body))).await?;
        }
        PortForwardingDelete { floating_ip_id, id } => {
            use openstack_sdk::api::network::v2::floatingip::port_forwarding::delete as m;
            x.send(m::Request::builder().floatingip_id(floating_ip_id.as_str()).id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // -------------------------------------------------------- Load balancers
        LoadBalancerCreate(spec) => {
            use openstack_sdk::api::load_balancer::v2::loadbalancer::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut lb = m::LoadbalancerBuilder::default();
            lb.vip_subnet_id(spec.vip_subnet_id.trim());
            if let Some(v) = opt(&spec.name) {
                lb.name(v);
            }
            if let Some(v) = opt(&spec.description) {
                lb.description(v);
            }
            if let Some(v) = opt(&spec.vip_address) {
                lb.vip_address(v);
            }
            if let Some(v) = opt(&spec.flavor_id) {
                lb.flavor_id(v);
            }
            if let Some(v) = opt(&spec.availability_zone) {
                lb.availability_zone(v);
            }
            // Only "down" is sent: the bodies stay as before for the default.
            if !spec.admin_up {
                lb.admin_state_up(false);
            }
            if let Some(l) = &spec.listener {
                let mut lb_l = m::ListenersBuilder::default();
                lb_l.protocol(enum_of::<m::ListenersProtocol>(&l.protocol, "listener protocol")?).protocol_port(i32::from(l.port));
                if let Some(v) = opt(&l.name) {
                    lb_l.name(v);
                }
                if let Some(v) = opt(&l.description) {
                    lb_l.description(v);
                }
                lb_l.connection_limit(l.connection_limit.map_or(-1, |n| n as i32));
                listener_options!(lb_l, &l.options);
                listener_certificates!(lb_l, l);
                if let Some(p) = &spec.pool {
                    let mut pool = m::DefaultPoolBuilder::default();
                    pool.protocol(enum_of::<m::DefaultPoolProtocol>(&p.protocol, "pool protocol")?)
                        .lb_algorithm(enum_of::<m::LbAlgorithm>(&p.algorithm, "algorithm")?);
                    if let Some(v) = opt(&p.name) {
                        pool.name(v);
                    }
                    if let Some(v) = opt(&p.description) {
                        pool.description(v);
                    }
                    if let Some(kind) = p.session_persistence.as_deref() {
                        let mut sp = m::SessionPersistenceBuilder::default();
                        sp._type(enum_of::<m::SessionPersistenceType>(kind, "session persistence")?);
                        if let Some(c) = opt(&p.cookie_name).filter(|_| kind == "APP_COOKIE") {
                            sp.cookie_name(c);
                        }
                        pool.session_persistence(sp.build().map_err(build_err)?);
                    }
                    pool_tls!(pool, p);
                    let members = spec
                        .members
                        .iter()
                        .map(|mb| {
                            let mut b = m::MembersBuilder::default();
                            member_fields!(b, mb);
                            b.build().map_err(build_err)
                        })
                        .collect::<R<Vec<_>>>()?;
                    if !members.is_empty() {
                        pool.members(members);
                    }
                    if let Some(hm) = &spec.monitor {
                        let mut b = m::HealthmonitorBuilder::default();
                        b._type(enum_of::<m::HealthmonitorType>(&hm.kind, "monitor type")?)
                            .delay(hm.delay as i32)
                            .timeout(hm.timeout as i32)
                            .max_retries(i32::from(hm.max_retries));
                        if let Some(n) = hm.max_retries_down {
                            b.max_retries_down(i32::from(n));
                        }
                        if let Some(v) = opt(&hm.name) {
                            b.name(v);
                        }
                        if let Some(v) = hm.http_method.as_deref() {
                            b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
                        }
                        if let Some(v) = opt(&hm.url_path) {
                            b.url_path(v);
                        }
                        if let Some(v) = opt(&hm.expected_codes) {
                            b.expected_codes(v);
                        }
                        if !hm.admin_up {
                            b.admin_state_up(false);
                        }
                        pool.healthmonitor(b.build().map_err(build_err)?);
                    }
                    lb_l.default_pool(pool.build().map_err(build_err)?);
                }
                lb.listeners(vec![lb_l.build().map_err(build_err)?]);
            }
            let body = lb.build().map_err(build_err)?;
            x.create(m::Request::builder().loadbalancer(body).build().map_err(build_err)?, "load balancer").await?;
        }
        LoadBalancerUpdate { id, name, description, admin_up } => {
            use openstack_sdk::api::load_balancer::v2::loadbalancer::set as m;
            let body = m::LoadbalancerBuilder::default()
                .name(name.trim())
                .description(description.trim())
                .admin_state_up(*admin_up)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).loadbalancer(body).build().map_err(build_err)?).await?;
        }
        ListenerCreate { loadbalancer_id, spec, default_pool_id } => {
            use openstack_sdk::api::load_balancer::v2::listener::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::ListenerBuilder::default();
            b.loadbalancer_id(loadbalancer_id.as_str())
                .protocol(enum_of::<m::ListenerProtocol>(&spec.protocol, "listener protocol")?)
                .protocol_port(i32::from(spec.port))
                .connection_limit(spec.connection_limit.map_or(-1, |n| n as i32));
            if let Some(v) = opt(&spec.name) {
                b.name(v);
            }
            if let Some(v) = opt(&spec.description) {
                b.description(v);
            }
            if let Some(v) = opt(default_pool_id) {
                b.default_pool_id(v);
            }
            listener_options!(b, &spec.options);
            listener_certificates!(b, spec);
            x.create(m::Request::builder().listener(b.build().map_err(build_err)?).build().map_err(build_err)?, "listener").await?;
        }
        // JSON: a cleared default pool must be sent as `null`, which the SDK cannot express.
        // Cleared options likewise: `null` timeouts, `{}` headers and `[]` CIDRs are
        // Octavia's "back to the default" (roadmap 54).
        ListenerUpdate { id, name, description, connection_limit, default_pool_id, admin_up, protocol, options, tls } => {
            let mut body = json!({"listener": {
                "name": name.trim(),
                "description": description.trim(),
                "connection_limit": connection_limit.map_or(-1, i64::from),
                "default_pool_id": opt(default_pool_id),
                "admin_state_up": admin_up,
            }});
            if let Some(o) = options {
                o.check(protocol).map_err(AppError::invalid)?;
                let l = &mut body["listener"];
                l["timeout_client_data"] = json!(o.timeout_client_data);
                l["timeout_member_connect"] = json!(o.timeout_member_connect);
                l["timeout_member_data"] = json!(o.timeout_member_data);
                l["timeout_tcp_inspect"] = json!(o.timeout_tcp_inspect);
                // Octavia refuses `insert_headers` on other listeners, even empty.
                if lb_inserts_headers(protocol) {
                    l["insert_headers"] = Value::Object(o.headers_map());
                }
                if let Some(c) = &o.allowed_cidrs {
                    l["allowed_cidrs"] = json!(c.iter().map(|x| x.trim()).collect::<Vec<_>>());
                }
                // Set or changed only: `null` would need Octavia 2.15 (like pools).
                if let Some(c) = o.ciphers() {
                    l["tls_ciphers"] = json!(c);
                }
            }
            if let Some((default_ref, sni)) = tls {
                check_listener_tls(protocol, Some(default_ref), sni).map_err(AppError::invalid)?;
                body["listener"]["default_tls_container_ref"] = json!(default_ref.trim());
                body["listener"]["sni_container_refs"] = json!(sni.iter().map(|r| r.trim()).collect::<Vec<_>>());
            }
            x.send(octavia(http::Method::PUT, format!("lbaas/listeners/{}", non_empty(id, "listener")?), body)).await?;
        }
        ListenerDelete { id } => {
            use openstack_sdk::api::load_balancer::v2::listener::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        PoolCreate { loadbalancer_id, listener_id, spec } => {
            use openstack_sdk::api::load_balancer::v2::pool::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::PoolBuilder::default();
            b.protocol(enum_of::<m::Protocol>(&spec.protocol, "pool protocol")?).lb_algorithm(enum_of::<m::LbAlgorithm>(&spec.algorithm, "algorithm")?);
            // Octavia takes one of the two: the listener implies its load balancer.
            match opt(listener_id) {
                Some(l) => b.listener_id(l),
                None => b.loadbalancer_id(non_empty(loadbalancer_id, "load balancer")?),
            };
            if let Some(v) = opt(&spec.name) {
                b.name(v);
            }
            if let Some(v) = opt(&spec.description) {
                b.description(v);
            }
            if let Some(kind) = spec.session_persistence.as_deref() {
                let mut sp = m::SessionPersistenceBuilder::default();
                sp._type(enum_of::<m::SessionPersistenceType>(kind, "session persistence")?);
                if let Some(c) = opt(&spec.cookie_name).filter(|_| kind == "APP_COOKIE") {
                    sp.cookie_name(c);
                }
                b.session_persistence(sp.build().map_err(build_err)?);
            }
            pool_tls!(b, spec);
            x.create(m::Request::builder().pool(b.build().map_err(build_err)?).build().map_err(build_err)?, "pool").await?;
        }
        // JSON: removing persistence means `"session_persistence": null`.
        PoolUpdate { id, name, description, algorithm, session_persistence, cookie_name, admin_up, protocol, tls } => {
            let (tls_enabled, tls_ciphers) = tls.clone().unwrap_or_default();
            LbPoolSpec {
                name: None,
                description: None,
                protocol: if protocol.is_empty() { "HTTP".into() } else { protocol.clone() },
                algorithm: algorithm.clone(),
                session_persistence: session_persistence.clone(),
                cookie_name: cookie_name.clone(),
                tls_enabled,
                tls_ciphers: tls_ciphers.clone(),
                admin_up: *admin_up,
            }
            .check()
            .map_err(AppError::invalid)?;
            let persistence = session_persistence.as_deref().map(|kind| match opt(cookie_name).filter(|_| kind == "APP_COOKIE") {
                Some(c) => json!({"type": kind, "cookie_name": c}),
                None => json!({"type": kind}),
            });
            let mut body = json!({"pool": {
                "name": name.trim(),
                "description": description.trim(),
                "lb_algorithm": algorithm,
                "session_persistence": persistence,
                "admin_state_up": admin_up,
            }});
            // Ciphers can be changed but not cleared here: `null` would need
            // Octavia 2.15 even when the dialog didn't offer them.
            if let Some((enabled, ciphers)) = tls {
                body["pool"]["tls_enabled"] = json!(enabled);
                if let Some(c) = opt(ciphers).filter(|_| *enabled) {
                    body["pool"]["tls_ciphers"] = json!(c);
                }
            }
            x.send(octavia(http::Method::PUT, format!("lbaas/pools/{}", non_empty(id, "pool")?), body)).await?;
        }
        PoolDelete { id } => {
            use openstack_sdk::api::load_balancer::v2::pool::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        MemberCreate { pool_id, spec } => {
            use openstack_sdk::api::load_balancer::v2::pool::member::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::MemberBuilder::default();
            member_fields!(b, spec);
            let ep = m::Request::builder().pool_id(pool_id.as_str()).member(b.build().map_err(build_err)?).build().map_err(build_err)?;
            x.create(ep, "member").await?;
        }
        MemberUpdate { pool_id, id, name, weight, backup, admin_up, monitor_address, monitor_port } => {
            if *weight > 256 {
                return Err(AppError::invalid("the weight must be 0-256"));
            }
            let monitor_address = opt(monitor_address);
            if let Some(a) = monitor_address {
                a.parse::<std::net::IpAddr>().map_err(|_| AppError::invalid(format!("not an IP address: {a}")))?;
            }
            if *monitor_port == Some(0) {
                return Err(AppError::invalid("the monitor port must be 1-65535"));
            }
            // JSON, not `member::set`: clearing the monitor address/port needs an
            // explicit `null`, which the SDK builder can't send (roadmap 54).
            let body = json!({"member": {
                "name": name.trim(),
                "weight": weight,
                "backup": backup,
                "admin_state_up": admin_up,
                "monitor_address": monitor_address,
                "monitor_port": monitor_port,
            }});
            let path = format!("lbaas/pools/{}/members/{}", non_empty(pool_id, "pool")?, non_empty(id, "member")?);
            x.send(octavia(http::Method::PUT, path, body)).await?;
        }
        AppCredentialCreate(spec) => {
            use openstack_sdk::api::identity::v3::user::application_credential::create as m;
            // Parsed first: the request borrows them until it is built.
            let rules = match opt(&spec.access_rules) {
                Some(text) => parse_access_rules(text).map_err(AppError::invalid)?,
                None => vec![],
            };
            let mut b = m::ApplicationCredentialBuilder::default();
            b.name(non_empty(&spec.name, "name")?).unrestricted(Some(spec.unrestricted));
            if let Some(v) = opt(&spec.description) {
                b.description(Some(Cow::from(v)));
            }
            if let Some(v) = opt(&spec.secret) {
                b.secret(Some(Cow::from(v)));
            }
            if let Some(v) = opt(&spec.expires_at) {
                b.expires_at(Some(Cow::from(v)));
            }
            if !rules.is_empty() {
                let list = rules
                    .iter()
                    .map(|r| {
                        let mut a = m::AccessRulesBuilder::default();
                        a.service(r.service.as_str()).path(r.path.as_str()).method(enum_of::<m::Method>(&r.method, "access rule method")?);
                        a.build().map_err(build_err)
                    })
                    .collect::<R<Vec<_>>>()?;
                b.access_rules(list);
            }
            // No roles: Keystone gives the credential all of the user's on the project.
            if !spec.roles.is_empty() {
                let roles = spec
                    .roles
                    .iter()
                    .map(|r| m::RolesBuilder::default().name(r.as_str()).build().map_err(build_err))
                    .collect::<R<Vec<_>>>()?;
                b.roles(roles);
            }
            let body = b.build().map_err(build_err)?;
            let ep = m::Request::builder().user_id(non_empty(&spec.user_id, "user")?).application_credential(body).build().map_err(build_err)?;
            // Keystone returns the secret once: shown to the user, never logged.
            if let Some(v) = x.call(ep).await? {
                let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let (id, name, secret) = (get("id"), get("name"), get("secret"));
                x.secret = Some(Secret {
                    file_name: format!("app-credential-{name}.txt"),
                    content: format!("Application credential ID: {id}\nName: {name}\nSecret: {secret}\n"),
                    kind: SecretKind::AppCredential,
                    fields: vec![("name".into(), name), ("id".into(), id), ("secret".into(), secret)],
                    extra: vec![],
                });
            }
        }
        ProjectCreate { name, description, enabled, domain_id } => {
            use openstack_sdk::api::identity::v3::project::create as m;
            let mut b = m::ProjectBuilder::default();
            // Always explicit: without it Keystone uses its default domain (roadmap 57).
            b.name(non_empty(name, "project name")?).enabled(*enabled).domain_id(Some(Cow::from(non_empty(domain_id, "domain")?)));
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            x.create(m::Request::builder().project(b.build().map_err(build_err)?).build().map_err(build_err)?, "project").await?;
        }
        ProjectUpdate { id, name, description, enabled } => {
            use openstack_sdk::api::identity::v3::project::set as m;
            let body = m::ProjectBuilder::default()
                .name(non_empty(name, "project name")?)
                .description(Some(Cow::from(description.trim())))
                .enabled(*enabled)
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(id, "project")?).project(body).build().map_err(build_err)?).await?;
        }
        ProjectDelete { id } => {
            use openstack_sdk::api::identity::v3::project::delete as m;
            x.send(m::Request::builder().id(non_empty(id, "project")?).build().map_err(build_err)?).await?;
        }
        ProjectRoleGrant { project_id, user_id, group_id, role_id } | ProjectRoleRevoke { project_id, user_id, group_id, role_id } => {
            let grant = matches!(action, ProjectRoleGrant { .. });
            let actor = match (opt(user_id), opt(group_id)) {
                (Some(u), None) => format!("users/{u}"),
                (None, Some(g)) => format!("groups/{g}"),
                _ => return Err(AppError::invalid("a role assignment needs a user or a group")),
            };
            let path = format!("projects/{}/{actor}/roles/{}", non_empty(project_id, "project")?, non_empty(role_id, "role")?);
            let method = if grant { http::Method::PUT } else { http::Method::DELETE };
            x.send(JsonRequest::new(method, openstack_sdk::types::ServiceType::Identity, path, None)).await?;
        }
        UserCreate(spec) => {
            let mut user = serde_json::Map::new();
            user.insert("name".into(), json!(non_empty(&spec.name, "user name")?));
            // Always explicit: without it Keystone uses its default domain (roadmap 57).
            user.insert("domain_id".into(), json!(non_empty(&spec.domain_id, "domain")?));
            if spec.password.is_empty() {
                return Err(AppError::invalid("a password is required"));
            }
            user.insert("password".into(), json!(spec.password));
            user.insert("enabled".into(), json!(spec.enabled));
            if let Some(v) = opt(&spec.description) {
                user.insert("description".into(), json!(v));
            }
            if let Some(v) = opt(&spec.email) {
                user.insert("email".into(), json!(v));
            }
            if let Some(v) = opt(&spec.project_id) {
                user.insert("default_project_id".into(), json!(v));
            }
            if spec.lock_password {
                user.insert("options".into(), json!({"lock_password": true}));
            }
            // JSON, not `user::create`: Keystone's `email` is an extra attribute the builder lacks.
            let ep = JsonRequest::new(http::Method::POST, openstack_sdk::types::ServiceType::Identity, "users", Some(json!({"user": user})));
            let new_id = x.create(ep, "user").await?;
            if let (Some(p), Some(r)) = (opt(&spec.project_id), opt(&spec.role_id)) {
                let path = format!("projects/{p}/users/{new_id}/roles/{r}");
                x.send(JsonRequest::new(http::Method::PUT, openstack_sdk::types::ServiceType::Identity, path, None)).await?;
            }
        }
        UserUpdate { id, name, email, description, project_id } => {
            let body = json!({"user": {
                "name": non_empty(name, "user name")?,
                "email": email.trim(),
                "description": description.trim(),
                "default_project_id": opt(project_id),
            }});
            x.send(JsonRequest::new(http::Method::PATCH, openstack_sdk::types::ServiceType::Identity, format!("users/{}", non_empty(id, "user")?), Some(body))).await?;
        }
        UserSetEnabled { id, enabled } => {
            let body = json!({"user": {"enabled": enabled}});
            x.send(JsonRequest::new(http::Method::PATCH, openstack_sdk::types::ServiceType::Identity, format!("users/{}", non_empty(id, "user")?), Some(body))).await?;
        }
        UserSetPassword { id, password } => {
            if password.is_empty() {
                return Err(AppError::invalid("a password is required"));
            }
            let body = json!({"user": {"password": password}});
            x.send(JsonRequest::new(http::Method::PATCH, openstack_sdk::types::ServiceType::Identity, format!("users/{}", non_empty(id, "user")?), Some(body))).await?;
        }
        UserDelete { id } => {
            use openstack_sdk::api::identity::v3::user::delete as m;
            x.send(m::Request::builder().id(non_empty(id, "user")?).build().map_err(build_err)?).await?;
        }
        GroupCreate { name, description, domain_id } => {
            use openstack_sdk::api::identity::v3::group::create as m;
            let mut b = m::GroupBuilder::default();
            // Always explicit: without it Keystone uses its default domain (roadmap 57).
            b.name(non_empty(name, "group name")?).domain_id(non_empty(domain_id, "domain")?);
            if let Some(d) = opt(description) {
                b.description(Some(Cow::from(d)));
            }
            x.create(m::Request::builder().group(b.build().map_err(build_err)?).build().map_err(build_err)?, "group").await?;
        }
        GroupUpdate { id, name, description } => {
            use openstack_sdk::api::identity::v3::group::set as m;
            let body = m::GroupBuilder::default().name(non_empty(name, "group name")?).description(Some(Cow::from(description.trim()))).build().map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(id, "group")?).group(body).build().map_err(build_err)?).await?;
        }
        GroupDelete { id } => {
            use openstack_sdk::api::identity::v3::group::delete as m;
            x.send(m::Request::builder().id(non_empty(id, "group")?).build().map_err(build_err)?).await?;
        }
        GroupAddUser { group_id, user_id } => {
            use openstack_sdk::api::identity::v3::group::user::set as m;
            x.send(m::Request::builder().group_id(non_empty(group_id, "group")?).id(non_empty(user_id, "user")?).build().map_err(build_err)?).await?;
        }
        GroupRemoveUser { group_id, user_id } => {
            use openstack_sdk::api::identity::v3::group::user::delete as m;
            x.send(m::Request::builder().group_id(non_empty(group_id, "group")?).id(non_empty(user_id, "user")?).build().map_err(build_err)?).await?;
        }
        RoleCreate { name } => {
            use openstack_sdk::api::identity::v3::role::create as m;
            let body = m::RoleBuilder::default().name(non_empty(name, "role name")?).build().map_err(build_err)?;
            x.create(m::Request::builder().role(body).build().map_err(build_err)?, "role").await?;
        }
        RoleUpdate { id, name } => {
            use openstack_sdk::api::identity::v3::role::set as m;
            let body = m::RoleBuilder::default().name(non_empty(name, "role name")?).build().map_err(build_err)?;
            x.send(m::Request::builder().id(non_empty(id, "role")?).role(body).build().map_err(build_err)?).await?;
        }
        RoleDelete { id } => {
            use openstack_sdk::api::identity::v3::role::delete as m;
            x.send(m::Request::builder().id(non_empty(id, "role")?).build().map_err(build_err)?).await?;
        }
        AppCredentialDelete { user_id, id } => {
            use openstack_sdk::api::identity::v3::user::application_credential::delete as m;
            x.send(m::Request::builder().user_id(non_empty(user_id, "user")?).id(non_empty(id, "application credential")?).build().map_err(build_err)?)
                .await?;
        }
        PoolSetMembers { pool_id, members } => {
            use openstack_sdk::api::load_balancer::v2::pool::member::replace as m;
            members.iter().try_for_each(LbMemberSpec::check).map_err(AppError::invalid)?;
            let list = members
                .iter()
                .map(|mb| {
                    let mut b = m::MembersBuilder::default();
                    member_fields!(b, mb);
                    b.build().map_err(build_err)
                })
                .collect::<R<Vec<_>>>()?;
            x.send(m::Request::builder().pool_id(non_empty(pool_id, "pool")?).members(list).build().map_err(build_err)?).await?;
        }
        MemberDelete { pool_id, id } => {
            use openstack_sdk::api::load_balancer::v2::pool::member::delete as m;
            x.send(m::Request::builder().pool_id(pool_id.as_str()).id(id.as_str()).build().map_err(build_err)?).await?;
        }
        MonitorCreate { pool_id, spec } => {
            use openstack_sdk::api::load_balancer::v2::healthmonitor::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::HealthmonitorBuilder::default();
            b.pool_id(pool_id.as_str())
                ._type(enum_of::<m::Type>(&spec.kind, "monitor type")?)
                .delay(spec.delay as i32)
                .timeout(spec.timeout as i32)
                .max_retries(i32::from(spec.max_retries));
            if let Some(n) = spec.max_retries_down {
                b.max_retries_down(i32::from(n));
            }
            if let Some(v) = opt(&spec.name) {
                b.name(v);
            }
            if let Some(v) = spec.http_method.as_deref() {
                b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
            }
            if let Some(v) = opt(&spec.url_path) {
                b.url_path(v);
            }
            if let Some(v) = opt(&spec.expected_codes) {
                b.expected_codes(v);
            }
            if !spec.admin_up {
                b.admin_state_up(false);
            }
            x.create(m::Request::builder().healthmonitor(b.build().map_err(build_err)?).build().map_err(build_err)?, "health monitor").await?;
        }
        MonitorUpdate { id, spec, admin_up } => {
            use openstack_sdk::api::load_balancer::v2::healthmonitor::set as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::HealthmonitorBuilder::default();
            b.name(spec.name.as_deref().map(str::trim).unwrap_or(""))
                .delay(spec.delay as i32)
                .timeout(spec.timeout as i32)
                .max_retries(i32::from(spec.max_retries))
                .admin_state_up(*admin_up);
            if let Some(n) = spec.max_retries_down {
                b.max_retries_down(i32::from(n));
            }
            if let Some(v) = spec.http_method.as_deref() {
                b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
            }
            if let Some(v) = opt(&spec.url_path) {
                b.url_path(v);
            }
            if let Some(v) = opt(&spec.expected_codes) {
                b.expected_codes(v);
            }
            x.send(m::Request::builder().id(id.as_str()).healthmonitor(b.build().map_err(build_err)?).build().map_err(build_err)?).await?;
        }
        MonitorDelete { id } => {
            use openstack_sdk::api::load_balancer::v2::healthmonitor::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        L7PolicyCreate { listener_id, spec } => {
            use openstack_sdk::api::load_balancer::v2::l7policy::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::L7policyBuilder::default();
            b.listener_id(non_empty(listener_id, "listener")?).action(enum_of::<m::Action>(&spec.action, "L7 action")?).admin_state_up(spec.admin_up);
            if let Some(v) = opt(&spec.name) {
                b.name(v);
            }
            if let Some(v) = opt(&spec.description) {
                b.description(v);
            }
            if let Some(p) = spec.position {
                b.position(p as i32);
            }
            match spec.targets() {
                (Some(pool), _) => {
                    b.redirect_pool_id(pool);
                }
                (_, Some(url)) => {
                    b.redirect_url(url);
                }
                _ => {}
            }
            x.create(m::Request::builder().l7policy(b.build().map_err(build_err)?).build().map_err(build_err)?, "L7 policy").await?;
        }
        L7PolicyUpdate { id, spec } => {
            spec.check().map_err(AppError::invalid)?;
            let (pool, url) = spec.targets();
            let mut p = json!({
                "name": opt(&spec.name).unwrap_or(""),
                "description": opt(&spec.description).unwrap_or(""),
                "action": spec.action,
                "redirect_pool_id": pool,
                "redirect_url": url,
                "admin_state_up": spec.admin_up,
            });
            // Octavia has no "last": an emptied position keeps the current one.
            if let Some(n) = spec.position {
                p["position"] = json!(n);
            }
            x.send(octavia(http::Method::PUT, format!("lbaas/l7policies/{}", non_empty(id, "L7 policy")?), json!({"l7policy": p}))).await?;
        }
        L7PolicyDelete { id } => {
            use openstack_sdk::api::load_balancer::v2::l7policy::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        L7RuleCreate { policy_id, spec } => {
            use openstack_sdk::api::load_balancer::v2::l7policy::rule::create as m;
            spec.check().map_err(AppError::invalid)?;
            let mut b = m::RuleBuilder::default();
            b._type(enum_of::<m::Type>(&spec.rule_type, "rule type")?)
                .compare_type(enum_of::<m::CompareType>(&spec.compare_type, "compare type")?)
                .value(spec.value.trim())
                .invert(spec.invert)
                .admin_state_up(spec.admin_up);
            if let Some(k) = opt(&spec.key) {
                b.key(k);
            }
            let ep = m::Request::builder().l7policy_id(non_empty(policy_id, "L7 policy")?).rule(b.build().map_err(build_err)?).build().map_err(build_err)?;
            x.create(ep, "L7 rule").await?;
        }
        L7RuleUpdate { policy_id, id, spec } => {
            spec.check().map_err(AppError::invalid)?;
            let body = json!({"rule": {
                "type": spec.rule_type,
                "compare_type": spec.compare_type,
                "key": opt(&spec.key),
                "value": spec.value.trim(),
                "invert": spec.invert,
                "admin_state_up": spec.admin_up,
            }});
            let path = format!("lbaas/l7policies/{}/rules/{}", non_empty(policy_id, "L7 policy")?, non_empty(id, "L7 rule")?);
            x.send(octavia(http::Method::PUT, path, body)).await?;
        }
        L7RuleDelete { policy_id, id } => {
            use openstack_sdk::api::load_balancer::v2::l7policy::rule::delete as m;
            x.send(m::Request::builder().l7policy_id(policy_id.as_str()).id(id.as_str()).build().map_err(build_err)?).await?;
        }
        LoadBalancerDelete { id, cascade } => {
            use openstack_sdk::api::load_balancer::v2::loadbalancer::delete as m;
            let mut b = m::Request::builder();
            b.id(id.as_str());
            if *cascade {
                b.cascade(true);
            }
            x.send(b.build().map_err(build_err)?).await?;
        }

        // -------------------------------------------------------- Security groups
        SecurityGroupCreate { name, description } => {
            use openstack_sdk::api::network::v2::security_group::create as m;
            let mut b = m::SecurityGroupBuilder::default();
            b.name(non_empty(name, "name")?);
            if let Some(d) = opt(description) {
                b.description(d.to_string());
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().security_group(body).build().map_err(build_err)?, "security group")
                .await?;
        }
        SecurityGroupUpdate { id, name, description } => {
            use openstack_sdk::api::network::v2::security_group::set as m;
            let body = m::SecurityGroupBuilder::default()
                .name(non_empty(name, "name")?)
                .description(opt(description).unwrap_or("").to_string())
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).security_group(body).build().map_err(build_err)?)
                .await?;
        }
        SecurityGroupDelete { id } => {
            use openstack_sdk::api::network::v2::security_group::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        RuleCreate(spec) => create_rule(x, spec).await?,
        RuleDelete { id } => {
            use openstack_sdk::api::network::v2::security_group_rule::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }

        // ----------------------------------------------------------- Floating IPs
        FloatingIpAllocate { network_id, description, port_id, qos_policy_id } => {
            use openstack_sdk::api::network::v2::floatingip::create as m;
            let mut b = m::FloatingipBuilder::default();
            b.floating_network_id(network_id.as_str());
            if let Some(q) = opt(qos_policy_id) {
                b.qos_policy_id(Some(Cow::from(q.to_string())));
            }
            if let Some(d) = opt(description) {
                b.description(d.to_string());
            }
            if let Some(p) = opt(port_id) {
                b.port_id(Some(Cow::from(p.to_string())));
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().floatingip(body).build().map_err(build_err)?, "floating ip").await?;
        }
        AdminFloatingIpAllocate { network_id, project_id, address, description } => {
            use openstack_sdk::api::network::v2::floatingip::create as m;
            let mut b = m::FloatingipBuilder::default();
            b.floating_network_id(non_empty(network_id, "pool")?).tenant_id(non_empty(project_id, "project")?);
            if let Some(a) = opt(address) {
                // Neutron would say "invalid input"; name the field instead.
                a.parse::<std::net::IpAddr>().map_err(|_| AppError::invalid(format!("not an IP address: {a}")))?;
                b.floating_ip_address(Some(Cow::from(a.to_string())));
            }
            if let Some(d) = opt(description) {
                b.description(d.to_string());
            }
            let body = b.build().map_err(build_err)?;
            x.create(m::Request::builder().floatingip(body).build().map_err(build_err)?, "floating ip").await?;
        }
        FloatingIpAssociate { id, port_id } => {
            use openstack_sdk::api::network::v2::floatingip::set as m;
            let body = m::FloatingipBuilder::default()
                .port_id(Some(Cow::from(port_id.to_string())))
                .build()
                .map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).floatingip(body).build().map_err(build_err)?).await?;
        }
        FloatingIpDisassociate { id } => {
            use openstack_sdk::api::network::v2::floatingip::set as m;
            let body = m::FloatingipBuilder::default().port_id(None::<Cow<str>>).build().map_err(build_err)?;
            x.send(m::Request::builder().id(id.as_str()).floatingip(body).build().map_err(build_err)?).await?;
        }
        FloatingIpRelease { id } => {
            use openstack_sdk::api::network::v2::floatingip::delete as m;
            x.send(m::Request::builder().id(id.as_str()).build().map_err(build_err)?).await?;
        }
        // ---------------------------------------------------------------- Bulk
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
    let mut subject = http::HeaderValue::from_str(token).map_err(|_| AppError::internal("token is not a valid header value"))?;
    subject.set_sensitive(true);
    x.send(m::Request::builder().header(http::HeaderName::from_static("x-subject-token"), subject).build().map_err(build_err)?).await
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
        x.results.push(ItemResult { label: label.clone(), error: r.err().map(|e| e.to_string()) });
    }
    Ok(())
}

async fn launch(x: &mut Exec<'_>, s: &LaunchSpec) -> R<()> {
    // 2.19 adds `description`; everything else is as in 2.1.
    use openstack_sdk::api::compute::v2::server::create_219 as m;
    let name = non_empty(&s.name, "name")?;
    let source_id = non_empty(&s.source_id, "boot source")?;
    if s.count == 0 || s.count > 100 {
        return Err(AppError::invalid("count must be between 1 and 100"));
    }
    let mut b = m::ServerBuilder::default();
    b.name(name).flavor_ref(non_empty(&s.flavor_id, "flavor")?);
    if s.count > 1 {
        b.min_count(s.count as i32).max_count(s.count as i32);
    }
    if let Some(z) = opt(&s.availability_zone) {
        b.availability_zone(z.to_string());
    }
    if let Some(k) = opt(&s.key_name) {
        b.key_name(k.to_string());
    }
    if let Some(d) = opt(&s.description) {
        b.description(Some(Cow::from(d.to_string())));
    }
    if let Some(u) = s.user_data.as_deref().filter(|u| !u.trim().is_empty()) {
        if u.len() > MAX_USER_DATA_BYTES {
            return Err(AppError::invalid("the customization script is larger than 16 KB"));
        }
        use base64::Engine;
        b.user_data(base64::engine::general_purpose::STANDARD.encode(u));
    }
    if s.config_drive {
        b.config_drive(true);
    }
    if let Some(p) = s.admin_pass.as_deref().filter(|p| !p.is_empty()) {
        b.admin_pass(p);
    }
    b.os_dcf_disk_config(if s.manual_disk_partition { m::OsDcfDiskConfig::Manual } else { m::OsDcfDiskConfig::Auto });
    // Boot source, following Horizon's launch wizard.
    let boot_volume = |src: m::SourceType, size: Option<u32>, delete: bool| {
        let mut bd = m::BlockDeviceMappingV2Builder::default();
        bd.boot_index(Some(Cow::from("0")))
            .uuid(source_id.to_string())
            .source_type(src)
            .destination_type(m::DestinationType::Volume)
            .delete_on_termination(delete);
        if let Some(gb) = size {
            bd.volume_size(gb as i32);
        }
        bd.build().map_err(build_err)
    };
    match s.source {
        BootSource::Image if s.create_volume => {
            let size = s.volume_size_gb.filter(|g| *g > 0).ok_or_else(|| AppError::invalid("volume size is required"))?;
            b.block_device_mapping_v2(vec![boot_volume(m::SourceType::Image, Some(size), s.delete_volume_on_terminate)?]);
        }
        BootSource::Image => {
            b.image_ref(source_id.to_string());
        }
        BootSource::Volume => {
            b.block_device_mapping_v2(vec![boot_volume(m::SourceType::Volume, None, s.delete_volume_on_terminate)?]);
        }
        BootSource::VolumeSnapshot => {
            b.block_device_mapping_v2(vec![boot_volume(
                m::SourceType::Snapshot,
                s.volume_size_gb.filter(|g| *g > 0),
                s.delete_volume_on_terminate,
            )?]);
        }
    }
    let nets = s
        .network_ids
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|n| m::NetworksBuilder::default().uuid(n.trim().to_string()).build().map_err(build_err))
        .chain(s.port_ids.iter().filter(|p| !p.trim().is_empty()).map(|p| {
            m::NetworksBuilder::default().port(Some(Cow::from(p.trim().to_string()))).build().map_err(build_err)
        }))
        .collect::<R<Vec<_>>>()?;
    if !nets.is_empty() {
        b.networks(nets);
    }
    let sgs = s
        .security_groups
        .iter()
        .filter(|g| !g.trim().is_empty())
        .map(|g| m::SecurityGroupsBuilder::default().name(g.trim().to_string()).build().map_err(build_err))
        .collect::<R<Vec<_>>>()?;
    if !sgs.is_empty() {
        b.security_groups(sgs);
    }
    let mut req = m::Request::builder();
    req.server(b.build().map_err(build_err)?);
    if let Some(g) = opt(&s.server_group_id) {
        let hints = m::OsSchedulerHintsBuilder::default()
            .group(vec![Cow::from(g.to_string())])
            .build()
            .map_err(build_err)?;
        req.os_scheduler_hints(hints);
    }
    x.create(req.build().map_err(build_err)?, "server").await?;
    Ok(())
}

async fn create_subnet(x: &mut Exec<'_>, network_id: &str, s: &SubnetSpec) -> R<()> {
    use openstack_sdk::api::network::v2::subnet::create as m;
    if s.ip_version != 4 && s.ip_version != 6 {
        return Err(AppError::invalid("ip version must be 4 or 6"));
    }
    let mut b = m::SubnetBuilder::default();
    b.network_id(network_id)
        .name(s.name.trim())
        .ip_version(s.ip_version as i32)
        .cidr(Some(Cow::from(non_empty(&s.cidr, "network address")?.to_string())))
        .enable_dhcp(s.enable_dhcp);
    match s.gateway_ip.as_deref().map(str::trim) {
        None => {}                                  // Neutron picks the first address
        Some("") => {
            b.gateway_ip(None::<Cow<str>>); // explicitly no gateway
        }
        Some(gw) => {
            b.gateway_ip(Some(Cow::from(gw.to_string())));
        }
    }
    if !s.dns_nameservers.is_empty() {
        b.dns_nameservers(s.dns_nameservers.iter().map(|d| Cow::from(d.trim().to_string())).collect::<Vec<_>>());
    }
    if !s.allocation_pools.is_empty() {
        let pools = s
            .allocation_pools
            .iter()
            .map(|(a, z)| {
                m::AllocationPoolsBuilder::default()
                    .start(a.trim().to_string())
                    .end(z.trim().to_string())
                    .build()
                    .map_err(build_err)
            })
            .collect::<R<Vec<_>>>()?;
        b.allocation_pools(pools);
    }
    // Horizon's *IPv6 Address Configuration Mode*: IPv6 subnets only (Neutron
    // refuses the modes on IPv4).
    if s.ip_version == 6 {
        if let Some(ra) = &s.ipv6_ra_mode {
            b.ipv6_ra_mode(enum_of::<m::Ipv6RaMode>(ra, "IPv6 RA mode")?);
        }
        if let Some(addr) = &s.ipv6_address_mode {
            b.ipv6_address_mode(enum_of::<m::Ipv6AddressMode>(addr, "IPv6 address mode")?);
        }
    }
    if !s.host_routes.is_empty() {
        let routes = s
            .host_routes
            .iter()
            .map(|(dest, hop)| {
                let (dest, hop) = crate::model::validate_route(dest, hop).map_err(AppError::invalid)?;
                m::HostRoutesBuilder::default().destination(dest).nexthop(hop).build().map_err(build_err)
            })
            .collect::<R<Vec<_>>>()?;
        b.host_routes(routes);
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().subnet(body).build().map_err(build_err)?, "subnet").await?;
    Ok(())
}

async fn create_rule(x: &mut Exec<'_>, s: &RuleSpec) -> R<()> {
    use openstack_sdk::api::network::v2::security_group_rule::create as m;
    let mut b = m::SecurityGroupRuleBuilder::default();
    b.security_group_id(s.security_group_id.as_str());
    b.direction(match s.direction.as_str() {
        "ingress" => m::Direction::Ingress,
        "egress" => m::Direction::Egress,
        _ => return Err(AppError::invalid("invalid direction")),
    });
    b.ethertype(match s.ethertype.as_str() {
        "IPv4" => m::Ethertype::Ipv4,
        "IPv6" => m::Ethertype::Ipv6,
        _ => return Err(AppError::invalid("invalid ethertype")),
    });
    if let Some(p) = opt(&s.protocol) {
        b.protocol(p.to_string());
    }
    if let (Some(a), Some(z)) = (s.port_min, s.port_max)
        && (a > z || a < 0 || z > 65535)
    {
        return Err(AppError::invalid("invalid port range"));
    }
    if let Some(a) = s.port_min {
        b.port_range_min(Some(a));
    }
    if let Some(z) = s.port_max.or(s.port_min) {
        b.port_range_max(Some(z));
    }
    match (opt(&s.remote_ip_prefix), opt(&s.remote_group_id)) {
        (Some(_), Some(_)) => return Err(AppError::invalid("use either a CIDR or a remote group, not both")),
        (Some(cidr), None) => {
            b.remote_ip_prefix(cidr.to_string());
        }
        (None, Some(g)) => {
            b.remote_group_id(g.to_string());
        }
        (None, None) => {}
    }
    if let Some(d) = opt(&s.description) {
        b.description(d.to_string());
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().security_group_rule(body).build().map_err(build_err)?, "rule").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EncryptionSpec;

    /// Dry-runs an action and returns the requests it would send.
    async fn dry(action: OsAction) -> Vec<RequestPreview> {
        // Every valid action of every test also passes `run_action`'s ID check (roadmap 48).
        action.check_ids().unwrap_or_else(|e| panic!("{action:?}: IDs refused: {e}"));
        assert_paths_checked(&action).await;
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &action).await.unwrap_or_else(|e| panic!("{action:?}: {e}"));
        x.outcome().requests
    }

    /// Roadmap 67: `check_ids` picks the fields to check by name (`id`, `*_id`…),
    /// so a path parameter named otherwise would skip it. Each string of the
    /// action is set in turn to a value with a `/`; the action must then be
    /// refused (by `check_ids` or by `apply`) or keep that value out of every
    /// request path. Run by `dry`, so on every action the tests build.
    async fn assert_paths_checked(action: &OsAction) {
        // `/` is what changes a path; dot segments would be normalised away
        // by the preview's URL and hide the marker.
        const MARKER: &str = "zz/zz";
        fn leaves(v: &Value, at: String, out: &mut Vec<String>) {
            match v {
                Value::String(_) => out.push(at),
                Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| leaves(x, format!("{at}/{i}"), out)),
                Value::Object(m) => m.iter().for_each(|(k, x)| leaves(x, format!("{at}/{k}"), out)),
                _ => {}
            }
        }
        let v = serde_json::to_value(action).unwrap();
        let mut at = Vec::new();
        leaves(&v, String::new(), &mut at);
        for field in at {
            let mut marked = v.clone();
            *marked.pointer_mut(&field).unwrap() = Value::String(MARKER.into());
            // Strings that are really enums (`kind`, `source`) don't deserialise.
            let Ok(a) = serde_json::from_value::<OsAction>(marked) else { continue };
            if a.check_ids().is_err() {
                continue;
            }
            let mut x = Exec::new(WriteMode::DryRun, None);
            if apply(&mut x, &a).await.is_err() {
                continue;
            }
            for r in x.outcome().requests {
                assert!(!r.path.contains(MARKER), "{}: `{field}` reaches the request path `{}` unchecked (roadmap 67)", a.kind(), r.path);
            }
        }
    }

    /// Roadmap 67: `assert_paths_checked` covers the actions the tests dry-run,
    /// so every variant must appear in this module. The list comes from serde's
    /// error for an unknown variant: a new variant needs no list to be updated.
    #[test]
    fn every_action_has_a_dry_run_test() {
        let err = serde_json::from_value::<OsAction>(serde_json::json!({"NoSuchAction": null})).unwrap_err().to_string();
        let names: Vec<&str> = err.split("expected one of ").nth(1).expect(&err).split('`').skip(1).step_by(2).collect();
        assert!(names.len() > 100, "variants read from serde: {err}");
        let src = include_str!("actions.rs");
        let tests = &src[src.find("\nmod tests").unwrap()..];
        let used = |n: &str| tests.match_indices(&format!("OsAction::{n}")).any(|(i, m)| !tests[i + m.len()..].starts_with(|c: char| c.is_alphanumeric()));
        let missing: Vec<&str> = names.into_iter().filter(|n| !used(n)).collect();
        assert!(missing.is_empty(), "OsAction variants without a dry-run test: {missing:?}");
    }

    fn body(r: &RequestPreview) -> Value {
        serde_json::from_str(r.body.as_deref().expect("request has a body")).unwrap()
    }

    fn id(s: &str) -> String {
        s.to_string()
    }

    #[tokio::test]
    async fn image_create_records_then_imports_the_url() {
        let spec = ImageSpec {
            name: id(" Debian "),
            url: Some(id("https://cloud.example/debian.qcow2")),
            disk_format: id("qcow2"),
            visibility: id("private"),
            min_disk_gb: Some(10),
            properties: vec![(id("architecture"), id("x86_64"))],
            ..Default::default()
        };
        let r = dry(OsAction::ImageCreate(spec.clone())).await;
        assert_eq!(calls(&r), [(id("POST"), id("/images")), (id("POST"), id("/images/%3Cnew%20image%20id%3E/import"))]);
        let b = body(&r[0]);
        assert_eq!((b["name"].as_str(), b["disk_format"].as_str(), b["container_format"].as_str()), (Some("Debian"), Some("qcow2"), Some("bare")));
        assert_eq!((b["visibility"].as_str(), b["protected"].as_bool(), b["min_disk"].as_i64(), b["architecture"].as_str()), (Some("private"), Some(false), Some(10), Some("x86_64")));
        assert_eq!(body(&r[1]), json!({"method": {"name": "web-download", "uri": "https://cloud.example/debian.qcow2"}}));
        let bare = dry(OsAction::ImageCreate(ImageSpec { url: None, ..spec.clone() })).await;
        assert_eq!(calls(&bare), [(id("POST"), id("/images"))], "no URL: only the record");
        // A file: the record, then the streamed PUT is only previewed (the browser sends it).
        let file = dry(OsAction::ImageCreate(ImageSpec { url: None, upload_bytes: Some(2048), ..spec.clone() })).await;
        assert_eq!(calls(&file), [(id("POST"), id("/images")), (id("PUT"), id("/images/<new image id>/file"))]);
        assert!(file[1].body.as_deref().unwrap_or_default().contains("2048 bytes"), "{:?}", file[1].body);
        let ami = dry(OsAction::ImageCreate(ImageSpec { disk_format: id("ami"), ..spec.clone() })).await;
        assert_eq!(body(&ami[0])["container_format"], "ami", "AMI-style images are their own container");
        for bad in [
            ImageSpec { url: Some(id("ftp://x/y")), ..spec.clone() },
            ImageSpec { url: Some(id("https://")), ..spec.clone() },
            ImageSpec { url: Some(id("https://a b")), ..spec.clone() },
            ImageSpec { upload_bytes: Some(1), ..spec.clone() },
            ImageSpec { url: None, upload_bytes: Some(0), ..spec.clone() },
            ImageSpec { name: id(" "), ..spec.clone() },
            ImageSpec { disk_format: id("exe"), ..spec.clone() },
            ImageSpec { visibility: id("everyone"), ..spec.clone() },
            ImageSpec { properties: vec![(id("status"), id("active"))], ..spec.clone() },
        ] {
            assert!(dry_err(OsAction::ImageCreate(bad.clone())).await, "{bad:?}");
        }
    }

    #[tokio::test]
    async fn off_mode_rejects_without_building() {
        let mut x = Exec::new(WriteMode::Off, None);
        assert!(apply(&mut x, &OsAction::ServerDelete { id: id("s1") }).await.is_err());
        assert!(x.outcome().requests.is_empty());
    }

    #[tokio::test]
    async fn dry_run_never_needs_a_client() {
        // `os` is None: if dry-run tried to send, it would fail with "no OpenStack connection".
        let r = dry(OsAction::ServerDelete { id: id("s1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/servers/s1"));
        assert_eq!(r[0].service, "compute");
    }

    #[tokio::test]
    async fn server_power_actions() {
        let cases = [
            (OsAction::ServerStart { id: id("s") }, "os-start"),
            (OsAction::ServerStop { id: id("s") }, "os-stop"),
            (OsAction::ServerPause { id: id("s") }, "pause"),
            (OsAction::ServerUnpause { id: id("s") }, "unpause"),
            (OsAction::ServerSuspend { id: id("s") }, "suspend"),
            (OsAction::ServerResume { id: id("s") }, "resume"),
            (OsAction::ServerLock { id: id("s") }, "lock"),
            (OsAction::ServerUnlock { id: id("s") }, "unlock"),
        ];
        for (action, key) in cases {
            let r = dry(action).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/servers/s/action"));
            assert!(body(&r[0]).get(key).is_some(), "{key}: {:?}", r[0].body);
        }
        let r = dry(OsAction::ServerReboot { id: id("s"), kind: RebootKind::Hard }).await;
        assert_eq!(body(&r[0]), json!({"reboot": {"type": "HARD"}}));
    }

    #[tokio::test]
    async fn server_rename_snapshot_and_volumes() {
        let r = dry(OsAction::ServerUpdate { id: id("s"), name: id(" web ") }).await;
        assert_eq!((r[0].method.as_str(), body(&r[0])), ("PUT", json!({"server": {"name": "web"}})));
        let r = dry(OsAction::ServerSnapshot { id: id("s"), name: id("snap") }).await;
        assert_eq!(body(&r[0])["createImage"]["name"], "snap");
        let r = dry(OsAction::ServerSetSecurityGroups { id: id("s"), add: vec![id("web")], remove: vec![id("default")] }).await;
        assert_eq!((r.len(), r[0].path.as_str(), r[1].path.as_str()), (2, "/servers/s/action", "/servers/s/action"));
        assert_eq!(body(&r[0]), json!({"addSecurityGroup": {"name": "web"}}), "Nova takes groups by name");
        assert_eq!(body(&r[1]), json!({"removeSecurityGroup": {"name": "default"}}));
        assert!(dry_err(OsAction::ServerSetSecurityGroups { id: id("s"), add: vec![], remove: vec![] }).await, "an empty change is refused");
        let r = dry(OsAction::ServerAttachVolume { server_id: id("s"), volume_id: id("v") }).await;
        assert_eq!(r[0].path, "/servers/s/os-volume_attachments");
        assert_eq!(body(&r[0])["volumeAttachment"]["volumeId"], "v");
        let r = dry(OsAction::ServerDetachVolume { server_id: id("s"), volume_id: id("v") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/servers/s/os-volume_attachments/v"));
    }

    fn launch_spec() -> LaunchSpec {
        LaunchSpec {
            name: "web".into(),
            availability_zone: Some("alh1".into()),
            count: 2,
            source: BootSource::Image,
            source_id: "img".into(),
            create_volume: true,
            volume_size_gb: Some(50),
            delete_volume_on_terminate: true,
            flavor_id: "f1".into(),
            network_ids: vec!["n1".into(), "".into()],
            security_groups: vec!["default".into()],
            key_name: Some("ops".into()),
            server_group_id: Some("g1".into()),
            port_ids: vec![],
            description: None,
            user_data: None,
            config_drive: false,
            manual_disk_partition: false,
            admin_pass: None,
        }
    }

    #[tokio::test]
    async fn launch_admin_password_is_sent_but_never_previewed() {
        let spec = LaunchSpec { admin_pass: Some("s3cr3t-Pa55".into()), ..launch_spec() };
        let r = dry(OsAction::ServerLaunch(spec)).await;
        let preview = r[0].body.as_deref().unwrap();
        assert!(preview.contains("\"adminPass\": \"********\"") && !preview.contains("s3cr3t"));
        let none = dry(OsAction::ServerLaunch(launch_spec())).await;
        assert!(!none[0].body.as_deref().unwrap().contains("adminPass"), "not sent by default");
    }

    #[tokio::test]
    async fn launch_boot_from_new_volume() {
        let r = dry(OsAction::ServerLaunch(launch_spec())).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/servers"));
        let b = body(&r[0]);
        let s = &b["server"];
        assert_eq!(s["name"], "web");
        assert_eq!(s["flavorRef"], "f1");
        assert_eq!((s["min_count"].clone(), s["max_count"].clone()), (json!(2), json!(2)));
        assert!(s.get("imageRef").is_none(), "boot from volume must not set imageRef");
        let bdm = &s["block_device_mapping_v2"][0];
        assert_eq!(bdm["source_type"], "image");
        assert_eq!(bdm["destination_type"], "volume");
        assert_eq!(bdm["volume_size"], 50);
        assert_eq!(bdm["boot_index"], "0");
        assert_eq!(s["networks"], json!([{"uuid": "n1"}]), "empty network ids are dropped");
        assert_eq!(s["security_groups"], json!([{"name": "default"}]));
        assert_eq!(s["key_name"], "ops");
        assert_eq!(b["os:scheduler_hints"]["group"], json!(["g1"]));
        assert_eq!(s["OS-DCF:diskConfig"], "AUTO");
        assert!(s.get("user_data").is_none() && s.get("description").is_none());
    }

    #[tokio::test]
    async fn launch_with_ports_description_and_customization_script() {
        let mut spec = launch_spec();
        spec.port_ids = vec!["p1".into()];
        spec.description = Some(" web tier ".into());
        spec.user_data = Some("#cloud-config\n".into());
        spec.config_drive = true;
        spec.manual_disk_partition = true;
        let s = body(&dry(OsAction::ServerLaunch(spec.clone())).await[0])["server"].clone();
        assert_eq!(s["networks"], json!([{"uuid": "n1"}, {"port": "p1"}]), "networks first, then ports");
        assert_eq!(s["description"], "web tier");
        assert_eq!(s["user_data"], "I2Nsb3VkLWNvbmZpZwo=");
        assert_eq!((s["config_drive"].clone(), s["OS-DCF:diskConfig"].clone()), (json!(true), json!("MANUAL")));

        spec.user_data = Some("x".repeat(MAX_USER_DATA_BYTES + 1));
        let mut x = Exec::new(WriteMode::DryRun, None);
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec)).await.is_err());
    }

    #[tokio::test]
    async fn launch_from_image_without_volume_and_validation() {
        let mut spec = launch_spec();
        spec.create_volume = false;
        spec.count = 1;
        spec.server_group_id = None;
        let b = body(&dry(OsAction::ServerLaunch(spec.clone())).await[0]);
        assert_eq!(b["server"]["imageRef"], "img");
        assert!(b["server"].get("block_device_mapping_v2").is_none());
        assert!(b["server"].get("min_count").is_none());

        let mut x = Exec::new(WriteMode::DryRun, None);
        spec.name = "  ".into();
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec.clone())).await.is_err());
        spec.name = "ok".into();
        spec.count = 0;
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec)).await.is_err());
    }

    #[tokio::test]
    async fn image_update_is_a_json_patch() {
        let rename = ImageChanges { name: Some(id(" n ")), ..Default::default() };
        let r = dry(OsAction::ImageUpdate { id: id("i"), changes: rename }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("PATCH", "/images/i", "image"));
        assert_eq!(body(&r[0]), json!([{"op": "replace", "path": "/name", "value": "n"}]), "only what changed");
        let all = ImageChanges {
            visibility: Some(id("shared")),
            protected: Some(true),
            min_disk_gb: Some(10),
            min_ram_mb: Some(512),
            disk_format: Some(id("qcow2")),
            set: vec![(id("architecture"), id("x86_64"))],
            remove: vec![id("description")],
            ..Default::default()
        };
        let r = dry(OsAction::ImageUpdate { id: id("i"), changes: all }).await;
        assert_eq!(
            body(&r[0]),
            json!([
                {"op": "replace", "path": "/visibility", "value": "shared"},
                {"op": "replace", "path": "/protected", "value": true},
                {"op": "replace", "path": "/min_disk", "value": 10},
                {"op": "replace", "path": "/min_ram", "value": 512},
                {"op": "replace", "path": "/disk_format", "value": "qcow2"},
                {"op": "remove", "path": "/description"},
                {"op": "add", "path": "/architecture", "value": "x86_64"},
            ]),
            "integers stay integers, properties are added or removed"
        );
        for bad in [
            ImageChanges { visibility: Some(id("everyone")), ..Default::default() },
            ImageChanges { name: Some(id(" ")), ..Default::default() },
            ImageChanges::default(),
        ] {
            let mut x = Exec::new(WriteMode::DryRun, None);
            assert!(apply(&mut x, &OsAction::ImageUpdate { id: id("i"), changes: bad.clone() }).await.is_err(), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn metadata_is_replaced_or_patched() {
        let md = vec![(id(" role "), id("web")), (id("env"), id("dev"))];
        let r = dry(OsAction::ServerSetMetadata { id: id("s"), metadata: md.clone() }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("PUT", "/servers/s/metadata", "compute"));
        assert_eq!(body(&r[0]), json!({"metadata": {"env": "dev", "role": "web"}}));
        let r = dry(OsAction::VolumeSetMetadata { id: id("v"), metadata: vec![] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/volumes/v/metadata"));
        assert_eq!(body(&r[0]), json!({"metadata": {}}), "empty removes everything");
        let r = dry(OsAction::SnapshotSetMetadata { id: id("sn"), metadata: md }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/snapshots/sn/metadata"));
        let r = dry(OsAction::ImageSetProperties { id: id("i"), set: vec![(id("os_distro"), id("debian")), (id("a/b"), id("1"))], remove: vec![id("old")] }).await;
        assert_eq!((r[0].method.as_str(), r[0].service.as_str()), ("PATCH", "image"));
        assert_eq!(
            body(&r[0]),
            json!([
                {"op": "remove", "path": "/old"},
                {"op": "add", "path": "/a~1b", "value": "1"},
                {"op": "add", "path": "/os_distro", "value": "debian"},
            ])
        );
        for bad in [
            OsAction::ServerSetMetadata { id: id("s"), metadata: vec![(id("k"), id("1")), (id("k"), id("2"))] },
            OsAction::ImageSetProperties { id: id("i"), set: vec![(id("min_disk"), id("10"))], remove: vec![] },
            OsAction::ImageSetProperties { id: id("i"), set: vec![], remove: vec![id("visibility")] },
            OsAction::ImageSetProperties { id: id("i"), set: vec![], remove: vec![] },
        ] {
            let mut x = Exec::new(WriteMode::DryRun, None);
            assert!(apply(&mut x, &bad).await.is_err(), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn quota_updates_send_only_the_changed_keys() {
        let q = |service, values: &[(&str, i64)]| OsAction::QuotaUpdate {
            service,
            project_id: id("p1"),
            values: values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        };
        let expect = [
            (QuotaService::Compute, "compute", "/os-quota-sets/p1", "quota_set", "cores"),
            (QuotaService::Volume, "block-storage", "/os-quota-sets/p1", "quota_set", "gigabytes"),
            (QuotaService::Network, "network", "/quotas/p1", "quota", "port"),
            (QuotaService::LoadBalancer, "load-balancer", "/lbaas/quotas/p1", "quota", "member"),
            (QuotaService::KeyManager, "key-manager", "/v1/project-quotas/p1", "project_quotas", "containers"),
        ];
        for (service, ty, path, wrap, key) in expect {
            let r = dry(q(service, &[(key, 20), (service.keys()[0], -1)])).await;
            assert_eq!((r.len(), r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), (1, "PUT", path, ty), "{service:?}");
            assert_eq!(body(&r[0]), json!({ wrap: { key: 20, service.keys()[0]: -1 } }), "{service:?}");
        }
        assert!(dry_err(q(QuotaService::Compute, &[])).await, "nothing changed");
        assert!(dry_err(q(QuotaService::Compute, &[("port", 3)])).await, "a Neutron key sent to Nova");
        assert!(dry_err(q(QuotaService::Network, &[("port", -2)])).await, "below -1");
    }

    #[tokio::test]
    async fn keypairs_and_server_groups() {
        let import = |key: &str, kind: &str| OsAction::KeyPairImport { name: id("k"), public_key: id(key), key_type: id(kind) };
        let r = dry(import("ssh-ed25519 AAAA", "ssh")).await;
        assert_eq!(body(&r[0]), json!({"keypair": {"name": "k", "public_key": "ssh-ed25519 AAAA", "type": "ssh"}}));
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
        let r = dry(import(pem, "x509")).await;
        assert_eq!(body(&r[0]), json!({"keypair": {"name": "k", "public_key": pem, "type": "x509"}}));
        assert!(dry_err(import(pem, "ssh")).await, "a certificate is not an SSH key");
        assert!(dry_err(import("ssh-ed25519 AAAA", "x509")).await, "an SSH key is not a certificate");
        assert!(dry_err(import("ssh-ed25519 AAAA", "rsa")).await, "unknown type");
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &OsAction::KeyPairGenerate { name: id(" k ") }).await.unwrap();
        let out = x.outcome();
        assert!(out.secret.is_none(), "dry-run never generates a usable private key");
        let kp = &body(&out.requests[0])["keypair"];
        assert_eq!((kp["name"].as_str(), kp["type"].as_str()), (Some("k"), Some("ssh")));
        assert!(kp["public_key"].as_str().unwrap().starts_with("ssh-ed25519 "));
        assert!(!format!("{out:?}").contains("PRIVATE KEY"));
        let r = dry(OsAction::KeyPairDelete { name: id("k") }).await;
        assert_eq!(r[0].path, "/os-keypairs/k");
        let r = dry(OsAction::ServerGroupCreate { name: id("g"), policy: id("soft-anti-affinity") }).await;
        assert_eq!(body(&r[0])["server_group"]["policy"], "soft-anti-affinity");
    }

    #[tokio::test]
    async fn volumes_and_snapshots() {
        let r = dry(OsAction::VolumeCreate {
            name: id("data"),
            description: None,
            size_gb: 10,
            volume_type: Some(id("ssd")),
            availability_zone: None,
            source: VolumeSource::Image,
            source_id: Some(id("img")),
        })
        .await;
        let v = &body(&r[0])["volume"];
        assert_eq!((v["size"].clone(), v["imageRef"].clone(), v["volume_type"].clone()), (json!(10), json!("img"), json!("ssd")));
        assert!(v.get("availability_zone").is_none());

        let mut x = Exec::new(WriteMode::DryRun, None);
        let no_src = OsAction::VolumeCreate {
            name: id("x"), description: None, size_gb: 1, volume_type: None,
            availability_zone: None, source: VolumeSource::Snapshot, source_id: None,
        };
        assert!(apply(&mut x, &no_src).await.is_err());

        let r = dry(OsAction::VolumeExtend { id: id("v"), new_size_gb: 20 }).await;
        assert_eq!(body(&r[0]), json!({"os-extend": {"new_size": 20}}));
        let r = dry(OsAction::VolumeSnapshotCreate { volume_id: id("v"), name: id("s"), description: None, force: true }).await;
        assert_eq!(body(&r[0])["snapshot"]["force"], true);
        let r = dry(OsAction::SnapshotDelete { id: id("s") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("DELETE", "/snapshots/s", "block-storage"));
    }

    #[tokio::test]
    async fn volume_backups_are_created_edited_restored_and_deleted() {
        let r = dry(OsAction::BackupCreate { volume_id: id("v1"), name: id("nightly"), description: None, incremental: false, force: false, snapshot_id: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/backups", "block-storage"));
        assert_eq!(body(&r[0]), json!({"backup": {"name": "nightly", "volume_id": "v1"}}), "flags are left out unless set");
        let r = dry(OsAction::BackupCreate { volume_id: id("v1"), name: id("n"), description: Some("d".into()), incremental: true, force: true, snapshot_id: Some("s1".into()) }).await;
        assert_eq!(body(&r[0]), json!({"backup": {"name": "n", "description": "d", "incremental": true, "force": true, "snapshot_id": "s1", "volume_id": "v1"}}));
        let r = dry(OsAction::BackupUpdate { id: id("b1"), name: id("n2"), description: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/backups/b1"));
        assert_eq!(body(&r[0]), json!({"backup": {"name": "n2", "description": ""}}), "an empty description clears it");
        let r = dry(OsAction::BackupRestore { id: id("b1"), volume_id: None, name: Some("copy".into()) }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/backups/b1/restore"));
        assert_eq!(body(&r[0]), json!({"restore": {"name": "copy"}}));
        let r = dry(OsAction::BackupRestore { id: id("b1"), volume_id: Some("v9".into()), name: Some("ignored".into()) }).await;
        assert_eq!(body(&r[0]), json!({"restore": {"volume_id": "v9"}}), "restoring over a volume sends no name");
        let r = dry(OsAction::BackupDelete { id: id("b1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/backups/b1"));
        assert_eq!(OsAction::BackupDelete { id: id("b1") }.plugins(), [Plugin::Backup]);
    }

    #[tokio::test]
    async fn volume_groups_and_group_snapshots_are_deleted() {
        let r = dry(OsAction::VolumeGroupDelete { id: id("g1"), delete_volumes: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/groups/g1/action", "block-storage"), "with the group's ID, unlike the SDK's delete_313");
        assert_eq!(body(&r[0]), json!({"delete": {"delete-volumes": true}}));
        let r = dry(OsAction::VolumeGroupDelete { id: id("g1"), delete_volumes: false }).await;
        assert_eq!(body(&r[0]), json!({"delete": {"delete-volumes": false}}), "sent explicitly: Cinder then refuses a group with volumes");
        let req = volume_group_delete("g1", false).unwrap();
        assert_eq!(req.headers.as_ref().unwrap()["openstack-api-version"], "volume 3.13", "group actions need 3.13");
        let r = dry(OsAction::GroupSnapshotDelete { id: id("gs1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/group_snapshots/gs1"));
    }

    #[tokio::test]
    async fn volume_group_writes() {
        let create = |types: &[&str], az: Option<&str>| OsAction::VolumeGroupCreate {
            name: id(" db "),
            description: Some(id("")),
            group_type_id: id("gt1"),
            volume_type_ids: types.iter().map(|t| t.to_string()).collect(),
            availability_zone: az.map(str::to_string),
        };
        let r = dry(create(&["vt1", "vt2"], Some("nova"))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/groups"));
        assert_eq!(body(&r[0]), json!({"group": {"name": "db", "group_type": "gt1", "volume_types": ["vt1", "vt2"], "availability_zone": "nova"}}));
        assert!(body(&dry(create(&["vt1"], None)).await[0])["group"].get("availability_zone").is_none(), "no zone: Cinder picks");
        assert!(dry_err(create(&[], None)).await, "Cinder needs at least one volume type");

        let from = |g: Option<&str>, s: Option<&str>| OsAction::VolumeGroupCreateFrom {
            name: id("copy"),
            description: Some(id("clone")),
            source_group_id: g.map(str::to_string),
            group_snapshot_id: s.map(str::to_string),
        };
        let r = dry(from(Some("g1"), None)).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/groups/action"));
        assert_eq!(body(&r[0]), json!({"create-from-src": {"name": "copy", "description": "clone", "source_group_id": "g1"}}));
        assert_eq!(body(&dry(from(None, Some("gs1"))).await[0]), json!({"create-from-src": {"name": "copy", "description": "clone", "group_snapshot_id": "gs1"}}));
        assert!(dry_err(from(None, None)).await && dry_err(from(Some("g1"), Some("gs1"))).await, "exactly one source");

        let r = dry(OsAction::VolumeGroupUpdate { id: id("g1"), name: id("db2"), description: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/groups/g1"));
        assert_eq!(body(&r[0]), json!({"group": {"name": "db2", "description": ""}}), "an empty description clears it");

        let set = |add: &[&str], remove: &[&str]| OsAction::VolumeGroupSetVolumes {
            id: id("g1"),
            add_volume_ids: add.iter().map(|t| t.to_string()).collect(),
            remove_volume_ids: remove.iter().map(|t| t.to_string()).collect(),
        };
        let r = dry(set(&["v1", "v2"], &["v3"])).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/groups/g1"));
        assert_eq!(body(&r[0]), json!({"group": {"add_volumes": "v1,v2", "remove_volumes": "v3"}}), "Cinder's comma-separated strings");
        assert_eq!(body(&dry(set(&[], &["v3"])).await[0]), json!({"group": {"remove_volumes": "v3"}}), "an empty list is left out");
        assert!(dry_err(set(&[], &[])).await);

        let r = dry(OsAction::GroupSnapshotCreate { group_id: id("g1"), name: id("before-upgrade"), description: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/group_snapshots"));
        assert_eq!(body(&r[0]), json!({"group_snapshot": {"group_id": "g1", "name": "before-upgrade"}}));
    }

    #[tokio::test]
    async fn network_with_subnet_chains_the_new_id() {
        let spec = SubnetSpec {
            name: "sub".into(),
            cidr: "10.0.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some("".into()),
            enable_dhcp: true,
            dns_nameservers: vec!["8.8.8.8".into()],
            allocation_pools: vec![("10.0.0.10".into(), "10.0.0.99".into())],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![],
        };
        let r = dry(OsAction::NetworkCreate { name: id("net"), admin_up: true, subnet: Some(spec) }).await;
        assert_eq!(r.len(), 2);
        assert_eq!(body(&r[0]), json!({"network": {"name": "net", "admin_state_up": true}}));
        let s = &body(&r[1])["subnet"];
        assert_eq!(s["network_id"], "<new network id>");
        assert_eq!(s["gateway_ip"], Value::Null, "empty gateway disables it");
        assert_eq!(s["allocation_pools"], json!([{"start": "10.0.0.10", "end": "10.0.0.99"}]));
        assert_eq!(r[1].path, "/subnets");
    }

    #[tokio::test]
    async fn admin_network_sends_owner_provider_and_flags() {
        let spec = |provider| AdminNetworkSpec {
            name: " ext ".into(), project_id: "p1".into(), provider, admin_up: true, shared: true, external: true, mtu: Some(1450),
            az_hints: vec!["az1".into()], subnet: None,
        };
        let vlan = ProviderNetwork { network_type: "vlan".into(), physical_network: Some("physnet1".into()), segmentation_id: Some(120) };
        let r = dry(OsAction::AdminNetworkCreate(Box::new(spec(Some(vlan))))).await;
        assert_eq!((r.len(), r[0].method.as_str(), r[0].path.as_str()), (1, "POST", "/networks"));
        assert_eq!(body(&r[0]), json!({"network": {
            "name": "ext", "admin_state_up": true, "shared": true, "router:external": true, "tenant_id": "p1",
            "mtu": 1450, "availability_zone_hints": ["az1"],
            "provider:network_type": "vlan", "provider:physical_network": "physnet1", "provider:segmentation_id": "120"
        }}));
        let r = dry(OsAction::AdminNetworkCreate(Box::new(spec(None)))).await;
        assert!(body(&r[0])["network"].get("provider:network_type").is_none(), "no provider: Neutron's default type");
    }

    #[tokio::test]
    async fn admin_allocates_a_floating_ip_to_another_project() {
        let alloc = |address: Option<&str>| OsAction::AdminFloatingIpAllocate {
            network_id: id("ext"), project_id: id("p2"), address: address.map(Into::into), description: Some(id("vpn")),
        };
        let r = dry(alloc(Some("203.0.113.10"))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/floatingips"));
        assert_eq!(body(&r[0]), json!({"floatingip": {"floating_network_id": "ext", "tenant_id": "p2", "floating_ip_address": "203.0.113.10", "description": "vpn"}}));
        assert!(body(&dry(alloc(None)).await[0])["floatingip"].get("floating_ip_address").is_none(), "no address: Neutron picks one");
        assert!(dry_err(alloc(Some("203.0.113"))).await, "not an IP address");
    }

    fn calls(r: &[RequestPreview]) -> Vec<(String, String)> {
        r.iter().map(|q| (q.method.clone(), q.path.clone())).collect()
    }

    #[tokio::test]
    async fn flavors_are_created_private_for_projects_and_edited_like_horizon() {
        let spec = |projects: Vec<String>| FlavorSpec {
            name: id(" m1.big "), flavor_id: Some(id("auto")), vcpus: 4, ram_mb: 8192, disk_gb: 40, ephemeral_gb: 0, swap_mb: 0, rxtx_factor: None, projects,
        };
        let r = dry(OsAction::FlavorCreate(Box::new(spec(vec![id("p1"), id("p2")])))).await;
        assert_eq!(
            body(&r[0]),
            json!({"flavor": {"name": "m1.big", "vcpus": 4, "ram": 8192, "disk": 40, "OS-FLV-EXT-DATA:ephemeral": 0, "swap": 0, "os-flavor-access:is_public": false}}),
            "`auto`: Nova picks the ID"
        );
        assert_eq!(calls(&r)[1..], [(id("POST"), id("/flavors/%3Cnew%20flavor%20id%3E/action")), (id("POST"), id("/flavors/%3Cnew%20flavor%20id%3E/action"))], "access on the new flavor (the dry-run placeholder, encoded)");
        assert_eq!(body(&r[2]), json!({"addTenantAccess": {"tenant": "p2"}}));
        let r = dry(OsAction::FlavorCreate(Box::new(FlavorSpec { flavor_id: Some(id("big")), rxtx_factor: Some(2.0), ..spec(vec![]) }))).await;
        assert_eq!((r.len(), &body(&r[0])["flavor"]["os-flavor-access:is_public"], &body(&r[0])["flavor"]["id"]), (1, &json!(true), &json!("big")), "no projects: public");
        assert_eq!(body(&r[0])["flavor"]["rxtx_factor"], "2");
        assert!(dry_err(OsAction::FlavorCreate(Box::new(FlavorSpec { vcpus: 0, ..spec(vec![]) }))).await, "at least one VCPU");
        assert_eq!(calls(&dry(OsAction::FlavorDelete { id: id("f1") }).await), [(id("DELETE"), id("/flavors/f1"))]);
        let r = dry(OsAction::FlavorSetExtraSpecs { id: id("f1"), set: vec![(id("hw:cpu_policy"), id("dedicated"))], unset: vec![id("quota:vif")] }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/flavors/f1/os-extra_specs")), (id("DELETE"), id("/flavors/f1/os-extra_specs/quota:vif"))]);
        assert_eq!(body(&r[0]), json!({"extra_specs": {"hw:cpu_policy": "dedicated"}}));
        assert!(dry_err(OsAction::FlavorSetExtraSpecs { id: id("f1"), set: vec![], unset: vec![] }).await, "nothing changed");
        let r = dry(OsAction::FlavorSetAccess { id: id("f1"), add: vec![id("p3")], remove: vec![id("p1")] }).await;
        assert_eq!((body(&r[0]), body(&r[1])), (json!({"addTenantAccess": {"tenant": "p3"}}), json!({"removeTenantAccess": {"tenant": "p1"}})));
    }

    #[tokio::test]
    async fn aggregates_hosts_and_metadata() {
        let r = dry(OsAction::AggregateCreate { name: id("gpu"), zone: Some(id("az1")), hosts: vec![id("c1")] }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/os-aggregates")), (id("POST"), id("/os-aggregates/%3Cnew%20aggregate%20id%3E/action"))], "add_host on the new aggregate (the dry-run placeholder, encoded)");
        assert_eq!((body(&r[0]), body(&r[1])), (json!({"aggregate": {"name": "gpu", "availability_zone": "az1"}}), json!({"add_host": {"host": "c1"}})));
        let r = dry(OsAction::AggregateUpdate { id: id("7"), name: id("gpu"), zone: None }).await;
        assert_eq!(calls(&r), [(id("PUT"), id("/os-aggregates/7"))]);
        assert_eq!(body(&r[0]), json!({"aggregate": {"name": "gpu", "availability_zone": null}}), "an empty zone clears it");
        let r = dry(OsAction::AggregateSetHosts { id: id("7"), add: vec![id("c2")], remove: vec![id("c1")] }).await;
        assert_eq!((body(&r[0]), body(&r[1])), (json!({"add_host": {"host": "c2"}}), json!({"remove_host": {"host": "c1"}})));
        let r = dry(OsAction::AggregateSetMetadata { id: id("7"), set: vec![(id("ssd"), id("true"))], unset: vec![id("gpu")] }).await;
        assert_eq!(body(&r[0]), json!({"set_metadata": {"metadata": {"ssd": "true", "gpu": null}}}), "removed keys are null");
        assert_eq!(calls(&dry(OsAction::AggregateDelete { id: id("7") }).await), [(id("DELETE"), id("/os-aggregates/7"))]);
    }

    #[tokio::test]
    async fn metadata_definitions_import_edit_associate_and_delete() {
        let def = r#"{"namespace": "OS::Acme::Gpu", "display_name": "GPU", "visibility": "public", "properties": {"gpu:model": {"type": "string"}}}"#;
        let r = dry(OsAction::MetadefNamespaceImport { definition: id(def), public: false, protected: true }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/metadefs/namespaces"))]);
        let b = body(&r[0]);
        assert_eq!((b["visibility"].clone(), b["protected"].clone(), b["properties"]["gpu:model"]["type"].clone()), (json!("private"), json!(true), json!("string")), "the form decides visibility; the rest is kept");
        assert!(dry_err(OsAction::MetadefNamespaceImport { definition: id("{not json"), public: true, protected: false }).await);
        assert!(dry_err(OsAction::MetadefNamespaceImport { definition: id(r#"{"display_name": "x"}"#), public: true, protected: false }).await, "a namespace is required");
        assert!(dry_err(OsAction::MetadefNamespaceImport { definition: id(r#"{"namespace": "a/b"}"#), public: true, protected: false }).await, "it becomes a path segment");
        let r = dry(OsAction::MetadefNamespaceUpdate { id: id("OS::Acme::Gpu"), display_name: Some(id("GPU")), description: None, public: true, protected: false }).await;
        assert_eq!(calls(&r), [(id("PUT"), id("/metadefs/namespaces/OS::Acme::Gpu"))]);
        assert_eq!(body(&r[0]), json!({"namespace": "OS::Acme::Gpu", "display_name": "GPU", "visibility": "public", "protected": false}));
        assert_eq!(calls(&dry(OsAction::MetadefNamespaceDelete { id: id("OS::Acme::Gpu") }).await), [(id("DELETE"), id("/metadefs/namespaces/OS::Acme::Gpu"))]);
        let r = dry(OsAction::MetadefNamespaceSetTypes { id: id("OS::Acme::Gpu"), add_type_ids: vec![id("OS::Nova::Flavor")], remove_type_ids: vec![id("OS::Glance::Image")] }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/metadefs/namespaces/OS::Acme::Gpu/resource_types")), (id("DELETE"), id("/metadefs/namespaces/OS::Acme::Gpu/resource_types/OS::Glance::Image"))]);
        assert_eq!(body(&r[0]), json!({"name": "OS::Nova::Flavor"}));
        assert!(dry_err(OsAction::MetadefNamespaceSetTypes { id: id("n"), add_type_ids: vec![], remove_type_ids: vec![] }).await, "nothing changed");
    }

    #[tokio::test]
    async fn default_quotas_update_the_default_class() {
        let values = |pairs: &[(&str, i64)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect::<std::collections::BTreeMap<_, _>>();
        let r = dry(OsAction::DefaultQuotaUpdate { service: QuotaService::Compute, values: values(&[("cores", 40), ("instances", -1)]) }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("PUT"), id("/os-quota-class-sets/default"))], json!({"quota_class_set": {"cores": 40, "instances": -1}})));
        let r = dry(OsAction::DefaultQuotaUpdate { service: QuotaService::Volume, values: values(&[("gigabytes", 2000)]) }).await;
        assert_eq!(body(&r[0]), json!({"quota_class_set": {"gigabytes": 2000}}));
        assert!(dry_err(OsAction::DefaultQuotaUpdate { service: QuotaService::Network, values: values(&[("port", 10)]) }).await, "Neutron has no quota classes");
        assert!(dry_err(OsAction::DefaultQuotaUpdate { service: QuotaService::Compute, values: values(&[("gigabytes", 1)]) }).await, "a Cinder key on Nova");
        assert!(dry_err(OsAction::DefaultQuotaUpdate { service: QuotaService::Compute, values: values(&[]) }).await, "nothing changed");
    }

    #[tokio::test]
    async fn volume_types_specs_qos_and_encryption() {
        let r = dry(OsAction::VolumeTypeCreate { name: id("ssd"), description: None, public: false }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/types"))], json!({"volume_type": {"name": "ssd", "os-volume-type-access:is_public": false}})), "not the SDK's types/{{id}}/action");
        let r = dry(OsAction::VolumeTypeUpdate { id: id("t1"), name: id("ssd"), description: None, public: true }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("PUT"), id("/types/t1"))], json!({"volume_type": {"name": "ssd", "description": "", "is_public": true}})), "an empty description clears it");
        assert_eq!(calls(&dry(OsAction::VolumeTypeDelete { id: id("t1") }).await), [(id("DELETE"), id("/types/t1"))]);
        let r = dry(OsAction::VolumeTypeSetExtraSpecs { id: id("t1"), set: vec![(id("volume_backend_name"), id("ceph"))], unset: vec![id("old")] }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/types/t1/extra_specs")), (id("DELETE"), id("/types/t1/extra_specs/old"))]);
        assert_eq!(body(&r[0]), json!({"extra_specs": {"volume_backend_name": "ceph"}}));
        assert!(dry_err(OsAction::VolumeTypeSetExtraSpecs { id: id("t1"), set: vec![], unset: vec![] }).await, "nothing changed");
        let r = dry(OsAction::VolumeTypeSetQos { id: id("t1"), qos_id: Some(id("q2")), current_qos_id: Some(id("q1")) }).await;
        assert_eq!(calls(&r), [(id("GET"), id("/qos-specs/q1/disassociate?vol_type_id=t1")), (id("GET"), id("/qos-specs/q2/associate?vol_type_id=t1"))], "the old one first");
        let r = dry(OsAction::VolumeTypeSetQos { id: id("t1"), qos_id: None, current_qos_id: Some(id("q1")) }).await;
        assert_eq!(calls(&r).len(), 1, "None only disassociates");
        assert!(dry_err(OsAction::VolumeTypeSetQos { id: id("t1"), qos_id: Some(id("q1")), current_qos_id: Some(id("q1")) }).await, "nothing changed");
        let spec = EncryptionSpec { provider: id("luks"), control_location: id("front-end"), cipher: Some(id("aes-xts-plain64")), key_size: Some(256) };
        let r = dry(OsAction::VolumeTypeEncryptionSet { id: id("t1"), spec: spec.clone(), update: false }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/types/t1/encryption"))]);
        assert_eq!(body(&r[0]), json!({"encryption": {"provider": "luks", "control_location": "front-end", "cipher": "aes-xts-plain64", "key_size": 256}}));
        let r = dry(OsAction::VolumeTypeEncryptionSet { id: id("t1"), spec: EncryptionSpec { cipher: None, key_size: None, ..spec.clone() }, update: true }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("PUT"), id("/types/t1/encryption/provider"))], json!({"encryption": {"provider": "luks", "control_location": "front-end"}})), "defaults left out");
        assert!(dry_err(OsAction::VolumeTypeEncryptionSet { id: id("t1"), spec: EncryptionSpec { control_location: id("middle"), ..spec }, update: false }).await);
        assert_eq!(calls(&dry(OsAction::VolumeTypeEncryptionDelete { id: id("t1") }).await), [(id("DELETE"), id("/types/t1/encryption/provider"))]);
        let r = dry(OsAction::QosSpecCreate { name: id("gold"), consumer: id("back-end"), specs: vec![(id("total_iops_sec"), id("1000"))] }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/qos-specs"))], json!({"qos_specs": {"name": "gold", "consumer": "back-end", "total_iops_sec": "1000"}})));
        assert!(dry_err(OsAction::QosSpecCreate { name: id("gold"), consumer: id("anywhere"), specs: vec![] }).await, "Horizon's consumers only");
        let r = dry(OsAction::QosSpecSetConsumer { id: id("q1"), consumer: id("both") }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("PUT"), id("/qos-specs/q1"))], json!({"qos_specs": {"consumer": "both"}})));
        let r = dry(OsAction::QosSpecSetSpecs { id: id("q1"), set: vec![(id("read_iops_sec"), id("500"))], unset: vec![id("total_iops_sec")] }).await;
        assert_eq!(calls(&r), [(id("PUT"), id("/qos-specs/q1")), (id("PUT"), id("/qos-specs/q1/delete_keys"))]);
        assert_eq!((body(&r[0]), body(&r[1])), (json!({"qos_specs": {"read_iops_sec": "500"}}), json!({"keys": ["total_iops_sec"]})));
        assert_eq!(calls(&dry(OsAction::QosSpecDelete { id: id("q1") }).await), [(id("DELETE"), id("/qos-specs/q1"))]);
    }

    #[tokio::test]
    async fn group_types_and_their_specs() {
        let r = dry(OsAction::GroupTypeCreate { name: id("cg"), description: Some(id("consistent")), public: true }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/group_types"))], json!({"group_type": {"name": "cg", "description": "consistent", "is_public": true}})));
        let r = dry(OsAction::GroupTypeUpdate { id: id("g1"), name: id("cg"), description: None, public: false }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("PUT"), id("/group_types/g1"))], json!({"group_type": {"name": "cg", "description": "", "is_public": false}})));
        assert_eq!(calls(&dry(OsAction::GroupTypeDelete { id: id("g1") }).await), [(id("DELETE"), id("/group_types/g1"))]);
        let r = dry(OsAction::GroupTypeSetSpecs { id: id("g1"), set: vec![(id("consistent_group_snapshot_enabled"), id("<is> True"))], unset: vec![id("x")] }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/group_types/g1/group_specs")), (id("DELETE"), id("/group_types/g1/group_specs/x"))]);
        assert_eq!(body(&r[0]), json!({"group_specs": {"consistent_group_snapshot_enabled": "<is> True"}}));
    }

    #[tokio::test]
    async fn admin_volume_status_migrate_manage_and_unmanage() {
        let r = dry(OsAction::VolumeResetStatus { id: id("v1"), status: id("available") }).await;
        assert_eq!(calls(&r), [(id("POST"), id("/volumes/v1/action"))]);
        assert_eq!(body(&r[0]), json!({"os-reset_status": {"status": "available"}}), "status only, as Horizon");
        assert!(dry_err(OsAction::VolumeResetStatus { id: id("v1"), status: id("fixed") }).await, "Horizon's statuses only");
        let r = dry(OsAction::VolumeMigrate { id: id("v1"), host: id("cinder@ceph#fast"), force_host_copy: true }).await;
        assert_eq!(body(&r[0]), json!({"os-migrate_volume": {"host": "cinder@ceph#fast", "force_host_copy": true}}));
        assert!(dry_err(OsAction::VolumeMigrate { id: id("v1"), host: id(" "), force_host_copy: false }).await, "a destination is required");
        let spec = ManageVolumeSpec {
            identifier: id("vol-0042"), by_id: false, host: id("cinder@ceph#fast"), name: Some(id("legacy")), description: None,
            volume_type: Some(id("ssd")), availability_zone: Some(String::new()), bootable: true, metadata: vec![(id("origin"), id("san"))],
        };
        let r = dry(OsAction::VolumeManage(Box::new(spec.clone()))).await;
        assert_eq!(calls(&r), [(id("POST"), id("/os-volume-manage"))]);
        assert_eq!(
            body(&r[0]),
            json!({"volume": {"ref": {"source-name": "vol-0042"}, "host": "cinder@ceph#fast", "name": "legacy", "volume_type": "ssd", "bootable": true, "metadata": {"origin": "san"}}}),
            "empty zone and description left out"
        );
        let r = dry(OsAction::VolumeManage(Box::new(ManageVolumeSpec { by_id: true, ..spec }))).await;
        assert_eq!(body(&r[0])["volume"]["ref"], json!({"source-id": "vol-0042"}));
        let r = dry(OsAction::VolumeUnmanage { id: id("v1") }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/volumes/v1/action"))], json!({"os-unmanage": null})));
        let r = dry(OsAction::SnapshotResetStatus { id: id("s1"), status: id("error") }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/snapshots/s1/action"))], json!({"os-reset_status": {"status": "error"}})));
        assert!(dry_err(OsAction::SnapshotResetStatus { id: id("s1"), status: id("in-use") }).await, "not a snapshot status");
    }

    #[tokio::test]
    async fn compute_services_and_migrations() {
        let r = dry(OsAction::ComputeServiceSet { id: id("u1"), enabled: false, reason: Some(id("maintenance")) }).await;
        assert_eq!(calls(&r), [(id("PUT"), id("/os-services/u1"))]);
        assert_eq!(body(&r[0]), json!({"status": "disabled", "disabled_reason": "maintenance"}));
        let r = dry(OsAction::ComputeServiceSet { id: id("u1"), enabled: true, reason: Some(id("stale")) }).await;
        assert_eq!(body(&r[0]), json!({"status": "enabled"}), "no reason when enabling");
        let r = dry(OsAction::ServerMigrate { id: id("s1"), host: None }).await;
        assert_eq!((calls(&r), body(&r[0])), (vec![(id("POST"), id("/servers/s1/action"))], json!({"migrate": null})));
        assert_eq!(body(&dry(OsAction::ServerMigrate { id: id("s1"), host: Some(id("c2")) }).await[0]), json!({"migrate": {"host": "c2"}}));
        let r = dry(OsAction::ServerLiveMigrate { id: id("s1"), host: None, block_migration: None }).await;
        assert_eq!(body(&r[0]), json!({"os-migrateLive": {"host": null, "block_migration": "auto"}}), "scheduler and auto block migration");
        let r = dry(OsAction::ServerLiveMigrate { id: id("s1"), host: Some(id("c2")), block_migration: Some(true) }).await;
        assert_eq!(body(&r[0]), json!({"os-migrateLive": {"host": "c2", "block_migration": true}}));
    }

    #[tokio::test]
    async fn admin_router_sends_owner_gateway_and_horizon_options() {
        let spec = |gw: Option<&str>, snat: Option<bool>| AdminRouterSpec {
            name: id(" edge "),
            project_id: id("p2"),
            admin_up: true,
            external_network_id: gw.map(Into::into),
            enable_snat: snat,
            az_hints: vec![id("az1")],
            distributed: Some(false),
            ha: Some(true),
        };
        let r = dry(OsAction::AdminRouterCreate(Box::new(spec(Some("ext"), Some(false))))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/routers"));
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "edge", "admin_state_up": true, "tenant_id": "p2",
                "external_gateway_info": {"network_id": "ext", "enable_snat": false},
                "availability_zone_hints": ["az1"], "distributed": false, "ha": true}})
        );
        let none = AdminRouterSpec { az_hints: vec![], distributed: None, ha: None, ..spec(None, None) };
        assert_eq!(
            body(&dry(OsAction::AdminRouterCreate(Box::new(none))).await[0]),
            json!({"router": {"name": "edge", "admin_state_up": true, "tenant_id": "p2"}}),
            "server defaults: nothing sent"
        );
        assert!(dry_err(OsAction::AdminRouterCreate(Box::new(spec(None, Some(true))))).await, "SNAT without a gateway");
        assert!(dry_err(OsAction::AdminRouterCreate(Box::new(AdminRouterSpec { project_id: id(""), ..spec(None, None) }))).await, "an owner is required");
    }

    #[tokio::test]
    async fn rbac_policies_are_created_and_deleted_like_horizon() {
        let create = |kind: &str, target: &str| OsAction::RbacPolicyCreate { kind: kind.into(), object_id: id("n1"), target_project: target.into() };
        let r = dry(create("external_network", "*")).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/rbac-policies"));
        assert_eq!(body(&r[0]), json!({"rbac_policy": {"object_type": "network", "action": "access_as_external", "object_id": "n1", "target_tenant": "*"}}));
        let r = dry(create("shared_qos_policy", "p2")).await;
        assert_eq!(body(&r[0])["rbac_policy"]["object_type"], "qos_policy");
        assert!(dry_err(create("external_qos_policy", "p2")).await, "only Horizon's three kinds");
        assert!(dry_err(create("shared_network", "")).await, "a target project is required");
        let r = dry(OsAction::RbacPolicyUpdate { id: id("r1"), target_project: id("*") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/rbac-policies/r1"));
        assert_eq!(body(&r[0]), json!({"rbac_policy": {"target_tenant": "*"}}), "Horizon's edit changes only the target");
        assert!(dry_err(OsAction::RbacPolicyUpdate { id: id("r1"), target_project: id(" ") }).await, "a target is required");
        let r = dry(OsAction::RbacPolicyDelete { id: id("r1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/rbac-policies/r1"));
    }

    #[tokio::test]
    async fn ipv6_modes_and_host_routes_reach_the_subnet() {
        let spec = |ip_version: u8, cidr: &str, routes: Vec<(String, String)>| SubnetSpec {
            name: String::new(),
            cidr: cidr.into(),
            ip_version,
            gateway_ip: None,
            enable_dhcp: true,
            dns_nameservers: vec![],
            allocation_pools: vec![],
            ipv6_ra_mode: Some("slaac".into()),
            ipv6_address_mode: Some("slaac".into()),
            host_routes: routes,
        };
        let v6 = spec(6, "2001:db8::/64", vec![("2001:db8:1::/64".into(), "2001:db8::1".into())]);
        let r = dry(OsAction::SubnetCreate { network_id: id("n1"), spec: v6 }).await;
        let s = &body(&r[0])["subnet"];
        assert_eq!((&s["ipv6_ra_mode"], &s["ipv6_address_mode"]), (&json!("slaac"), &json!("slaac")));
        assert_eq!(s["host_routes"], json!([{"destination": "2001:db8:1::/64", "nexthop": "2001:db8::1"}]));
        let r = dry(OsAction::SubnetCreate { network_id: id("n1"), spec: spec(4, "10.0.0.0/24", vec![]) }).await;
        assert!(body(&r[0])["subnet"].get("ipv6_ra_mode").is_none(), "IPv4: the modes are not sent (Neutron refuses them)");
        let bad = spec(4, "10.0.0.0/24", vec![("10.1.0.0/24".into(), "2001:db8::1".into())]);
        assert!(dry_err(OsAction::SubnetCreate { network_id: id("n1"), spec: bad }).await, "a route's next hop must match its IP version");
    }

    /// Edits and creates no other test dry-ran (found by roadmap 67's
    /// `every_action_has_a_dry_run_test`). An empty description clears it.
    #[tokio::test]
    async fn renames_subnets_gateways_and_security_groups() {
        let one = |r: Vec<RequestPreview>| {
            assert_eq!(r.len(), 1);
            (r[0].method.clone(), r[0].path.clone(), body(&r[0]))
        };
        let r = dry(OsAction::VolumeUpdate { id: id("v1"), name: id(" data "), description: None }).await;
        assert_eq!(one(r), (id("PUT"), id("/volumes/v1"), json!({"volume": {"name": "data", "description": ""}})));
        let r = dry(OsAction::SnapshotUpdate { id: id("s1"), name: id("snap"), description: Some(id("daily")) }).await;
        assert_eq!(one(r), (id("PUT"), id("/snapshots/s1"), json!({"snapshot": {"name": "snap", "description": "daily"}})));
        let r = dry(OsAction::NetworkUpdate { id: id("n1"), name: id("net"), admin_up: false }).await;
        assert_eq!(one(r), (id("PUT"), id("/networks/n1"), json!({"network": {"name": "net", "admin_state_up": false}})));
        let r = dry(OsAction::AdminNetworkUpdate { id: id("n1"), name: id(" ext "), admin_up: true, shared: true, external: false }).await;
        assert_eq!(
            one(r),
            (id("PUT"), id("/networks/n1"), json!({"network": {"name": "ext", "admin_state_up": true, "shared": true, "router:external": false}})),
            "Horizon's admin edit sends shared and external too, never provider attributes"
        );
        let spec = SubnetSpec {
            name: "sub".into(),
            cidr: "10.1.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some("10.1.0.1".into()),
            enable_dhcp: true,
            dns_nameservers: vec![],
            allocation_pools: vec![],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![],
        };
        let (method, path, b) = one(dry(OsAction::SubnetCreate { network_id: id("n1"), spec }).await);
        assert_eq!((method.as_str(), path.as_str()), ("POST", "/subnets"));
        assert_eq!(
            b,
            json!({"subnet": {"network_id": "n1", "name": "sub", "ip_version": 4, "cidr": "10.1.0.0/24", "gateway_ip": "10.1.0.1", "enable_dhcp": true}})
        );
        let r = dry(OsAction::SubnetUpdate { id: id("s1"), name: id("sub"), enable_dhcp: false, dns_nameservers: vec![id("1.1.1.1")] }).await;
        assert_eq!(one(r), (id("PUT"), id("/subnets/s1"), json!({"subnet": {"name": "sub", "enable_dhcp": false, "dns_nameservers": ["1.1.1.1"]}})));
        let r = dry(OsAction::RouterSetGateway { id: id("r1"), network_id: id("ext") }).await;
        assert_eq!(one(r), (id("PUT"), id("/routers/r1"), json!({"router": {"external_gateway_info": {"network_id": "ext"}}})));
        let (method, path, b) = one(dry(OsAction::SecurityGroupCreate { name: id("web"), description: Some(id("HTTP in")) }).await);
        assert_eq!((method.as_str(), path.as_str()), ("POST", "/security-groups"));
        assert_eq!(b, json!({"security_group": {"name": "web", "description": "HTTP in"}}));
        let r = dry(OsAction::SecurityGroupUpdate { id: id("g1"), name: id("web"), description: None }).await;
        assert_eq!(one(r), (id("PUT"), id("/security-groups/g1"), json!({"security_group": {"name": "web", "description": ""}})));
        let empty = OsAction::SecurityGroupCreate { name: id(" "), description: None };
        assert!(apply(&mut Exec::new(WriteMode::DryRun, None), &empty).await.is_err(), "a security group needs a name");
    }

    /// Roadmap 54, slice 2: listener timeouts, headers and CIDRs, and pool
    /// re-encryption, in the wizard body, the single creates and the edits.
    #[tokio::test]
    async fn listener_and_pool_options() {
        let options = LbListenerOptions {
            timeout_client_data: Some(60_000),
            timeout_tcp_inspect: Some(0),
            insert_headers: vec![id("X-Forwarded-For"), id("X-Forwarded-Proto")],
            allowed_cidrs: Some(vec![id(" 192.0.2.0/24 ")]),
            ..Default::default()
        };
        let listener = LbListenerSpec { name: None, description: None, protocol: id("HTTP"), port: 80, connection_limit: None, options: options.clone(), default_tls_container_ref: None, sni_container_refs: vec![], admin_up: true  };
        let pool = LbPoolSpec {
            name: Some(id("web")),
            description: None,
            protocol: id("HTTP"),
            algorithm: id("ROUND_ROBIN"),
            session_persistence: None,
            cookie_name: None,
            tls_enabled: true,
            tls_ciphers: Some(id("ECDHE-RSA-AES256-GCM-SHA384")), admin_up: true,
        };
        let wizard = LbCreateSpec {
            name: None,
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(listener.clone()),
            pool: Some(pool.clone()),
            members: vec![],
            monitor: None, admin_up: true,
        };
        let r = dry(OsAction::LoadBalancerCreate(Box::new(wizard))).await;
        let l = &body(&r[0])["loadbalancer"]["listeners"][0];
        let want_listener = json!({
            "timeout_client_data": 60000,
            "timeout_tcp_inspect": 0,
            "insert_headers": {"X-Forwarded-For": "true", "X-Forwarded-Proto": "true"},
            "allowed_cidrs": ["192.0.2.0/24"],
        });
        for (k, v) in want_listener.as_object().unwrap() {
            assert_eq!(&l[k], v, "wizard listener {k}");
        }
        assert!(l.get("timeout_member_data").is_none(), "unset timeouts keep Octavia's defaults");
        assert_eq!((l["default_pool"]["tls_enabled"].clone(), l["default_pool"]["tls_ciphers"].clone()), (json!(true), json!("ECDHE-RSA-AES256-GCM-SHA384")));

        let r = dry(OsAction::ListenerCreate { loadbalancer_id: id("lb1"), spec: listener.clone(), default_pool_id: None }).await;
        assert_eq!(body(&r[0])["listener"]["insert_headers"], want_listener["insert_headers"]);
        let r = dry(OsAction::PoolCreate { loadbalancer_id: id("lb1"), listener_id: None, spec: pool.clone() }).await;
        assert_eq!(body(&r[0])["pool"]["tls_enabled"], true);

        // Edits send cleared values as Octavia's "back to the default".
        let cleared = LbListenerOptions { allowed_cidrs: Some(vec![]), ..Default::default() };
        let upd = |protocol: &str, options| OsAction::ListenerUpdate {
            id: id("ls1"),
            name: id("web"),
            description: String::new(),
            connection_limit: None,
            default_pool_id: None,
            admin_up: true,
            protocol: id(protocol),
            options,
            tls: None,
        };
        let l = body(&dry(upd("HTTP", Some(cleared.clone()))).await[0])["listener"].clone();
        assert_eq!((l["timeout_client_data"].clone(), l["insert_headers"].clone(), l["allowed_cidrs"].clone()), (Value::Null, json!({}), json!([])));
        let l = body(&dry(upd("TCP", Some(LbListenerOptions::default()))).await[0])["listener"].clone();
        assert!(l.get("insert_headers").is_none() && l.get("allowed_cidrs").is_none(), "TCP: no headers; CIDRs not offered: not sent");
        let headers_on_tcp = upd("TCP", Some(LbListenerOptions { insert_headers: vec![id("X-Forwarded-For")], ..Default::default() }));
        assert!(apply(&mut Exec::new(WriteMode::DryRun, None), &headers_on_tcp).await.is_err());

        let pool_upd = |tls| OsAction::PoolUpdate {
            id: id("pool1"),
            name: id("web"),
            description: String::new(),
            algorithm: id("ROUND_ROBIN"),
            session_persistence: None,
            cookie_name: None,
            admin_up: true,
            protocol: id("HTTP"),
            tls,
        };
        let p = body(&dry(pool_upd(Some((false, None)))).await[0])["pool"].clone();
        assert_eq!((p["tls_enabled"].clone(), p.get("tls_ciphers")), (json!(false), None));
        let ciphers_without_tls = pool_upd(Some((false, Some(id("AES256")))));
        assert!(apply(&mut Exec::new(WriteMode::DryRun, None), &ciphers_without_tls).await.is_err(), "ciphers need TLS");
    }

    /// Roadmap 54, slice 3: `TERMINATED_HTTPS` with a Barbican certificate, SNI
    /// certificates and ciphers; the pool behind it is HTTP.
    #[tokio::test]
    async fn tls_terminated_listeners_carry_their_certificates() {
        let cert = "https://barbican.example/v1/containers/c1";
        let sni = "https://barbican.example/v1/secrets/s2";
        let listener = LbListenerSpec {
            name: Some(id("https")),
            description: None,
            protocol: id("TERMINATED_HTTPS"),
            port: 443,
            connection_limit: None,
            options: LbListenerOptions { tls_ciphers: Some(id("ECDHE-RSA-AES256-GCM-SHA384")), insert_headers: vec![id("X-Forwarded-Proto")], ..Default::default() },
            default_tls_container_ref: Some(id(cert)),
            sni_container_refs: vec![id(sni)], admin_up: true,
        };
        let r = dry(OsAction::ListenerCreate { loadbalancer_id: id("lb1"), spec: listener.clone(), default_pool_id: None }).await;
        let l = &body(&r[0])["listener"];
        assert_eq!(
            (l["protocol"].clone(), l["default_tls_container_ref"].clone(), l["sni_container_refs"].clone(), l["tls_ciphers"].clone(), l["insert_headers"].clone()),
            (json!("TERMINATED_HTTPS"), json!(cert), json!([sni]), json!("ECDHE-RSA-AES256-GCM-SHA384"), json!({"X-Forwarded-Proto": "true"}))
        );
        let wizard = LbCreateSpec {
            name: None,
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(listener.clone()),
            pool: None,
            members: vec![],
            monitor: None, admin_up: true,
        };
        let r = dry(OsAction::LoadBalancerCreate(Box::new(wizard.clone()))).await;
        assert_eq!(body(&r[0])["loadbalancer"]["listeners"][0]["default_tls_container_ref"], cert);
        assert!(OsAction::LoadBalancerCreate(Box::new(wizard)).plugins().contains(&Plugin::KeyManager), "Barbican certificates need the key-manager plugin");

        let no_cert = LbListenerSpec { default_tls_container_ref: None, ..listener.clone() };
        assert!(apply(&mut Exec::new(WriteMode::DryRun, None), &OsAction::ListenerCreate { loadbalancer_id: id("lb1"), spec: no_cert, default_pool_id: None }).await.is_err());
        let http_with_cert = LbListenerSpec { protocol: id("HTTP"), options: Default::default(), ..listener };
        assert!(http_with_cert.check().is_err(), "certificates only on TERMINATED_HTTPS");

        let upd = OsAction::ListenerUpdate {
            id: id("ls1"),
            name: id("https"),
            description: String::new(),
            connection_limit: None,
            default_pool_id: None,
            admin_up: true,
            protocol: id("TERMINATED_HTTPS"),
            options: None,
            tls: Some((id(cert), vec![])),
        };
        assert!(upd.plugins().contains(&Plugin::KeyManager));
        let l = body(&dry(upd).await[0])["listener"].clone();
        assert_eq!((l["default_tls_container_ref"].clone(), l["sni_container_refs"].clone()), (json!(cert), json!([])), "an empty list removes the SNI certificates");
    }

    /// Roadmap 54, slice 4: L7 policies and rules, created with the SDK and
    /// saved as JSON so the unused redirect target and a removed key are `null`.
    #[tokio::test]
    async fn l7_policies_and_rules() {
        let to_pool = LbL7PolicySpec {
            name: Some(id("api")),
            description: None,
            action: id("REDIRECT_TO_POOL"),
            position: Some(1),
            redirect_pool_id: Some(id("pool1")),
            redirect_url: Some(id("https://ignored.example")),
            admin_up: true,
        };
        let r = dry(OsAction::L7PolicyCreate { listener_id: id("ls1"), spec: to_pool.clone() }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/lbaas/l7policies"));
        assert_eq!(
            body(&r[0]),
            json!({"l7policy": {"listener_id": "ls1", "name": "api", "action": "REDIRECT_TO_POOL", "position": 1, "redirect_pool_id": "pool1", "admin_state_up": true}}),
            "only the target of the action"
        );
        let to_url = LbL7PolicySpec { action: id("REDIRECT_TO_URL"), position: None, redirect_url: Some(id("https://www.example.com")), ..to_pool.clone() };
        let r = dry(OsAction::L7PolicyUpdate { id: id("pol1"), spec: to_url }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/l7policies/pol1"));
        let p = &body(&r[0])["l7policy"];
        assert_eq!((p["redirect_pool_id"].clone(), p["redirect_url"].clone(), p.get("position")), (Value::Null, json!("https://www.example.com"), None));
        let r = dry(OsAction::L7PolicyDelete { id: id("pol1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/lbaas/l7policies/pol1"));

        let header = LbL7RuleSpec { rule_type: id("HEADER"), compare_type: id("EQUAL_TO"), key: Some(id("X-Env")), value: id(" test "), invert: true, admin_up: true };
        let r = dry(OsAction::L7RuleCreate { policy_id: id("pol1"), spec: header.clone() }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/lbaas/l7policies/pol1/rules"));
        assert_eq!(body(&r[0]), json!({"rule": {"type": "HEADER", "compare_type": "EQUAL_TO", "key": "X-Env", "value": "test", "invert": true, "admin_state_up": true}}));
        let path = LbL7RuleSpec { rule_type: id("PATH"), compare_type: id("STARTS_WITH"), key: None, value: id("/api"), invert: false, admin_up: true };
        let r = dry(OsAction::L7RuleUpdate { policy_id: id("pol1"), id: id("r1"), spec: path }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/l7policies/pol1/rules/r1"));
        assert_eq!(body(&r[0])["rule"]["key"], Value::Null, "a PATH rule has no key");
        let r = dry(OsAction::L7RuleDelete { policy_id: id("pol1"), id: id("r1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/lbaas/l7policies/pol1/rules/r1"));

        let bad = |a: OsAction| async move { apply(&mut Exec::new(WriteMode::DryRun, None), &a).await.is_err() };
        assert!(bad(OsAction::L7PolicyCreate { listener_id: id("ls1"), spec: LbL7PolicySpec { redirect_pool_id: None, ..to_pool.clone() } }).await, "a pool redirect needs the pool");
        assert!(bad(OsAction::L7RuleCreate { policy_id: id("pol1"), spec: LbL7RuleSpec { key: None, ..header.clone() } }).await, "a header rule needs its name");
        assert!(bad(OsAction::L7RuleCreate { policy_id: id("pol1"), spec: LbL7RuleSpec { rule_type: id("FILE_TYPE"), key: None, compare_type: id("STARTS_WITH"), ..header } }).await);
    }

    /// Roadmap 54, slice 5: the availability zone, and *Admin State Up* off at
    /// every level, sent only when off (default bodies unchanged).
    #[tokio::test]
    async fn wizard_sends_the_zone_and_admin_state_down() {
        let mut w = lb_wizard();
        w.availability_zone = Some(id("az-1"));
        w.admin_up = false;
        w.listener.as_mut().unwrap().admin_up = false;
        w.pool.as_mut().unwrap().admin_up = false;
        w.monitor.as_mut().unwrap().admin_up = false;
        let b = body(&dry(OsAction::LoadBalancerCreate(Box::new(w))).await[0])["loadbalancer"].clone();
        let l = &b["listeners"][0];
        assert_eq!(
            (b["availability_zone"].clone(), b["admin_state_up"].clone(), l["admin_state_up"].clone(), l["default_pool"]["admin_state_up"].clone(), l["default_pool"]["healthmonitor"]["admin_state_up"].clone()),
            (json!("az-1"), json!(false), json!(false), json!(false), json!(false))
        );
        let b = body(&dry(OsAction::LoadBalancerCreate(Box::new(lb_wizard()))).await[0])["loadbalancer"].clone();
        assert!(b.get("admin_state_up").is_none() && b["listeners"][0].get("admin_state_up").is_none(), "up: not sent");
    }

    #[tokio::test]
    async fn ports_routers_and_interfaces() {
        let r = dry(OsAction::PortCreate { network_id: id("n"), name: id("p"), admin_up: true, fixed_ip: Some(id("10.0.0.5")) }).await;
        assert_eq!(body(&r[0])["port"]["fixed_ips"], json!([{"ip_address": "10.0.0.5"}]));
        let r = dry(OsAction::RouterCreate { name: id("r"), admin_up: true, external_network_id: Some(id("ext")), qos_policy_id: None }).await;
        assert_eq!(body(&r[0])["router"]["external_gateway_info"]["network_id"], "ext");
        let r = dry(OsAction::RouterUpdate { id: id("r"), name: id(" gw "), admin_up: true, snat: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r"));
        assert_eq!(body(&r[0]), json!({"router": {"name": "gw", "admin_state_up": true}}), "rename keeps the gateway");
        let r = dry(OsAction::RouterUpdate { id: id("r"), name: id("gw"), admin_up: false, snat: Some((id("ext"), false)) }).await;
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "gw", "admin_state_up": false,
                "external_gateway_info": {"network_id": "ext", "enable_snat": false}}})
        );
        let r = dry(OsAction::RouterClearGateway { id: id("r") }).await;
        assert_eq!(body(&r[0]), json!({"router": {"external_gateway_info": null}}));
        let r = dry(OsAction::RouterAddInterface { id: id("r"), subnet_id: id("s") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r/add_router_interface"));
        assert_eq!(body(&r[0]), json!({"subnet_id": "s"}));
        let r = dry(OsAction::RouterRemoveInterface { id: id("r"), port_id: id("p") }).await;
        assert_eq!(body(&r[0]), json!({"port_id": "p"}));
    }

    #[tokio::test]
    async fn static_routes_are_added_and_removed_one_at_a_time() {
        let r = dry(OsAction::RouterAddRoute { id: id("r"), destination: id(" 10.9.0.0/16"), nexthop: id("10.0.0.1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r/add_extraroutes"));
        assert_eq!(body(&r[0]), json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.1"}]}}));
        let r = dry(OsAction::RouterRemoveRoute { id: id("r"), destination: id("10.9.0.0/16"), nexthop: id("10.0.0.1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r/remove_extraroutes"));
        assert_eq!(body(&r[0]), json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.1"}]}}));
    }

    #[tokio::test]
    async fn security_group_rules() {
        let rule = RuleSpec {
            security_group_id: "sg".into(),
            direction: "ingress".into(),
            ethertype: "IPv4".into(),
            protocol: Some("tcp".into()),
            port_min: Some(22),
            port_max: None,
            remote_ip_prefix: Some("10.0.0.0/8".into()),
            remote_group_id: None,
            description: None,
        };
        let r = dry(OsAction::RuleCreate(rule.clone())).await;
        let b = &body(&r[0])["security_group_rule"];
        assert_eq!(b["port_range_min"], 22);
        assert_eq!(b["port_range_max"], 22, "single port fills both ends");
        assert_eq!(b["direction"], "ingress");
        assert_eq!(b["ethertype"], "IPv4");

        let mut x = Exec::new(WriteMode::DryRun, None);
        let mut both = rule.clone();
        both.remote_group_id = Some("other".into());
        assert!(apply(&mut x, &OsAction::RuleCreate(both)).await.is_err());
        let mut bad = rule;
        bad.port_min = Some(100);
        bad.port_max = Some(10);
        assert!(apply(&mut x, &OsAction::RuleCreate(bad)).await.is_err());
    }

    #[tokio::test]
    async fn floating_ips() {
        let r = dry(OsAction::FloatingIpAllocate { network_id: id("ext"), description: None, port_id: None, qos_policy_id: None }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"floating_network_id": "ext"}}));
        let r = dry(OsAction::FloatingIpAssociate { id: id("f"), port_id: id("p") }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"port_id": "p"}}));
        let r = dry(OsAction::FloatingIpDisassociate { id: id("f") }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"port_id": null}}));
        let r = dry(OsAction::FloatingIpRelease { id: id("f") }).await;
        assert_eq!((r[0].method.as_str(), r[0].service.as_str()), ("DELETE", "network"));
    }

    #[tokio::test]
    async fn every_delete_is_a_delete() {
        let deletes = [
            OsAction::ImageDelete { id: id("x") },
            OsAction::ServerGroupDelete { id: id("x") },
            OsAction::VolumeDelete { id: id("x") },
            OsAction::NetworkDelete { id: id("x") },
            OsAction::SubnetDelete { id: id("x") },
            OsAction::PortDelete { id: id("x") },
            OsAction::RouterDelete { id: id("x") },
            OsAction::SecurityGroupDelete { id: id("x") },
            OsAction::RuleDelete { id: id("x") },
            OsAction::LoadBalancerDelete { id: id("x"), cascade: false },
        ];
        for a in deletes {
            let r = dry(a.clone()).await;
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].method, "DELETE", "{a:?}");
            assert!(r[0].path.ends_with("/x"), "{a:?} → {}", r[0].path);
            assert!(r[0].body.is_none());
        }
    }

    fn forwarding(internal: &str, external: &str, ranges: bool) -> PortForwardingSpec {
        PortForwardingSpec {
            floating_ip_id: id("fip"),
            protocol: id("tcp"),
            internal_port_id: id("port"),
            internal_ip: id("10.0.0.5"),
            internal: id(internal),
            external: id(external),
            description: Some(id("ssh")),
            ranges,
        }
    }

    #[tokio::test]
    async fn port_forwarding_create_sends_integer_ports() {
        let r = dry(OsAction::PortForwardingCreate(forwarding("22", "2222", false))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/floatingips/fip/port_forwardings", "network"));
        assert_eq!(
            body(&r[0]),
            json!({"port_forwarding": {"protocol": "tcp", "internal_port_id": "port", "internal_ip_address": "10.0.0.5",
                   "internal_port": 22, "external_port": 2222, "description": "ssh"}})
        );
    }

    #[tokio::test]
    async fn port_forwarding_ranges_update_and_delete() {
        let r = dry(OsAction::PortForwardingUpdate { id: id("pf1"), spec: forwarding("8080-8090", "80:90", true) }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/floatingips/fip/port_forwardings/pf1"));
        let pf = &body(&r[0])["port_forwarding"];
        assert_eq!((pf["internal_port_range"].clone(), pf["external_port_range"].clone()), (json!("8080:8090"), json!("80:90")));
        assert!(pf.get("internal_port").is_none());
        let r = dry(OsAction::PortForwardingDelete { floating_ip_id: id("fip"), id: id("pf1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/floatingips/fip/port_forwardings/pf1"));
        assert!(dry_err(OsAction::PortForwardingCreate(forwarding("80:90", "80:91", true))).await, "sizes differ");
        assert!(dry_err(OsAction::PortForwardingCreate(forwarding("80:90", "80:90", false))).await, "no ranges extension");
    }

    fn lb_wizard() -> LbCreateSpec {
        LbCreateSpec {
            name: Some(id("web-lb")),
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(LbListenerSpec { name: Some(id("http")), description: None, protocol: id("HTTP"), port: 80, connection_limit: None, options: Default::default(), default_tls_container_ref: None, sni_container_refs: vec![], admin_up: true  }),
            pool: Some(LbPoolSpec {
                name: Some(id("web")),
                description: None,
                protocol: id("HTTP"),
                algorithm: id("ROUND_ROBIN"),
                session_persistence: Some(id("APP_COOKIE")),
                cookie_name: Some(id("JSESSIONID")),
                tls_enabled: false,
                tls_ciphers: None, admin_up: true,
            }),
            members: vec![LbMemberSpec {
                name: None,
                address: id("10.0.0.11"),
                port: 8080,
                subnet_id: Some(id("sub-1")),
                weight: Some(10),
                backup: false,
                monitor_address: None,
                monitor_port: None,
                admin_up: true,
            }],
            monitor: Some(LbMonitorSpec {
                name: None,
                kind: id("HTTP"),
                delay: 5,
                timeout: 3,
                max_retries: 3,
                max_retries_down: None,
                http_method: Some(id("GET")),
                url_path: Some(id("/healthz")),
                expected_codes: Some(id("200")), admin_up: true,
            }), admin_up: true,
        }
    }

    #[tokio::test]
    async fn application_credentials_are_created_with_the_secret_masked_and_deleted() {
        let spec = AppCredentialSpec {
            user_id: id("u1"),
            name: id("ci"),
            description: Some(id("pipeline")),
            secret: Some(id("s3cret")),
            expires_at: Some(id("2026-12-31T00:00:00")),
            roles: vec![id("power_user")],
            unrestricted: false,
            access_rules: None,
        };
        let r = dry(OsAction::AppCredentialCreate(Box::new(spec.clone()))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/users/u1/application_credentials", "identity"));
        assert_eq!(
            body(&r[0]),
            json!({"application_credential": {"name": "ci", "description": "pipeline", "secret": "********",
                "expires_at": "2026-12-31T00:00:00", "roles": [{"name": "power_user"}], "unrestricted": false}}),
            "the chosen secret never shows in the preview"
        );
        let bare = AppCredentialSpec { description: None, secret: None, expires_at: None, roles: vec![], ..spec.clone() };
        assert_eq!(body(&dry(OsAction::AppCredentialCreate(Box::new(bare))).await[0]), json!({"application_credential": {"name": "ci", "unrestricted": false}}), "no roles = all of the user's");
        assert!(dry_err(OsAction::AppCredentialCreate(Box::new(AppCredentialSpec { user_id: String::new(), ..spec }))).await, "run_action sets the user");
        let r = dry(OsAction::AppCredentialDelete { user_id: id("u1"), id: id("a1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/users/u1/application_credentials/a1"));
    }

    #[tokio::test]
    async fn access_rules_are_read_as_json_or_yaml_like_horizon() {
        let json = r#"[{"service": "compute", "method": "POST", "path": "/v2.1/servers"}]"#;
        let yaml = "- service: compute\n  method: post\n  path: /v2.1/servers\n";
        let want = vec![AccessRule { service: id("compute"), method: id("POST"), path: id("/v2.1/servers") }];
        assert_eq!(parse_access_rules(json).unwrap(), want, "Horizon's JSON example");
        assert_eq!(parse_access_rules(yaml).unwrap(), want, "Horizon's YAML example; the method upper-cased");
        assert!(parse_access_rules("service: compute").unwrap_err().contains("must be a list"));
        assert!(parse_access_rules("- service: nova compute\n  method: GET\n  path: /").unwrap_err().contains("rule 1"));
        assert!(parse_access_rules("- service: compute\n  method: FETCH\n  path: /").unwrap_err().contains("method"));
        assert!(parse_access_rules("- service: compute\n  method: GET\n  path: v2.1").unwrap_err().contains("start with /"));
        assert!(parse_access_rules("[unclosed").is_err());

        let spec = AppCredentialSpec {
            user_id: id("u1"),
            name: id("ci"),
            description: None,
            secret: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
            access_rules: Some(id(yaml)),
        };
        let r = dry(OsAction::AppCredentialCreate(Box::new(spec.clone()))).await;
        assert_eq!(body(&r[0])["application_credential"]["access_rules"], json!([{"service": "compute", "method": "POST", "path": "/v2.1/servers"}]));
        assert!(dry_err(OsAction::AppCredentialCreate(Box::new(AppCredentialSpec { access_rules: Some(id("nope")), ..spec }))).await, "refused before sending");
    }

    #[tokio::test]
    async fn roles_are_created_renamed_and_deleted() {
        let r = dry(OsAction::RoleCreate { name: id(" auditor ") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/roles", "identity"));
        assert_eq!(body(&r[0]), json!({"role": {"name": "auditor"}}), "trimmed");
        let r = dry(OsAction::RoleUpdate { id: id("r1"), name: id("auditors") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PATCH", "/roles/r1"));
        assert_eq!(body(&r[0]), json!({"role": {"name": "auditors"}}));
        let r = dry(OsAction::RoleDelete { id: id("r1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/roles/r1"));
        assert!(dry_err(OsAction::RoleCreate { name: id(" ") }).await, "a name is required");
    }

    #[tokio::test]
    async fn identity_admin_requests_are_horizons() {
        let idy = |r: &RequestPreview| (r.method.clone(), r.path.clone());
        let r = dry(OsAction::ProjectCreate { name: id("web"), description: Some(id("Web team")), enabled: true, domain_id: id("d-spain") }).await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            ((id("POST"), id("/projects")), json!({"project": {"name": "web", "description": "Web team", "enabled": true, "domain_id": "d-spain"}})),
            "the domain is always explicit: Keystone would use Default"
        );
        assert!(dry_err(OsAction::ProjectCreate { name: id("web"), description: None, enabled: true, domain_id: String::new() }).await, "run_action sets the domain");
        let r = dry(OsAction::ProjectUpdate { id: id("p1"), name: id("web"), description: String::new(), enabled: false }).await;
        assert_eq!((idy(&r[0]), body(&r[0])), ((id("PATCH"), id("/projects/p1")), json!({"project": {"name": "web", "description": "", "enabled": false}})));
        assert_eq!(idy(&dry(OsAction::ProjectDelete { id: id("p1") }).await[0]), (id("DELETE"), id("/projects/p1")));
        let grant = OsAction::ProjectRoleGrant { project_id: id("p1"), user_id: Some(id("u1")), group_id: None, role_id: id("r1") };
        assert_eq!(idy(&dry(grant).await[0]), (id("PUT"), id("/projects/p1/users/u1/roles/r1")));
        let revoke = OsAction::ProjectRoleRevoke { project_id: id("p1"), user_id: None, group_id: Some(id("g1")), role_id: id("r1") };
        assert_eq!(idy(&dry(revoke).await[0]), (id("DELETE"), id("/projects/p1/groups/g1/roles/r1")));
        assert!(dry_err(OsAction::ProjectRoleGrant { project_id: id("p1"), user_id: None, group_id: None, role_id: id("r1") }).await, "a user or a group");

        let spec = UserSpec {
            domain_id: id("d-spain"),
            name: id("ana"),
            description: None,
            email: Some(id("ana@example.com")),
            password: id("Secr3t!"),
            project_id: Some(id("p1")),
            role_id: Some(id("r1")),
            enabled: true,
            lock_password: true,
        };
        let r = dry(OsAction::UserCreate(Box::new(spec.clone()))).await;
        assert_eq!(idy(&r[0]), (id("POST"), id("/users")));
        assert_eq!(
            body(&r[0]),
            json!({"user": {"name": "ana", "domain_id": "d-spain", "password": "********", "enabled": true, "email": "ana@example.com", "default_project_id": "p1", "options": {"lock_password": true}}}),
            "the password is masked in the preview"
        );
        assert_eq!(idy(&r[1]), (id("PUT"), id("/projects/p1/users/%3Cnew%20user%20id%3E/roles/r1")), "then the role on the primary project (the dry-run placeholder, encoded)");
        assert!(dry_err(OsAction::UserCreate(Box::new(UserSpec { password: String::new(), ..spec }))).await);
        let r = dry(OsAction::UserUpdate { id: id("u1"), name: id("ana"), email: String::new(), description: id("ops"), project_id: None }).await;
        assert_eq!(body(&r[0]), json!({"user": {"name": "ana", "email": "", "description": "ops", "default_project_id": null}}), "null clears the primary project");
        let r = dry(OsAction::UserSetEnabled { id: id("u1"), enabled: false }).await;
        assert_eq!((idy(&r[0]), body(&r[0])), ((id("PATCH"), id("/users/u1")), json!({"user": {"enabled": false}})));
        let r = dry(OsAction::UserSetPassword { id: id("u1"), password: id("n3w") }).await;
        assert_eq!(body(&r[0]), json!({"user": {"password": "********"}}));
        assert_eq!(idy(&dry(OsAction::UserDelete { id: id("u1") }).await[0]), (id("DELETE"), id("/users/u1")));

        let r = dry(OsAction::GroupCreate { name: id("ops"), description: None, domain_id: id("d-spain") }).await;
        assert_eq!((idy(&r[0]), body(&r[0])), ((id("POST"), id("/groups")), json!({"group": {"name": "ops", "domain_id": "d-spain"}})));
        let r = dry(OsAction::GroupUpdate { id: id("g1"), name: id("ops"), description: id("Operators") }).await;
        assert_eq!((idy(&r[0]), body(&r[0])), ((id("PATCH"), id("/groups/g1")), json!({"group": {"name": "ops", "description": "Operators"}})));
        assert_eq!(idy(&dry(OsAction::GroupDelete { id: id("g1") }).await[0]), (id("DELETE"), id("/groups/g1")));
        assert_eq!(idy(&dry(OsAction::GroupAddUser { group_id: id("g1"), user_id: id("u1") }).await[0]), (id("PUT"), id("/groups/g1/users/u1")));
        assert_eq!(idy(&dry(OsAction::GroupRemoveUser { group_id: id("g1"), user_id: id("u1") }).await[0]), (id("DELETE"), id("/groups/g1/users/u1")));
    }

    #[tokio::test]
    async fn app_credential_delete_path() {
        let r = dry(OsAction::AppCredentialDelete { user_id: id("u1"), id: id("a1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/users/u1/application_credentials/a1"));
    }

    #[tokio::test]
    async fn pool_members_are_replaced_in_one_batch_request() {
        let web = lb_wizard().members[0].clone();
        let external = LbMemberSpec {
            name: Some(id("legacy")),
            address: id("192.0.2.10"),
            port: 8443,
            subnet_id: Some(id("sub-ext")),
            weight: None,
            backup: true,
            monitor_address: Some(id("192.0.2.11")),
            monitor_port: Some(9000),
            admin_up: false,
        };
        let r = dry(OsAction::PoolSetMembers { pool_id: id("pool1"), members: vec![web.clone(), external.clone()] }).await;
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/pools/pool1/members"));
        assert_eq!(
            body(&r[0]),
            json!({"members": [
                {"address": "10.0.0.11", "protocol_port": 8080, "subnet_id": "sub-1", "weight": 10, "backup": false, "admin_state_up": true},
                {"name": "legacy", "address": "192.0.2.10", "protocol_port": 8443, "subnet_id": "sub-ext", "backup": true,
                 "admin_state_up": false, "monitor_address": "192.0.2.11", "monitor_port": 9000}
            ]}),
            "each member with its own port; the external one with its monitor"
        );
        let r = dry(OsAction::PoolSetMembers { pool_id: id("pool1"), members: vec![] }).await;
        assert_eq!(body(&r[0]), json!({"members": []}), "an empty list removes every member");
        assert!(dry_err(OsAction::PoolSetMembers { pool_id: id("pool1"), members: vec![LbMemberSpec { port: 0, ..web.clone() }] }).await);
        assert!(dry_err(OsAction::PoolSetMembers { pool_id: id(" "), members: vec![web] }).await, "the pool is required");
    }

    #[tokio::test]
    async fn load_balancer_wizard_is_one_fully_populated_request() {
        let r = dry(OsAction::LoadBalancerCreate(Box::new(lb_wizard()))).await;
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/lbaas/loadbalancers", "load-balancer"));
        assert_eq!(
            body(&r[0]),
            json!({"loadbalancer": {"name": "web-lb", "vip_subnet_id": "sub-1", "listeners": [{
                "name": "http", "protocol": "HTTP", "protocol_port": 80, "connection_limit": -1,
                "default_pool": {
                    "name": "web", "protocol": "HTTP", "lb_algorithm": "ROUND_ROBIN",
                    "session_persistence": {"type": "APP_COOKIE", "cookie_name": "JSESSIONID"},
                    "members": [{"address": "10.0.0.11", "protocol_port": 8080, "subnet_id": "sub-1", "weight": 10, "backup": false,
                                 "admin_state_up": true}],
                    "healthmonitor": {"type": "HTTP", "delay": 5, "timeout": 3, "max_retries": 3,
                                      "http_method": "GET", "url_path": "/healthz", "expected_codes": "200"}
                }
            }]}})
        );
        let bare = LbCreateSpec { listener: None, pool: None, members: vec![], monitor: None, ..lb_wizard() };
        assert_eq!(body(&dry(OsAction::LoadBalancerCreate(Box::new(bare))).await[0]), json!({"loadbalancer": {"name": "web-lb", "vip_subnet_id": "sub-1"}}));
        assert!(dry_err(OsAction::LoadBalancerCreate(Box::new(LbCreateSpec { listener: None, ..lb_wizard() }))).await, "a pool needs a listener");
        for name in [None, Some(id(" "))] {
            let mut unnamed = lb_wizard();
            unnamed.pool.as_mut().unwrap().name = name;
            assert!(dry_err(OsAction::LoadBalancerCreate(Box::new(unnamed))).await, "Octavia rejects an unnamed default pool");
        }
        let slow = LbMonitorSpec { timeout: 10, ..lb_wizard().monitor.unwrap() };
        assert!(dry_err(OsAction::LoadBalancerCreate(Box::new(LbCreateSpec { monitor: Some(slow), ..lb_wizard() }))).await, "timeout > delay");
    }

    #[tokio::test]
    async fn load_balancer_parts_are_created_updated_and_deleted() {
        let w = lb_wizard();
        let r = dry(OsAction::ListenerCreate { loadbalancer_id: id("lb1"), spec: w.listener.clone().unwrap(), default_pool_id: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/lbaas/listeners"));
        assert_eq!(body(&r[0])["listener"], json!({"loadbalancer_id": "lb1", "name": "http", "protocol": "HTTP", "protocol_port": 80, "connection_limit": -1}));

        let r = dry(OsAction::ListenerUpdate { id: id("ls1"), name: id("web"), description: String::new(), connection_limit: Some(500), default_pool_id: None, admin_up: true, protocol: id("HTTP"), options: None, tls: None }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/listeners/ls1"));
        assert!(body(&r[0])["listener"].get("timeout_client_data").is_none(), "no options: left alone");
        assert_eq!(body(&r[0])["listener"]["default_pool_id"], Value::Null, "an explicit null clears it");
        assert_eq!(body(&r[0])["listener"]["connection_limit"], 500);

        let r = dry(OsAction::PoolCreate { loadbalancer_id: id("lb1"), listener_id: Some(id("ls1")), spec: w.pool.clone().unwrap() }).await;
        let p = &body(&r[0])["pool"];
        assert_eq!((p["listener_id"].clone(), p.get("loadbalancer_id").cloned()), (json!("ls1"), None), "the listener implies the load balancer");
        let r = dry(OsAction::PoolCreate { loadbalancer_id: id("lb1"), listener_id: None, spec: w.pool.clone().unwrap() }).await;
        assert_eq!(body(&r[0])["pool"]["loadbalancer_id"], "lb1");

        let r = dry(OsAction::PoolUpdate {
            id: id("pool1"),
            name: id("web"),
            description: String::new(),
            algorithm: id("LEAST_CONNECTIONS"),
            session_persistence: None,
            cookie_name: None,
            admin_up: true,
            protocol: id("HTTP"),
            tls: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/pools/pool1"));
        assert!(body(&r[0])["pool"].get("tls_enabled").is_none(), "an old Octavia never sees the key");
        assert_eq!(body(&r[0])["pool"]["session_persistence"], Value::Null, "removing persistence");

        let r = dry(OsAction::MemberCreate { pool_id: id("pool1"), spec: w.members[0].clone() }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/lbaas/pools/pool1/members"));
        let upd = |weight, monitor_address: Option<&str>, monitor_port| OsAction::MemberUpdate {
            pool_id: id("pool1"),
            id: id("m1"),
            name: String::new(),
            weight,
            backup: true,
            admin_up: false,
            monitor_address: monitor_address.map(id),
            monitor_port,
        };
        let r = dry(upd(0, None, None)).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/pools/pool1/members/m1"));
        assert_eq!(
            body(&r[0]),
            json!({"member": {"name": "", "weight": 0, "backup": true, "admin_state_up": false, "monitor_address": null, "monitor_port": null}}),
            "explicit nulls: health-check the member itself"
        );
        let r = dry(upd(1, Some(" 10.0.9.1 "), Some(9000))).await;
        assert_eq!((body(&r[0])["member"]["monitor_address"].clone(), body(&r[0])["member"]["monitor_port"].clone()), (json!("10.0.9.1"), json!(9000)));
        assert!(dry_err(upd(300, None, None)).await, "weight 0-256");
        assert!(dry_err(upd(1, Some("web"), None)).await, "monitor address must be an IP");
        assert!(dry_err(upd(1, None, Some(0))).await, "monitor port 1-65535");

        let r = dry(OsAction::MonitorCreate { pool_id: id("pool1"), spec: w.monitor.clone().unwrap() }).await;
        assert_eq!((r[0].path.as_str(), body(&r[0])["healthmonitor"]["pool_id"].clone()), ("/lbaas/healthmonitors", json!("pool1")));
        let tcp = LbMonitorSpec { kind: id("TCP"), http_method: None, url_path: None, expected_codes: None, ..w.monitor.clone().unwrap() };
        let r = dry(OsAction::MonitorUpdate { id: id("hm1"), spec: tcp, admin_up: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/lbaas/healthmonitors/hm1"));
        assert!(body(&r[0])["healthmonitor"].get("type").is_none(), "the type cannot change");

        for (a, path) in [
            (OsAction::ListenerDelete { id: id("x") }, "/lbaas/listeners/x"),
            (OsAction::PoolDelete { id: id("x") }, "/lbaas/pools/x"),
            (OsAction::MemberDelete { pool_id: id("p"), id: id("x") }, "/lbaas/pools/p/members/x"),
            (OsAction::MonitorDelete { id: id("x") }, "/lbaas/healthmonitors/x"),
        ] {
            let r = dry(a).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", path));
        }
        let r = dry(OsAction::LoadBalancerUpdate { id: id("lb1"), name: id("web"), description: id("front"), admin_up: false }).await;
        assert_eq!(body(&r[0]), json!({"loadbalancer": {"name": "web", "description": "front", "admin_state_up": false}}));
    }

    #[tokio::test]
    async fn firewall_rules_policies_and_groups() {
        let ssh = FirewallRuleSpec {
            name: id(" ssh "),
            protocol: id("tcp"),
            action: id("allow"),
            ip_version: 4,
            destination_ip: id("10.0.0.0/24"),
            destination_port: id("22"),
            enabled: true,
            ..Default::default()
        };
        let r = dry(OsAction::FirewallRuleCreate(ssh.clone())).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/fwaas/firewall_rules", "network"));
        assert_eq!(
            body(&r[0]),
            json!({"firewall_rule": {"name": "ssh", "description": "", "protocol": "tcp", "action": "allow", "ip_version": 4,
                                     "source_ip_address": null, "source_port": null, "destination_ip_address": "10.0.0.0/24",
                                     "destination_port": "22", "enabled": true}}),
            "no `shared`: the Project panel never sends it (Admin only, roadmap 94)"
        );
        let any = FirewallRuleSpec { protocol: id("any"), destination_port: String::new(), ..ssh.clone() };
        let r = dry(OsAction::FirewallRuleUpdate { id: id("r1"), spec: any }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/fwaas/firewall_rules/r1"));
        assert_eq!(body(&r[0])["firewall_rule"]["protocol"], Value::Null, "any: null");
        assert!(dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec { protocol: id("icmp"), ..ssh.clone() })).await, "no ports on ICMP");
        assert!(dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec { destination_port: id("90:80"), ..ssh.clone() })).await, "range low to high");
        assert!(dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec { destination_ip: id("fd00::/64"), ..ssh.clone() })).await, "IPv6 CIDR on an IPv4 rule");
        assert!(dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec { action: id("drop"), ..ssh.clone() })).await);

        let r = dry(OsAction::FirewallPolicyCreate { name: id("web"), description: id(" front "), rule_ids: vec![id("r2"), id("r1")], audited: false }).await;
        assert_eq!(r[0].path, "/fwaas/firewall_policies");
        assert_eq!(body(&r[0]), json!({"firewall_policy": {"name": "web", "description": "front", "firewall_rules": ["r2", "r1"], "audited": false}}), "rules in order");
        let r = dry(OsAction::FirewallPolicyUpdate { id: id("fp1"), name: id("web"), description: String::new(), audited: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/fwaas/firewall_policies/fp1"));
        assert_eq!(body(&r[0]), json!({"firewall_policy": {"name": "web", "description": "", "audited": true}}));
        let r = dry(OsAction::FirewallPolicyInsertRule { id: id("fp1"), rule_id: id("r3"), before_id: None, after_id: Some(id("r1")) }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/fwaas/firewall_policies/fp1/insert_rule"));
        assert_eq!(body(&r[0]), json!({"firewall_rule_id": "r3", "insert_after": "r1"}));
        let r = dry(OsAction::FirewallPolicyInsertRule { id: id("fp1"), rule_id: id("r3"), before_id: None, after_id: None }).await;
        assert_eq!(body(&r[0]), json!({"firewall_rule_id": "r3"}), "neither: first");
        assert!(dry_err(OsAction::FirewallPolicyInsertRule { id: id("fp1"), rule_id: id("r3"), before_id: Some(id("r1")), after_id: Some(id("r2")) }).await);
        let r = dry(OsAction::FirewallPolicyRemoveRule { id: id("fp1"), rule_id: id("r3") }).await;
        assert_eq!((r[0].path.as_str(), body(&r[0])), ("/fwaas/firewall_policies/fp1/remove_rule", json!({"firewall_rule_id": "r3"})));

        let r = dry(OsAction::FirewallGroupCreate {
            name: id("gw"),
            description: String::new(),
            ingress_policy_id: Some(id("fp1")),
            egress_policy_id: None,
            port_ids: vec![id("p1")],
            admin_up: true,
        })
        .await;
        assert_eq!(r[0].path, "/fwaas/firewall_groups");
        assert_eq!(body(&r[0]), json!({"firewall_group": {"name": "gw", "description": "", "ingress_firewall_policy_id": "fp1", "ports": ["p1"], "admin_state_up": true}}));
        let r = dry(OsAction::FirewallGroupUpdate { id: id("g1"), name: id("gw"), description: String::new(), ingress_policy_id: None, egress_policy_id: Some(id("fp2")), admin_up: false }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/fwaas/firewall_groups/g1"));
        assert_eq!(
            body(&r[0]),
            json!({"firewall_group": {"name": "gw", "description": "", "ingress_firewall_policy_id": null, "egress_firewall_policy_id": "fp2", "admin_state_up": false}}),
            "no policy: null clears it"
        );
        let r = dry(OsAction::FirewallGroupSetPorts { id: id("g1"), port_ids: vec![] }).await;
        assert_eq!(body(&r[0]), json!({"firewall_group": {"ports": []}}), "the whole list; empty removes every port");

        for (a, path) in [
            (OsAction::FirewallRuleDelete { id: id("x") }, "/fwaas/firewall_rules/x"),
            (OsAction::FirewallPolicyDelete { id: id("x") }, "/fwaas/firewall_policies/x"),
            (OsAction::FirewallGroupDelete { id: id("x") }, "/fwaas/firewall_groups/x"),
        ] {
            let r = dry(a).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", path));
        }
    }

    #[tokio::test]
    async fn trunks_and_their_subports() {
        let vlan = |p: &str, n: u16| SubportSpec { port_id: id(p), segmentation_type: id("vlan"), segmentation_id: Some(n) };
        let r = dry(OsAction::TrunkCreate { name: id("t1"), description: None, port_id: id("parent"), admin_up: true, subports: vec![vlan("sp1", 101)] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("POST", "/trunks", "network"));
        assert_eq!(
            body(&r[0]),
            json!({"trunk": {"name": "t1", "port_id": "parent", "admin_state_up": true,
                             "sub_ports": [{"port_id": "sp1", "segmentation_type": "vlan", "segmentation_id": 101}]}})
        );
        let inherit = SubportSpec { port_id: id("sp2"), segmentation_type: id("inherit"), segmentation_id: None };
        let r = dry(OsAction::TrunkAddSubports { id: id("t1"), subports: vec![inherit] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/trunks/t1/add_subports"));
        assert_eq!(body(&r[0]), json!({"sub_ports": [{"port_id": "sp2", "segmentation_type": "inherit"}]}));
        let r = dry(OsAction::TrunkRemoveSubports { id: id("t1"), port_ids: vec![id("sp1")] }).await;
        assert_eq!((r[0].path.as_str(), body(&r[0])), ("/trunks/t1/remove_subports", json!({"sub_ports": [{"port_id": "sp1"}]})));
        let r = dry(OsAction::TrunkUpdate { id: id("t1"), name: id("t1"), description: id("x"), admin_up: false }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/trunks/t1"));
        let r = dry(OsAction::TrunkDelete { id: id("t1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/trunks/t1"));
        assert!(dry_err(OsAction::TrunkAddSubports { id: id("t1"), subports: vec![vlan("sp", 0)] }).await, "VLAN 0");
        assert!(dry_err(OsAction::TrunkAddSubports { id: id("t1"), subports: vec![vlan("sp", 4095)] }).await, "VLAN 4095");
        assert!(dry_err(OsAction::TrunkRemoveSubports { id: id("t1"), port_ids: vec![] }).await);
    }

    #[tokio::test]
    async fn qos_policies_and_rules() {
        let r = dry(OsAction::QosPolicyCreate { name: id("gold"), description: Some(id("10G")), shared: false }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/qos/policies"));
        assert_eq!(body(&r[0]), json!({"policy": {"name": "gold", "description": "10G", "shared": false}}));
        let r = dry(OsAction::QosPolicyUpdate { id: id("q1"), name: id("gold"), description: String::new(), shared: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/qos/policies/q1"));
        let spec = QosRuleSpec { kind: id("bandwidth_limit"), direction: Some(id("egress")), max_kbps: Some(10_000), ..Default::default() };
        let r = dry(OsAction::QosRuleCreate { policy_id: id("q1"), spec: spec.clone() }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/qos/policies/q1/bandwidth_limit_rules"));
        assert_eq!(body(&r[0]), json!({"bandwidth_limit_rule": {"direction": "egress", "max_kbps": 10000}}));
        let r = dry(OsAction::QosRuleUpdate { policy_id: id("q1"), id: id("r1"), spec }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/qos/policies/q1/bandwidth_limit_rules/r1"));
        let r = dry(OsAction::QosRuleDelete { policy_id: id("q1"), kind: id("dscp_marking"), id: id("r2") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/qos/policies/q1/dscp_marking_rules/r2"));
        assert!(dry_err(OsAction::QosRuleDelete { policy_id: id("q1"), kind: id("../x"), id: id("r") }).await, "no path tricks");
        let r = dry(OsAction::QosPolicyDelete { id: id("q1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/qos/policies/q1"));
    }

    #[tokio::test]
    async fn router_gateway_and_floating_ip_take_a_qos_policy() {
        let r = dry(OsAction::RouterCreate { name: id("r"), admin_up: true, external_network_id: Some(id("ext")), qos_policy_id: Some(id("q1")) }).await;
        assert_eq!(body(&r[0])["router"]["external_gateway_info"], json!({"network_id": "ext", "qos_policy_id": "q1"}));
        assert!(dry_err(OsAction::RouterCreate { name: id("r"), admin_up: true, external_network_id: None, qos_policy_id: Some(id("q1")) }).await);
        let r = dry(OsAction::FloatingIpAllocate { network_id: id("ext"), description: None, port_id: None, qos_policy_id: Some(id("q1")) }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"floating_network_id": "ext", "qos_policy_id": "q1"}}));
        let r = dry(OsAction::FloatingIpAllocate { network_id: id("ext"), description: None, port_id: None, qos_policy_id: None }).await;
        assert!(body(&r[0])["floatingip"].get("qos_policy_id").is_none(), "not sent unless chosen");
    }

    #[tokio::test]
    async fn load_balancer_delete_cascades_only_when_asked() {
        let r = dry(OsAction::LoadBalancerDelete { id: id("lb1"), cascade: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/lbaas/loadbalancers/lb1?cascade=true"));
        assert_eq!(r[0].service, "load-balancer");
        let r = dry(OsAction::LoadBalancerDelete { id: id("lb1"), cascade: false }).await;
        assert_eq!(r[0].path, "/lbaas/loadbalancers/lb1");
    }

    fn deletes(ids: &[&str]) -> OsAction {
        OsAction::Batch(ids.iter().map(|i| (format!("vol {i}"), OsAction::VolumeDelete { id: id(i) })).collect())
    }

    #[tokio::test]
    async fn logout_revocation_is_a_keystone_delete_that_never_shows_the_token() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        revoke_token(&mut x, "gAAAAAB-secret").await.unwrap();
        let r = x.requests();
        assert_eq!((r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()), ("DELETE", "/auth/tokens", "identity"));
        assert!(!format!("{r:?}").contains("gAAAAAB-secret"), "the token is only in a header, never in the preview");
        let mut off = Exec::new(WriteMode::Off, None);
        assert!(revoke_token(&mut off, "t").await.is_err(), "writes off: nothing sent");
    }

    #[tokio::test]
    async fn batch_builds_one_request_per_item_in_order() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &deletes(&["a", "b", "c"])).await.unwrap();
        let out = x.outcome();
        let paths: Vec<_> = out.requests.iter().map(|r| (r.method.as_str(), r.path.as_str())).collect();
        assert_eq!(paths, [("DELETE", "/volumes/a"), ("DELETE", "/volumes/b"), ("DELETE", "/volumes/c")]);
        let labels: Vec<_> = out.results.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["vol a", "vol b", "vol c"]);
        assert!(out.results.iter().all(|r| r.error.is_none()));
    }

    #[tokio::test]
    async fn batch_keeps_going_after_a_failing_item() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        let action = OsAction::Batch(vec![
            ("bad".into(), OsAction::ServerUpdate { id: id("s1"), name: "  ".into() }),
            ("good".into(), OsAction::ServerDelete { id: id("s2") }),
        ]);
        apply(&mut x, &action).await.unwrap();
        let out = x.outcome();
        assert!(out.results[0].error.as_deref().unwrap().contains("name"));
        assert_eq!(out.results[1].error, None);
        assert_eq!(out.requests.last().unwrap().path, "/servers/s2");
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

    #[tokio::test]
    async fn server_lifecycle_actions_parity() {
        let one = |a| async move {
            let r = dry(a).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/servers/s/action"));
            body(&r[0])
        };
        assert_eq!(one(OsAction::ServerResize { id: id("s"), flavor_id: id("f2") }).await, json!({"resize": {"flavorRef": "f2"}}));
        assert_eq!(one(OsAction::ServerConfirmResize { id: id("s") }).await, json!({"confirmResize": null}));
        assert_eq!(one(OsAction::ServerRevertResize { id: id("s") }).await, json!({"revertResize": null}));
        assert_eq!(one(OsAction::ServerRebuild { id: id("s"), image_id: id("img") }).await, json!({"rebuild": {"imageRef": "img"}}));
        assert_eq!(one(OsAction::ServerShelve { id: id("s") }).await, json!({"shelve": null}));
        assert_eq!(one(OsAction::ServerUnshelve { id: id("s"), availability_zone: None }).await, json!({"unshelve": null}));
        assert_eq!(
            one(OsAction::ServerUnshelve { id: id("s"), availability_zone: Some(id("az1")) }).await,
            json!({"unshelve": {"availability_zone": "az1"}})
        );
        assert_eq!(one(OsAction::ServerRescue { id: id("s"), image_id: None }).await, json!({"rescue": {}}));
        assert_eq!(
            one(OsAction::ServerRescue { id: id("s"), image_id: Some(id("r")) }).await,
            json!({"rescue": {"rescue_image_ref": "r"}})
        );
        assert_eq!(one(OsAction::ServerUnrescue { id: id("s") }).await, json!({"unrescue": null}));
        assert!(dry_err(OsAction::ServerResize { id: id("s"), flavor_id: id(" ") }).await);
        assert!(dry_err(OsAction::ServerRebuild { id: id("s"), image_id: id("") }).await);
    }

    #[tokio::test]
    async fn rescue_in_dry_run_has_no_secret() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &OsAction::ServerRescue { id: id("s"), image_id: None }).await.unwrap();
        assert!(x.outcome().secret.is_none());
    }

    #[tokio::test]
    async fn interfaces_attach_by_network_or_port_and_detach() {
        let r = dry(OsAction::ServerAttachInterface { id: id("s"), network_id: Some(id("n")), port_id: None, fixed_ip: Some(id("10.0.0.9")) }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/servers/s/os-interface"));
        assert_eq!(body(&r[0]), json!({"interfaceAttachment": {"net_id": "n", "fixed_ips": [{"ip_address": "10.0.0.9"}]}}));
        let r = dry(OsAction::ServerAttachInterface { id: id("s"), network_id: None, port_id: Some(id("p")), fixed_ip: Some(id("ignored")) }).await;
        assert_eq!(body(&r[0]), json!({"interfaceAttachment": {"port_id": "p"}}));
        for (net, port) in [(None, None), (Some(id("n")), Some(id("p")))] {
            assert!(dry_err(OsAction::ServerAttachInterface { id: id("s"), network_id: net, port_id: port, fixed_ip: None }).await);
        }
        let r = dry(OsAction::ServerDetachInterface { id: id("s"), port_id: id("p") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/servers/s/os-interface/p"));
    }

    #[tokio::test]
    async fn port_security_groups_and_allocate_with_association() {
        let r = dry(OsAction::PortSetSecurityGroups { id: id("p"), security_group_ids: vec![id("g1"), id("g2")] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/ports/p"));
        assert_eq!(body(&r[0]), json!({"port": {"security_groups": ["g1", "g2"]}}));
        let r = dry(OsAction::PortSetSecurityGroups { id: id("p"), security_group_ids: vec![] }).await;
        assert_eq!(body(&r[0]), json!({"port": {"security_groups": []}}));
        let r = dry(OsAction::FloatingIpAllocate { network_id: id("ext"), description: None, port_id: Some(id("p")), qos_policy_id: None }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"floating_network_id": "ext", "port_id": "p"}}));
    }

    #[tokio::test]
    async fn port_update_sends_only_the_changed_fields() {
        let upd = |name: Option<&str>, port_security, groups: Option<Vec<String>>| OsAction::PortUpdate {
            id: id("p"),
            name: name.map(id),
            admin_up: None,
            vnic_type: None,
            port_security,
            security_group_ids: groups,
        };
        let r = dry(upd(Some(" web "), None, None)).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/ports/p"));
        assert_eq!(body(&r[0]), json!({"port": {"name": "web"}}), "rename only");
        let r = dry(upd(None, Some(false), Some(vec![]))).await;
        assert_eq!(body(&r[0]), json!({"port": {"port_security_enabled": false, "security_groups": []}}), "port security off");
        let r = dry(upd(None, None, Some(vec![id("g1"), id("g2")]))).await;
        assert_eq!(body(&r[0]), json!({"port": {"security_groups": ["g1", "g2"]}}), "groups only");
        let r = dry(OsAction::PortUpdate {
            id: id("p"),
            name: None,
            admin_up: Some(false),
            vnic_type: Some(id("direct")),
            port_security: None,
            security_group_ids: None,
        })
        .await;
        assert_eq!(body(&r[0]), json!({"port": {"admin_state_up": false, "binding:vnic_type": "direct"}}));
        let bad = OsAction::PortUpdate { id: id("p"), name: None, admin_up: None, vnic_type: Some(id("warp")), port_security: None, security_group_ids: None };
        assert!(dry_err(bad).await, "unknown VNIC type is refused before sending");
    }

    // --- Key Manager (roadmap 55)

    fn secret_spec() -> SecretSpec {
        SecretSpec {
            name: Some(id("db-pass")),
            secret_type: id("passphrase"),
            payload: Some(Hidden(id("s3cr3t"))),
            payload_content_type: id("text/plain"),
            algorithm: None,
            bit_length: None,
            mode: None,
            expiration: Some(id("2027-01-01T00:00:00")),
        }
    }

    #[tokio::test]
    async fn store_secret_sends_barbicans_body_and_masks_the_payload() {
        let r = dry(OsAction::SecretCreate(secret_spec())).await;
        assert_eq!((r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()), ("key-manager", "POST", "/v1/secrets"));
        assert_eq!(
            body(&r[0]),
            json!({"name": "db-pass", "secret_type": "passphrase", "payload": "********", "payload_content_type": "text/plain", "expiration": "2027-01-01T00:00:00"}),
            "the preview never shows the payload"
        );
        assert!(!format!("{:?}", OsAction::SecretCreate(secret_spec())).contains("s3cr3t"), "nor does Debug (logs)");
        let ep = JsonRequest::key_manager(http::Method::POST, "secrets", Some(secret_body(&secret_spec()).unwrap()));
        assert_eq!(serde_json::from_slice::<Value>(&ep.body().unwrap().unwrap().1).unwrap()["payload"], "s3cr3t", "sent as typed");
    }

    #[test]
    fn secret_values_are_wiped() {
        let mut v = json!({"secret": {"payload": "s3cr3t", "name": "keep"}, "auth": [{"password": "pw"}]});
        wipe_secrets(&mut v);
        assert_eq!(v, json!({"secret": {"payload": "", "name": "keep"}, "auth": [{"password": ""}]}));
    }

    #[tokio::test]
    async fn store_secret_binary_payloads_are_base64_and_metadata_only_has_no_content_type() {
        let bin = SecretSpec { payload: Some(Hidden(id("AAEC\nAw=="))), payload_content_type: id("application/octet-stream"), secret_type: id("symmetric"), algorithm: Some(id("aes")), bit_length: Some(256), mode: Some(id("cbc")), expiration: None, ..secret_spec() };
        let b = secret_body(&bin).unwrap();
        assert_eq!((b["payload"].as_str(), b["payload_content_encoding"].as_str()), (Some("AAECAw=="), Some("base64")), "whitespace dropped");
        assert_eq!((b["algorithm"].as_str(), b["bit_length"].as_i64(), b["mode"].as_str()), (Some("aes"), Some(256), Some("cbc")));
        let bad = SecretSpec { payload: Some(Hidden(id("not base64!"))), ..bin.clone() };
        assert!(dry_err(OsAction::SecretCreate(bad)).await, "invalid base64 is refused before sending");
        let empty = secret_body(&SecretSpec { payload: None, ..secret_spec() }).unwrap();
        assert!(empty.get("payload_content_type").is_none(), "Barbican refuses a content type without a payload");
        assert!(dry_err(OsAction::SecretCreate(SecretSpec { secret_type: id("rsa"), ..secret_spec() })).await, "unknown type");
        assert!(dry_err(OsAction::SecretCreate(SecretSpec { bit_length: Some(0), ..secret_spec() })).await);
    }

    #[tokio::test]
    async fn containers_take_secret_refs_by_role() {
        let cert = |role: &str, s: &str| (id(role), format!("https://kms:9311/v1/secrets/{s}"));
        let r = dry(OsAction::ContainerCreate { name: Some(id("web")), container_type: id("certificate"), secrets: vec![cert("certificate", "s1"), cert("private_key", "s2")] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/v1/containers"));
        assert_eq!(
            body(&r[0]),
            json!({"name": "web", "type": "certificate", "secret_refs": [
                {"name": "certificate", "secret_ref": "https://kms:9311/v1/secrets/s1"},
                {"name": "private_key", "secret_ref": "https://kms:9311/v1/secrets/s2"}
            ]})
        );
        let make = |t: &str, s: Vec<(String, String)>| OsAction::ContainerCreate { name: None, container_type: id(t), secrets: s };
        assert!(dry_err(make("certificate", vec![cert("private_key", "s2")])).await, "a certificate container needs its certificate");
        assert!(dry_err(make("certificate", vec![cert("certificate", "s1"), cert("certificate", "s3")])).await, "one secret per role");
        assert!(dry_err(make("rsa", vec![cert("certificate", "s1")])).await, "roles follow the type");
        assert!(dry_err(make("generic", vec![(id("a"), id("s1"))])).await, "a ref, not an ID");
        assert!(dry_err(make("generic", vec![cert("a", "../x")])).await, "no path tricks in the ref");
        let g = dry(make("generic", vec![cert("anything", "s1")])).await;
        assert!(body(&g[0]).get("name").is_none(), "unnamed");
    }

    #[tokio::test]
    async fn stack_actions_and_delete() {
        let (name, sid) = (id("web"), id("s1"));
        for (action, verb) in [
            (OsAction::StackCheck { name: name.clone(), id: sid.clone() }, "check"),
            (OsAction::StackSuspend { name: name.clone(), id: sid.clone() }, "suspend"),
            (OsAction::StackResume { name: name.clone(), id: sid.clone() }, "resume"),
        ] {
            let r = dry(action.clone()).await;
            assert_eq!((r[0].service.as_str(), r[0].method.as_str()), ("orchestration", "POST"));
            assert!(r[0].path.ends_with("/stacks/web/s1/actions"), "{}", r[0].path);
            assert_eq!(body(&r[0]), json!({ verb: null }));
            assert_eq!(action.plugins(), [Plugin::Orchestration]);
        }
        let r = dry(OsAction::StackDelete { name: name.clone(), id: sid.clone() }).await;
        assert_eq!(r[0].method.as_str(), "DELETE");
        assert!(r[0].path.ends_with("/stacks/web/s1"), "{}", r[0].path);
        assert!(OsAction::StackDelete { name: id("a/b"), id: sid.clone() }.check_ids().is_err(), "the name is a path segment");
        assert!(dry_err(OsAction::StackDelete { name: String::new(), id: sid }).await, "a malformed selection key");
    }

    fn spec() -> StackSpec {
        StackSpec {
            name: id("web"),
            source: TemplateSource { template: "heat_template_version: 2018-08-31".into(), url: None, environment: Some("parameters: {}".into()) },
            parameters: vec![("flavor".into(), "m1.small".into()), ("db_password".into(), "s3cret".into())],
            hidden: vec!["db_password".into()],
            timeout_mins: Some(30),
            disable_rollback: true,
        }
    }

    #[tokio::test]
    async fn stack_create_and_update_bodies() {
        let r = dry(OsAction::StackCreate(spec())).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/stacks"));
        assert_eq!(body(&r[0]), json!({
            "stack_name": "web", "template": "heat_template_version: 2018-08-31", "environment": "parameters: {}",
            "parameters": {"flavor": "m1.small", "db_password": "********"}, "timeout_mins": 30, "disable_rollback": true
        }), "hidden parameters masked in the preview");
        let r = dry(OsAction::StackUpdate { id: id("s1"), spec: spec() }).await;
        assert_eq!(r[0].method.as_str(), "PATCH", "PATCH keeps the parameters the form leaves out");
        assert!(r[0].path.ends_with("/stacks/web/s1"), "{}", r[0].path);
        assert!(body(&r[0]).get("stack_name").is_none());
        assert_eq!(OsAction::StackCreate(spec()).plugins(), [Plugin::Orchestration]);
    }

    #[tokio::test]
    async fn stack_bodies_are_checked() {
        let url = StackSpec { source: TemplateSource { template: String::new(), url: Some("https://git.example/t.yaml".into()), environment: None }, ..spec() };
        let r = dry(OsAction::StackCreate(url)).await;
        assert_eq!(body(&r[0])["template_url"], json!("https://git.example/t.yaml"));
        assert!(body(&r[0]).get("environment").is_none());
        let bad = |f: fn(&mut StackSpec)| {
            let mut s = spec();
            f(&mut s);
            OsAction::StackCreate(s)
        };
        assert!(dry_err(bad(|s| s.name = "1web".into())).await, "names start with a letter");
        assert!(dry_err(bad(|s| s.source.template = " ".into())).await, "a template is required");
        assert!(dry_err(bad(|s| s.source.url = Some("file:///etc/passwd".into()))).await, "http(s) URLs only");
        assert!(dry_err(bad(|s| s.source.template = "x".repeat(MAX_TEMPLATE_BYTES + 1))).await, "Heat's size limit");
        assert!(dry_err(bad(|s| s.timeout_mins = Some(0))).await);
    }

    #[tokio::test]
    async fn secret_acl_set_and_reset() {
        let u = || vec![id("u1"), id("u2")];
        let r = dry(OsAction::SecretAclSet { id: id("s1"), project_access: false, users: u() }).await;
        assert_eq!((r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()), ("key-manager", "PUT", "/v1/secrets/s1/acl"));
        assert_eq!(body(&r[0]), json!({"read": {"project-access": false, "users": ["u1", "u2"]}}), "PUT replaces the whole read ACL");
        let r = dry(OsAction::SecretAclDelete { id: id("s1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/v1/secrets/s1/acl"));
        assert!(dry_err(OsAction::SecretAclSet { id: id("s1"), project_access: true, users: vec!["u1 u2".into()] }).await, "one ID per entry");
        assert!(dry_err(OsAction::SecretAclSet { id: id("s1"), project_access: true, users: vec!["a/b".into()] }).await);
        assert_eq!(OsAction::SecretAclDelete { id: id("s1") }.plugins(), [Plugin::KeyManager]);
        let r = dry(OsAction::ContainerAclSet { id: id("c1"), project_access: true, users: vec![id("u1")] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/v1/containers/c1/acl"));
        assert_eq!(body(&r[0]), json!({"read": {"project-access": true, "users": ["u1"]}}));
        let r = dry(OsAction::ContainerAclDelete { id: id("c1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/v1/containers/c1/acl"));
        assert!(dry_err(OsAction::ContainerAclSet { id: id("c1"), project_access: true, users: vec!["u1,u2".into()] }).await);
    }

    #[tokio::test]
    async fn deleting_secrets_and_containers() {
        let r = dry(OsAction::SecretDelete { id: id("s1") }).await;
        assert_eq!((r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()), ("key-manager", "DELETE", "/v1/secrets/s1"));
        let r = dry(OsAction::ContainerDelete { id: id("c1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/v1/containers/c1"));
        assert_eq!(OsAction::SecretDelete { id: id("s1") }.plugins(), [Plugin::KeyManager]);
    }

    async fn dry_err(action: OsAction) -> bool {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &action).await.is_err()
    }

    #[tokio::test]
    async fn volume_actions_parity() {
        let r = dry(OsAction::VolumeRetype { id: id("v"), new_type: id("fast"), migrate: true }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/volumes/v/action"));
        assert_eq!(body(&r[0]), json!({"os-retype": {"new_type": "fast", "migration_policy": "on-demand"}}));
        let r = dry(OsAction::VolumeRetype { id: id("v"), new_type: id("fast"), migrate: false }).await;
        assert_eq!(body(&r[0])["os-retype"]["migration_policy"], "never");
        let up = |format: &str, visibility: Option<&str>| OsAction::VolumeUploadImage {
            id: id("v"),
            image_name: id("img"),
            disk_format: id(format),
            force: true,
            visibility: visibility.map(id),
        };
        let r = dry(up("qcow2", None)).await;
        assert_eq!(
            body(&r[0]),
            json!({"os-volume_upload_image": {"image_name": "img", "disk_format": "qcow2", "container_format": "bare", "force": true}})
        );
        let r = dry(up("raw", Some("community"))).await;
        assert_eq!(body(&r[0])["os-volume_upload_image"]["visibility"], "community");
        assert!(dry_err(up("iso", None)).await);
        assert!(dry_err(up("raw", Some("everyone"))).await);
        assert!(dry_err(OsAction::VolumeRetype { id: id("v"), new_type: id(" "), migrate: false }).await);
    }

    #[tokio::test]
    async fn volume_transfers() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &OsAction::VolumeTransferCreate { volume_id: id("v"), name: Some(id("to ops")) }).await.unwrap();
        let out = x.outcome();
        assert_eq!((out.requests[0].method.as_str(), out.requests[0].path.as_str()), ("POST", "/os-volume-transfer"));
        assert_eq!(body(&out.requests[0]), json!({"transfer": {"volume_id": "v", "name": "to ops"}}));
        assert!(out.secret.is_none(), "dry-run creates no key");
        let r = dry(OsAction::VolumeTransferAccept { transfer_id: id("t"), auth_key: id("k3y") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/os-volume-transfer/t/accept"));
        assert_eq!(body(&r[0]), json!({"accept": {"auth_key": "k3y"}}));
        assert!(dry_err(OsAction::VolumeTransferAccept { transfer_id: id("t"), auth_key: id("") }).await);
    }

    #[tokio::test]
    async fn router_routes_as_a_whole_list() {
        let r = dry(OsAction::RouterSetRoutes { id: id("r"), routes: vec![(id("10.9.0.0/16"), id("10.0.0.5"))] }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r"));
        assert_eq!(body(&r[0]), json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.5"}]}}));
        let r = dry(OsAction::RouterSetRoutes { id: id("r"), routes: vec![] }).await;
        assert_eq!(body(&r[0]), json!({"router": {"routes": []}}), "removing the last route sends an empty list");
    }

    fn alarm_spec() -> AlarmSpec {
        AlarmSpec {
            name: id("cpu-high"),
            alarm_type: id("prometheus"),
            severity: id("critical"),
            enabled: true,
            alarm_actions: vec![id("https://hooks.example/alarm"), id("log://")],
            query: id("rate(cpu[5m])"),
            comparison: id("gt"),
            threshold: Some(0.8),
            // Ignored: not a field of prometheus alarms.
            metric: id("cpu"),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn alarm_create_and_update_send_the_whole_alarm() {
        let r = dry(OsAction::AlarmCreate(alarm_spec())).await;
        assert_eq!((r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()), ("alarming", "POST", "/v2/alarms"));
        assert_eq!(
            body(&r[0]),
            json!({"name": "cpu-high", "description": "", "type": "prometheus", "severity": "critical", "enabled": true,
                   "repeat_actions": false, "alarm_actions": ["https://hooks.example/alarm", "log://"], "ok_actions": [],
                   "insufficient_data_actions": [], "time_constraints": [],
                   "prometheus_rule": {"query": "rate(cpu[5m])", "comparison_operator": "gt", "threshold": 0.8}})
        );
        assert_eq!(OsAction::AlarmCreate(alarm_spec()).plugins(), [Plugin::Alarming]);
        let gnocchi = AlarmSpec {
            alarm_type: id("gnocchi_resources_threshold"),
            resource_type: id("instance"),
            resource_id: id("srv1"),
            aggregation: id("mean"),
            granularity: Some(300),
            evaluation_periods: Some(3),
            time_constraints: vec![AlarmTimeConstraint { name: id("office"), start: id("0 9 * * 1-5"), duration: Some(32400), timezone: None, description: None }],
            ..alarm_spec()
        };
        let r = dry(OsAction::AlarmUpdate { id: id("a1"), spec: gnocchi }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/v2/alarms/a1"));
        let b = body(&r[0]);
        assert_eq!(
            b["gnocchi_resources_threshold_rule"],
            json!({"metric": "cpu", "resource_type": "instance", "resource_id": "srv1", "aggregation_method": "mean",
                   "comparison_operator": "gt", "threshold": 0.8, "granularity": 300, "evaluation_periods": 3})
        );
        assert_eq!(b["time_constraints"], json!([{"name": "office", "start": "0 9 * * 1-5", "duration": 32400}]), "kept: the PUT replaces the alarm");
        assert!(b.get("prometheus_rule").is_none());
        let by_metrics = AlarmSpec { alarm_type: id("gnocchi_aggregation_by_metrics_threshold"), metrics: vec![id("m1"), id(" m2 ")], aggregation: id("mean"), granularity: Some(60), evaluation_periods: Some(1), ..alarm_spec() };
        assert_eq!(body(&dry(OsAction::AlarmCreate(by_metrics)).await[0])["gnocchi_aggregation_by_metrics_threshold_rule"]["metrics"], json!(["m1", "m2"]));
        let by_resources = AlarmSpec {
            alarm_type: id("gnocchi_aggregation_by_resources_threshold"),
            resource_type: id("instance"),
            query: id(r#"{"=": {"server_group": "web"}}"#),
            aggregation: id("mean"),
            granularity: Some(60),
            evaluation_periods: Some(1),
            ..alarm_spec()
        };
        assert_eq!(body(&dry(OsAction::AlarmCreate(by_resources)).await[0])["gnocchi_aggregation_by_resources_threshold_rule"]["query"], json!(r#"{"=": {"server_group": "web"}}"#), "Aodh stores the search as text");
        let event = AlarmSpec { alarm_type: id("event"), event_type: id("compute.instance.delete.end"), event_query: vec![id("traits.instance_id = srv1"), id("traits.state ≠ error")], ..alarm_spec() };
        assert_eq!(
            body(&dry(OsAction::AlarmCreate(event)).await[0])["event_rule"],
            json!({"event_type": "compute.instance.delete.end", "query": [{"field": "traits.instance_id", "op": "eq", "value": "srv1"}, {"field": "traits.state", "op": "ne", "value": "error"}]}),
            "filters as the detail page shows them"
        );
    }

    #[test]
    fn created_ids_of_every_create_response() {
        assert_eq!(created_id(&json!({"id": "n1"})).as_deref(), Some("n1"));
        assert_eq!(created_id(&json!({"stack": {"id": "s1", "links": []}})).as_deref(), Some("s1"));
        assert_eq!(created_id(&json!({"secret_ref": "https://kms/v1/secrets/k1"})).as_deref(), Some("k1"));
        assert_eq!(created_id(&json!({"alarm_id": "a1", "name": "cpu"})).as_deref(), Some("a1"), "Aodh's create returns the alarm");
        assert_eq!(created_id(&json!({"name": "x"})), None);
    }

    #[tokio::test]
    async fn alarm_specs_are_checked() {
        let bad = |f: fn(&mut AlarmSpec)| {
            let mut s = alarm_spec();
            f(&mut s);
            OsAction::AlarmCreate(s)
        };
        assert!(dry_err(bad(|s| s.name = id(" "))).await, "name");
        assert!(dry_err(bad(|s| s.alarm_type = id("composite"))).await, "type the form doesn't offer");
        assert!(dry_err(bad(|s| s.severity = id("urgent"))).await, "severity");
        assert!(dry_err(bad(|s| s.threshold = None)).await, "threshold");
        assert!(dry_err(bad(|s| s.comparison = id(">"))).await, "operator by name");
        assert!(dry_err(bad(|s| s.query = id(""))).await, "PromQL");
        assert!(dry_err(bad(|s| s.ok_actions = vec![id("ftp://x")])).await, "action scheme");
        assert!(dry_err(bad(|s| s.alarm_actions = vec![id("https://")])).await, "a scheme alone");
        assert!(dry_err(bad(|s| {
            s.alarm_type = id("gnocchi_aggregation_by_resources_threshold");
            s.resource_type = id("instance");
            s.query = id("{not json");
            s.aggregation = id("mean");
            s.granularity = Some(60);
            s.evaluation_periods = Some(1);
        }))
        .await, "the Gnocchi search must be JSON");
        assert!(dry_err(bad(|s| {
            s.alarm_type = id("gnocchi_resources_threshold");
            s.resource_type = id("instance");
            s.resource_id = id("srv1");
            s.aggregation = id("mean");
            s.granularity = Some(0);
            s.evaluation_periods = Some(1);
        }))
        .await, "granularity 0");
        assert!(dry_err(bad(|s| {
            s.alarm_type = id("event");
            s.event_query = vec![id("traits.x")];
        }))
        .await, "filter without operator");
        assert!(dry_err(OsAction::AlarmUpdate { id: id(""), spec: alarm_spec() }).await, "no ID");
    }

}
