//! Barbican writes (roadmap 55): secrets, containers, ACLs; orders (roadmap 58).

use super::*;

/// Barbican's secret types (`POST /v1/secrets` `secret_type`).
pub const SECRET_TYPES: [&str; 6] = ["symmetric", "public", "private", "passphrase", "certificate", "opaque"];

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
                let mut decoded = base64::engine::general_purpose::STANDARD
                    .decode(&clean)
                    .map_err(|_| AppError::invalid("the payload is not valid base64"))?;
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
        "certificate" => Some((
            &["certificate", "private_key", "private_key_passphrase", "intermediates"],
            &["certificate"],
        )),
        "rsa" => Some((
            &["public_key", "private_key", "private_key_passphrase"],
            &["public_key", "private_key"],
        )),
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
        let id = r
            .trim_end_matches('/')
            .rsplit_once("/secrets/")
            .map(|(_, id)| id)
            .filter(|id| !id.contains('/'));
        match id {
            Some(id) if r.starts_with("https://") || r.starts_with("http://") => check_id(id)?,
            _ => return Err(AppError::invalid(format!("not a secret reference: {r}"))),
        }
        refs.push(json!({"name": role, "secret_ref": r}));
    }
    if let Some((allowed, required)) = container_roles(container_type) {
        let roles: Vec<&str> = secrets.iter().map(|(r, _)| r.trim()).collect();
        if let Some(r) = roles.iter().find(|r| !allowed.contains(r)) {
            return Err(AppError::invalid(format!(
                "a {container_type} container has no {r} role"
            )));
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

/// `POST /v1/orders` (roadmap 58): `{"type", "meta": {…}}`. The spec is checked
/// again here ([`OrderSpec::check`]), whatever the dialog allowed; the
/// passphrase is masked in previews (`SECRET_KEYS`).
fn order_body(o: &OrderSpec) -> R<Value> {
    o.check().map_err(AppError::invalid)?;
    let mut meta = serde_json::Map::new();
    if let Some(n) = opt(&o.name) {
        meta.insert("name".into(), json!(n));
    }
    meta.insert("algorithm".into(), json!(o.algorithm()));
    meta.insert("bit_length".into(), json!(o.bit_length));
    if let Some(m) = opt(&o.mode) {
        meta.insert("mode".into(), json!(m));
    }
    meta.insert("payload_content_type".into(), json!("application/octet-stream"));
    if let Some(e) = opt(&o.expiration) {
        meta.insert("expiration".into(), json!(e));
    }
    if let Some(p) = o.passphrase.as_ref().filter(|p| !p.0.is_empty()) {
        meta.insert("passphrase".into(), json!(p.0));
    }
    Ok(json!({"type": o.order_type, "meta": meta}))
}

pub(super) async fn order_create(x: &mut Exec<'_>, spec: &OrderSpec) -> R<()> {
    x.create(
        JsonRequest::key_manager(http::Method::POST, "orders", Some(order_body(spec)?)),
        "order",
    )
    .await?;
    Ok(())
}

pub(super) async fn order_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::key_manager(
        http::Method::DELETE,
        format!("orders/{}", non_empty(id, "order")?),
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn secret_create(x: &mut Exec<'_>, spec: &SecretSpec) -> R<()> {
    x.create(
        JsonRequest::key_manager(http::Method::POST, "secrets", Some(secret_body(spec)?)),
        "secret",
    )
    .await?;
    Ok(())
}

pub(super) async fn secret_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::key_manager(
        http::Method::DELETE,
        format!("secrets/{}", non_empty(id, "secret")?),
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn container_create(
    x: &mut Exec<'_>,
    name: &Option<String>,
    container_type: &str,
    secrets: &[(String, String)],
) -> R<()> {
    let body = container_body(name.as_deref(), container_type, secrets)?;
    x.create(
        JsonRequest::key_manager(http::Method::POST, "containers", Some(body)),
        "container",
    )
    .await?;
    Ok(())
}

pub(super) async fn container_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::key_manager(
        http::Method::DELETE,
        format!("containers/{}", non_empty(id, "container")?),
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn secret_acl_set(x: &mut Exec<'_>, id: &str, project_access: &bool, users: &[String]) -> R<()> {
    x.send(acl_put(
        &format!("secrets/{}/acl", non_empty(id, "secret")?),
        *project_access,
        users,
    )?)
    .await?;
    Ok(())
}

pub(super) async fn secret_acl_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::key_manager(
        http::Method::DELETE,
        format!("secrets/{}/acl", non_empty(id, "secret")?),
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn container_acl_set(x: &mut Exec<'_>, id: &str, project_access: &bool, users: &[String]) -> R<()> {
    x.send(acl_put(
        &format!("containers/{}/acl", non_empty(id, "container")?),
        *project_access,
        users,
    )?)
    .await?;
    Ok(())
}

pub(super) async fn container_acl_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::key_manager(
        http::Method::DELETE,
        format!("containers/{}/acl", non_empty(id, "container")?),
        None,
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    // --- Key Manager (roadmap 55)

    #[tokio::test]
    async fn orders_send_barbicans_body_and_mask_the_passphrase() {
        let key = OrderSpec {
            order_type: id("key"),
            name: Some(id("db-key")),
            bit_length: 256,
            mode: Some(id("cbc")),
            expiration: Some(id("2027-01-01T00:00:00")),
            passphrase: None,
        };
        let r = dry(OsAction::OrderCreate(key)).await;
        assert_eq!(
            (r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()),
            ("key-manager", "POST", "/v1/orders")
        );
        assert_eq!(
            body(&r[0]),
            json!({"type": "key", "meta": {"name": "db-key", "algorithm": "aes", "bit_length": 256, "mode": "cbc",
                "payload_content_type": "application/octet-stream", "expiration": "2027-01-01T00:00:00"}})
        );
        let pair = OrderSpec {
            order_type: id("asymmetric"),
            name: None,
            bit_length: 2048,
            mode: None,
            expiration: None,
            passphrase: Some(Hidden(id("pw-123"))),
        };
        let r = dry(OsAction::OrderCreate(pair.clone())).await;
        assert_eq!(
            body(&r[0]),
            json!({"type": "asymmetric", "meta": {"algorithm": "rsa", "bit_length": 2048,
                "payload_content_type": "application/octet-stream", "passphrase": "********"}}),
            "the passphrase never shows in the preview"
        );
        assert!(
            !format!("{:?}", OsAction::OrderCreate(pair)).contains("pw-123"),
            "nor in Debug (logs)"
        );
        let bad = OrderSpec {
            order_type: id("key"),
            name: None,
            bit_length: 2048,
            mode: None,
            expiration: None,
            passphrase: None,
        };
        assert!(order_body(&bad).unwrap_err().to_string().contains("bit length"));
        assert!(
            dry_err(OsAction::OrderCreate(bad)).await,
            "the server checks the size too"
        );
        let r = dry(OsAction::OrderDelete { id: id("o1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/v1/orders/o1"));
    }

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
        assert_eq!(
            (r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()),
            ("key-manager", "POST", "/v1/secrets")
        );
        assert_eq!(
            body(&r[0]),
            json!({"name": "db-pass", "secret_type": "passphrase", "payload": "********", "payload_content_type": "text/plain", "expiration": "2027-01-01T00:00:00"}),
            "the preview never shows the payload"
        );
        assert!(
            !format!("{:?}", OsAction::SecretCreate(secret_spec())).contains("s3cr3t"),
            "nor does Debug (logs)"
        );
        let ep = JsonRequest::key_manager(
            http::Method::POST,
            "secrets",
            Some(secret_body(&secret_spec()).unwrap()),
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&ep.body().unwrap().unwrap().1).unwrap()["payload"],
            "s3cr3t",
            "sent as typed"
        );
    }

    #[tokio::test]
    async fn store_secret_binary_payloads_are_base64_and_metadata_only_has_no_content_type() {
        let bin = SecretSpec {
            payload: Some(Hidden(id("AAEC\nAw=="))),
            payload_content_type: id("application/octet-stream"),
            secret_type: id("symmetric"),
            algorithm: Some(id("aes")),
            bit_length: Some(256),
            mode: Some(id("cbc")),
            expiration: None,
            ..secret_spec()
        };
        let b = secret_body(&bin).unwrap();
        assert_eq!(
            (b["payload"].as_str(), b["payload_content_encoding"].as_str()),
            (Some("AAECAw=="), Some("base64")),
            "whitespace dropped"
        );
        assert_eq!(
            (b["algorithm"].as_str(), b["bit_length"].as_i64(), b["mode"].as_str()),
            (Some("aes"), Some(256), Some("cbc"))
        );
        let bad = SecretSpec {
            payload: Some(Hidden(id("not base64!"))),
            ..bin.clone()
        };
        assert!(
            dry_err(OsAction::SecretCreate(bad)).await,
            "invalid base64 is refused before sending"
        );
        let empty = secret_body(&SecretSpec {
            payload: None,
            ..secret_spec()
        })
        .unwrap();
        assert!(
            empty.get("payload_content_type").is_none(),
            "Barbican refuses a content type without a payload"
        );
        assert!(
            dry_err(OsAction::SecretCreate(SecretSpec {
                secret_type: id("rsa"),
                ..secret_spec()
            }))
            .await,
            "unknown type"
        );
        assert!(
            dry_err(OsAction::SecretCreate(SecretSpec {
                bit_length: Some(0),
                ..secret_spec()
            }))
            .await
        );
    }

    #[tokio::test]
    async fn containers_take_secret_refs_by_role() {
        let cert = |role: &str, s: &str| (id(role), format!("https://kms:9311/v1/secrets/{s}"));
        let r = dry(OsAction::ContainerCreate {
            name: Some(id("web")),
            container_type: id("certificate"),
            secrets: vec![cert("certificate", "s1"), cert("private_key", "s2")],
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/v1/containers"));
        assert_eq!(
            body(&r[0]),
            json!({"name": "web", "type": "certificate", "secret_refs": [
                {"name": "certificate", "secret_ref": "https://kms:9311/v1/secrets/s1"},
                {"name": "private_key", "secret_ref": "https://kms:9311/v1/secrets/s2"}
            ]})
        );
        let make = |t: &str, s: Vec<(String, String)>| OsAction::ContainerCreate {
            name: None,
            container_type: id(t),
            secrets: s,
        };
        assert!(
            dry_err(make("certificate", vec![cert("private_key", "s2")])).await,
            "a certificate container needs its certificate"
        );
        assert!(
            dry_err(make(
                "certificate",
                vec![cert("certificate", "s1"), cert("certificate", "s3")]
            ))
            .await,
            "one secret per role"
        );
        assert!(
            dry_err(make("rsa", vec![cert("certificate", "s1")])).await,
            "roles follow the type"
        );
        assert!(
            dry_err(make("generic", vec![(id("a"), id("s1"))])).await,
            "a ref, not an ID"
        );
        assert!(
            dry_err(make("generic", vec![cert("a", "../x")])).await,
            "no path tricks in the ref"
        );
        let g = dry(make("generic", vec![cert("anything", "s1")])).await;
        assert!(body(&g[0]).get("name").is_none(), "unnamed");
    }

    #[tokio::test]
    async fn secret_acl_set_and_reset() {
        let u = || vec![id("u1"), id("u2")];
        let r = dry(OsAction::SecretAclSet {
            id: id("s1"),
            project_access: false,
            users: u(),
        })
        .await;
        assert_eq!(
            (r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()),
            ("key-manager", "PUT", "/v1/secrets/s1/acl")
        );
        assert_eq!(
            body(&r[0]),
            json!({"read": {"project-access": false, "users": ["u1", "u2"]}}),
            "PUT replaces the whole read ACL"
        );
        let r = dry(OsAction::SecretAclDelete { id: id("s1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/v1/secrets/s1/acl")
        );
        assert!(
            dry_err(OsAction::SecretAclSet {
                id: id("s1"),
                project_access: true,
                users: vec!["u1 u2".into()]
            })
            .await,
            "one ID per entry"
        );
        assert!(
            dry_err(OsAction::SecretAclSet {
                id: id("s1"),
                project_access: true,
                users: vec!["a/b".into()]
            })
            .await
        );
        assert_eq!(
            OsAction::SecretAclDelete { id: id("s1") }.plugins(),
            [Plugin::KeyManager]
        );
        let r = dry(OsAction::ContainerAclSet {
            id: id("c1"),
            project_access: true,
            users: vec![id("u1")],
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/v1/containers/c1/acl")
        );
        assert_eq!(body(&r[0]), json!({"read": {"project-access": true, "users": ["u1"]}}));
        let r = dry(OsAction::ContainerAclDelete { id: id("c1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/v1/containers/c1/acl")
        );
        assert!(
            dry_err(OsAction::ContainerAclSet {
                id: id("c1"),
                project_access: true,
                users: vec!["u1,u2".into()]
            })
            .await
        );
    }

    #[tokio::test]
    async fn deleting_secrets_and_containers() {
        let r = dry(OsAction::SecretDelete { id: id("s1") }).await;
        assert_eq!(
            (r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()),
            ("key-manager", "DELETE", "/v1/secrets/s1")
        );
        let r = dry(OsAction::ContainerDelete { id: id("c1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/v1/containers/c1")
        );
        assert_eq!(OsAction::SecretDelete { id: id("s1") }.plugins(), [Plugin::KeyManager]);
    }
}
