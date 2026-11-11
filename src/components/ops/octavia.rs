//! Load balancers (Octavia): listeners, pools, members, monitors, L7.

use super::*;

pub(super) fn u16_of(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u16>, String> {
    let bad = || format!("{}: 1-65535", t(lang, label));
    match v.num(k).map_err(|_| bad())? {
        None => Ok(None),
        Some(n) => u16::try_from(n).map(Some).map_err(|_| bad()),
    }
}

pub(super) fn u32_of(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u32>, String> {
    num_u32(v, k, label, lang)
}

/// Fixed choices. Optional selects get their *— None —* entry from the
/// select itself (`Combo`), so it is never added here.
pub(super) fn static_refs(items: &[(String, String)]) -> Source {
    Source::Static(items.to_vec())
}

/// Shown errors of a spec check: the field the user can fix, then the detail.
pub(super) fn invalid(lang: Lang, e: String) -> String {
    format!("{}: {e}", t(lang, K::InvalidSettings))
}

/// A listener's *Advanced* options (roadmap 54) behind one check box, as
/// octavia-dashboard has them: each only when the cloud's Octavia has it
/// (`needs::LB_*`). `headers_if`: when *Insert Headers* shows (HTTP listeners),
/// `None` for none. `when`: the condition of the check box itself (the wizard's
/// *Create Listener*). Starts open when `existing` sets any of them.
pub(super) fn listener_option_fields(
    lang: Lang,
    existing: Option<&LbListenerOptions>,
    headers_if: Option<fn(&Vals) -> bool>,
    ciphers_if: Option<fn(&Vals) -> bool>,
    when: Option<(&'static str, &'static str)>,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let c = caps();
    let o = existing.cloned().unwrap_or_default();
    let set = o
        != LbListenerOptions {
            allowed_cidrs: o.allowed_cidrs.clone().filter(|c| !c.is_empty()),
            ..Default::default()
        };
    let mut adv = Field::check("lb_advanced", tr(K::LbAdvanced), set).help(tr(K::LbAdvancedHelp));
    if let Some((f, want)) = when {
        adv = adv.when(f, want);
    }
    let mut fields = vec![adv];
    let ms = |x: Option<u32>| x.map(|n| n.to_string()).unwrap_or_default();
    if needs::LB_TIMEOUTS.met(&c) {
        for (name, label, help, value) in [
            // octavia-dashboard's order.
            (
                "timeout_client_data",
                K::TimeoutClientData,
                K::TimeoutClientDataHelp,
                o.timeout_client_data,
            ),
            (
                "timeout_tcp_inspect",
                K::TimeoutTcpInspect,
                K::TimeoutTcpInspectHelp,
                o.timeout_tcp_inspect,
            ),
            (
                "timeout_member_connect",
                K::TimeoutMemberConnect,
                K::TimeoutMemberConnectHelp,
                o.timeout_member_connect,
            ),
            (
                "timeout_member_data",
                K::TimeoutMemberData,
                K::TimeoutMemberDataHelp,
                o.timeout_member_data,
            ),
        ] {
            fields.push(
                Field::number(name, tr(label), 0)
                    .value(ms(value))
                    .help(tr(help))
                    .when("lb_advanced", "true"),
            );
        }
    }
    if let Some(cond) = headers_if {
        fields.push(
            Field::multi("insert_headers", tr(K::InsertHeaders), options_raw(&LB_INSERT_HEADERS))
                .value(o.insert_headers.join(","))
                .help(tr(K::InsertHeadersHelp))
                .when("lb_advanced", "true")
                .show_if(cond),
        );
    }
    // On an edit, only when the listener reports the key (Octavia >= 2.12 there).
    if needs::LB_ALLOWED_CIDRS.met(&c) && (existing.is_none() || o.allowed_cidrs.is_some()) {
        fields.push(
            Field::area("allowed_cidrs", tr(K::AllowedCidrs))
                .value(o.allowed_cidrs.unwrap_or_default().join("\n"))
                .help(tr(K::AllowedCidrsHelp))
                .when("lb_advanced", "true"),
        );
    }
    // `TERMINATED_HTTPS` listeners only (roadmap 54); named apart from the pool's
    // field, both being in the wizard.
    if let Some(cond) = ciphers_if.filter(|_| needs::LB_POOL_CIPHERS.met(&c)) {
        fields.push(
            Field::text("listener_ciphers", tr(K::TlsCiphers))
                .value(o.tls_ciphers.clone().unwrap_or_default())
                .help(tr(K::TlsCiphersHelp))
                .when("lb_advanced", "true")
                .show_if(cond),
        );
    }
    fields
}

/// Listener protocols for a new listener: `TERMINATED_HTTPS` too when the
/// `key-manager` plugin is on and Barbican is in the catalog (roadmap 54).
pub(super) fn listener_protocols() -> Source {
    let mut p = LB_PROTOCOLS.to_vec();
    if needs::LB_TLS_TERMINATION.met(&caps()) {
        p.push(LB_TLS_PROTOCOL);
    }
    options_raw(&p)
}

/// A `TERMINATED_HTTPS` listener's certificate and SNI certificates (roadmap
/// 54), from `ChoiceSet::LbCertificates`. `tls_if`: when they show (the new
/// listener's protocol); `None` on an edit, which only offers them for such a
/// listener.
pub(super) fn listener_certificate_fields(
    lang: Lang,
    existing: Option<&LbListener>,
    tls_if: Option<fn(&Vals) -> bool>,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let mut cert = Field::select(
        "certificate",
        tr(K::LbCertificate),
        Source::Set(ChoiceSet::LbCertificates),
    )
    .required()
    .value(
        existing
            .and_then(|l| l.default_tls_container_ref.clone())
            .unwrap_or_default(),
    )
    .help(tr(K::LbCertificateHelp));
    let mut sni = Field::multi("sni", tr(K::SniCertificates), Source::Set(ChoiceSet::LbCertificates))
        .value(existing.map(|l| l.sni_container_refs.join(",")).unwrap_or_default())
        .help(tr(K::SniCertificatesHelp));
    if let Some(cond) = tls_if {
        (cert, sni) = (cert.show_if(cond), sni.show_if(cond));
    }
    vec![cert, sni]
}

/// Whether a new listener's protocol (the wizard's or the dialog's field) is
/// `TERMINATED_HTTPS` / inserts headers.
fn wizard_tls(v: &Vals) -> bool {
    v.get("listener_protocol").map(String::as_str) == Some(LB_TLS_PROTOCOL)
}

fn wizard_headers(v: &Vals) -> bool {
    v.get("listener_protocol").is_some_and(|p| lb_inserts_headers(p))
}

pub(super) fn dialog_tls(v: &Vals) -> bool {
    v.get("protocol").map(String::as_str) == Some(LB_TLS_PROTOCOL)
}

pub(super) fn dialog_headers(v: &Vals) -> bool {
    v.get("protocol").is_some_and(|p| lb_inserts_headers(p))
}

/// The options typed in [`listener_option_fields`]; `None` when the box is
/// unticked (an edit then leaves them alone, a create keeps Octavia's defaults).
pub(super) fn listener_options_of(v: &Values, lang: Lang) -> Result<Option<LbListenerOptions>, String> {
    if !v.flag("lb_advanced") {
        return Ok(None);
    }
    Ok(Some(LbListenerOptions {
        timeout_client_data: u32_of(v, "timeout_client_data", K::TimeoutClientData, lang)?,
        timeout_member_connect: u32_of(v, "timeout_member_connect", K::TimeoutMemberConnect, lang)?,
        timeout_member_data: u32_of(v, "timeout_member_data", K::TimeoutMemberData, lang)?,
        timeout_tcp_inspect: u32_of(v, "timeout_tcp_inspect", K::TimeoutTcpInspect, lang)?,
        insert_headers: v.list("insert_headers"),
        allowed_cidrs: v.0.contains_key("allowed_cidrs").then(|| v.list("allowed_cidrs")),
        tls_ciphers: v.opt("listener_ciphers"),
    }))
}

/// *TLS Enabled* and *TLS Cipher String* of a pool (roadmap 54), when the
/// cloud's Octavia has them (2.8, 2.15).
pub(super) fn pool_tls_fields(
    lang: Lang,
    existing: Option<&LbPool>,
    when: Option<(&'static str, &'static str)>,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let c = caps();
    if !needs::LB_POOL_TLS.met(&c) {
        return vec![];
    }
    let mut on = Field::check(
        "tls_enabled",
        tr(K::PoolTlsEnabled),
        existing.is_some_and(|p| p.tls_enabled),
    )
    .help(tr(K::PoolTlsEnabledHelp));
    if let Some((f, want)) = when {
        on = on.when(f, want);
    }
    let mut fields = vec![on];
    if needs::LB_POOL_CIPHERS.met(&c) {
        fields.push(
            Field::text("tls_ciphers", tr(K::TlsCiphers))
                .value(existing.and_then(|p| p.tls_ciphers.clone()).unwrap_or_default())
                .help(tr(K::TlsCiphersHelp))
                .when("tls_enabled", "true"),
        );
    }
    fields
}

/// `(tls_enabled, tls_ciphers)` from [`pool_tls_fields`]; `None` when not offered.
pub(super) fn pool_tls_of(v: &Values) -> Option<(bool, Option<String>)> {
    v.0.contains_key("tls_enabled").then(|| {
        (
            v.flag("tls_enabled"),
            v.opt("tls_ciphers").filter(|_| v.flag("tls_enabled")),
        )
    })
}

/// Moves a listener's port between 80 and 443 when its protocol changes to or
/// from HTTPS / TERMINATED_HTTPS, unless the user typed another port (roadmap 54;
/// octavia-dashboard leaves the port empty instead).
pub(super) fn suggest_port(changed: &str, v: &mut Vals, protocol: &str, port: &str) {
    if changed != protocol {
        return;
    }
    let tls = matches!(v.get(protocol).map(String::as_str), Some("HTTPS" | LB_TLS_PROTOCOL));
    let next = match (v.get(port).map(String::as_str), tls) {
        (Some("80") | None, true) => "443",
        (Some("443"), false) => "80",
        _ => return,
    };
    v.insert(port.to_string(), next.to_string());
}

/// An *Admin State Up* box of a create form (roadmap 54): on unless shown
/// and unticked, so a form without it creates the resource up.
fn admin_up_of(v: &Values, k: &str) -> bool {
    !v.0.contains_key(k) || v.flag(k)
}

pub(super) fn persistence_of(v: &Values) -> (Option<String>, Option<String>) {
    let kind = v.opt("persistence");
    let cookie = v.opt("cookie_name").filter(|_| kind.as_deref() == Some("APP_COOKIE"));
    (kind, cookie)
}

pub(super) fn monitor_spec(v: &Values, lang: Lang, kind: String) -> Result<LbMonitorSpec, String> {
    let http = kind.starts_with("HTTP");
    let retries = |k: &str, label| -> Result<Option<u8>, String> {
        match v.num(k).map_err(|_| format!("{}: 1-10", t(lang, label)))? {
            None => Ok(None),
            Some(n) => u8::try_from(n)
                .map(Some)
                .map_err(|_| format!("{}: 1-10", t(lang, label))),
        }
    };
    Ok(LbMonitorSpec {
        name: v.opt("monitor_name"),
        delay: u32_of(v, "delay", K::DelaySeconds, lang)?.unwrap_or(5),
        timeout: u32_of(v, "timeout", K::TimeoutSeconds, lang)?.unwrap_or(5),
        max_retries: retries("max_retries", K::MaxRetries)?.unwrap_or(3),
        max_retries_down: retries("max_retries_down", K::MaxRetriesDown)?,
        http_method: v.opt("http_method").filter(|_| http),
        url_path: v.opt("url_path").filter(|_| http),
        expected_codes: v.opt("expected_codes").filter(|_| http),
        admin_up: admin_up_of(v, "monitor_admin_up"),
        kind,
    })
}

pub(super) fn monitor_fields(
    lang: Lang,
    when: Option<(&'static str, &'static str)>,
    m: Option<&LbMonitor>,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let w = |f: Field| match when {
        Some((a, b)) => f.when(a, b),
        None => f,
    };
    let http: fn(&Vals) -> bool = |v| v.get("monitor_type").is_some_and(|t| t.starts_with("HTTP"));
    let num = |n: Option<i64>, d: &str| n.map(|n| n.to_string()).unwrap_or_else(|| d.to_string());
    let mut out = vec![];
    if m.is_none() {
        out.push(w(Field::select(
            "monitor_type",
            tr(K::Type),
            options_raw(&LB_MONITOR_TYPES),
        )
        .value("HTTP")
        .required()));
    }
    out.extend([
        w(Field::text("monitor_name", tr(K::Name)).value(m.map(|m| m.name.clone()).unwrap_or_default())),
        w(Field::number("delay", tr(K::DelaySeconds), 1)
            .value(num(m.and_then(|m| m.delay), "5"))
            .required()),
        w(Field::number("timeout", tr(K::TimeoutSeconds), 1)
            .value(num(m.and_then(|m| m.timeout), "5"))
            .help(tr(K::TimeoutHelp))
            .required()),
        w(Field::number("max_retries", tr(K::MaxRetries), 1)
            .value(num(m.and_then(|m| m.max_retries), "3"))
            .required()),
        w(Field::number("max_retries_down", tr(K::MaxRetriesDown), 1)
            .value(num(m.and_then(|m| m.max_retries_down), "3"))),
    ]);
    // Edit: the type is fixed, so the HTTP fields follow the monitor's own type.
    let is_http = m.map(|m| m.kind.starts_with("HTTP"));
    let hf = |f: Field| match is_http {
        Some(true) => w(f),
        Some(false) => f.show_if(|_| false),
        None => w(f).show_if(http),
    };
    out.extend([
        hf(
            Field::select("http_method", tr(K::HttpMethod), options_raw(&LB_HTTP_METHODS))
                .value(m.and_then(|m| m.http_method.clone()).unwrap_or_else(|| "GET".into())),
        ),
        hf(Field::text("url_path", tr(K::UrlPath))
            .value(m.and_then(|m| m.url_path.clone()).unwrap_or_else(|| "/".into()))),
        hf(Field::text("expected_codes", tr(K::ExpectedCodes))
            .value(m.and_then(|m| m.expected_codes.clone()).unwrap_or_else(|| "200".into()))
            .help(tr(K::ExpectedCodesHelp))),
    ]);
    // A new monitor; an edit has its own *Enable Admin State* box (roadmap 54).
    if m.is_none() {
        out.push(w(Field::check("monitor_admin_up", tr(K::EnableAdminState), true)));
    }
    out
}

/// Horizon's *Create Load Balancer* wizard: details, then an optional listener,
/// its default pool, members and health monitor. Sent as one fully populated
/// request, so Octavia creates everything or nothing.
pub fn create_load_balancer_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let listener: fn(&Vals) -> bool = |v| v.get("with_listener").map(String::as_str) == Some("true");
    let pool: fn(&Vals) -> bool = |v| {
        v.get("with_listener").map(String::as_str) == Some("true")
            && v.get("with_pool").map(String::as_str) == Some("true")
    };
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::text("description", tr(K::Description)),
        Field::select("vip_subnet", tr(K::Subnet), Source::Set(ChoiceSet::LbVipSubnets)).required(),
        Field::text("vip_address", tr(K::IpAddress)).help(tr(K::VipAddressHelp)),
        Field::select("flavor", tr(K::Flavor), Source::Set(ChoiceSet::LbFlavors)),
        // octavia-dashboard's details step (roadmap 54); empty = the cloud's default.
        Field::select(
            "lb_az",
            tr(K::AvailabilityZone),
            Source::Set(ChoiceSet::LbAvailabilityZones),
        ),
        Field::check("lb_admin_up", tr(K::EnableAdminState), true),
        Field::yes_no("with_listener", tr(K::CreateListener), true),
        Field::select("listener_protocol", tr(K::Protocol), listener_protocols())
            .value("HTTP")
            .required()
            .when("with_listener", "true"),
        Field::number("listener_port", tr(K::Port), 1)
            .value("80")
            .required()
            .when("with_listener", "true"),
    ];
    // Right after the port, as in octavia-dashboard's listener form (roadmap 54).
    fields.extend(
        listener_certificate_fields(lang, None, Some(wizard_tls))
            .into_iter()
            .map(|f| f.when("with_listener", "true")),
    );
    fields.extend([
        Field::text("listener_name", tr(K::Name)).when("with_listener", "true"),
        Field::number("connection_limit", tr(K::ConnectionLimit), 1)
            .help(tr(K::UnlimitedWhenEmpty))
            .when("with_listener", "true"),
        Field::check("listener_admin_up", tr(K::EnableAdminState), true).when("with_listener", "true"),
    ]);
    fields.extend(listener_option_fields(
        lang,
        None,
        Some(wizard_headers),
        Some(wizard_tls),
        Some(("with_listener", "true")),
    ));
    fields.extend([
        Field::yes_no("with_pool", tr(K::CreatePool), true).when("with_listener", "true"),
        // Octavia's single-call create rejects an unnamed default pool.
        Field::text("pool_name", tr(K::Name))
            .value("pool-1")
            .required()
            .when("with_pool", "true"),
        Field::select("algorithm", tr(K::Algorithm), options_raw(&LB_ALGORITHMS))
            .value("ROUND_ROBIN")
            .required()
            .when("with_pool", "true"),
        Field::select("persistence", tr(K::SessionPersistence), options_raw(&LB_PERSISTENCE)).when("with_pool", "true"),
        Field::text("cookie_name", tr(K::CookieName)).when("persistence", "APP_COOKIE"),
        Field::check("pool_admin_up", tr(K::EnableAdminState), true).when("with_pool", "true"),
        // One row per member, new rows on the listener's port (roadmap 54).
        Field::member_rows("members", tr(K::Members), &[], Some("listener_port"), vec![]).when("with_pool", "true"),
        Field::yes_no("with_monitor", tr(K::CreateHealthMonitor), true).when("with_pool", "true"),
    ]);
    fields.extend(pool_tls_fields(lang, None, Some(("with_pool", "true"))));
    fields.extend(monitor_fields(lang, Some(("with_monitor", "true")), None));
    let steps = vec![
        Step::new(
            tr(K::LoadBalancer),
            &[
                "name",
                "description",
                "vip_subnet",
                "vip_address",
                "flavor",
                "lb_az",
                "lb_admin_up",
            ],
        )
        .help(tr(K::LbDetailsHelp))
        .required(),
        Step::new(
            tr(K::Listener),
            &[
                "with_listener",
                "listener_protocol",
                "listener_port",
                "certificate",
                "sni",
                "listener_name",
                "connection_limit",
                "listener_admin_up",
                "lb_advanced",
                "timeout_client_data",
                "timeout_tcp_inspect",
                "timeout_member_connect",
                "timeout_member_data",
                "insert_headers",
                "allowed_cidrs",
                "listener_ciphers",
            ],
        )
        .help(tr(K::ListenerHelp)),
        Step::new(
            tr(K::Pool),
            &[
                "with_pool",
                "pool_name",
                "algorithm",
                "persistence",
                "cookie_name",
                "pool_admin_up",
                "tls_enabled",
                "tls_ciphers",
            ],
        )
        .help(tr(K::PoolHelp))
        .enabled_if(listener),
        Step::new(tr(K::Members), &["members"])
            .help(tr(K::MembersHelp))
            .enabled_if(pool),
        Step::new(
            tr(K::HealthMonitor),
            &[
                "with_monitor",
                "monitor_type",
                "monitor_name",
                "delay",
                "timeout",
                "max_retries",
                "max_retries_down",
                "http_method",
                "url_path",
                "expected_codes",
                "monitor_admin_up",
            ],
        )
        .help(tr(K::MonitorHelp))
        .enabled_if(pool),
    ];
    Dialog::form(tr(K::CreateLoadBalancer), tr(K::CreateLoadBalancer), fields, move |v| {
        let with_listener = v.flag("with_listener");
        let with_pool = with_listener && v.flag("with_pool");
        let protocol = v.get("listener_protocol");
        let listener = if with_listener {
            Some(LbListenerSpec {
                name: v.opt("listener_name"),
                description: None,
                protocol: protocol.clone(),
                port: u16_of(v, "listener_port", K::Port, lang)?
                    .ok_or_else(|| format!("{}: {}", t(lang, K::Port), t(lang, K::RequiredField)))?,
                connection_limit: u32_of(v, "connection_limit", K::ConnectionLimit, lang)?,
                options: listener_options_of(v, lang)?.unwrap_or_default(),
                default_tls_container_ref: v.opt("certificate").filter(|_| protocol == LB_TLS_PROTOCOL),
                sni_container_refs: if protocol == LB_TLS_PROTOCOL {
                    v.list("sni")
                } else {
                    vec![]
                },
                admin_up: admin_up_of(v, "listener_admin_up"),
            })
        } else {
            None
        };
        let (persistence, cookie) = persistence_of(v);
        let (tls_enabled, tls_ciphers) = pool_tls_of(v).unwrap_or_default();
        let pool = with_pool.then(|| LbPoolSpec {
            name: v.opt("pool_name"),
            description: None,
            protocol: pool_protocol_for(&protocol).to_string(),
            algorithm: v.get("algorithm"),
            session_persistence: persistence,
            cookie_name: cookie,
            tls_enabled,
            tls_ciphers,
            admin_up: admin_up_of(v, "pool_admin_up"),
        });
        let members = if with_pool {
            member_specs(&v.get("members")).map_err(|e| invalid(lang, e))?
        } else {
            vec![]
        };
        let monitor = if with_pool && v.flag("with_monitor") {
            Some(monitor_spec(v, lang, v.get("monitor_type"))?)
        } else {
            None
        };
        let spec = LbCreateSpec {
            name: v.opt("name"),
            description: v.opt("description"),
            vip_subnet_id: req(v, "vip_subnet", K::Subnet, lang)?,
            vip_address: v.opt("vip_address"),
            flavor_id: v.opt("flavor"),
            availability_zone: v.opt("lb_az"),
            admin_up: admin_up_of(v, "lb_admin_up"),
            listener,
            pool,
            members,
            monitor,
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        Ok(OsAction::LoadBalancerCreate(Box::new(spec)))
    })
    .on_change(|f, v, _| suggest_port(f, v, "listener_protocol", "listener_port"))
    .wizard(steps)
}

/// Header and row menu of a load balancer. `fip`: its VIP's floating IP
/// `(id, address)`, known on the detail page only.
pub fn load_balancer_items(lang: Lang, l: &LoadBalancerRow) -> Vec<MenuItem> {
    load_balancer_menu(lang, l, None)
}

/// The load balancer detail header's menu.
pub fn load_balancer_detail_items(lang: Lang, d: &LoadBalancerDetail) -> Vec<MenuItem> {
    load_balancer_menu(lang, &d.row, Some(d))
}

fn load_balancer_menu(lang: Lang, l: &LoadBalancerRow, d: Option<&LoadBalancerDetail>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    let (i, n, desc, up) = (
        l.id.clone(),
        l.name.clone(),
        l.description.clone().unwrap_or_default(),
        l.admin_up,
    );
    items.push(MenuItem::dialog(t(lang, K::EditLoadBalancer), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditLoadBalancer),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()),
                Field::text("description", t(lang, K::Description)).value(desc.clone()),
                Field::check("admin_up", t(lang, K::EnableAdminState), up),
            ],
            move |v| {
                Ok(OsAction::LoadBalancerUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    admin_up: v.flag("admin_up"),
                })
            },
        )
    }));
    if let Some(d) = d {
        match (d.floating_ip_id.clone(), d.vip_port_id.clone()) {
            (Some(fip), _) => {
                let ip = d.floating_ip.clone().unwrap_or_default();
                items.push(MenuItem::danger(t(lang, K::DisassociateFloatingIp), move || {
                    Dialog::confirm(
                        lang,
                        t(lang, K::DisassociateFloatingIp),
                        ip.clone(),
                        OsAction::FloatingIpDisassociate { id: fip.clone() },
                    )
                }));
            }
            (None, Some(port)) => {
                items.push(MenuItem::dialog(t(lang, K::AssociateFloatingIp), move || {
                    let port = port.clone();
                    Dialog::form(
                        t(lang, K::AssociateFloatingIp),
                        t(lang, K::Associate),
                        vec![
                            Field::select("fip", t(lang, K::FloatingIp), Source::Set(ChoiceSet::FreeFloatingIps))
                                .required(),
                        ],
                        move |v| {
                            Ok(OsAction::FloatingIpAssociate {
                                id: req(v, "fip", K::FloatingIp, lang)?,
                                port_id: port.clone(),
                            })
                        },
                    )
                }));
            }
            _ => {}
        }
    }
    let (i, n) = (l.id.clone(), l.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteLoadBalancer), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::DeleteLoadBalancer),
            n.clone(),
            OsAction::LoadBalancerDelete {
                id: i.clone(),
                cascade: true,
            },
        );
        d.intro.insert(1, t(lang, K::LbCascade).into());
        d
    }));
    items
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn load_balancer_wizard_builds_one_spec_and_skips_what_is_switched_off() {
        let d = create_load_balancer_dialog(Lang::EnUs);
        assert_eq!(d.steps.len(), 5);
        let full = vals(&[
            ("name", "web-lb"),
            ("vip_subnet", "sub-1"),
            ("with_listener", "true"),
            ("listener_protocol", "HTTPS"),
            ("listener_port", "443"),
            ("with_pool", "true"),
            ("pool_name", "web-pool"),
            ("algorithm", "LEAST_CONNECTIONS"),
            ("persistence", ""),
            (
                "members",
                r#"[{"address":"10.0.0.11","subnet_id":"sub-1","label":"web-1: 10.0.0.11","port":"8443","weight":"1"},
                           {"address":"192.0.2.10","subnet_id":"sub-ext","port":"9443","weight":"","backup":true,"monitor_port":"9000"}]"#,
            ),
            ("with_monitor", "true"),
            ("monitor_type", "TCP"),
            ("delay", "5"),
            ("timeout", "3"),
            ("max_retries", "3"),
            ("http_method", "GET"),
            ("url_path", "/"),
            ("expected_codes", "200"),
        ]);
        let OsAction::LoadBalancerCreate(spec) = (d.build)(&full).unwrap() else {
            panic!("not a create")
        };
        assert_eq!(
            spec.pool.as_ref().map(|p| p.protocol.as_str()),
            Some("HTTPS"),
            "pool protocol follows the listener"
        );
        assert_eq!(spec.pool.as_ref().and_then(|p| p.name.as_deref()), Some("web-pool"));
        let field = |n: &str| d.fields.iter().find(|f| f.name == n).unwrap();
        assert_eq!(
            (field("pool_name").default.as_str(), field("pool_name").required),
            ("pool-1", true),
            "Octavia needs a pool name"
        );
        let Kind::Select(Source::Static(persistence)) = &field("persistence").kind else {
            panic!("not a static select")
        };
        assert!(
            persistence.iter().all(|(v, _)| !v.is_empty()),
            "the select adds *— None —* itself"
        );
        let mut unnamed = full.clone();
        unnamed.0.insert("pool_name".into(), String::new());
        assert!(
            (d.build)(&unnamed).is_err(),
            "an empty pool name is refused before sending"
        );
        assert_eq!(spec.members.len(), 2);
        assert_eq!(
            (
                spec.members[0].address.as_str(),
                spec.members[0].port,
                spec.members[0].subnet_id.as_deref()
            ),
            ("10.0.0.11", 8443, Some("sub-1"))
        );
        let ext = &spec.members[1];
        assert_eq!(
            (ext.address.as_str(), ext.port, ext.weight, ext.backup, ext.monitor_port),
            ("192.0.2.10", 9443, None, true, Some(9000)),
            "an external member with its own port (roadmap 54)"
        );
        assert!(ext.admin_up, "a row without admin state is enabled");
        let mut twice = full.clone();
        twice.0.insert(
            "members".into(),
            r#"[{"address":"10.0.0.11","port":"80","weight":"1"},{"address":"10.0.0.11","port":"80","weight":"1"}]"#
                .into(),
        );
        assert!((d.build)(&twice).unwrap_err().contains("listed twice"));
        let m = spec.monitor.unwrap();
        assert_eq!(
            (m.kind.as_str(), m.http_method.clone()),
            ("TCP", None),
            "HTTP fields only for HTTP monitors"
        );

        let bare = vals(&[
            ("vip_subnet", "sub-1"),
            ("with_listener", "false"),
            ("with_pool", "true"),
            ("members", r#"[{"address":"10.0.0.11","port":"80","weight":"1"}]"#),
        ]);
        let OsAction::LoadBalancerCreate(spec) = (d.build)(&bare).unwrap() else {
            panic!("not a create")
        };
        assert!(spec.listener.is_none() && spec.pool.is_none() && spec.members.is_empty() && spec.monitor.is_none());
        assert!(
            (d.build)(&vals(&[("with_listener", "false")])).is_err(),
            "a subnet is required"
        );
    }

    /// Roadmap 54: TLS listeners move the default port to 443 and back, but a
    /// typed port stays.
    #[test]
    fn the_port_follows_the_protocol_until_typed() {
        let mut v: Vals = [
            ("listener_protocol".to_string(), "TERMINATED_HTTPS".to_string()),
            ("listener_port".to_string(), "80".to_string()),
        ]
        .into();
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "443");
        v.insert("listener_protocol".into(), "TCP".into());
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "80");
        v.insert("listener_port".into(), "8443".into());
        v.insert("listener_protocol".into(), "HTTPS".into());
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "8443", "typed: kept");
        suggest_port("listener_name", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "8443");
    }
}
