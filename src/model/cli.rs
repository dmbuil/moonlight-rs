//! The `openstack` CLI commands that do what an action does: the *CLI equivalent* of a result
//! dialog (`[ui] show_cli`). Covers instance, network, router and volume creation and the
//! deletion of instances, networks, routers and volumes (alone or as a batch); any other action
//! yields nothing. Secrets never appear: the admin password is a placeholder and the
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
        OsAction::ServerDelete { .. }
        | OsAction::NetworkDelete { .. }
        | OsAction::RouterDelete { .. }
        | OsAction::VolumeDelete { .. } => {
            delete_command(std::slice::from_ref(action)).into_iter().collect()
        }
        OsAction::Batch(items) => {
            let actions: Vec<OsAction> = items.iter().map(|(_, a)| a.clone()).collect();
            // A batch of deletions of one kind is one command with every ID.
            if let Some(c) = delete_command(&actions) {
                return vec![c];
            }
            actions.iter().flat_map(cli_equivalent).collect()
        }
        _ => vec![],
    }
}

/// `openstack <server|network|router|volume> delete <id>…` when every action deletes the same kind of
/// resource (the CLI takes several IDs); `None` otherwise, or for no action.
fn delete_id(a: &OsAction) -> Option<(&'static str, &str)> {
    match a {
        OsAction::ServerDelete { id } => Some(("server", id)),
        OsAction::NetworkDelete { id } => Some(("network", id)),
        OsAction::RouterDelete { id } => Some(("router", id)),
        OsAction::VolumeDelete { id } => Some(("volume", id)),
        _ => None,
    }
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
            OsAction::ServerStart { id: "s".into() },
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
            ["openstack volume create \\\n  --size 10 \\\n  --image img1 \\\n  --type ssd \\\n  --description 'my data' \\\n  data"]
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
    fn other_actions_have_no_equivalent() {
        assert!(cli_equivalent(&OsAction::ServerStart { id: "s".into() }).is_empty());
    }
}
