//! Barbican (roadmap 55): secrets, containers, ACLs, consumers.

use super::*;

/// A Barbican timestamp, `None` when empty or `null`.
fn km_when(v: &Value, k: &str) -> Option<String> {
    s_ne(v, k)
}

/// Secrets' metadata, sorted by name like barbican-ui.
pub fn secrets(raw: &[Value]) -> Vec<SecretRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|x| {
            let secret_ref = s(x, "secret_ref").unwrap_or_default();
            let id = ref_id(&secret_ref);
            SecretRow {
                name: s_ne(x, "name").unwrap_or_else(|| id.clone()),
                id,
                secret_type: s(x, "secret_type").unwrap_or_else(|| "opaque".into()),
                algorithm: s_ne(x, "algorithm"),
                bit_length: i(x, "bit_length"),
                mode: s_ne(x, "mode"),
                content_type: x.get("content_types").and_then(|c| s_ne(c, "default")),
                status: s(x, "status").unwrap_or_default(),
                expiration: km_when(x, "expiration"),
                created: s(x, "created").unwrap_or_default(),
                updated: km_when(x, "updated"),
                creator_id: s_ne(x, "creator_id"),
                secret_ref,
            }
        })
        .collect();
    out.sort_by_key(|x| x.name.to_lowercase());
    out
}

/// Secrets to put in a container: value `secret_ref` (Barbican takes refs),
/// label the name, one cell with the type.
pub fn secret_choices(raw: &[Value]) -> Vec<Choice> {
    secrets(raw)
        .into_iter()
        .map(|x| Choice {
            cells: vec![x.secret_type.clone()],
            ..Choice::new(x.secret_ref, format!("{} ({})", x.name, x.secret_type))
        })
        .collect()
}

/// Certificates a `TERMINATED_HTTPS` listener can use (roadmap 54), as
/// octavia-dashboard lists them: `certificate` containers holding both a
/// `certificate` and a `private_key`, then opaque secrets (PKCS#12 bundles) that
/// aren't part of one. The value is the Barbican URL Octavia takes; the label the
/// name and the certificate's expiry (`secrets`, the project's, give it); the
/// first cell that expiry, `""` when it never expires.
pub fn lb_certificate_choices(containers: &[Value], secrets: &[Value]) -> Vec<Choice> {
    let expiry: HashMap<String, Option<String>> = secrets
        .iter()
        .filter_map(|x| Some((s(x, "secret_ref")?, km_when(x, "expiration"))))
        .collect();
    let date = |e: &Option<String>| {
        e.as_deref()
            .map(|d| d.get(..10).unwrap_or(d).to_string())
            .unwrap_or_default()
    };
    let mut used = std::collections::HashSet::new();
    let mut out: Vec<Choice> = containers
        .iter()
        .filter(|c| s(c, "type").as_deref() == Some("certificate"))
        .filter_map(|c| {
            let refs: HashMap<String, String> = arr(c, "secret_refs")
                .iter()
                .filter_map(|r| Some((s(r, "name")?, s(r, "secret_ref")?)))
                .collect();
            let cert = refs.get("certificate")?;
            refs.get("private_key")?;
            used.extend(refs.values().cloned());
            let container_ref = s(c, "container_ref")?;
            let name = s_ne(c, "name").unwrap_or_else(|| ref_id(&container_ref));
            let when = date(expiry.get(cert).unwrap_or(&None));
            Some((name, when, container_ref))
        })
        .map(|(name, when, r)| Choice {
            cells: vec![when.clone()],
            ..Choice::new(
                r,
                if when.is_empty() {
                    name
                } else {
                    format!("{name} ({when})")
                },
            )
        })
        .collect();
    let mut bundles: Vec<Choice> = secrets
        .iter()
        .filter(|x| s(x, "secret_type").as_deref() == Some("opaque"))
        .filter_map(|x| {
            let r = s(x, "secret_ref")?;
            if used.contains(&r) {
                return None;
            }
            let name = s_ne(x, "name").unwrap_or_else(|| ref_id(&r));
            let when = date(&km_when(x, "expiration"));
            Some(Choice {
                cells: vec![when.clone()],
                ..Choice::new(
                    r,
                    if when.is_empty() {
                        name
                    } else {
                        format!("{name} ({when})")
                    },
                )
            })
        })
        .collect();
    out.sort_by_key(|c| c.label.to_lowercase());
    bundles.sort_by_key(|c| c.label.to_lowercase());
    out.extend(bundles);
    out
}

/// A secret's `read` ACL (`GET /v1/secrets/{id}/acl`): the users granted
/// access, and `project-access` (Barbican's default is `true`).
pub fn secret_acl(acl: &Value) -> (Vec<String>, Option<bool>) {
    let read = acl.get("read").unwrap_or(&Value::Null);
    let project = read
        .get("project-access")
        .and_then(Value::as_bool)
        .or(acl.is_object().then_some(true));
    (strs(read, "users"), project)
}

/// What uses a secret (roadmap 75): its consumers (`service resource_type
/// resource_id`), then each container holding it that has consumers
/// (`container NAME: consumer, …`). Octavia registers on the container, not on
/// its secrets, so the second part is what finds a load balancer.
pub fn secret_in_use(id: &str, consumers: &[SecretConsumer], containers: &[ContainerRow]) -> Vec<String> {
    let own = consumers
        .iter()
        .map(|c| format!("{} {} {}", c.service, c.resource_type, c.resource_id));
    let holders = containers
        .iter()
        .filter(|c| !c.consumers.is_empty() && c.secrets.iter().any(|x| x.id == id))
        .map(|c| {
            format!(
                "container {}: {}",
                c.name,
                c.consumers.iter().map(consumer_text).collect::<Vec<_>>().join(", ")
            )
        });
    own.chain(holders).collect()
}

/// The Barbican refs Octavia's listeners and pools point at, as `(ID, use)`:
/// listeners' `default_tls_container_ref`, `sni_container_refs`,
/// `client_ca_tls_container_ref`, `client_crl_container_ref`; pools'
/// `tls_container_ref`, `ca_tls_container_ref`, `crl_container_ref`. Load
/// balancers are named from `load_balancers` (the project's), else by ID.
/// Octavia registers no Barbican consumer, so this is how a load balancer using a
/// certificate is found (roadmap 75).
pub fn octavia_cert_refs(listeners: &[Value], pools: &[Value], load_balancers: &[Value]) -> Vec<(String, OctaviaUse)> {
    let names = names_by_id(load_balancers);
    let mut out = vec![];
    let mut add = |x: &Value, kind: &str, keys: &[&str], list: Option<&str>| {
        let lb_id = arr(x, "loadbalancers").first().and_then(|l| s(l, "id"));
        let lb_name = lb_id
            .as_ref()
            .map(|id| names.get(id).cloned().unwrap_or_else(|| id.clone()))
            .unwrap_or_else(|| "-".into());
        let one = keys.iter().filter_map(|k| x.get(*k)?.as_str());
        let many = list
            .and_then(|k| x.get(k)?.as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        for r in one.chain(many).filter(|r| !r.is_empty()) {
            let used = OctaviaUse {
                lb_id: lb_id.clone(),
                lb_name: lb_name.clone(),
                kind: kind.into(),
                name: name_or_id(x),
                via: None,
            };
            out.push((ref_id(r), used));
        }
    };
    for l in listeners {
        add(
            l,
            "listener",
            &[
                "default_tls_container_ref",
                "client_ca_tls_container_ref",
                "client_crl_container_ref",
            ],
            Some("sni_container_refs"),
        );
    }
    for p in pools {
        add(
            p,
            "pool",
            &["tls_container_ref", "ca_tls_container_ref", "crl_container_ref"],
            None,
        );
    }
    out
}

/// The uses of `id` among [`octavia_cert_refs`], each once; for a secret also
/// those through the `holders` (containers holding it), marked with `via`.
pub fn octavia_uses(refs: &[(String, OctaviaUse)], id: &str, holders: &[ContainerRow]) -> Vec<OctaviaUse> {
    let direct = refs.iter().filter(|(r, _)| r == id).map(|(_, u)| u.clone());
    let through = holders
        .iter()
        .filter(|c| c.secrets.iter().any(|x| x.id == id))
        .flat_map(|c| {
            refs.iter().filter(|(r, _)| *r == c.id).map(|(_, u)| OctaviaUse {
                via: Some((c.id.clone(), c.name.clone())),
                ..u.clone()
            })
        });
    let mut out: Vec<OctaviaUse> = vec![];
    for u in direct.chain(through) {
        if !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

/// What uses a container: its consumers.
pub fn container_in_use(c: &ContainerRow) -> Vec<String> {
    c.consumers.iter().map(consumer_text).collect()
}

/// A container consumer as `name url` (Octavia: `octavia lbaas://region/loadbalancer/ID`).
fn consumer_text(c: &ContainerConsumer) -> String {
    format!("{} {}", c.name, c.url).trim().to_string()
}

/// `GET /v1/secrets/{id}/consumers`.
pub fn secret_consumers(raw: &[Value]) -> Vec<SecretConsumer> {
    raw.iter()
        .map(|c| SecretConsumer {
            service: s(c, "service").unwrap_or_default(),
            resource_type: s(c, "resource_type").unwrap_or_default(),
            resource_id: s(c, "resource_id").unwrap_or_default(),
        })
        .collect()
}

/// Containers with their secrets named from `secrets` (the project's, from
/// `GET /v1/secrets`; an unknown secret keeps its ID).
pub fn containers(raw: &[Value], secrets: &[Value]) -> Vec<ContainerRow> {
    let names: HashMap<String, String> = self::secrets(secrets).into_iter().map(|x| (x.id, x.name)).collect();
    let mut out: Vec<_> = raw
        .iter()
        .map(|c| {
            let container_ref = s(c, "container_ref").unwrap_or_default();
            let id = ref_id(&container_ref);
            let mut members: Vec<_> = arr(c, "secret_refs")
                .iter()
                .map(|r| {
                    let id = ref_id(&s(r, "secret_ref").unwrap_or_default());
                    ContainerSecret {
                        role: s_ne(r, "name").unwrap_or_else(|| "-".into()),
                        name: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
                        id,
                    }
                })
                .collect();
            members.sort_by(|a, b| (&a.role, &a.name).cmp(&(&b.role, &b.name)));
            ContainerRow {
                name: s_ne(c, "name").unwrap_or_else(|| id.clone()),
                id,
                container_type: s(c, "type").unwrap_or_else(|| "generic".into()),
                status: s(c, "status").unwrap_or_default(),
                created: s(c, "created").unwrap_or_default(),
                updated: km_when(c, "updated"),
                creator_id: s_ne(c, "creator_id"),
                container_ref,
                secrets: members,
                consumers: arr(c, "consumers")
                    .iter()
                    .map(|k| ContainerConsumer {
                        name: s(k, "name").unwrap_or_default(),
                        url: s(k, "URL").unwrap_or_default(),
                    })
                    .collect(),
            }
        })
        .collect();
    out.sort_by_key(|c| c.name.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Roadmap 54, slice 3: like octavia-dashboard, certificate containers with
    /// a private key, then opaque secrets not in one, with the certificate's expiry.
    #[test]
    fn lb_certificates_are_complete_containers_and_loose_bundles() {
        let b = "https://kms/v1";
        let containers = [
            json!({"container_ref": format!("{b}/containers/c1"), "name": "web", "type": "certificate",
                   "secret_refs": [{"name": "certificate", "secret_ref": format!("{b}/secrets/crt")}, {"name": "private_key", "secret_ref": format!("{b}/secrets/key")}]}),
            json!({"container_ref": format!("{b}/containers/c2"), "name": "no key", "type": "certificate",
                   "secret_refs": [{"name": "certificate", "secret_ref": format!("{b}/secrets/crt2")}]}),
            json!({"container_ref": format!("{b}/containers/c3"), "name": "generic", "type": "generic", "secret_refs": []}),
        ];
        let secrets = [
            json!({"secret_ref": format!("{b}/secrets/crt"), "name": "crt", "secret_type": "certificate", "expiration": "2027-03-01T00:00:00"}),
            json!({"secret_ref": format!("{b}/secrets/key"), "name": "key", "secret_type": "opaque"}),
            json!({"secret_ref": format!("{b}/secrets/p12"), "name": "bundle", "secret_type": "opaque", "expiration": null}),
            json!({"secret_ref": format!("{b}/secrets/sym"), "name": "aes", "secret_type": "symmetric"}),
        ];
        let c = lb_certificate_choices(&containers, &secrets);
        let got: Vec<(&str, &str, &str)> = c
            .iter()
            .map(|x| (x.value.as_str(), x.label.as_str(), x.cells[0].as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (format!("{b}/containers/c1").as_str(), "web (2027-03-01)", "2027-03-01"),
                (format!("{b}/secrets/p12").as_str(), "bundle", "")
            ]
        );
    }

    #[test]
    fn key_manager_quotas_take_usage_from_list_totals() {
        let q = json!({"quotas": {"secrets": 10, "orders": -1, "containers": 5, "consumers": -1, "cas": 2}});
        let secrets = json!({"secrets": [{}], "total": 3});
        let r = key_manager_quota_rows(
            &q,
            &[
                ("secrets", &secrets),
                ("containers", &Value::Null),
                ("orders", &json!({"orders": [], "total": 0})),
            ],
        );
        let got: Vec<_> = r.iter().map(|x| (x.key.as_str(), x.used, x.limit)).collect();
        assert_eq!(
            got,
            [
                ("secrets", Some(3), Some(10)),
                ("containers", None, Some(5)),
                ("orders", Some(0), None),
                ("consumers", None, None),
                ("cas", None, Some(2))
            ],
            "editable order; a failed list leaves usage unknown"
        );
    }

    #[test]
    fn secrets_map_barbican_metadata() {
        let raw = [
            json!({"secret_ref": "https://kms/v1/secrets/b2", "name": "web-cert", "secret_type": "certificate", "algorithm": null,
                   "bit_length": null, "mode": null, "content_types": {"default": "text/plain"}, "status": "ACTIVE",
                   "expiration": "2026-12-31T00:00:00", "created": "2026-10-01T10:00:00", "updated": "2026-10-01T10:00:00", "creator_id": "u1"}),
            json!({"secret_ref": "https://kms/v1/secrets/a1", "name": null, "secret_type": "symmetric", "algorithm": "aes",
                   "bit_length": 256, "mode": "cbc", "status": "ACTIVE", "expiration": null, "created": "2026-09-01T10:00:00"}),
        ];
        let out = secrets(&raw);
        assert_eq!(out[0].name, "a1", "unnamed: the ID from the ref, sorted first");
        assert_eq!(
            (out[0].algorithm.as_deref(), out[0].bit_length, out[0].mode.as_deref()),
            (Some("aes"), Some(256), Some("cbc"))
        );
        assert_eq!(out[0].content_type, None, "no payload yet: no content types");
        assert_eq!(out[0].expiration, None);
        assert_eq!(out[1].id, "b2");
        assert_eq!(out[1].content_type.as_deref(), Some("text/plain"));
        assert_eq!(out[1].expiration.as_deref(), Some("2026-12-31T00:00:00"));
        assert_eq!(out[1].secret_ref, "https://kms/v1/secrets/b2");
        let c = secret_choices(&raw);
        assert_eq!(
            (c[1].value.as_str(), c[1].label.as_str()),
            ("https://kms/v1/secrets/b2", "web-cert (certificate)"),
            "containers take refs"
        );
    }

    #[test]
    fn in_use_finds_consumers_and_the_containers_holding_a_secret() {
        let container = |name: &str, secret: &str, consumers: &[(&str, &str)]| ContainerRow {
            id: name.into(),
            name: name.into(),
            container_type: "certificate".into(),
            status: "ACTIVE".into(),
            created: String::new(),
            updated: None,
            creator_id: None,
            container_ref: String::new(),
            secrets: vec![ContainerSecret {
                role: "certificate".into(),
                id: secret.into(),
                name: secret.into(),
            }],
            consumers: consumers
                .iter()
                .map(|(n, u)| ContainerConsumer {
                    name: n.to_string(),
                    url: u.to_string(),
                })
                .collect(),
        };
        let cs = [
            container("web", "s1", &[("octavia", "lbaas://es/loadbalancer/lb1")]),
            container("idle", "s1", &[]),
            container("other", "s2", &[("octavia", "lbaas://es/loadbalancer/lb2")]),
        ];
        let own = [SecretConsumer {
            service: "image".into(),
            resource_type: "image".into(),
            resource_id: "i1".into(),
        }];
        assert_eq!(
            secret_in_use("s1", &own, &cs),
            ["image image i1", "container web: octavia lbaas://es/loadbalancer/lb1"]
        );
        assert!(secret_in_use("s3", &[], &cs).is_empty());
        assert_eq!(container_in_use(&cs[0]), ["octavia lbaas://es/loadbalancer/lb1"]);
    }

    #[test]
    fn octavia_listeners_and_pools_name_the_refs_they_use() {
        let kms = |kind: &str, id: &str| format!("https://kms/v1/{kind}/{id}");
        let listeners = [
            json!({"id": "l1", "name": "https", "default_tls_container_ref": kms("containers", "c1"), "sni_container_refs": [kms("secrets", "p12")],
                   "client_ca_tls_container_ref": null}),
            json!({"id": "l2", "name": "", "default_tls_container_ref": null, "sni_container_refs": []}),
        ];
        let pools = [
            json!({"id": "p1", "name": "backend", "tls_container_ref": kms("containers", "c2"), "ca_tls_container_ref": "",
                            "loadbalancers": [{"id": "lb2"}]}),
        ];
        let lbs = [json!({"id": "lb2", "name": "web"})];
        let refs = octavia_cert_refs(&listeners, &pools, &lbs);
        let pairs: Vec<_> = refs
            .iter()
            .map(|(i, u)| (i.as_str(), u.kind.as_str(), u.name.as_str(), u.lb_name.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("c1", "listener", "https", "-"),
                ("p12", "listener", "https", "-"),
                ("c2", "pool", "backend", "web")
            ]
        );
        let l = [
            json!({"id": "l3", "name": "", "default_tls_container_ref": kms("containers", "c3"), "loadbalancers": [{"id": "lb9"}]}),
        ];
        let r = octavia_cert_refs(&l, &[], &[]);
        assert_eq!(
            (r[0].1.lb_id.as_deref(), r[0].1.lb_name.as_str(), r[0].1.name.as_str()),
            (Some("lb9"), "lb9", "l3"),
            "unnamed: IDs"
        );
        // A secret is used directly (a PKCS12 SNI ref) and through container c2.
        let holder = ContainerRow {
            id: "c2".into(),
            name: "web-tls".into(),
            container_type: "certificate".into(),
            status: "ACTIVE".into(),
            created: String::new(),
            updated: None,
            creator_id: None,
            container_ref: String::new(),
            secrets: vec![ContainerSecret {
                role: "certificate".into(),
                id: "p12".into(),
                name: "cert".into(),
            }],
            consumers: vec![],
        };
        let uses = octavia_uses(&refs, "p12", std::slice::from_ref(&holder));
        assert_eq!(uses.len(), 2);
        assert_eq!(
            (uses[0].via.clone(), uses[1].via.clone()),
            (None, Some(("c2".to_string(), "web-tls".to_string())))
        );
        assert!(octavia_uses(&refs, "nothing", &[holder]).is_empty());
    }

    #[test]
    fn secret_acl_defaults_to_project_access() {
        assert_eq!(
            secret_acl(&json!({"read": {"project-access": false, "users": ["u1", "u2"]}})),
            (vec!["u1".to_string(), "u2".to_string()], Some(false))
        );
        assert_eq!(
            secret_acl(&json!({"read": {}})),
            (vec![], Some(true)),
            "no ACL set: Barbican's default"
        );
        assert_eq!(secret_acl(&Value::Null), (vec![], None), "ACL unreadable");
    }

    #[test]
    fn containers_name_their_secrets_by_role() {
        let secrets_raw = [json!({"secret_ref": "https://kms/v1/secrets/s1", "name": "web-cert"})];
        let raw = [json!({
            "container_ref": "https://kms/v1/containers/c1", "name": "", "type": "certificate", "status": "ACTIVE",
            "created": "2026-10-01T10:00:00", "updated": null,
            "secret_refs": [
                {"name": "private_key", "secret_ref": "https://kms/v1/secrets/s2"},
                {"name": "certificate", "secret_ref": "https://kms/v1/secrets/s1"}
            ],
            "consumers": [{"name": "octavia", "URL": "https://lb/v2/lbaas/listeners/l1"}]
        })];
        let c = &containers(&raw, &secrets_raw)[0];
        assert_eq!(
            (c.id.as_str(), c.name.as_str(), c.container_type.as_str()),
            ("c1", "c1", "certificate"),
            "empty name: the ID"
        );
        assert_eq!(c.updated, None);
        let roles: Vec<_> = c.secrets.iter().map(|x| (x.role.as_str(), x.name.as_str())).collect();
        assert_eq!(
            roles,
            [("certificate", "web-cert"), ("private_key", "s2")],
            "by role; an unknown secret keeps its ID"
        );
        assert_eq!(
            c.consumers,
            [ContainerConsumer {
                name: "octavia".into(),
                url: "https://lb/v2/lbaas/listeners/l1".into()
            }]
        );
    }
}
