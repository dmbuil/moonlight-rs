//! Project › Quotas.

use super::*;

/// A service's card title on Project › Quotas.
pub fn quota_service_label(s: QuotaService) -> K {
    match s {
        QuotaService::Compute => K::Compute,
        QuotaService::Volume => K::VolumesGroup,
        QuotaService::Network => K::NetworkGroup,
        QuotaService::LoadBalancer => K::LoadBalancers,
        QuotaService::KeyManager => K::KeyManager,
    }
}

/// A quota's label (Horizon's *Modify Quotas* wording). Cinder's per-type keys
/// read "Volumes (ceph)"; a key Moonlight doesn't know is shown as the API names it.
pub fn quota_label(lang: Lang, s: QuotaService, key: &str) -> String {
    use QuotaService::*;
    let k = match (s, key) {
        (Compute, "instances") => K::Instances,
        (Compute, "cores") => K::Vcpus,
        (Compute, "ram") => K::Ram,
        (Compute, "key_pairs") => K::KeyPairs,
        (Compute, "server_groups") => K::ServerGroups,
        (Compute, "server_group_members") => K::QServerGroupMembers,
        (Compute, "metadata_items") => K::QMetadataItems,
        (Compute, "injected_files") => K::QInjectedFiles,
        (Compute, "injected_file_content_bytes") => K::QInjectedFileContent,
        (Compute, "injected_file_path_bytes") => K::QInjectedFilePath,
        (Volume, "volumes") => K::Volumes,
        (Volume, "snapshots") => K::VolumeSnapshots,
        (Volume, "gigabytes") => K::QGigabytes,
        (Volume, "backups") => K::QBackups,
        (Volume, "backup_gigabytes") => K::QBackupGigabytes,
        (Volume, "groups") => K::QVolumeGroups,
        (Volume, "per_volume_gigabytes") => K::QPerVolumeGigabytes,
        (Network, "network") => K::Networks,
        (Network, "subnet") => K::QSubnets,
        (Network, "subnetpool") => K::QSubnetPools,
        (Network, "port") => K::Ports,
        (Network, "router") => K::Routers,
        (Network, "floatingip") => K::FloatingIps,
        (Network, "security_group") => K::SecurityGroups,
        (Network, "security_group_rule") => K::SecurityGroupRules,
        (Network, "rbac_policy") => K::QRbacPolicies,
        (Network, "trunk") => K::Trunks,
        // neutron-fwaas v2: read-only rows, not in `keys()`.
        (Network, "firewall_group") => K::FirewallGroups,
        (Network, "firewall_policy") => K::FirewallPolicies,
        (Network, "firewall_rule") => K::FirewallRules,
        (LoadBalancer, "load_balancer") => K::LoadBalancers,
        (LoadBalancer, "listener") => K::Listeners,
        (LoadBalancer, "pool") => K::Pools,
        (LoadBalancer, "member") => K::Members,
        (LoadBalancer, "health_monitor") => K::HealthMonitors,
        (LoadBalancer, "l7policy") => K::L7Policies,
        (LoadBalancer, "l7rule") => K::L7Rules,
        (KeyManager, "secrets") => K::Secrets,
        (KeyManager, "containers") => K::Containers,
        (KeyManager, "orders") => K::QOrders,
        (KeyManager, "consumers") => K::Consumers,
        (KeyManager, "cas") => K::QCertificateAuthorities,
        (Volume, _) => {
            let per_type = [
                ("volumes_", K::Volumes),
                ("gigabytes_", K::QGigabytes),
                ("snapshots_", K::VolumeSnapshots),
            ];
            return match per_type.iter().find_map(|(p, k)| key.strip_prefix(p).map(|ty| (k, ty))) {
                Some((k, ty)) => format!("{} ({ty})", t(lang, *k)),
                None => key.to_string(),
            };
        }
        _ => return key.to_string(),
    };
    t(lang, k).to_string()
}

/// *Edit Quotas* of one service: a number per editable quota the service
/// reported, sending only what changed. Shown as `[ui] quota_edit` says (to
/// everyone by default): whether the user may is then the cloud's
/// policy, unknown beforehand, so a 403 gets its own message instead.
pub fn edit_quotas_dialog(lang: Lang, card: &QuotaCard) -> Dialog {
    let project_id = card.project_id.clone();
    let title = format!(
        "{}: {}",
        t(lang, K::EditQuotas),
        t(lang, quota_service_label(card.service))
    );
    quotas_form(lang, card, title, move |service, values| OsAction::QuotaUpdate {
        service,
        project_id: project_id.clone(),
        values,
    })
    .forbidden(t(lang, K::QuotaForbidden))
}

/// Admin › Defaults' *Update Defaults*: the same form over the
/// `default` quota class of Compute or Volume.
pub fn update_defaults_dialog(lang: Lang, card: &QuotaCard) -> Dialog {
    let title = format!(
        "{}: {}",
        t(lang, K::UpdateDefaults),
        t(lang, quota_service_label(card.service))
    );
    let mut d = quotas_form(lang, card, title, |service, values| OsAction::DefaultQuotaUpdate {
        service,
        values,
    });
    // Defaults have no usage: Project's "can't be below what is used" doesn't apply.
    d.intro = vec![t(lang, K::DefaultQuotaHelp).into()];
    d
}

/// One number per limit `card` shows (`-1` unlimited); `make` gets the changed
/// ones only, and an unchanged form is refused.
fn quotas_form(
    lang: Lang,
    card: &QuotaCard,
    title: String,
    make: impl Fn(QuotaService, std::collections::BTreeMap<String, i64>) -> OsAction + Send + Sync + 'static,
) -> Dialog {
    let service = card.service;
    let editable: Vec<&'static str> = service
        .keys()
        .iter()
        .copied()
        .filter(|k| card.rows.iter().any(|r| r.key == *k))
        .collect();
    let fields = editable
        .iter()
        .map(|k| {
            let row = card.rows.iter().find(|r| r.key == *k).expect("filtered above");
            let mut label = quota_label(lang, service, k);
            if service.unit(k) != QuotaUnit::Count {
                label = format!(
                    "{label} ({})",
                    if service.unit(k) == QuotaUnit::Mib {
                        "MiB"
                    } else {
                        "GiB"
                    }
                );
            }
            Field::number(k, label, -1).value(row.limit.unwrap_or(-1).to_string())
        })
        .collect();
    let rows = card.rows.clone();
    let mut d = Dialog::form(title, t(lang, K::Save), fields, move |v| {
        let mut wanted = Vec::new();
        for k in &editable {
            if let Some(n) = v.num(k).map_err(|_| format!("{}: ?", quota_label(lang, service, k)))? {
                wanted.push((k.to_string(), n));
            }
        }
        let values = quota_changes(&rows, &wanted).map_err(|e| {
            // `key: …` → the label the user saw.
            let (k, rest) = e.split_once(':').unwrap_or(("", &e));
            format!("{}:{rest}", quota_label(lang, service, k))
        })?;
        if values.is_empty() {
            return Err(t(lang, K::NothingChanged).into());
        }
        Ok(make(service, values))
    });
    d.intro = vec![t(lang, K::QuotaLimitHelp).into()];
    d
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn update_defaults_sends_the_changes_to_the_default_class() {
        let row = |key: &str, limit| QuotaRow {
            key: key.into(),
            used: None,
            reserved: None,
            limit,
        };
        let card = QuotaCard {
            service: QuotaService::Volume,
            project_id: "p1".into(),
            rows: vec![row("volumes", Some(10)), row("gigabytes", Some(1000))],
            error: None,
        };
        let d = update_defaults_dialog(Lang::EnUs, &card);
        assert!(
            !d.intro.concat().contains("used"),
            "defaults have no usage to stay above"
        );
        assert!(d.title.starts_with("Update Defaults"), "{}", d.title);
        let a = (d.build)(&vals(&[("volumes", "10"), ("gigabytes", "-1")])).unwrap();
        assert_eq!(
            a,
            OsAction::DefaultQuotaUpdate {
                service: QuotaService::Volume,
                values: [("gigabytes".to_string(), -1)].into()
            },
            "only what changed"
        );
        assert!(
            (d.build)(&vals(&[("volumes", "10"), ("gigabytes", "1000")])).is_err(),
            "nothing changed"
        );
    }

    #[test]
    fn edit_quotas_sends_only_changes_and_names_the_quota() {
        let row = |key: &str, used, limit| QuotaRow {
            key: key.into(),
            used,
            reserved: None,
            limit,
        };
        let card = QuotaCard {
            service: QuotaService::Compute,
            project_id: "p1".into(),
            rows: vec![
                row("instances", Some(4), Some(10)),
                row("ram", Some(2048), None),
                row("unknown_key", None, Some(1)),
            ],
            error: None,
        };
        let d = edit_quotas_dialog(Lang::EnUs, &card);
        let names: Vec<_> = d.fields.iter().map(|f| (f.name, f.default.as_str())).collect();
        assert_eq!(
            names,
            [("instances", "10"), ("ram", "-1")],
            "editable keys only; unlimited shown as -1"
        );
        assert_eq!(d.fields[1].label, "RAM (MiB)");
        assert!(
            d.forbidden
                .as_deref()
                .is_some_and(|m| m.contains("no permission to edit quotas"))
        );
        let a = (d.build)(&vals(&[("instances", "12"), ("ram", "-1")])).unwrap();
        let OsAction::QuotaUpdate {
            service,
            project_id,
            values,
        } = a
        else {
            panic!("{a:?}")
        };
        assert_eq!(
            (service, project_id.as_str(), values.into_iter().collect::<Vec<_>>()),
            (QuotaService::Compute, "p1", vec![("instances".to_string(), 12)])
        );
        assert_eq!(
            (d.build)(&vals(&[("instances", "10")])).unwrap_err(),
            "Nothing changed."
        );
        assert!(
            (d.build)(&vals(&[("instances", "3")]))
                .unwrap_err()
                .starts_with("Instances: 3 is below")
        );
    }

    #[test]
    fn quota_labels_cover_per_type_and_unknown_keys() {
        assert_eq!(
            quota_label(Lang::EnUs, QuotaService::Volume, "gigabytes_ceph"),
            "Total Size of Volumes and Snapshots (ceph)"
        );
        assert_eq!(
            quota_label(Lang::EsEs, QuotaService::Volume, "volumes_ssd"),
            "Volúmenes (ssd)"
        );
        assert_eq!(quota_label(Lang::EnUs, QuotaService::Network, "new_thing"), "new_thing");
        assert_eq!(
            quota_label(Lang::EnUs, QuotaService::Network, "firewall_group"),
            "Firewall Groups"
        );
        assert_eq!(
            quota_label(Lang::EsEs, QuotaService::Network, "firewall_policy"),
            "Políticas de cortafuegos"
        );
        assert_eq!(
            quota_label(Lang::EnUs, QuotaService::Network, "firewall_rule"),
            "Firewall Rules"
        );
        for s in [
            QuotaService::Compute,
            QuotaService::Volume,
            QuotaService::Network,
            QuotaService::LoadBalancer,
            QuotaService::KeyManager,
        ] {
            for k in s.keys() {
                assert_ne!(quota_label(Lang::EnUs, s, k), *k, "{s:?} {k} has no label");
            }
        }
    }
}
