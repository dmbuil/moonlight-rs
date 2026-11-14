//! Barbican secrets, containers and ACLs (roadmap 55), and orders (roadmap 58).

use super::*;

/// A Barbican secret's metadata (never its payload). Barbican's dates have no
/// zone; they are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretRow {
    /// The last segment of `secret_ref`.
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    /// `symmetric`, `public`, `private`, `passphrase`, `certificate` or `opaque`.
    pub secret_type: String,
    pub algorithm: Option<String>,
    pub bit_length: Option<i64>,
    pub mode: Option<String>,
    /// `content_types.default`; `None` while the secret has no payload.
    pub content_type: Option<String>,
    pub status: String,
    /// `None` = never expires.
    pub expiration: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    pub creator_id: Option<String>,
    /// The secret's URL, as other services take it (Octavia's `default_tls_container_ref`).
    pub secret_ref: String,
}

/// How close a secret is to its expiration (list and detail styling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    Never,
    Later,
    /// Within [`EXPIRY_WARNING_DAYS`].
    Soon,
    Past,
}

/// Days before its expiration a secret is flagged.
pub const EXPIRY_WARNING_DAYS: i64 = 30;

/// Barbican's `expiration` (UTC, with or without a zone) against `now`.
/// Unreadable dates count as `Later`: nothing to flag.
pub fn expiry(expiration: Option<&str>, now: chrono::NaiveDateTime) -> Expiry {
    let Some(e) = expiration else { return Expiry::Never };
    let at = chrono::DateTime::parse_from_rfc3339(e)
        .map(|d| d.naive_utc())
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(e, "%Y-%m-%dT%H:%M:%S%.f"));
    match at {
        Ok(at) if at <= now => Expiry::Past,
        Ok(at) if at - now <= chrono::Duration::days(EXPIRY_WARNING_DAYS) => Expiry::Soon,
        _ => Expiry::Later,
    }
}

/// A service using a secret (Barbican's consumers API, newer releases).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretConsumer {
    pub service: String,
    pub resource_type: String,
    pub resource_id: String,
}

/// Key Manager › Secrets › one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretDetail {
    pub secret: SecretRow,
    /// The `read` ACL, shown and edited on the detail page (roadmap 75).
    pub acl: KmAcl,
    /// `None` when this Barbican has no consumers API for secrets.
    pub consumers: Option<Vec<SecretConsumer>>,
    /// Whether `[key_manager] allow_payload` offers *Show Payload*.
    pub payload_allowed: bool,
}

/// A secret's or container's `read` ACL as the detail pages show and edit it
/// (roadmap 75).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KmAcl {
    /// Users granted read access besides the project.
    pub users: Vec<String>,
    /// `project-access`: whether every project member can read it (`None` when
    /// the ACL could not be read).
    pub project_access: Option<bool>,
    /// `(id, name)` of the `users` Keystone let the viewer read (`users/{id}`);
    /// the others are shown by ID (a non-admin gets 403 there).
    pub user_names: Vec<(String, String)>,
    /// Whether the viewer may change it: [`can_edit_secret_acl`].
    pub can_edit: bool,
    /// Barbican's own record of what uses it: its consumers, and for a secret the
    /// containers holding it that have consumers.
    pub in_use: Vec<String>,
    /// The Octavia listeners and pools whose certificate refs point at it,
    /// directly or (for a secret) through a container. Octavia never registers a
    /// Barbican consumer (it reads with `check_only=True`), so this is how a load
    /// balancer is found; it reads with its own user from the ACL, and loses the
    /// certificate at its next read (a failover) when that user is removed.
    #[serde(default)]
    pub octavia: Vec<OctaviaUse>,
}

/// An Octavia listener or pool using a Barbican secret or container (roadmap 75).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OctaviaUse {
    /// The load balancer's ID, for the link; `None` when Octavia didn't say.
    pub lb_id: Option<String>,
    /// Its name, else its ID.
    pub lb_name: String,
    /// `listener` or `pool`.
    pub kind: String,
    /// The listener's or pool's name, else its ID.
    pub name: String,
    /// For a secret used through a container: `(container ID, name)`.
    pub via: Option<(String, String)>,
}

impl KmAcl {
    /// Whether an ACL is set at all (else it is Barbican's default and *Reset
    /// Access* has nothing to do).
    pub fn is_set(&self) -> bool {
        self.project_access == Some(false) || !self.users.is_empty()
    }
    /// Whether anything is known to use it (the dialogs then warn).
    pub fn in_use(&self) -> bool {
        !self.in_use.is_empty() || !self.octavia.is_empty()
    }
    /// A user as `name (id)` when Keystone named it, else the ID.
    pub fn user_label(&self, id: &str) -> String {
        match self.user_names.iter().find(|(i, _)| i == id) {
            Some((_, name)) => format!("{name} ({id})"),
            None => id.to_string(),
        }
    }
}

/// Key Manager › Containers › one: the container and its `read` ACL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerDetail {
    pub container: ContainerRow,
    pub acl: KmAcl,
}

/// Barbican's default policy (`secret_acls:put_patch` / `delete`, and the same
/// for `container_acls`): the creator or a project admin. *Edit Access* and *Reset Access* are hidden for
/// everyone else rather than failing with 403 (roadmap 75).
pub fn can_edit_secret_acl(creator_id: Option<&str>, user_id: &str, roles: &[String]) -> bool {
    creator_id == Some(user_id) || roles.iter().any(|r| r == "admin")
}

/// User IDs typed for a secret's ACL: split on commas, spaces and new lines,
/// deduplicated, in the order given.
pub fn parse_user_ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for id in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|x| !x.is_empty())
    {
        if !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// A secret in a container, under its role (`certificate`, `private_key`…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSecret {
    pub role: String,
    pub id: String,
    /// The secret's name, else its ID.
    pub name: String,
}

/// A service registered as using a container (`{name, URL}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerConsumer {
    pub name: String,
    pub url: String,
}

/// A Barbican container: secrets grouped by role.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerRow {
    /// The last segment of `container_ref`.
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    /// `generic`, `certificate` or `rsa`.
    pub container_type: String,
    pub status: String,
    pub created: String,
    pub updated: Option<String>,
    pub creator_id: Option<String>,
    pub container_ref: String,
    pub secrets: Vec<ContainerSecret>,
    pub consumers: Vec<ContainerConsumer>,
}

/// Sensitive text (a secret's payload): never shown in `Debug` output (logs),
/// and overwritten with zeros when dropped, on the server and in the browser.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
#[serde(transparent)]
pub struct Hidden(pub String);

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\"********\"")
    }
}

/// *Show Payload*'s answer (roadmap 55).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretPayload {
    pub payload: Hidden,
    /// The type Barbican returned it in (`text/plain`, `application/octet-stream`…).
    pub content_type: String,
    /// Binary payloads are sent base64-encoded.
    pub base64: bool,
}

/// *Store Secret*, as Barbican's `POST /v1/secrets` takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretSpec {
    pub name: Option<String>,
    pub secret_type: String,
    /// `None`: metadata only (the payload can be added later; Barbican's two-step store).
    pub payload: Option<Hidden>,
    /// `text/plain` or `application/octet-stream` (then `payload` is base64).
    pub payload_content_type: String,
    pub algorithm: Option<String>,
    pub bit_length: Option<i64>,
    pub mode: Option<String>,
    /// ISO 8601 in UTC (`2026-12-31T00:00:00`); `None` = never expires.
    pub expiration: Option<String>,
}

/// A Barbican order (roadmap 58): Barbican generating a symmetric key
/// (`key`, stored as a secret) or an RSA key pair (`asymmetric`, stored as an
/// `rsa` container). Orders run asynchronously: `PENDING`, then `ACTIVE` with
/// the result's ref, or `ERROR` with a reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderRow {
    /// The last segment of `order_ref`.
    pub id: String,
    /// `meta.name`, else the ID.
    pub name: String,
    /// `key`, `asymmetric` or `certificate` (deprecated, listed but never offered).
    pub order_type: String,
    /// `PENDING`, `ACTIVE` or `ERROR`.
    pub status: String,
    pub algorithm: Option<String>,
    pub bit_length: Option<i64>,
    pub mode: Option<String>,
    /// The generated secret's expiration (UTC); `None` = never expires.
    pub expiration: Option<String>,
    /// `error_status_code` and `error_reason`, set when `status` is `ERROR`.
    pub error_code: Option<String>,
    pub error_reason: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    pub creator_id: Option<String>,
    /// The generated secret's ID (`key` orders, once `ACTIVE`).
    pub secret_id: Option<String>,
    /// The generated `rsa` container's ID (`asymmetric` orders, once `ACTIVE`).
    pub container_id: Option<String>,
}

impl OrderRow {
    /// Still being processed: the list polls while any order is (roadmap 46).
    pub fn busy(&self) -> bool {
        self.status.eq_ignore_ascii_case("PENDING")
    }
}

/// The order types *Order Key* offers, each with its algorithm and the bit
/// lengths Barbican's simple crypto plugin generates (`certificate` orders
/// need a CA plugin and are deprecated: not offered).
pub const ORDER_TYPES: [(&str, &str, &[i64]); 2] = [
    ("key", "aes", &[128, 192, 256]),
    ("asymmetric", "rsa", &[2048, 3072, 4096]),
];

/// *Order Key*, as Barbican's `POST /v1/orders` takes it (roadmap 58).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderSpec {
    /// `key` or `asymmetric` ([`ORDER_TYPES`]).
    pub order_type: String,
    pub name: Option<String>,
    pub bit_length: i64,
    /// Cipher mode of a `key` (`cbc`); `None` for key pairs.
    pub mode: Option<String>,
    /// ISO 8601 in UTC (`2026-12-31T00:00:00`); `None` = never expires.
    pub expiration: Option<String>,
    /// An `asymmetric` order's private key passphrase (stored as the container's
    /// `private_key_passphrase`); `None` = no passphrase.
    pub passphrase: Option<Hidden>,
}

impl OrderSpec {
    /// The order's algorithm: `aes` for keys, `rsa` for key pairs.
    pub fn algorithm(&self) -> Option<&'static str> {
        ORDER_TYPES
            .iter()
            .find(|(t, _, _)| *t == self.order_type)
            .map(|(_, a, _)| *a)
    }

    /// Checks the type, bit length and fields that only one type takes; run
    /// by the dialog and again by the server.
    pub fn check(&self) -> Result<(), String> {
        let Some((_, alg, bits)) = ORDER_TYPES.iter().find(|(t, _, _)| *t == self.order_type) else {
            return Err(format!("unknown order type {}", self.order_type));
        };
        if !bits.contains(&self.bit_length) {
            let list: Vec<String> = bits.iter().map(i64::to_string).collect();
            return Err(format!("{alg} takes a bit length of {}", list.join(", ")));
        }
        match self.order_type.as_str() {
            "key" if self.passphrase.as_ref().is_some_and(|p| !p.0.is_empty()) => {
                Err("only key pairs take a passphrase".into())
            }
            "asymmetric" if self.mode.as_deref().is_some_and(|m| !m.trim().is_empty()) => {
                Err("key pairs take no cipher mode".into())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod secret_tests {
    use super::Secret;

    #[test]
    fn debug_never_prints_the_secret() {
        let s = Secret {
            file_name: "k.pem".into(),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
            kind: super::SecretKind::PrivateKey,
            fields: vec![("secret".into(), "app-cred-secret".into())],
            extra: vec![(
                "clouds.yaml".into(),
                "application_credential_secret: app-cred-secret".into(),
            )],
        };
        let dbg = format!("{s:?}");
        assert!(dbg.contains("k.pem") && dbg.contains("<redacted>"));
        assert!(
            !dbg.contains("PRIVATE") && !dbg.contains("app-cred-secret"),
            "fields and extra downloads hold it too"
        );
    }
}

#[cfg(test)]
mod secret_acl_tests {
    use super::*;

    #[test]
    fn secret_acl_is_for_its_creator_and_admins() {
        let roles = |r: &[&str]| r.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(
            can_edit_secret_acl(Some("u1"), "u1", &roles(&["member"])),
            "the creator"
        );
        assert!(
            can_edit_secret_acl(Some("u2"), "u1", &roles(&["admin"])),
            "a project admin"
        );
        assert!(!can_edit_secret_acl(Some("u2"), "u1", &roles(&["member", "reader"])));
        assert!(
            !can_edit_secret_acl(None, "u1", &roles(&[])),
            "creator unknown: not the creator"
        );
    }

    #[test]
    fn typed_user_ids_split_and_dedupe() {
        assert_eq!(parse_user_ids(" a1, b2\nc3  a1,,"), ["a1", "b2", "c3"]);
        assert!(parse_user_ids(" , ").is_empty());
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    fn spec(order_type: &str, bits: i64) -> OrderSpec {
        OrderSpec {
            order_type: order_type.into(),
            name: None,
            bit_length: bits,
            mode: (order_type == "key").then(|| "cbc".into()),
            expiration: None,
            passphrase: None,
        }
    }

    #[test]
    fn orders_take_aes_keys_and_rsa_pairs_of_known_sizes() {
        assert_eq!(spec("key", 256).check(), Ok(()));
        assert_eq!(spec("key", 256).algorithm(), Some("aes"));
        assert_eq!(spec("asymmetric", 4096).check(), Ok(()));
        assert_eq!(spec("asymmetric", 2048).algorithm(), Some("rsa"));
        assert!(spec("key", 2048).check().unwrap_err().contains("128, 192, 256"));
        assert!(spec("asymmetric", 256).check().is_err());
        assert!(spec("certificate", 2048).check().is_err(), "deprecated: never ordered");
        let mut pair = spec("asymmetric", 2048);
        pair.passphrase = Some(Hidden("pw".into()));
        assert_eq!(pair.check(), Ok(()));
        pair.mode = Some("cbc".into());
        assert!(pair.check().is_err(), "a key pair has no cipher mode");
        let mut key = spec("key", 128);
        key.passphrase = Some(Hidden("pw".into()));
        assert!(key.check().is_err(), "a key has no passphrase");
        key.passphrase = Some(Hidden(String::new()));
        assert_eq!(key.check(), Ok(()), "an empty passphrase is none");
    }

    #[test]
    fn pending_orders_are_busy() {
        let o = |status: &str| OrderRow {
            id: "o1".into(),
            name: "o1".into(),
            order_type: "key".into(),
            status: status.into(),
            algorithm: None,
            bit_length: None,
            mode: None,
            expiration: None,
            error_code: None,
            error_reason: None,
            created: String::new(),
            updated: None,
            creator_id: None,
            secret_id: None,
            container_id: None,
        };
        assert!(o("PENDING").busy());
        assert!(!o("ACTIVE").busy() && !o("ERROR").busy());
    }
}
