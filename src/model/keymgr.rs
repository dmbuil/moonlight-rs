//! Barbican secrets, containers and ACLs (roadmap 55).

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
