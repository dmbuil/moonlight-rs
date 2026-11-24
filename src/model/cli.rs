//! The `openstack` CLI commands that do what an action does: the *CLI equivalent* of a result
//! dialog (`[ui] show_cli`). Covers the project's compute, network, volume, image and security
//! actions (creations, the instance lifecycle, attachments, floating IPs, deletions alone or as a
//! batch); the Admin, Octavia, Heat and other actions yield nothing. Secrets never appear: the admin password is a placeholder and the
//! customization script a file name.

use super::*;

/// A shell word: plain when it has only safe characters, else in single quotes.
fn quote(s: &str) -> String {
    let plain = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// One command: its words, grouped so that each option sits on its own line.
struct Cmd {
    head: String,
    groups: Vec<String>,
}

impl Cmd {
    fn new(head: &str) -> Self {
        Cmd {
            head: head.to_string(),
            groups: vec![],
        }
    }
    /// `--flag`.
    fn flag(&mut self, flag: &str) -> &mut Self {
        self.groups.push(flag.to_string());
        self
    }
    /// `--flag value`, quoted as needed.
    fn opt(&mut self, flag: &str, value: &str) -> &mut Self {
        self.groups.push(format!("{flag} {}", quote(value)));
        self
    }
    /// `--flag "$VAR"`: an ID a previous command of the chain captured.
    fn var(&mut self, flag: &str, var: &str) -> &mut Self {
        self.groups.push(format!("{flag} \"${var}\""));
        self
    }
    /// A word as it is (a quoted variable).
    fn raw(&mut self, word: &str) -> &mut Self {
        self.groups.push(word.to_string());
        self
    }
    /// A positional argument.
    fn arg(&mut self, value: &str) -> &mut Self {
        self.groups.push(quote(value));
        self
    }
    /// The command on several lines, `\` continued; `NAME=$(… -f value -c id)` when `capture`.
    fn render(&self, capture: Option<&str>) -> String {
        let mut lines = vec![self.head.clone()];
        lines.extend(self.groups.iter().cloned());
        let body = lines.join(" \\\n  ");
        match capture {
            Some(var) => format!("{var}=$({body} \\\n  -f value -c id)"),
            None => body,
        }
    }
}

/// The commands that do `action`, in order; empty for an action without an equivalent.
pub fn cli_equivalent(action: &OsAction) -> Vec<String> {
    match action {
        OsAction::ServerLaunch(s) => vec![server_create(s).render(None)],
        OsAction::NetworkCreate { name, admin_up, subnet } => {
            let mut net = Cmd::new("openstack network create");
            if !admin_up {
                net.flag("--disable");
            }
            net.arg(name.trim());
            network_chain(net, subnet.as_ref())
        }
        OsAction::AdminNetworkCreate(spec) => {
            let mut net = Cmd::new("openstack network create");
            net.opt("--project", &spec.project_id);
            if !spec.admin_up {
                net.flag("--disable");
            }
            if spec.shared {
                net.flag("--share");
            }
            if spec.external {
                net.flag("--external");
            }
            if let Some(p) = &spec.provider {
                net.opt("--provider-network-type", &p.network_type);
                if let Some(phys) = &p.physical_network {
                    net.opt("--provider-physical-network", phys.trim());
                }
                if let Some(seg) = p.segmentation_id {
                    net.opt("--provider-segment", &seg.to_string());
                }
            }
            for z in &spec.az_hints {
                net.opt("--availability-zone-hint", z);
            }
            net.arg(spec.name.trim());
            network_chain(net, spec.subnet.as_ref())
        }
        OsAction::RouterCreate {
            name,
            admin_up,
            external_network_id,
            qos_policy_id,
        } => {
            let mut r = Cmd::new("openstack router create");
            if !admin_up {
                r.flag("--disable");
            }
            if let Some(net) = external_network_id.as_deref().filter(|n| !n.trim().is_empty()) {
                r.opt("--external-gateway", net.trim());
            }
            r.arg(name.trim());
            router_chain(r, qos_policy_id.as_deref())
        }
        OsAction::AdminRouterCreate(spec) => {
            let mut r = Cmd::new("openstack router create");
            r.opt("--project", &spec.project_id);
            if !spec.admin_up {
                r.flag("--disable");
            }
            if let Some(net) = spec.external_network_id.as_deref().filter(|n| !n.trim().is_empty()) {
                r.opt("--external-gateway", net.trim());
                match spec.enable_snat {
                    Some(true) => {
                        r.flag("--enable-snat");
                    }
                    Some(false) => {
                        r.flag("--disable-snat");
                    }
                    None => {}
                }
            }
            for z in &spec.az_hints {
                r.opt("--availability-zone-hint", z);
            }
            match spec.distributed {
                Some(true) => {
                    r.flag("--distributed");
                }
                Some(false) => {
                    r.flag("--centralized");
                }
                None => {}
            }
            match spec.ha {
                Some(true) => {
                    r.flag("--ha");
                }
                Some(false) => {
                    r.flag("--no-ha");
                }
                None => {}
            }
            r.arg(spec.name.trim());
            router_chain(r, None)
        }
        OsAction::VolumeCreate {
            name,
            description,
            size_gb,
            volume_type,
            availability_zone,
            source,
            source_id,
        } => {
            let mut c = Cmd::new("openstack volume create");
            c.opt("--size", &size_gb.to_string());
            let src = source_id.as_deref().map(str::trim).filter(|s| !s.is_empty());
            match (source, src) {
                (VolumeSource::Image, Some(i)) => c.opt("--image", i),
                (VolumeSource::Snapshot, Some(i)) => c.opt("--snapshot", i),
                (VolumeSource::Volume, Some(i)) => c.opt("--source", i),
                _ => &mut c,
            };
            if let Some(t) = volume_type.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                c.opt("--type", t);
            }
            if let Some(z) = availability_zone.as_deref().map(str::trim).filter(|z| !z.is_empty()) {
                c.opt("--availability-zone", z);
            }
            if let Some(d) = description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
                c.opt("--description", d);
            }
            // The name is optional in the CLI; an unnamed volume has none.
            if !name.trim().is_empty() {
                c.arg(name.trim());
            }
            vec![c.render(None)]
        }
        // A chosen IP: the port is made first, as the dialog does, then added by its ID.
        OsAction::RouterAddInterface {
            id,
            subnet_id,
            ip_address: Some(ip),
            network_id: Some(net),
        } if !ip.trim().is_empty() && !net.trim().is_empty() => {
            let mut port = Cmd::new("openstack port create");
            port.opt("--network", net.trim())
                .opt("--fixed-ip", &format!("subnet={subnet_id},ip-address={}", ip.trim()))
                .arg("");
            let mut add = Cmd::new("openstack router add port");
            add.arg(id).raw("\"$PORT_ID\"");
            vec![port.render(Some("PORT_ID")), add.render(None)]
        }
        _ if delete_id(action).is_some() => delete_command(std::slice::from_ref(action)).into_iter().collect(),
        OsAction::Batch(items) => {
            let actions: Vec<OsAction> = items.iter().map(|(_, a)| a.clone()).collect();
            // A batch of deletions of one kind is one command with every ID.
            if let Some(c) = delete_command(&actions) {
                return vec![c];
            }
            actions.iter().flat_map(cli_equivalent).collect()
        }
        _ => match octavia_commands(action) {
            Some(cmds) => cmds,
            None => simple_command(action).into_iter().map(|c| c.render(None)).collect(),
        },
    }
}

/// `openstack <kind> delete <id>…` when every action deletes the same kind of
/// resource (the CLI takes several IDs); `None` otherwise, or for no action.
fn delete_id(a: &OsAction) -> Option<(&'static str, &str)> {
    Some(match a {
        OsAction::ServerDelete { id } => ("server", id),
        OsAction::NetworkDelete { id } => ("network", id),
        OsAction::RouterDelete { id } => ("router", id),
        OsAction::VolumeDelete { id } => ("volume", id),
        OsAction::ImageDelete { id } => ("image", id),
        OsAction::SubnetDelete { id } => ("subnet", id),
        OsAction::PortDelete { id } => ("port", id),
        OsAction::SnapshotDelete { id } => ("volume snapshot", id),
        OsAction::SecurityGroupDelete { id } => ("security group", id),
        OsAction::RuleDelete { id } => ("security group rule", id),
        OsAction::ServerGroupDelete { id } => ("server group", id),
        OsAction::KeyPairDelete { name } => ("keypair", name),
        OsAction::FloatingIpRelease { id } => ("floating ip", id),
        OsAction::ListenerDelete { id } => ("loadbalancer listener", id),
        OsAction::PoolDelete { id } => ("loadbalancer pool", id),
        OsAction::MonitorDelete { id } => ("loadbalancer healthmonitor", id),
        OsAction::L7PolicyDelete { id } => ("loadbalancer l7policy", id),
        _ => return None,
    })
}

fn delete_command(actions: &[OsAction]) -> Option<String> {
    let (kind, _) = delete_id(actions.first()?)?;
    let mut c = Cmd::new(&format!("openstack {kind} delete"));
    for a in actions {
        match delete_id(a) {
            Some((k, id)) if k == kind => c.arg(id),
            _ => return None,
        };
    }
    Some(c.render(None))
}

/// `openstack server <verb> <id>`: the lifecycle actions that take no options.
fn server_verb(verb: &str, id: &str) -> Cmd {
    let mut c = Cmd::new(&format!("openstack server {verb}"));
    c.arg(id);
    c
}

/// The one command of an action that is a single, small call (everything but creations of
/// several resources, deletions and batches, which have their own code).
fn simple_command(action: &OsAction) -> Option<Cmd> {
    let non_empty = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(match action {
        OsAction::ServerUpdate { id, name } => {
            let mut c = Cmd::new("openstack server set");
            c.opt("--name", name.trim()).arg(id);
            c
        }
        OsAction::ServerStart { id } => server_verb("start", id),
        OsAction::ServerStop { id } => server_verb("stop", id),
        OsAction::ServerReboot { id, kind } => {
            let mut c = Cmd::new("openstack server reboot");
            c.flag(match kind {
                RebootKind::Soft => "--soft",
                RebootKind::Hard => "--hard",
            })
            .arg(id);
            c
        }
        OsAction::ServerPause { id } => server_verb("pause", id),
        OsAction::ServerUnpause { id } => server_verb("unpause", id),
        OsAction::ServerSuspend { id } => server_verb("suspend", id),
        OsAction::ServerResume { id } => server_verb("resume", id),
        OsAction::ServerLock { id } => server_verb("lock", id),
        OsAction::ServerUnlock { id } => server_verb("unlock", id),
        OsAction::ServerShelve { id } => server_verb("shelve", id),
        OsAction::ServerUnshelve { id, availability_zone } => {
            let mut c = Cmd::new("openstack server unshelve");
            if let Some(z) = non_empty(availability_zone) {
                c.opt("--availability-zone", &z);
            }
            c.arg(id);
            c
        }
        OsAction::ServerRescue { id, image_id } => {
            let mut c = Cmd::new("openstack server rescue");
            if let Some(i) = non_empty(image_id) {
                c.opt("--image", &i);
            }
            c.arg(id);
            c
        }
        OsAction::ServerUnrescue { id } => server_verb("unrescue", id),
        OsAction::ServerSnapshot { id, name } => {
            let mut c = Cmd::new("openstack server image create");
            c.opt("--name", name.trim()).arg(id);
            c
        }
        OsAction::ServerResize { id, flavor_id } => {
            let mut c = Cmd::new("openstack server resize");
            c.opt("--flavor", flavor_id).arg(id);
            c
        }
        OsAction::ServerConfirmResize { id } => {
            let mut c = Cmd::new("openstack server resize confirm");
            c.arg(id);
            c
        }
        OsAction::ServerRevertResize { id } => {
            let mut c = Cmd::new("openstack server resize revert");
            c.arg(id);
            c
        }
        OsAction::ServerRebuild { id, image_id } => {
            let mut c = Cmd::new("openstack server rebuild");
            c.opt("--image", image_id).arg(id);
            c
        }
        OsAction::ServerAttachVolume { server_id, volume_id } => {
            let mut c = Cmd::new("openstack server add volume");
            c.arg(server_id).arg(volume_id);
            c
        }
        OsAction::ServerDetachVolume { server_id, volume_id } => {
            let mut c = Cmd::new("openstack server remove volume");
            c.arg(server_id).arg(volume_id);
            c
        }
        OsAction::KeyPairGenerate { name } => {
            // The private key goes to a file instead of the terminal.
            let mut c = Cmd::new("openstack keypair create");
            c.opt("--private-key", &format!("./{}.pem", name.trim()))
                .arg(name.trim());
            c
        }
        OsAction::KeyPairImport { name, key_type, .. } => {
            // The public key is the text typed in the dialog: it goes in a file.
            let mut c = Cmd::new("openstack keypair create");
            c.opt("--public-key", &format!("./{}.pub", name.trim()));
            if !key_type.trim().is_empty() {
                c.opt("--type", key_type.trim());
            }
            c.arg(name.trim());
            c
        }
        OsAction::ServerGroupCreate { name, policy } => {
            let mut c = Cmd::new("openstack server group create");
            c.opt("--policy", policy).arg(name.trim());
            c
        }
        OsAction::VolumeExtend { id, new_size_gb } => {
            let mut c = Cmd::new("openstack volume set");
            c.opt("--size", &new_size_gb.to_string()).arg(id);
            c
        }
        OsAction::VolumeSnapshotCreate {
            volume_id,
            name,
            description,
            force,
        } => {
            let mut c = Cmd::new("openstack volume snapshot create");
            c.opt("--volume", volume_id);
            if let Some(d) = non_empty(description) {
                c.opt("--description", &d);
            }
            if *force {
                c.flag("--force");
            }
            c.arg(name.trim());
            c
        }
        OsAction::SecurityGroupCreate { name, description } => {
            let mut c = Cmd::new("openstack security group create");
            if let Some(d) = non_empty(description) {
                c.opt("--description", &d);
            }
            c.arg(name.trim());
            c
        }
        OsAction::RuleCreate(r) => rule_create(r),
        OsAction::FloatingIpAllocate {
            network_id,
            description,
            port_id,
            qos_policy_id,
        } => {
            let mut c = Cmd::new("openstack floating ip create");
            if let Some(p) = non_empty(port_id) {
                c.opt("--port", &p);
            }
            if let Some(q) = non_empty(qos_policy_id) {
                c.opt("--qos-policy", &q);
            }
            if let Some(d) = non_empty(description) {
                c.opt("--description", &d);
            }
            c.arg(network_id);
            c
        }
        OsAction::AdminFloatingIpAllocate {
            network_id,
            project_id,
            address,
            description,
        } => {
            let mut c = Cmd::new("openstack floating ip create");
            c.opt("--project", project_id);
            if let Some(a) = non_empty(address) {
                c.opt("--floating-ip-address", &a);
            }
            if let Some(d) = non_empty(description) {
                c.opt("--description", &d);
            }
            c.arg(network_id);
            c
        }
        OsAction::FloatingIpAssociate { id, port_id } => {
            let mut c = Cmd::new("openstack floating ip set");
            c.opt("--port", port_id).arg(id);
            c
        }
        OsAction::FloatingIpDisassociate { id } => {
            let mut c = Cmd::new("openstack floating ip unset");
            c.flag("--port").arg(id);
            c
        }
        OsAction::RouterSetGateway { id, network_id } => {
            let mut c = Cmd::new("openstack router set");
            c.opt("--external-gateway", network_id).arg(id);
            c
        }
        OsAction::RouterClearGateway { id } => {
            let mut c = Cmd::new("openstack router unset");
            c.flag("--external-gateway").arg(id);
            c
        }
        OsAction::RouterAddInterface {
            id,
            subnet_id,
            ip_address,
            ..
        } if non_empty(ip_address).is_none() => {
            let mut c = Cmd::new("openstack router add subnet");
            c.arg(id).arg(subnet_id);
            c
        }
        // Horizon removes an interface by its port; so does `router remove port`.
        OsAction::RouterRemoveInterface { id, port_id } => {
            let mut c = Cmd::new("openstack router remove port");
            c.arg(id).arg(port_id);
            c
        }
        OsAction::RouterAddRoute {
            id,
            destination,
            nexthop,
        } => {
            let mut c = Cmd::new("openstack router set");
            c.opt("--route", &route(destination, nexthop)).arg(id);
            c
        }
        OsAction::RouterRemoveRoute {
            id,
            destination,
            nexthop,
        } => {
            let mut c = Cmd::new("openstack router unset");
            c.opt("--route", &route(destination, nexthop)).arg(id);
            c
        }
        OsAction::RouterSetRoutes { id, routes } => {
            let mut c = Cmd::new("openstack router set");
            c.flag("--no-route");
            for (d, n) in routes {
                c.opt("--route", &route(d, n));
            }
            c.arg(id);
            c
        }
        OsAction::RouterUpdate {
            id,
            name,
            admin_up,
            snat,
            distributed,
            ha,
        } => {
            let mut c = Cmd::new("openstack router set");
            c.opt("--name", name.trim())
                .flag(if *admin_up { "--enable" } else { "--disable" });
            // SNAT is a property of the gateway: the CLI sets it together with `--external-gateway`.
            if let Some((net, on)) = snat {
                c.opt("--external-gateway", net)
                    .flag(if *on { "--enable-snat" } else { "--disable-snat" });
            }
            match distributed {
                Some(true) => c.flag("--distributed"),
                Some(false) => c.flag("--centralized"),
                None => &mut c,
            };
            match ha {
                Some(true) => c.flag("--ha"),
                Some(false) => c.flag("--no-ha"),
                None => &mut c,
            };
            c.arg(id);
            c
        }
        OsAction::PortCreate {
            network_id,
            name,
            admin_up,
            fixed_ip,
        } => {
            let mut c = Cmd::new("openstack port create");
            c.opt("--network", network_id);
            if !admin_up {
                c.flag("--disable");
            }
            if let Some(ip) = non_empty(fixed_ip) {
                c.opt("--fixed-ip", &format!("ip-address={ip}"));
            }
            c.arg(name.trim());
            c
        }
        OsAction::PortUpdate {
            id,
            name,
            admin_up,
            vnic_type,
            port_security,
            security_group_ids,
        } => {
            let mut c = Cmd::new("openstack port set");
            if let Some(n) = name {
                c.opt("--name", n.trim());
            }
            if let Some(up) = admin_up {
                c.flag(if *up { "--enable" } else { "--disable" });
            }
            if let Some(v) = non_empty(vnic_type) {
                c.opt("--vnic-type", &v);
            }
            if let Some(on) = port_security {
                c.flag(if *on {
                    "--enable-port-security"
                } else {
                    "--disable-port-security"
                });
            }
            if let Some(ids) = security_group_ids {
                port_security_groups(&mut c, ids);
            }
            c.arg(id);
            c
        }
        OsAction::PortSetSecurityGroups { id, security_group_ids } => {
            let mut c = Cmd::new("openstack port set");
            port_security_groups(&mut c, security_group_ids);
            c.arg(id);
            c
        }
        OsAction::PortForwardingCreate(spec) => {
            let mut c = Cmd::new("openstack floating ip port forwarding create");
            port_forwarding(&mut c, spec);
            c.opt("--port", &spec.internal_port_id);
            c.arg(&spec.floating_ip_id);
            c
        }
        OsAction::PortForwardingUpdate { id, spec } => {
            let mut c = Cmd::new("openstack floating ip port forwarding set");
            port_forwarding(&mut c, spec);
            c.opt("--port", &spec.internal_port_id);
            c.arg(&spec.floating_ip_id).arg(id);
            c
        }
        OsAction::PortForwardingDelete { floating_ip_id, id } => {
            let mut c = Cmd::new("openstack floating ip port forwarding delete");
            c.arg(floating_ip_id).arg(id);
            c
        }
        _ => return None,
    })
}

/// The Octavia commands (`openstack loadbalancer …`, from `python-octaviaclient`) of an action;
/// `None` for any other. Octavia refuses a change while its load balancer is busy, so the
/// commands that create a part wait (`--wait`) until it is active.
fn octavia_commands(action: &OsAction) -> Option<Vec<String>> {
    let non_empty = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let one = |c: Cmd| Some(vec![c.render(None)]);
    match action {
        OsAction::LoadBalancerDelete { id, cascade } => {
            let mut c = Cmd::new("openstack loadbalancer delete");
            if *cascade {
                c.flag("--cascade");
            }
            c.flag("--wait").arg(id);
            one(c)
        }
        OsAction::LoadBalancerCreate(spec) => Some(lb_chain(spec)),
        OsAction::LoadBalancerUpdate {
            id,
            name,
            description,
            admin_up,
        } => {
            let mut c = Cmd::new("openstack loadbalancer set");
            c.opt("--name", name.trim()).opt("--description", description.trim());
            c.flag(if *admin_up { "--enable" } else { "--disable" }).arg(id);
            one(c)
        }
        OsAction::ListenerCreate {
            loadbalancer_id,
            spec,
            default_pool_id,
        } => {
            let mut c = Cmd::new("openstack loadbalancer listener create");
            listener_options(&mut c, spec);
            if let Some(p) = non_empty(default_pool_id) {
                c.opt("--default-pool", &p);
            }
            c.flag("--wait").arg(loadbalancer_id);
            one(c)
        }
        OsAction::ListenerUpdate {
            id,
            name,
            description,
            connection_limit,
            default_pool_id,
            admin_up,
            options,
            tls,
            ..
        } => {
            let mut c = Cmd::new("openstack loadbalancer listener set");
            c.opt("--name", name.trim()).opt("--description", description.trim());
            c.opt(
                "--connection-limit",
                &connection_limit.map_or(-1, i64::from).to_string(),
            );
            if let Some(p) = non_empty(default_pool_id) {
                c.opt("--default-pool", &p);
            }
            if let Some(o) = options {
                timeout_options(&mut c, o);
            }
            if let Some((container, snis)) = tls {
                if !container.trim().is_empty() {
                    c.opt("--default-tls-container-ref", container.trim());
                }
                if !snis.is_empty() {
                    c.opt("--sni-container-refs", &snis.join(" "));
                }
            }
            c.flag(if *admin_up { "--enable" } else { "--disable" }).arg(id);
            one(c)
        }
        OsAction::PoolCreate {
            loadbalancer_id,
            listener_id,
            spec,
        } => {
            let mut c = Cmd::new("openstack loadbalancer pool create");
            pool_options(&mut c, spec);
            match non_empty(listener_id) {
                Some(l) => c.opt("--listener", &l),
                None => c.opt("--loadbalancer", loadbalancer_id),
            };
            c.flag("--wait");
            one(c)
        }
        OsAction::PoolUpdate {
            id,
            name,
            description,
            algorithm,
            session_persistence,
            cookie_name,
            admin_up,
            tls,
            ..
        } => {
            let mut c = Cmd::new("openstack loadbalancer pool set");
            c.opt("--name", name.trim()).opt("--description", description.trim());
            c.opt("--lb-algorithm", algorithm);
            match session_persistence.as_deref() {
                Some(kind) => c.opt("--session-persistence", &persistence(kind, cookie_name.as_deref())),
                None => c.flag("--disable-session-persistence"),
            };
            if let Some((on, ciphers)) = tls {
                c.flag(if *on { "--enable-tls" } else { "--disable-tls" });
                if let Some(x) = non_empty(ciphers) {
                    c.opt("--tls-ciphers", &x);
                }
            }
            c.flag(if *admin_up { "--enable" } else { "--disable" }).arg(id);
            one(c)
        }
        OsAction::MemberCreate { pool_id, spec } => one(member_create(pool_id, spec)),
        OsAction::MemberUpdate {
            pool_id,
            id,
            name,
            weight,
            backup,
            admin_up,
            monitor_address,
            monitor_port,
        } => {
            let mut c = Cmd::new("openstack loadbalancer member set");
            c.opt("--name", name.trim()).opt("--weight", &weight.to_string());
            c.flag(if *backup { "--enable-backup" } else { "--disable-backup" });
            if let Some(a) = non_empty(monitor_address) {
                c.opt("--monitor-address", &a);
            }
            if let Some(p) = monitor_port {
                c.opt("--monitor-port", &p.to_string());
            }
            c.flag(if *admin_up { "--enable" } else { "--disable" })
                .arg(pool_id)
                .arg(id);
            one(c)
        }
        OsAction::MemberDelete { pool_id, id } => {
            let mut c = Cmd::new("openstack loadbalancer member delete");
            c.arg(pool_id).arg(id);
            one(c)
        }
        OsAction::MonitorCreate { pool_id, spec } => one(monitor_create(pool_id, spec)),
        OsAction::MonitorUpdate { id, spec, admin_up } => {
            let mut c = Cmd::new("openstack loadbalancer healthmonitor set");
            monitor_options(&mut c, spec);
            c.flag(if *admin_up { "--enable" } else { "--disable" }).arg(id);
            one(c)
        }
        _ => None,
    }
}

/// `--session-persistence type=…[,cookie_name=…]`; the cookie only for `APP_COOKIE`.
fn persistence(kind: &str, cookie: Option<&str>) -> String {
    match cookie.map(str::trim).filter(|c| !c.is_empty() && kind == "APP_COOKIE") {
        Some(c) => format!("type={kind},cookie_name={c}"),
        None => format!("type={kind}"),
    }
}

/// The four timeouts, the inserted headers (`X-Forwarded-For=true`), the allowed CIDRs and the TLS
/// ciphers of a listener.
fn timeout_options(c: &mut Cmd, o: &LbListenerOptions) {
    for (flag, v) in [
        ("--timeout-client-data", o.timeout_client_data),
        ("--timeout-tcp-inspect", o.timeout_tcp_inspect),
        ("--timeout-member-connect", o.timeout_member_connect),
        ("--timeout-member-data", o.timeout_member_data),
    ] {
        if let Some(v) = v {
            c.opt(flag, &v.to_string());
        }
    }
    for h in &o.insert_headers {
        c.opt("--insert-headers", &format!("{h}=true"));
    }
    for cidr in o.allowed_cidrs.iter().flatten() {
        c.opt("--allowed-cidr", cidr);
    }
    if let Some(x) = o.ciphers() {
        c.opt("--tls-ciphers", x);
    }
}

/// The options of `loadbalancer listener create`, but for the load balancer and `--wait`.
fn listener_options(c: &mut Cmd, l: &LbListenerSpec) {
    if let Some(n) = l.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        c.opt("--name", n);
    }
    if let Some(d) = l.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        c.opt("--description", d);
    }
    c.opt("--protocol", &l.protocol)
        .opt("--protocol-port", &l.port.to_string());
    if let Some(n) = l.connection_limit {
        c.opt("--connection-limit", &n.to_string());
    }
    timeout_options(c, &l.options);
    if let Some(r) = l.default_tls_container_ref.as_deref().filter(|r| !r.trim().is_empty()) {
        c.opt("--default-tls-container-ref", r.trim());
    }
    if !l.sni_container_refs.is_empty() {
        c.opt("--sni-container-refs", &l.sni_container_refs.join(" "));
    }
    if !l.admin_up {
        c.flag("--disable");
    }
}

/// The options of `loadbalancer pool create`, but for the listener or load balancer and `--wait`.
fn pool_options(c: &mut Cmd, p: &LbPoolSpec) {
    if let Some(n) = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        c.opt("--name", n);
    }
    if let Some(d) = p.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        c.opt("--description", d);
    }
    c.opt("--protocol", &p.protocol).opt("--lb-algorithm", &p.algorithm);
    if let Some(kind) = p.session_persistence.as_deref() {
        c.opt("--session-persistence", &persistence(kind, p.cookie_name.as_deref()));
    }
    if p.tls_enabled {
        c.flag("--enable-tls");
        if let Some(x) = p.tls_ciphers.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
            c.opt("--tls-ciphers", x);
        }
    }
    if !p.admin_up {
        c.flag("--disable");
    }
}

/// The pool as the last word: an ID, or a variable a previous command of the chain captured.
fn pool_word(c: &mut Cmd, pool: &str) {
    if pool.starts_with('"') {
        c.raw(pool);
    } else {
        c.arg(pool);
    }
}

/// `openstack loadbalancer member create … <pool>`.
fn member_create(pool: &str, m: &LbMemberSpec) -> Cmd {
    let mut c = Cmd::new("openstack loadbalancer member create");
    if let Some(n) = m.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        c.opt("--name", n);
    }
    c.opt("--address", &m.address)
        .opt("--protocol-port", &m.port.to_string());
    if let Some(sn) = m.subnet_id.as_deref().filter(|s| !s.trim().is_empty()) {
        c.opt("--subnet-id", sn.trim());
    }
    if let Some(w) = m.weight {
        c.opt("--weight", &w.to_string());
    }
    if m.backup {
        c.flag("--enable-backup");
    }
    if let Some(a) = m.monitor_address.as_deref().filter(|a| !a.trim().is_empty()) {
        c.opt("--monitor-address", a.trim());
    }
    if let Some(p) = m.monitor_port {
        c.opt("--monitor-port", &p.to_string());
    }
    if !m.admin_up {
        c.flag("--disable");
    }
    c.flag("--wait");
    pool_word(&mut c, pool);
    c
}

/// The options of a health monitor that create and update share.
fn monitor_options(c: &mut Cmd, m: &LbMonitorSpec) {
    if let Some(n) = m.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        c.opt("--name", n);
    }
    c.opt("--delay", &m.delay.to_string())
        .opt("--timeout", &m.timeout.to_string())
        .opt("--max-retries", &m.max_retries.to_string());
    if let Some(n) = m.max_retries_down {
        c.opt("--max-retries-down", &n.to_string());
    }
    for (flag, v) in [
        ("--http-method", &m.http_method),
        ("--url-path", &m.url_path),
        ("--expected-codes", &m.expected_codes),
    ] {
        if let Some(v) = v.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            c.opt(flag, v);
        }
    }
}

/// `openstack loadbalancer healthmonitor create … <pool>`.
fn monitor_create(pool: &str, m: &LbMonitorSpec) -> Cmd {
    let mut c = Cmd::new("openstack loadbalancer healthmonitor create");
    monitor_options(&mut c, m);
    c.opt("--type", &m.kind);
    if !m.admin_up {
        c.flag("--disable");
    }
    c.flag("--wait");
    pool_word(&mut c, pool);
    c
}

/// The wizard's tree as a chain: the load balancer, then its listener, pool, members and monitor,
/// each capturing the ID the next one needs (`LB_ID`, `LISTENER_ID`, `POOL_ID`). Octavia's REST
/// request is one call; the CLI has no equivalent of that, so it builds the parts in order.
fn lb_chain(spec: &LbCreateSpec) -> Vec<String> {
    let nonblank = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let needs_ids =
        spec.listener.is_some() || spec.pool.is_some() || !spec.members.is_empty() || spec.monitor.is_some();
    let mut lb = Cmd::new("openstack loadbalancer create");
    if let Some(n) = nonblank(&spec.name) {
        lb.opt("--name", &n);
    }
    if let Some(d) = nonblank(&spec.description) {
        lb.opt("--description", &d);
    }
    lb.opt("--vip-subnet-id", spec.vip_subnet_id.trim());
    if let Some(a) = nonblank(&spec.vip_address) {
        lb.opt("--vip-address", &a);
    }
    if let Some(f) = nonblank(&spec.flavor_id) {
        lb.opt("--flavor", &f);
    }
    if let Some(z) = nonblank(&spec.availability_zone) {
        lb.opt("--availability-zone", &z);
    }
    if !spec.admin_up {
        lb.flag("--disable");
    }
    lb.flag("--wait");
    let mut out = vec![if needs_ids {
        lb.render(Some("LB_ID"))
    } else {
        lb.render(None)
    }];
    if let Some(l) = &spec.listener {
        let mut c = Cmd::new("openstack loadbalancer listener create");
        listener_options(&mut c, l);
        c.flag("--wait").raw("\"$LB_ID\"");
        out.push(c.render(Some("LISTENER_ID")));
    }
    if let Some(p) = &spec.pool {
        let mut c = Cmd::new("openstack loadbalancer pool create");
        pool_options(&mut c, p);
        if spec.listener.is_some() {
            c.var("--listener", "LISTENER_ID");
        } else {
            c.var("--loadbalancer", "LB_ID");
        }
        c.flag("--wait");
        out.push(c.render(Some("POOL_ID")));
        for m in &spec.members {
            out.push(member_create("\"$POOL_ID\"", m).render(None));
        }
        if let Some(m) = &spec.monitor {
            out.push(monitor_create("\"$POOL_ID\"", m).render(None));
        }
    }
    out
}

/// `destination=<cidr>,gateway=<ip>`: the value of `router set --route`.
fn route(destination: &str, nexthop: &str) -> String {
    format!("destination={},gateway={}", destination.trim(), nexthop.trim())
}

/// The port's security groups replace the old ones: `--no-security-group` then one each.
fn port_security_groups(c: &mut Cmd, ids: &[String]) {
    c.flag("--no-security-group");
    for g in ids {
        c.opt("--security-group", g);
    }
}

/// The options of a port forwarding that create and update share.
fn port_forwarding(c: &mut Cmd, s: &PortForwardingSpec) {
    c.opt("--protocol", &s.protocol)
        .opt("--internal-ip-address", s.internal_ip.trim())
        .opt("--internal-protocol-port", s.internal.trim())
        .opt("--external-protocol-port", s.external.trim());
    if let Some(d) = s.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        c.opt("--description", d);
    }
}

/// `openstack security group rule create`; a single port is `--dst-port <n>`, a range `<min>:<max>`.
fn rule_create(r: &RuleSpec) -> Cmd {
    let mut c = Cmd::new("openstack security group rule create");
    c.flag(if r.direction == "egress" {
        "--egress"
    } else {
        "--ingress"
    });
    c.opt("--ethertype", &r.ethertype);
    if let Some(p) = r.protocol.as_deref().filter(|p| !p.is_empty()) {
        c.opt("--protocol", p);
    }
    match (r.port_min, r.port_max) {
        (Some(a), Some(b)) if a != b => {
            c.opt("--dst-port", &format!("{a}:{b}"));
        }
        (Some(a), _) | (None, Some(a)) => {
            c.opt("--dst-port", &a.to_string());
        }
        (None, None) => {}
    }
    if let Some(ip) = r.remote_ip_prefix.as_deref().filter(|s| !s.is_empty()) {
        c.opt("--remote-ip", ip);
    } else if let Some(g) = r.remote_group_id.as_deref().filter(|s| !s.is_empty()) {
        c.opt("--remote-group", g);
    }
    if let Some(d) = r.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        c.opt("--description", d);
    }
    c.arg(&r.security_group_id);
    c
}

/// The network command, plain when alone, else capturing its ID for the subnet's.
fn network_chain(net: Cmd, subnet: Option<&SubnetSpec>) -> Vec<String> {
    match subnet {
        None => vec![net.render(None)],
        Some(s) => vec![net.render(Some("NETWORK_ID")), subnet_create(s).render(None)],
    }
}

/// The router command, plain when alone, else capturing its ID for the gateway's QoS policy
/// (`router create` has no such option).
fn router_chain(r: Cmd, qos_policy_id: Option<&str>) -> Vec<String> {
    match qos_policy_id.map(str::trim).filter(|q| !q.is_empty()) {
        None => vec![r.render(None)],
        Some(q) => {
            let mut set = Cmd::new("openstack router set");
            set.opt("--qos-policy", q).raw("\"$ROUTER_ID\"");
            vec![r.render(Some("ROUTER_ID")), set.render(None)]
        }
    }
}

/// `openstack subnet create`, on the network the previous command captured.
fn subnet_create(s: &SubnetSpec) -> Cmd {
    let mut c = Cmd::new("openstack subnet create");
    c.var("--network", "NETWORK_ID")
        .opt("--subnet-range", s.cidr.trim())
        .opt("--ip-version", &s.ip_version.to_string());
    match s.gateway_ip.as_deref().map(str::trim) {
        None => {}
        Some("") => {
            c.opt("--gateway", "none");
        }
        Some(gw) => {
            c.opt("--gateway", gw);
        }
    }
    c.flag(if s.enable_dhcp { "--dhcp" } else { "--no-dhcp" });
    for d in &s.dns_nameservers {
        c.opt("--dns-nameserver", d.trim());
    }
    for (a, z) in &s.allocation_pools {
        c.opt("--allocation-pool", &format!("start={},end={}", a.trim(), z.trim()));
    }
    for (dest, hop) in &s.host_routes {
        c.opt(
            "--host-route",
            &format!("destination={},gateway={}", dest.trim(), hop.trim()),
        );
    }
    if let Some(m) = &s.ipv6_ra_mode {
        c.opt("--ipv6-ra-mode", m);
    }
    if let Some(m) = &s.ipv6_address_mode {
        c.opt("--ipv6-address-mode", m);
    }
    // The CLI wants a name; `quote` writes an empty one as `''`, as the REST request sends it.
    c.arg(s.name.trim());
    c
}

/// `openstack server create`, from the *Launch Instance* wizard's result.
fn server_create(s: &LaunchSpec) -> Cmd {
    let mut c = Cmd::new("openstack server create");
    c.opt("--flavor", s.flavor_id.trim());
    let src = s.source_id.trim();
    let delete = if s.delete_volume_on_terminate { "true" } else { "false" };
    let size = s.volume_size_gb.filter(|g| *g > 0);
    // A new boot volume is a block device mapping, as in the REST request.
    let bdm = |kind: &str, size: Option<u32>| {
        let mut v = format!("source_type={kind},uuid={src},destination_type=volume,boot_index=0");
        if let Some(g) = size {
            v.push_str(&format!(",volume_size={g}"));
        }
        v.push_str(&format!(",delete_on_termination={delete}"));
        v
    };
    match s.source {
        BootSource::Image if s.create_volume => {
            c.opt("--block-device", &bdm("image", size));
        }
        BootSource::Image => {
            c.opt("--image", src);
        }
        BootSource::Volume => {
            c.opt("--block-device", &bdm("volume", None));
        }
        BootSource::VolumeSnapshot => {
            c.opt("--block-device", &bdm("snapshot", size));
        }
    }
    for n in s.network_ids.iter().filter(|n| !n.trim().is_empty()) {
        c.opt("--network", n.trim());
    }
    for p in s.port_ids.iter().filter(|p| !p.trim().is_empty()) {
        c.opt("--port", p.trim());
    }
    for g in &s.security_groups {
        c.opt("--security-group", g);
    }
    if let Some(k) = s.key_name.as_deref().filter(|k| !k.trim().is_empty()) {
        c.opt("--key-name", k.trim());
    }
    if let Some(g) = s.server_group_id.as_deref().filter(|g| !g.trim().is_empty()) {
        c.opt("--hint", &format!("group={}", g.trim()));
    }
    if let Some(z) = s.availability_zone.as_deref().filter(|z| !z.trim().is_empty()) {
        c.opt("--availability-zone", z.trim());
    }
    if let Some(d) = s.description.as_deref().filter(|d| !d.trim().is_empty()) {
        c.opt("--description", d.trim());
    }
    if s.user_data.as_deref().is_some_and(|u| !u.trim().is_empty()) {
        // The script itself is not repeated: save the one typed into this file.
        c.opt("--user-data", "./user-data.txt");
    }
    if s.config_drive {
        c.flag("--use-config-drive");
    }
    if s.admin_pass.as_deref().is_some_and(|p| !p.is_empty()) {
        c.opt("--password", "<password>");
    }
    if s.count > 1 {
        c.opt("--min", &s.count.to_string()).opt("--max", &s.count.to_string());
    }
    c.arg(s.name.trim());
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> LaunchSpec {
        LaunchSpec {
            name: "web".into(),
            availability_zone: None,
            count: 1,
            source: BootSource::Image,
            source_id: "img".into(),
            create_volume: false,
            volume_size_gb: None,
            delete_volume_on_terminate: false,
            flavor_id: "f1".into(),
            network_ids: vec!["n1".into(), "".into()],
            port_ids: vec![],
            security_groups: vec!["default".into()],
            key_name: None,
            server_group_id: None,
            description: None,
            user_data: None,
            config_drive: false,
            manual_disk_partition: false,
            admin_pass: None,
        }
    }

    fn subnet() -> SubnetSpec {
        SubnetSpec {
            name: "sn".into(),
            cidr: "10.0.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some(String::new()),
            enable_dhcp: false,
            dns_nameservers: vec!["1.1.1.1".into()],
            allocation_pools: vec![("10.0.0.10".into(), "10.0.0.20".into())],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![("10.9.0.0/16".into(), "10.0.0.1".into())],
        }
    }

    #[test]
    fn quoting_keeps_safe_words_and_wraps_the_rest() {
        assert_eq!(quote("web-1"), "web-1");
        assert_eq!(quote("my web"), "'my web'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn instance_from_an_image_is_one_plain_command() {
        let c = cli_equivalent(&OsAction::ServerLaunch(launch()));
        assert_eq!(
            c,
            vec![
                "openstack server create \\\n  --flavor f1 \\\n  --image img \\\n  --network n1 \\\n  --security-group default \\\n  web"
            ]
        );
    }

    #[test]
    fn instance_boot_from_a_new_volume_with_everything_else() {
        let s = LaunchSpec {
            name: "my web".into(),
            create_volume: true,
            volume_size_gb: Some(50),
            delete_volume_on_terminate: true,
            count: 2,
            port_ids: vec!["p1".into()],
            key_name: Some("ops".into()),
            server_group_id: Some("g1".into()),
            availability_zone: Some("az1".into()),
            description: Some("d".into()),
            user_data: Some("#!/bin/sh\necho SECRET".into()),
            config_drive: true,
            admin_pass: Some("hunter2".into()),
            ..launch()
        };
        let c = cli_equivalent(&OsAction::ServerLaunch(s)).remove(0);
        for want in [
            "--block-device source_type=image,uuid=img,destination_type=volume,boot_index=0,volume_size=50,delete_on_termination=true",
            "--port p1",
            "--key-name ops",
            "--hint group=g1",
            "--availability-zone az1",
            "--user-data ./user-data.txt",
            "--use-config-drive",
            "--password '<password>'",
            "--min 2 \\\n  --max 2",
            "'my web'",
        ] {
            assert!(c.contains(want), "{want} missing in:\n{c}");
        }
        assert!(!c.contains("hunter2") && !c.contains("SECRET"), "secrets stay out");
        assert!(!c.contains("--image"), "the volume replaces the image");
    }

    #[test]
    fn network_alone_is_plain_and_with_a_subnet_a_chain_through_a_variable() {
        let alone = OsAction::NetworkCreate {
            name: "net".into(),
            admin_up: false,
            subnet: None,
        };
        assert_eq!(
            cli_equivalent(&alone),
            vec!["openstack network create \\\n  --disable \\\n  net"]
        );
        let chain = cli_equivalent(&OsAction::NetworkCreate {
            name: "net".into(),
            admin_up: true,
            subnet: Some(subnet()),
        });
        assert_eq!(
            chain[0],
            "NETWORK_ID=$(openstack network create \\\n  net \\\n  -f value -c id)"
        );
        assert_eq!(
            chain[1],
            "openstack subnet create \\\n  --network \"$NETWORK_ID\" \\\n  --subnet-range 10.0.0.0/24 \\\n  --ip-version 4 \\\n  --gateway none \\\n  --no-dhcp \\\n  --dns-nameserver 1.1.1.1 \\\n  --allocation-pool start=10.0.0.10,end=10.0.0.20 \\\n  --host-route destination=10.9.0.0/16,gateway=10.0.0.1 \\\n  sn"
        );
        let open = SubnetSpec {
            gateway_ip: None,
            enable_dhcp: true,
            name: String::new(),
            dns_nameservers: vec![],
            allocation_pools: vec![],
            host_routes: vec![],
            ..subnet()
        };
        let c = subnet_create(&open).render(None);
        assert!(
            c.contains("--dhcp") && !c.contains("--gateway") && c.ends_with("''"),
            "{c}"
        );
    }

    #[test]
    fn admin_network_has_owner_provider_and_flags() {
        let c = cli_equivalent(&OsAction::AdminNetworkCreate(Box::new(AdminNetworkSpec {
            name: "ext".into(),
            project_id: "p1".into(),
            provider: Some(ProviderNetwork {
                network_type: "vlan".into(),
                physical_network: Some("physnet1".into()),
                segmentation_id: Some(100),
            }),
            admin_up: true,
            shared: true,
            external: true,
            mtu: None,
            az_hints: vec!["az1".into()],
            subnet: None,
        })))
        .remove(0);
        assert_eq!(
            c,
            "openstack network create \\\n  --project p1 \\\n  --share \\\n  --external \\\n  --provider-network-type vlan \\\n  --provider-physical-network physnet1 \\\n  --provider-segment 100 \\\n  --availability-zone-hint az1 \\\n  ext"
        );
    }

    #[test]
    fn router_with_and_without_gateway_and_policy() {
        let plain = OsAction::RouterCreate {
            name: "r".into(),
            admin_up: true,
            external_network_id: None,
            qos_policy_id: None,
        };
        assert_eq!(cli_equivalent(&plain), vec!["openstack router create \\\n  r"]);
        let gw = OsAction::RouterCreate {
            name: "r".into(),
            admin_up: false,
            external_network_id: Some("ext".into()),
            qos_policy_id: None,
        };
        assert_eq!(
            cli_equivalent(&gw),
            vec!["openstack router create \\\n  --disable \\\n  --external-gateway ext \\\n  r"]
        );
        let qos = OsAction::RouterCreate {
            name: "r".into(),
            admin_up: true,
            external_network_id: Some("ext".into()),
            qos_policy_id: Some("q1".into()),
        };
        let c = cli_equivalent(&qos);
        assert_eq!(
            c[0],
            "ROUTER_ID=$(openstack router create \\\n  --external-gateway ext \\\n  r \\\n  -f value -c id)"
        );
        assert_eq!(c[1], "openstack router set \\\n  --qos-policy q1 \\\n  \"$ROUTER_ID\"");
    }

    #[test]
    fn admin_router_options() {
        let c = cli_equivalent(&OsAction::AdminRouterCreate(Box::new(AdminRouterSpec {
            name: "r".into(),
            project_id: "p1".into(),
            admin_up: true,
            external_network_id: Some("ext".into()),
            enable_snat: Some(false),
            az_hints: vec![],
            distributed: Some(true),
            ha: Some(false),
        })))
        .remove(0);
        assert_eq!(
            c,
            "openstack router create \\\n  --project p1 \\\n  --external-gateway ext \\\n  --disable-snat \\\n  --distributed \\\n  --no-ha \\\n  r"
        );
    }

    #[test]
    fn deleting_instances_networks_and_routers_is_one_command_with_every_id() {
        let one = cli_equivalent(&OsAction::ServerDelete { id: "s1".into() });
        assert_eq!(one, ["openstack server delete \\\n  s1"]);
        let batch = |v: Vec<OsAction>| OsAction::Batch(v.into_iter().map(|a| ("n".to_string(), a)).collect());
        let nets = cli_equivalent(&batch(vec![
            OsAction::NetworkDelete { id: "n1".into() },
            OsAction::NetworkDelete { id: "n2".into() },
        ]));
        assert_eq!(nets, ["openstack network delete \\\n  n1 \\\n  n2"]);
        let routers = cli_equivalent(&batch(vec![OsAction::RouterDelete { id: "r 1".into() }]));
        assert_eq!(routers, ["openstack router delete \\\n  'r 1'"]);
        // Mixed kinds: one command per item; an item without an equivalent adds none.
        let mixed = cli_equivalent(&batch(vec![
            OsAction::ServerDelete { id: "s1".into() },
            OsAction::RouterDelete { id: "r1".into() },
            OsAction::UserDelete { id: "u1".into() },
        ]));
        assert_eq!(mixed.len(), 2);
        assert!(cli_equivalent(&OsAction::Batch(vec![])).is_empty());
    }

    #[test]
    fn volumes_are_created_and_deleted() {
        let make = |source, source_id: Option<&str>, name: &str| {
            cli_equivalent(&OsAction::VolumeCreate {
                name: name.into(),
                description: Some("my data".into()),
                size_gb: 10,
                volume_type: Some("ssd".into()),
                availability_zone: None,
                source,
                source_id: source_id.map(Into::into),
            })
        };
        assert_eq!(
            make(VolumeSource::Image, Some("img1"), "data"),
            [
                "openstack volume create \\\n  --size 10 \\\n  --image img1 \\\n  --type ssd \\\n  --description 'my data' \\\n  data"
            ]
        );
        assert!(make(VolumeSource::Snapshot, Some("s1"), "d")[0].contains("--snapshot s1"));
        assert!(make(VolumeSource::Volume, Some("v1"), "d")[0].contains("--source v1"));
        let empty = &make(VolumeSource::Empty, None, "")[0];
        assert!(!empty.contains("--image") && !empty.ends_with("''"), "{empty}");
        assert_eq!(
            cli_equivalent(&OsAction::VolumeDelete { id: "v1".into() }),
            ["openstack volume delete \\\n  v1"]
        );
    }

    #[test]
    fn instance_lifecycle_actions() {
        let c = |a: OsAction| cli_equivalent(&a).join("\n");
        let id = || "s 1".to_string();
        assert_eq!(
            c(OsAction::ServerStart { id: "s1".into() }),
            "openstack server start \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerReboot {
                id: "s1".into(),
                kind: RebootKind::Hard
            }),
            "openstack server reboot \\\n  --hard \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerUpdate {
                id: id(),
                name: " new name ".into()
            }),
            "openstack server set \\\n  --name 'new name' \\\n  's 1'"
        );
        assert_eq!(
            c(OsAction::ServerSnapshot {
                id: "s1".into(),
                name: "snap".into()
            }),
            "openstack server image create \\\n  --name snap \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerResize {
                id: "s1".into(),
                flavor_id: "f2".into()
            }),
            "openstack server resize \\\n  --flavor f2 \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerConfirmResize { id: "s1".into() }),
            "openstack server resize confirm \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerRevertResize { id: "s1".into() }),
            "openstack server resize revert \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerRebuild {
                id: "s1".into(),
                image_id: "i1".into()
            }),
            "openstack server rebuild \\\n  --image i1 \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerUnshelve {
                id: "s1".into(),
                availability_zone: Some("az1".into())
            }),
            "openstack server unshelve \\\n  --availability-zone az1 \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerUnshelve {
                id: "s1".into(),
                availability_zone: Some(" ".into())
            }),
            "openstack server unshelve \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerRescue {
                id: "s1".into(),
                image_id: None
            }),
            "openstack server rescue \\\n  s1"
        );
        assert_eq!(
            c(OsAction::ServerRescue {
                id: "s1".into(),
                image_id: Some("i1".into())
            }),
            "openstack server rescue \\\n  --image i1 \\\n  s1"
        );
        for (action, verb) in [
            (OsAction::ServerStop { id: "s1".into() }, "stop"),
            (OsAction::ServerPause { id: "s1".into() }, "pause"),
            (OsAction::ServerUnpause { id: "s1".into() }, "unpause"),
            (OsAction::ServerSuspend { id: "s1".into() }, "suspend"),
            (OsAction::ServerResume { id: "s1".into() }, "resume"),
            (OsAction::ServerLock { id: "s1".into() }, "lock"),
            (OsAction::ServerUnlock { id: "s1".into() }, "unlock"),
            (OsAction::ServerShelve { id: "s1".into() }, "shelve"),
            (OsAction::ServerUnrescue { id: "s1".into() }, "unrescue"),
        ] {
            assert_eq!(c(action), format!("openstack server {verb} \\\n  s1"));
        }
        assert_eq!(
            c(OsAction::ServerAttachVolume {
                server_id: "s1".into(),
                volume_id: "v1".into()
            }),
            "openstack server add volume \\\n  s1 \\\n  v1"
        );
        assert_eq!(
            c(OsAction::ServerDetachVolume {
                server_id: "s1".into(),
                volume_id: "v1".into()
            }),
            "openstack server remove volume \\\n  s1 \\\n  v1"
        );
    }

    #[test]
    fn floating_ips_security_groups_keys_and_snapshots() {
        let c = |a: OsAction| cli_equivalent(&a).join("\n");
        assert_eq!(
            c(OsAction::FloatingIpAllocate {
                network_id: "ext".into(),
                description: Some("web".into()),
                port_id: Some("p1".into()),
                qos_policy_id: None
            }),
            "openstack floating ip create \\\n  --port p1 \\\n  --description web \\\n  ext"
        );
        assert_eq!(
            c(OsAction::AdminFloatingIpAllocate {
                network_id: "ext".into(),
                project_id: "pr".into(),
                address: Some("203.0.113.5".into()),
                description: None
            }),
            "openstack floating ip create \\\n  --project pr \\\n  --floating-ip-address 203.0.113.5 \\\n  ext"
        );
        assert_eq!(
            c(OsAction::FloatingIpAssociate {
                id: "f1".into(),
                port_id: "p1".into()
            }),
            "openstack floating ip set \\\n  --port p1 \\\n  f1"
        );
        assert_eq!(
            c(OsAction::FloatingIpDisassociate { id: "f1".into() }),
            "openstack floating ip unset \\\n  --port \\\n  f1"
        );
        assert_eq!(
            c(OsAction::FloatingIpRelease { id: "f1".into() }),
            "openstack floating ip delete \\\n  f1"
        );
        assert_eq!(
            c(OsAction::SecurityGroupCreate {
                name: "web".into(),
                description: Some("".into())
            }),
            "openstack security group create \\\n  web"
        );
        let rule = |protocol: Option<&str>, min, max, ip: Option<&str>, group: Option<&str>| {
            c(OsAction::RuleCreate(RuleSpec {
                security_group_id: "sg1".into(),
                direction: "ingress".into(),
                ethertype: "IPv4".into(),
                protocol: protocol.map(Into::into),
                port_min: min,
                port_max: max,
                remote_ip_prefix: ip.map(Into::into),
                remote_group_id: group.map(Into::into),
                description: None,
            }))
        };
        assert_eq!(
            rule(Some("tcp"), Some(22), Some(22), Some("0.0.0.0/0"), None),
            "openstack security group rule create \\\n  --ingress \\\n  --ethertype IPv4 \\\n  --protocol tcp \\\n  --dst-port 22 \\\n  --remote-ip 0.0.0.0/0 \\\n  sg1"
        );
        let range = rule(Some("udp"), Some(1000), Some(2000), None, Some("g1"));
        assert!(
            range.contains("--dst-port 1000:2000") && range.contains("--remote-group g1"),
            "{range}"
        );
        let any = rule(None, None, None, None, None);
        assert!(
            !any.contains("--protocol") && !any.contains("--dst-port") && !any.contains("--remote"),
            "{any}"
        );
        assert_eq!(
            c(OsAction::KeyPairGenerate { name: "k1".into() }),
            "openstack keypair create \\\n  --private-key ./k1.pem \\\n  k1"
        );
        let imported = c(OsAction::KeyPairImport {
            name: "k1".into(),
            public_key: "ssh-ed25519 AAAA secret".into(),
            key_type: "ssh".into(),
        });
        assert_eq!(
            imported,
            "openstack keypair create \\\n  --public-key ./k1.pub \\\n  --type ssh \\\n  k1"
        );
        assert_eq!(
            c(OsAction::ServerGroupCreate {
                name: "ha".into(),
                policy: "anti-affinity".into()
            }),
            "openstack server group create \\\n  --policy anti-affinity \\\n  ha"
        );
        assert_eq!(
            c(OsAction::VolumeExtend {
                id: "v1".into(),
                new_size_gb: 20
            }),
            "openstack volume set \\\n  --size 20 \\\n  v1"
        );
        assert_eq!(
            c(OsAction::VolumeSnapshotCreate {
                volume_id: "v1".into(),
                name: "s".into(),
                description: None,
                force: true
            }),
            "openstack volume snapshot create \\\n  --volume v1 \\\n  --force \\\n  s"
        );
    }

    #[test]
    fn more_resources_are_deleted_by_one_command() {
        let c = |a: OsAction| cli_equivalent(&a).join("\n");
        assert_eq!(
            c(OsAction::ImageDelete { id: "i1".into() }),
            "openstack image delete \\\n  i1"
        );
        assert_eq!(
            c(OsAction::SubnetDelete { id: "s1".into() }),
            "openstack subnet delete \\\n  s1"
        );
        assert_eq!(
            c(OsAction::PortDelete { id: "p1".into() }),
            "openstack port delete \\\n  p1"
        );
        assert_eq!(
            c(OsAction::SnapshotDelete { id: "s1".into() }),
            "openstack volume snapshot delete \\\n  s1"
        );
        assert_eq!(
            c(OsAction::SecurityGroupDelete { id: "g1".into() }),
            "openstack security group delete \\\n  g1"
        );
        assert_eq!(
            c(OsAction::RuleDelete { id: "r1".into() }),
            "openstack security group rule delete \\\n  r1"
        );
        assert_eq!(
            c(OsAction::ServerGroupDelete { id: "g1".into() }),
            "openstack server group delete \\\n  g1"
        );
        assert_eq!(
            c(OsAction::KeyPairDelete { name: "k1".into() }),
            "openstack keypair delete \\\n  k1"
        );
    }

    #[test]
    fn routers_gateways_ports_and_forwardings() {
        let c = |a: OsAction| cli_equivalent(&a).join("\n");
        let r = |x: &str| x.to_string();
        assert_eq!(
            c(OsAction::RouterSetGateway {
                id: r("r1"),
                network_id: r("ext")
            }),
            "openstack router set \\\n  --external-gateway ext \\\n  r1"
        );
        assert_eq!(
            c(OsAction::RouterClearGateway { id: r("r1") }),
            "openstack router unset \\\n  --external-gateway \\\n  r1"
        );
        assert_eq!(
            c(OsAction::RouterAddInterface {
                id: r("r1"),
                subnet_id: r("sn1"),
                ip_address: None,
                network_id: None
            }),
            "openstack router add subnet \\\n  r1 \\\n  sn1"
        );
        let with_ip = cli_equivalent(&OsAction::RouterAddInterface {
            id: r("r1"),
            subnet_id: r("sn1"),
            ip_address: Some(r(" 10.0.0.9 ")),
            network_id: Some(r("n1")),
        });
        assert_eq!(
            with_ip,
            [
                "PORT_ID=$(openstack port create \\\n  --network n1 \\\n  --fixed-ip subnet=sn1,ip-address=10.0.0.9 \\\n  '' \\\n  -f value -c id)",
                "openstack router add port \\\n  r1 \\\n  \"$PORT_ID\""
            ]
        );
        assert_eq!(
            c(OsAction::RouterRemoveInterface {
                id: r("r1"),
                port_id: r("p1")
            }),
            "openstack router remove port \\\n  r1 \\\n  p1"
        );
        assert_eq!(
            c(OsAction::RouterAddRoute {
                id: r("r1"),
                destination: r("10.9.0.0/16"),
                nexthop: r("10.0.0.1")
            }),
            "openstack router set \\\n  --route destination=10.9.0.0/16,gateway=10.0.0.1 \\\n  r1"
        );
        assert!(
            c(OsAction::RouterRemoveRoute {
                id: r("r1"),
                destination: r("a"),
                nexthop: r("b")
            })
            .starts_with("openstack router unset")
        );
        assert_eq!(
            c(OsAction::RouterSetRoutes {
                id: r("r1"),
                routes: vec![]
            }),
            "openstack router set \\\n  --no-route \\\n  r1"
        );
        let upd = c(OsAction::RouterUpdate {
            id: r("r1"),
            name: r("core"),
            admin_up: false,
            snat: Some((r("ext"), false)),
            distributed: Some(false),
            ha: Some(true),
        });
        assert_eq!(
            upd,
            "openstack router set \\\n  --name core \\\n  --disable \\\n  --external-gateway ext \\\n  --disable-snat \\\n  --centralized \\\n  --ha \\\n  r1"
        );
        assert_eq!(
            c(OsAction::PortCreate {
                network_id: r("n1"),
                name: r("db"),
                admin_up: false,
                fixed_ip: Some(r("10.0.0.5"))
            }),
            "openstack port create \\\n  --network n1 \\\n  --disable \\\n  --fixed-ip ip-address=10.0.0.5 \\\n  db"
        );
        let set = c(OsAction::PortUpdate {
            id: r("p1"),
            name: Some(r("web")),
            admin_up: Some(true),
            vnic_type: Some(r("direct")),
            port_security: Some(false),
            security_group_ids: Some(vec![r("g1"), r("g2")]),
        });
        assert_eq!(
            set,
            "openstack port set \\\n  --name web \\\n  --enable \\\n  --vnic-type direct \\\n  --disable-port-security \\\n  --no-security-group \\\n  --security-group g1 \\\n  --security-group g2 \\\n  p1"
        );
        let bare = c(OsAction::PortUpdate {
            id: r("p1"),
            name: None,
            admin_up: None,
            vnic_type: None,
            port_security: None,
            security_group_ids: None,
        });
        assert_eq!(bare, "openstack port set \\\n  p1");
        assert_eq!(
            c(OsAction::PortSetSecurityGroups {
                id: r("p1"),
                security_group_ids: vec![]
            }),
            "openstack port set \\\n  --no-security-group \\\n  p1"
        );
        let spec = PortForwardingSpec {
            floating_ip_id: r("f1"),
            protocol: r("tcp"),
            internal_port_id: r("p1"),
            internal_ip: r("10.0.0.5"),
            internal: r("22"),
            external: r("2222"),
            description: Some(r("ssh")),
            ranges: false,
        };
        assert_eq!(
            c(OsAction::PortForwardingCreate(spec.clone())),
            "openstack floating ip port forwarding create \\\n  --protocol tcp \\\n  --internal-ip-address 10.0.0.5 \\\n  --internal-protocol-port 22 \\\n  --external-protocol-port 2222 \\\n  --description ssh \\\n  --port p1 \\\n  f1"
        );
        assert!(c(OsAction::PortForwardingUpdate { id: r("pf1"), spec }).ends_with("--port p1 \\\n  f1 \\\n  pf1"));
        assert_eq!(
            c(OsAction::PortForwardingDelete {
                floating_ip_id: r("f1"),
                id: r("pf1")
            }),
            "openstack floating ip port forwarding delete \\\n  f1 \\\n  pf1"
        );
    }

    fn lb_listener() -> LbListenerSpec {
        LbListenerSpec {
            name: Some("http".into()),
            description: None,
            protocol: "HTTP".into(),
            port: 80,
            connection_limit: None,
            options: LbListenerOptions {
                timeout_client_data: Some(60000),
                insert_headers: vec!["X-Forwarded-For".into()],
                allowed_cidrs: Some(vec!["192.0.2.0/24".into()]),
                ..Default::default()
            },
            default_tls_container_ref: None,
            sni_container_refs: vec![],
            admin_up: true,
        }
    }

    fn lb_pool() -> LbPoolSpec {
        LbPoolSpec {
            name: Some("web pool".into()),
            description: None,
            protocol: "HTTP".into(),
            algorithm: "ROUND_ROBIN".into(),
            session_persistence: Some("APP_COOKIE".into()),
            cookie_name: Some("sid".into()),
            tls_enabled: false,
            tls_ciphers: None,
            admin_up: true,
        }
    }

    fn lb_member() -> LbMemberSpec {
        LbMemberSpec {
            name: None,
            address: "10.0.0.5".into(),
            port: 8080,
            subnet_id: Some("sn1".into()),
            weight: Some(2),
            backup: true,
            monitor_address: None,
            monitor_port: None,
            admin_up: true,
        }
    }

    fn lb_monitor() -> LbMonitorSpec {
        LbMonitorSpec {
            name: None,
            kind: "HTTP".into(),
            delay: 5,
            timeout: 3,
            max_retries: 3,
            max_retries_down: None,
            http_method: Some("GET".into()),
            url_path: Some("/health".into()),
            expected_codes: Some("200".into()),
            admin_up: true,
        }
    }

    #[test]
    fn a_load_balancer_wizard_is_a_chain_through_variables() {
        let spec = LbCreateSpec {
            name: Some("lb1".into()),
            description: None,
            vip_subnet_id: "sn1".into(),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            admin_up: true,
            listener: Some(lb_listener()),
            pool: Some(lb_pool()),
            members: vec![lb_member()],
            monitor: Some(lb_monitor()),
        };
        let cmds = cli_equivalent(&OsAction::LoadBalancerCreate(Box::new(spec.clone())));
        assert_eq!(cmds.len(), 5);
        assert_eq!(
            cmds[0],
            "LB_ID=$(openstack loadbalancer create \\\n  --name lb1 \\\n  --vip-subnet-id sn1 \\\n  --wait \\\n  -f value -c id)"
        );
        assert!(
            cmds[1].starts_with("LISTENER_ID=$(openstack loadbalancer listener create"),
            "{}",
            cmds[1]
        );
        for want in [
            "--protocol HTTP",
            "--protocol-port 80",
            "--timeout-client-data 60000",
            "--insert-headers X-Forwarded-For=true",
            "--allowed-cidr 192.0.2.0/24",
            "\"$LB_ID\"",
        ] {
            assert!(cmds[1].contains(want), "{want} in {}", cmds[1]);
        }
        assert!(cmds[2].starts_with("POOL_ID=$("), "{}", cmds[2]);
        for want in [
            "--name 'web pool'",
            "--lb-algorithm ROUND_ROBIN",
            "--session-persistence type=APP_COOKIE,cookie_name=sid",
            "--listener \"$LISTENER_ID\"",
        ] {
            assert!(cmds[2].contains(want), "{want} in {}", cmds[2]);
        }
        assert_eq!(
            cmds[3],
            "openstack loadbalancer member create \\\n  --address 10.0.0.5 \\\n  --protocol-port 8080 \\\n  --subnet-id sn1 \\\n  --weight 2 \\\n  --enable-backup \\\n  --wait \\\n  \"$POOL_ID\""
        );
        assert!(
            cmds[4].contains("--type HTTP") && cmds[4].ends_with("\"$POOL_ID\""),
            "{}",
            cmds[4]
        );
        // Without a listener the pool hangs from the load balancer; a bare balancer is one plain command.
        let no_listener = LbCreateSpec {
            listener: None,
            ..spec.clone()
        };
        let c = cli_equivalent(&OsAction::LoadBalancerCreate(Box::new(no_listener)));
        assert!(c[1].contains("--loadbalancer \"$LB_ID\""), "{}", c[1]);
        let bare = LbCreateSpec {
            listener: None,
            pool: None,
            members: vec![],
            monitor: None,
            admin_up: false,
            ..spec
        };
        let c = cli_equivalent(&OsAction::LoadBalancerCreate(Box::new(bare)));
        assert_eq!(c.len(), 1);
        assert!(
            c[0].starts_with("openstack loadbalancer create") && c[0].contains("--disable"),
            "{}",
            c[0]
        );
    }

    #[test]
    fn octavia_parts_are_created_edited_and_deleted() {
        let c = |a: OsAction| cli_equivalent(&a).join("\n");
        let r = |x: &str| x.to_string();
        assert_eq!(
            c(OsAction::LoadBalancerDelete {
                id: r("lb1"),
                cascade: true
            }),
            "openstack loadbalancer delete \\\n  --cascade \\\n  --wait \\\n  lb1"
        );
        let pool = c(OsAction::PoolCreate {
            loadbalancer_id: r("lb1"),
            listener_id: None,
            spec: lb_pool(),
        });
        assert!(
            pool.contains("--loadbalancer lb1") && !pool.contains("--listener"),
            "{pool}"
        );
        let listener = c(OsAction::ListenerCreate {
            loadbalancer_id: r("lb1"),
            spec: lb_listener(),
            default_pool_id: Some(r("p1")),
        });
        assert!(
            listener.contains("--default-pool p1") && listener.ends_with("--wait \\\n  lb1"),
            "{listener}"
        );
        assert_eq!(
            c(OsAction::MemberCreate {
                pool_id: r("p1"),
                spec: lb_member()
            })
            .lines()
            .last()
            .unwrap(),
            "  p1"
        );
        assert_eq!(
            c(OsAction::MemberDelete {
                pool_id: r("p1"),
                id: r("m1")
            }),
            "openstack loadbalancer member delete \\\n  p1 \\\n  m1"
        );
        let mon = c(OsAction::MonitorUpdate {
            id: r("h1"),
            spec: lb_monitor(),
            admin_up: false,
        });
        assert!(
            mon.starts_with("openstack loadbalancer healthmonitor set")
                && mon.contains("--disable")
                && mon.ends_with("h1"),
            "{mon}"
        );
        let set = c(OsAction::PoolUpdate {
            id: r("p1"),
            name: r("web"),
            description: String::new(),
            algorithm: r("SOURCE_IP"),
            session_persistence: None,
            cookie_name: None,
            admin_up: true,
            protocol: r("HTTP"),
            tls: Some((true, Some(r("ECDHE")))),
        });
        for want in [
            "--disable-session-persistence",
            "--enable-tls",
            "--tls-ciphers ECDHE",
            "--enable",
        ] {
            assert!(set.contains(want), "{want} in {set}");
        }
        let ls = c(OsAction::ListenerUpdate {
            id: r("l1"),
            name: r("web"),
            description: String::new(),
            connection_limit: None,
            default_pool_id: None,
            admin_up: false,
            protocol: r("HTTP"),
            options: None,
            tls: None,
        });
        assert!(ls.contains("--connection-limit -1") && ls.contains("--disable"), "{ls}");
        assert_eq!(
            c(OsAction::PoolDelete { id: r("p1") }),
            "openstack loadbalancer pool delete \\\n  p1"
        );
        assert_eq!(
            c(OsAction::ListenerDelete { id: r("l1") }),
            "openstack loadbalancer listener delete \\\n  l1"
        );
        assert_eq!(
            c(OsAction::MonitorDelete { id: r("h1") }),
            "openstack loadbalancer healthmonitor delete \\\n  h1"
        );
    }

    #[test]
    fn other_actions_have_no_equivalent() {
        assert!(cli_equivalent(&OsAction::UserDelete { id: "u".into() }).is_empty());
        assert!(cli_equivalent(&OsAction::RoleCreate { name: "r".into() }).is_empty());
    }
}
