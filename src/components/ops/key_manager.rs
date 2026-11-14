//! Key Manager (Barbican), orders included.

use super::*;

/// Barbican's `secret_type` values, in the order *Store Secret* offers them.
const SECRET_TYPES: &[&str] = &["opaque", "passphrase", "symmetric", "certificate", "private", "public"];

/// The role fields of *Create Container*, per type: (field, role). A role in
/// two types gets a field for each, since a field shows for one value only.
const CONTAINER_ROLE_FIELDS: &[(&str, &str, &str)] = &[
    ("certificate", "cert_certificate", "certificate"),
    ("certificate", "cert_private_key", "private_key"),
    ("certificate", "cert_passphrase", "private_key_passphrase"),
    ("certificate", "cert_intermediates", "intermediates"),
    ("rsa", "rsa_public_key", "public_key"),
    ("rsa", "rsa_private_key", "private_key"),
    ("rsa", "rsa_passphrase", "private_key_passphrase"),
];

/// Barbican terms as select choices: the API value, labelled in the user's language.
fn km_options(lang: Lang, items: &[&str]) -> Source {
    Source::Static(
        items
            .iter()
            .map(|v| (v.to_string(), crate::i18n::key_manager_term(lang, v)))
            .collect(),
    )
}

/// *Store Secret* (Barbican `POST /v1/secrets`): name, type, payload (text, or
/// base64 for binary; empty = metadata only), algorithm, bit length, mode and
/// expiration (UTC). A file picker for binary payloads is not offered yet.
pub fn store_secret_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).help(tr(K::SecretNameHelp)),
        Field::select("secret_type", tr(K::SecretType), km_options(lang, SECRET_TYPES))
            .value("opaque")
            .required()
            .help(tr(K::SecretTypeHelp)),
        Field::select(
            "format",
            tr(K::PayloadFormat),
            options(
                &[
                    ("text/plain", K::PayloadText),
                    ("application/octet-stream", K::PayloadBase64),
                ],
                lang,
            ),
        )
        .value("text/plain")
        .help(tr(K::PayloadFormatHelp)),
        Field::area("payload", tr(K::Payload)).help(tr(K::PayloadHelp)),
        Field::text("algorithm", tr(K::Algorithm)).help(tr(K::AlgorithmHelp)),
        Field::number("bit_length", tr(K::BitLength), 1).help(tr(K::BitLengthHelp)),
        Field::text("mode", tr(K::CipherMode)).help(tr(K::CipherModeHelp)),
        // One value, so on one line: date left, time right.
        Field::date("expiration_date", tr(K::ExpirationDate), true).half(),
        Field::time("expiration_time", tr(K::ExpirationTime))
            .half()
            .help(tr(K::SecretExpirationHelp)),
    ];
    Dialog::form(tr(K::StoreSecret), tr(K::StoreSecret), fields, move |v| {
        let now = chrono::Utc::now().naive_utc();
        let expiration = app_credential_expiry(&v.get("expiration_date"), &v.get("expiration_time"), now)
            .map_err(|e| invalid(lang, e))?;
        Ok(OsAction::SecretCreate(SecretSpec {
            name: v.opt("name"),
            secret_type: req(v, "secret_type", K::SecretType, lang)?,
            // Not trimmed: whitespace can be part of a secret.
            payload: Some(v.get("payload")).filter(|p| !p.is_empty()).map(Hidden),
            payload_content_type: v.get("format"),
            algorithm: v.opt("algorithm"),
            bit_length: v.num("bit_length").map_err(|e| invalid(lang, e))?,
            mode: v.opt("mode"),
            expiration,
        }))
    })
    .sized(DialogSize::Medium)
}

/// A secret row's menu: *Delete Secret* (its consumers are only known on the
/// detail page, so the dialog warns about them in general).
pub fn secret_items(lang: Lang, x: &SecretRow) -> Vec<MenuItem> {
    vec![delete_secret(lang, x, None)]
}

/// The detail page's menu: *Edit Access* and *Reset Access*, then
/// *Delete Secret*, listing the secret's consumers.
pub fn secret_detail_items(lang: Lang, d: &SecretDetail) -> Vec<MenuItem> {
    let id = d.secret.id.clone();
    let target = move |project_access, users| OsAction::SecretAclSet {
        id: id.clone(),
        project_access,
        users,
    };
    let reset = OsAction::SecretAclDelete {
        id: d.secret.id.clone(),
    };
    let mut items = acl_items(lang, &d.secret.name, &d.acl, target, reset, None);
    items.push(delete_secret(lang, &d.secret, d.consumers.clone()));
    items
}

/// The container detail page's menu: *Edit Access* and *Reset Access* (the
/// dialog reminds that the secrets inside have their own ACLs), then
/// *Delete Container*.
pub fn container_detail_items(lang: Lang, d: &ContainerDetail) -> Vec<MenuItem> {
    let c = &d.container;
    let id = c.id.clone();
    let target = move |project_access, users| OsAction::ContainerAclSet {
        id: id.clone(),
        project_access,
        users,
    };
    let reset = OsAction::ContainerAclDelete { id: c.id.clone() };
    let mut items = acl_items(lang, &c.name, &d.acl, target, reset, Some(K::ContainerAclSecrets));
    items.extend(container_items(lang, c));
    items
}

/// *Edit Access* and *Reset Access* for whoever may change the ACL (the
/// creator or an admin, `KmAcl::can_edit`); *Reset* only when an ACL is set.
/// Both warn first when something uses the resource ([`KmAcl::in_use`]).
fn acl_items(
    lang: Lang,
    name: &str,
    acl: &KmAcl,
    set: impl Fn(bool, Vec<String>) -> OsAction + Send + Sync + 'static,
    reset: OsAction,
    note: Option<K>,
) -> Vec<MenuItem> {
    if !acl.can_edit {
        return vec![];
    }
    let warning = in_use_warning(lang, acl);
    let mut items = vec![edit_access(lang, acl, set, warning.clone(), note)];
    if acl.is_set() {
        items.push(reset_access(lang, name, acl, reset, warning));
    }
    items
}

/// When something uses the resource (the *Consumers* section of its page lists
/// what): removing a service's user (Octavia's) from the ACL breaks it.
fn in_use_warning(lang: Lang, acl: &KmAcl) -> Option<String> {
    acl.in_use()
        .then(|| format!("{} {}", t(lang, K::AclInUse), t(lang, K::AclInUseWarning)))
}

/// *Edit Access*: whether every project member can read it, and which other
/// users can. Sent whole (`PUT …/acl`); *Nothing changed* when untouched.
fn edit_access(
    lang: Lang,
    acl: &KmAcl,
    set: impl Fn(bool, Vec<String>) -> OsAction + Send + Sync + 'static,
    warning: Option<String>,
    note: Option<K>,
) -> MenuItem {
    let (before_access, before_users) = (acl.project_access.unwrap_or(true), acl.users.clone());
    let set = std::sync::Arc::new(set);
    MenuItem::dialog(t(lang, K::EditAccess), move || {
        let tr = |k| t(lang, k).to_string();
        let fields = vec![
            Field::check("project_access", tr(K::ProjectAccessField), before_access),
            Field::area("users", tr(K::AclUsersField))
                .value(before_users.join("\n"))
                .help(tr(K::AclUsersHelp)),
        ];
        let (set, before_users) = (set.clone(), before_users.clone());
        let mut d = Dialog::form(tr(K::EditAccess), tr(K::Save), fields, move |v| {
            let (project_access, users) = (
                v.flag("project_access"),
                parse_user_ids(&v.opt("users").unwrap_or_default()),
            );
            if project_access == before_access && users == before_users {
                return Err(t(lang, K::NothingChanged).to_string());
            }
            Ok(set(project_access, users))
        });
        d.intro = warning.iter().cloned().chain(note.map(tr)).collect();
        d
    })
}

/// *Reset Access*: Barbican's default again; lists the users who lose access.
fn reset_access(lang: Lang, name: &str, acl: &KmAcl, reset: OsAction, warning: Option<String>) -> MenuItem {
    let users: Vec<String> = acl.users.iter().map(|u| acl.user_label(u)).collect();
    let name = name.to_string();
    MenuItem::danger(t(lang, K::ResetAccess), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::ResetAccess), name.clone(), reset.clone());
        // "You have selected:" stays first: the dialog lists the selection under it.
        d.intro = vec![t(lang, K::YouSelected).into(), t(lang, K::ResetAccessIntro).into()];
        if !users.is_empty() {
            d.intro
                .push(format!("{}: {}", t(lang, K::AclUsersLoseAccess), users.join(", ")));
        }
        d.intro.extend(warning.clone());
        d
    })
}

/// *Delete Secret* (`OsAction::SecretDelete`). The confirmation lists `consumers` when known (the
/// detail page), and warns in general when `None` (a list row, which does not fetch them).
fn delete_secret(lang: Lang, x: &SecretRow, consumers: Option<Vec<SecretConsumer>>) -> MenuItem {
    let (i, n) = (x.id.clone(), x.name.clone());
    MenuItem::danger(t(lang, K::DeleteSecret), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::DeleteSecret),
            n.clone(),
            OsAction::SecretDelete { id: i.clone() },
        );
        match consumers.as_deref() {
            Some([]) => {}
            Some(cs) => {
                let list: Vec<String> = cs
                    .iter()
                    .map(|c| format!("{} {} {}", c.service, c.resource_type, c.resource_id))
                    .collect();
                d.intro
                    .push(format!("{}: {}", t(lang, K::SecretConsumersOf), list.join(", ")));
            }
            None => d.intro.push(t(lang, K::SecretInUse).into()),
        }
        d
    })
}

/// The (role, `secret_ref`) pairs of *Create Container*: one per role field of
/// the type, or, for `generic`, each picked secret as `secret_1`, `secret_2`…
/// (Barbican's generic roles are free names).
pub fn container_pairs(
    container_type: &str,
    get: impl Fn(&str) -> Option<String>,
    generic: Vec<String>,
) -> Vec<(String, String)> {
    if container_type == "generic" {
        return generic
            .into_iter()
            .enumerate()
            .map(|(n, r)| (format!("secret_{}", n + 1), r))
            .collect();
    }
    CONTAINER_ROLE_FIELDS
        .iter()
        .filter(|(ty, _, _)| *ty == container_type)
        .filter_map(|(_, field, role)| get(field).map(|r| (role.to_string(), r)))
        .collect()
}

/// *Create Container*: a name, a type, and secrets per role (the server checks
/// the roles the type needs).
pub fn create_container_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::select(
            "container_type",
            tr(K::ContainerType),
            km_options(lang, &["certificate", "rsa", "generic"]),
        )
        .value("certificate")
        .required(),
        Field::multi("generic", tr(K::Secrets), Source::Set(ChoiceSet::Secrets)).when("container_type", "generic"),
    ];
    for &(ty, field, role) in CONTAINER_ROLE_FIELDS {
        let f = Field::select(
            field,
            crate::i18n::key_manager_term(lang, role),
            Source::Set(ChoiceSet::Secrets),
        )
        .when("container_type", ty);
        fields.push(
            if role == "certificate" || (ty == "rsa" && role != "private_key_passphrase") {
                f.required()
            } else {
                f
            },
        );
    }
    let mut d = Dialog::form(tr(K::CreateContainer), tr(K::CreateContainer), fields, move |v| {
        let ty = req(v, "container_type", K::ContainerType, lang)?;
        Ok(OsAction::ContainerCreate {
            name: v.opt("name"),
            secrets: container_pairs(&ty, |f| v.opt(f), v.list("generic")),
            container_type: ty,
        })
    })
    .sized(DialogSize::Medium);
    d.intro = vec![tr(K::ContainerSecretsHelp)];
    d
}

/// A container's menu: *Delete Container* (its secrets stay).
pub fn container_items(lang: Lang, c: &ContainerRow) -> Vec<MenuItem> {
    let (i, n) = (c.id.clone(), c.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteContainer), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::DeleteContainer),
            n.clone(),
            OsAction::ContainerDelete { id: i.clone() },
        );
        d.intro.push(t(lang, K::ContainerKeepsSecrets).into());
        d
    })]
}

/// *Order Key* (Barbican `POST /v1/orders`): Barbican generates
/// an AES key (a secret) or an RSA key pair (an `rsa` container), so the
/// material never goes through the browser. Bit lengths are those Barbican's
/// simple crypto plugin makes ([`ORDER_TYPES`]); one field per type, since a
/// field's options can't follow another field.
pub fn order_key_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let bits = |order_type: &str| -> Vec<&'static str> {
        let all: &[(&str, &[&'static str])] = &[
            ("key", &["128", "192", "256"]),
            ("asymmetric", &["2048", "3072", "4096"]),
        ];
        all.iter()
            .find(|(t, _)| *t == order_type)
            .map(|(_, b)| b.to_vec())
            .unwrap_or_default()
    };
    let fields = vec![
        Field::select("order_type", tr(K::OrderType), km_options(lang, &["key", "asymmetric"]))
            .value("key")
            .required()
            .help(tr(K::OrderTypeHelp)),
        Field::text("name", tr(K::Name)).help(tr(K::OrderNameHelp)),
        Field::select("key_bits", tr(K::BitLength), options_raw(&bits("key")))
            .value("256")
            .required()
            .when("order_type", "key")
            .half(),
        Field::text("mode", tr(K::CipherMode))
            .value("cbc")
            .when("order_type", "key")
            .half()
            .help(tr(K::OrderModeHelp)),
        Field::select("rsa_bits", tr(K::BitLength), options_raw(&bits("asymmetric")))
            .value("2048")
            .required()
            .when("order_type", "asymmetric")
            .help(tr(K::OrderBitLengthHelp)),
        Field::password("passphrase", tr(K::OrderPassphrase))
            .when("order_type", "asymmetric")
            .help(tr(K::OrderPassphraseHelp)),
        Field::date("expiration_date", tr(K::ExpirationDate), true).half(),
        Field::time("expiration_time", tr(K::ExpirationTime))
            .half()
            .help(tr(K::OrderExpirationHelp)),
    ];
    let mut d = Dialog::form(tr(K::OrderKey), tr(K::OrderKey), fields, move |v| {
        let order_type = req(v, "order_type", K::OrderType, lang)?;
        let pair = order_type == "asymmetric";
        let bits_field = if pair { "rsa_bits" } else { "key_bits" };
        let bit_length = v
            .num(bits_field)
            .map_err(|e| invalid(lang, e))?
            .ok_or_else(|| invalid(lang, "a bit length is required".into()))?;
        let now = chrono::Utc::now().naive_utc();
        let expiration = app_credential_expiry(&v.get("expiration_date"), &v.get("expiration_time"), now)
            .map_err(|e| invalid(lang, e))?;
        let spec = OrderSpec {
            name: v.opt("name"),
            bit_length,
            mode: if pair { None } else { v.opt("mode") },
            expiration,
            // Not trimmed: spaces can be part of a passphrase.
            passphrase: v
                .0
                .get("passphrase")
                .filter(|p| pair && !p.is_empty())
                .cloned()
                .map(Hidden),
            order_type,
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        Ok(OsAction::OrderCreate(spec))
    })
    .sized(DialogSize::Medium);
    d.intro = vec![tr(K::OrderIntro)];
    d
}

/// An order's menu, row and detail page alike: *Delete Order*, which removes
/// the record only (Barbican keeps what it generated; the dialog says so).
pub fn order_items(lang: Lang, o: &OrderRow) -> Vec<MenuItem> {
    let (i, n) = (o.id.clone(), o.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteOrder), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::DeleteOrder),
            n.clone(),
            OsAction::OrderDelete { id: i.clone() },
        );
        d.intro.push(t(lang, K::OrderKeepsResult).into());
        d
    })]
}

// --- Orchestration (Heat) -----------------------------------------

/// A stack's menu, row and detail header alike: *Change Stack Template* (a page), *Check*,
/// *Suspend* or *Resume* (whichever applies), *Delete*. Heat refuses actions while it works on the
/// stack (`*_IN_PROGRESS`), so only *Delete* is offered then.
pub fn stack_items(lang: Lang, x: &StackRow) -> Vec<MenuItem> {
    let (name, id) = (x.name.clone(), x.id.clone());
    let simple = |label: K, make: fn(String, String) -> OsAction| {
        let (n, i) = (name.clone(), id.clone());
        MenuItem::dialog(t(lang, label), move || {
            Dialog::simple(lang, t(lang, label), n.clone(), make(n.clone(), i.clone()))
        })
    };
    let mut items = vec![];
    if !stack_busy(&x.status) {
        items.push(MenuItem::link(
            t(lang, K::ChangeStackTemplate),
            format!("/project/stacks/{}/change", x.id),
        ));
        items.push(simple(K::CheckStack, |name, id| OsAction::StackCheck { name, id }));
        if stack_suspended(&x.status) {
            items.push(simple(K::ResumeStack, |name, id| OsAction::StackResume { name, id }));
        } else {
            items.push(simple(K::SuspendStack, |name, id| OsAction::StackSuspend { name, id }));
        }
    }
    items.push(MenuItem::danger(t(lang, K::DeleteStack), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::DeleteStack),
            name.clone(),
            OsAction::StackDelete {
                name: name.clone(),
                id: id.clone(),
            },
        );
        d.intro.push(t(lang, K::StackDeletesResources).into());
        d
    }));
    items
}

#[cfg(test)]
mod key_manager_tests {
    use super::*;

    #[test]
    fn order_key_builds_a_key_or_a_key_pair() {
        use super::super::testing::vals;
        let d = order_key_dialog(Lang::EnUs);
        let key = (d.build)(&vals(&[
            ("order_type", "key"),
            ("name", " db-key "),
            ("key_bits", "256"),
            ("mode", "cbc"),
            ("rsa_bits", "2048"),
            ("passphrase", "ignored"),
        ]))
        .unwrap();
        assert_eq!(
            key,
            OsAction::OrderCreate(OrderSpec {
                order_type: "key".into(),
                name: Some("db-key".into()),
                bit_length: 256,
                mode: Some("cbc".into()),
                expiration: None,
                passphrase: None,
            }),
            "a key takes its own size and mode; the key pair's fields are dropped"
        );
        let pair = (d.build)(&vals(&[
            ("order_type", "asymmetric"),
            ("key_bits", "256"),
            ("mode", "cbc"),
            ("rsa_bits", "4096"),
            ("passphrase", " pw "),
        ]))
        .unwrap();
        assert_eq!(
            pair,
            OsAction::OrderCreate(OrderSpec {
                order_type: "asymmetric".into(),
                name: None,
                bit_length: 4096,
                mode: None,
                expiration: None,
                passphrase: Some(Hidden(" pw ".into())),
            }),
            "a key pair: RSA size, no mode, the passphrase untrimmed"
        );
        assert!((d.build)(&vals(&[("order_type", "key"), ("key_bits", "2048")])).is_err());
    }

    #[test]
    fn deleting_an_order_says_the_key_stays() {
        let o = OrderRow {
            id: "o1".into(),
            name: "db-key".into(),
            order_type: "key".into(),
            status: "ACTIVE".into(),
            algorithm: None,
            bit_length: None,
            mode: None,
            expiration: None,
            error_code: None,
            error_reason: None,
            created: String::new(),
            updated: None,
            creator_id: None,
            secret_id: Some("s1".into()),
            container_id: None,
        };
        let d = super::super::testing::dialog_of(&order_items(Lang::EnUs, &o), "Delete Order");
        assert!(d.intro.iter().any(|l| l.contains("order record")));
    }

    fn secret(can_edit: bool, project_access: Option<bool>, users: &[&str]) -> SecretDetail {
        SecretDetail {
            secret: SecretRow {
                id: "s1".into(),
                name: "db".into(),
                secret_type: "opaque".into(),
                algorithm: None,
                bit_length: None,
                mode: None,
                content_type: None,
                status: "ACTIVE".into(),
                expiration: None,
                created: String::new(),
                updated: None,
                creator_id: Some("u0".into()),
                secret_ref: "https://kms/v1/secrets/s1".into(),
            },
            acl: KmAcl {
                users: users.iter().map(|u| u.to_string()).collect(),
                project_access,
                user_names: vec![("u1".into(), "octavia".into())],
                can_edit,
                in_use: vec![],
                octavia: vec![],
            },
            consumers: Some(vec![]),
            payload_allowed: false,
        }
    }

    #[test]
    fn secret_access_items_follow_policy_and_acl() {
        let labels = |d: SecretDetail| {
            secret_detail_items(Lang::EnUs, &d)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(secret(false, Some(false), &["u1"])),
            ["Delete Secret"],
            "neither creator nor admin"
        );
        assert_eq!(
            labels(secret(true, Some(true), &[])),
            ["Edit Access", "Delete Secret"],
            "Barbican's default: nothing to reset"
        );
        assert_eq!(
            labels(secret(true, Some(true), &["u1"])),
            ["Edit Access", "Reset Access", "Delete Secret"]
        );
        assert_eq!(
            labels(secret(true, Some(false), &[])),
            ["Edit Access", "Reset Access", "Delete Secret"],
            "private"
        );
        assert_eq!(secret(true, None, &[]).acl.user_label("u1"), "octavia (u1)");
        assert_eq!(secret(true, None, &[]).acl.user_label("u9"), "u9");
    }

    #[test]
    fn access_dialogs_warn_when_something_uses_it() {
        let mut d = secret(true, Some(true), &["u1"]);
        assert_eq!(in_use_warning(Lang::EnUs, &d.acl), None);
        d.acl.octavia = vec![OctaviaUse {
            lb_id: Some("lb1".into()),
            lb_name: "web".into(),
            kind: "listener".into(),
            name: "https".into(),
            via: None,
        }];
        let w = in_use_warning(Lang::EnUs, &d.acl).unwrap();
        assert!(w.contains("Consumers"), "points at the page's list: {w}");
        let items = secret_detail_items(Lang::EnUs, &d);
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else {
            panic!("Reset Access opens a dialog")
        };
        assert!(open().intro.contains(&w), "Reset Access shows the warning");
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else {
            panic!("Edit Access opens a dialog")
        };
        assert!(open().intro.contains(&w), "Edit Access too");
    }

    #[test]
    fn container_pairs_follow_the_type() {
        let picked = |f: &str| match f {
            "cert_certificate" => Some("https://kms/v1/secrets/c".to_string()),
            "cert_private_key" => Some("https://kms/v1/secrets/k".to_string()),
            "rsa_public_key" => Some("https://kms/v1/secrets/p".to_string()),
            _ => None,
        };
        let refs = |v: Vec<(String, String)>| v.into_iter().map(|(r, s)| format!("{r}={s}")).collect::<Vec<_>>();
        assert_eq!(
            refs(container_pairs("certificate", picked, vec![])),
            [
                "certificate=https://kms/v1/secrets/c",
                "private_key=https://kms/v1/secrets/k"
            ],
            "fields of another type are ignored"
        );
        assert_eq!(
            refs(container_pairs("rsa", picked, vec![])),
            ["public_key=https://kms/v1/secrets/p"]
        );
        assert_eq!(
            refs(container_pairs("generic", picked, vec!["a".into(), "b".into()])),
            ["secret_1=a", "secret_2=b"]
        );
    }
}
