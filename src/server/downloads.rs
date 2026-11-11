//! `openrc` and `clouds.yaml` downloads (API Access, and a new application
//! credential's), rendered from Jinja templates like Horizon's
//! (`OPENRC_CUSTOM_TEMPLATE`, `OPENSTACK_CLOUDS_YAML_CUSTOM_TEMPLATE`). The
//! built-in ones live in `server/templates/`; `[api_access] *_template` points
//! at an operator's own file, checked at start-up.
//!
//! Variables use Horizon's names so its templates work unchanged: `auth_url`,
//! `tenant_id`, `tenant_name` (and `project_id`, `project_name`), `user.username`,
//! `user.id`, `user_domain_name`, `project_domain_id`, `region`, `regions`,
//! `interface`, `cloud_name`, `profile`, `os_identity_api_version`,
//! `os_auth_version`; plus `application_credential_id`, `_name`, `_secret`, and
//! for WebSSO sessions `identity_provider` (may be empty) and `protocol`.
//! Filters: `sh` (a single-quoted shell word), `yaml` (a double-quoted YAML
//! scalar) and Horizon's `shellfilter`; its `{% load … %}` lines are ignored.

use std::sync::OnceLock;

use minijinja::{AutoEscape, Environment, Value, context};

use crate::model::Credentials;

/// The five downloads, by template name.
pub const OPENRC: &str = "openrc.sh";
pub const CLOUDS_YAML: &str = "clouds.yaml";
pub const APP_OPENRC: &str = "app_credential_openrc.sh";
pub const APP_CLOUDS_YAML: &str = "app_credential_clouds.yaml";
/// API Access's `clouds.yaml` for a WebSSO session (`auth_type: v3websso`,
/// keystoneauth-websso), which has no password to use (roadmap 80).
pub const WEBSSO_CLOUDS_YAML: &str = "websso_clouds.yaml";

const DEFAULTS: [(&str, &str); 5] = [
    (OPENRC, include_str!("templates/openrc.sh.j2")),
    (CLOUDS_YAML, include_str!("templates/clouds.yaml.j2")),
    (APP_OPENRC, include_str!("templates/app_credential_openrc.sh.j2")),
    (APP_CLOUDS_YAML, include_str!("templates/app_credential_clouds.yaml.j2")),
    (WEBSSO_CLOUDS_YAML, include_str!("templates/websso_clouds.yaml.j2")),
];

static ENV: OnceLock<Environment<'static>> = OnceLock::new();

/// Single-quoted shell word: safe for any value.
fn sh(v: &str) -> String {
    format!("'{}'", v.replace('\'', r"'\''"))
}

/// Double-quoted YAML scalar.
fn yaml(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', r"\\").replace('"', "\\\""))
}

/// Horizon's `shellfilter`, for its templates (values inside double quotes).
fn shellfilter(v: &str) -> String {
    [
        ("\\", "\\\\"),
        ("`", "\\`"),
        ("$", "\\$"),
        ("!", "\\!"),
        ("'", "\\'"),
        ("\"", "\\\""),
    ]
    .iter()
    .fold(v.to_string(), |acc, (from, to)| acc.replace(from, to))
}

/// Drops Django's `{% load … %}` tags, which Jinja doesn't know.
fn without_django_loads(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("{% load ") {
        out.push_str(&rest[..i]);
        match rest[i..].find("%}") {
            Some(j) => rest = &rest[i + j + 2..],
            None => {
                rest = &rest[i..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The environment: built-in templates, replaced by `overrides` (`(name, source)`).
fn build(overrides: &[(&'static str, String)]) -> Result<Environment<'static>, String> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);
    // Files end with a newline, like Horizon's (Jinja drops it by default).
    env.set_keep_trailing_newline(true);
    env.add_filter("sh", |v: String| sh(&v));
    env.add_filter("yaml", |v: String| yaml(&v));
    env.add_filter("shellfilter", |v: String| shellfilter(&v));
    for (name, src) in DEFAULTS {
        let src = overrides
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| without_django_loads(s))
            .unwrap_or_else(|| src.to_string());
        env.add_template_owned(name, src)
            .map_err(|e| format!("template {name}: {e}"))?;
    }
    // Render each once with sample values, so a broken template stops start-up.
    let sample = sample_credentials();
    for (name, _) in DEFAULTS {
        env.get_template(name)
            .and_then(|t| t.render(context(&sample, Some(("id", "name", "secret")))))
            .map_err(|e| format!("template {name}: {e}"))?;
    }
    Ok(env)
}

/// Loads `[api_access] *_template` files over the built-in templates. Called
/// once from `config::init`; an unreadable or broken file is a start-up error.
pub fn init(paths: &[(&'static str, Option<String>)]) -> Result<(), String> {
    let mut overrides = Vec::new();
    for (name, path) in paths {
        if let Some(p) = path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            let src = std::fs::read_to_string(p).map_err(|e| format!("[api_access] template for {name} ({p}): {e}"))?;
            overrides.push((*name, src));
        }
    }
    let env = build(&overrides)?;
    let _ = ENV.set(env);
    Ok(())
}

fn env() -> &'static Environment<'static> {
    // Before `init` (unit tests): the built-in templates.
    ENV.get_or_init(|| build(&[]).expect("built-in templates are valid"))
}

fn sample_credentials() -> Credentials {
    Credentials {
        user_name: "user".into(),
        user_id: "u".into(),
        user_domain: "Default".into(),
        project_name: "project".into(),
        project_id: "p".into(),
        region: "region".into(),
        auth_url: "https://keystone.example/v3".into(),
        interface: "public".into(),
        cloud_name: "openstack".into(),
        profile: None,
        ..Default::default()
    }
}

/// Horizon's template variables for `c`, plus an application credential's
/// `(id, name, secret)` when given.
fn context(c: &Credentials, app: Option<(&str, &str, &str)>) -> Value {
    let (id, name, secret) = app.unwrap_or_default();
    context! {
        auth_url => c.auth_url,
        tenant_id => c.project_id,
        tenant_name => c.project_name,
        project_id => c.project_id,
        project_name => c.project_name,
        user => context! { username => c.user_name, id => c.user_id },
        user_domain_name => c.user_domain,
        project_domain_id => "",
        region => c.region,
        regions => Vec::<String>::new(),
        interface => c.interface,
        cloud_name => c.cloud_name,
        profile => c.profile,
        os_identity_api_version => 3,
        os_auth_version => 3,
        application_credential_id => id,
        application_credential_name => name,
        application_credential_secret => secret,
        identity_provider => c.identity_provider.clone().unwrap_or_default(),
        protocol => c.protocol.clone().unwrap_or_default(),
    }
}

fn render(name: &str, c: &Credentials, app: Option<(&str, &str, &str)>) -> String {
    match env().get_template(name).and_then(|t| t.render(context(c, app))) {
        Ok(s) => s,
        Err(e) => {
            // Checked at start-up; a failure here is a bug worth seeing in the file.
            tracing::warn!("template {name}: {e}");
            format!("# template {name} failed: {e}\n")
        }
    }
}

/// API Access: `(openrc, clouds.yaml)` for the session; a WebSSO session gets
/// the `v3websso` `clouds.yaml`.
pub fn api_access(c: &Credentials) -> (String, String) {
    (
        render(OPENRC, c, None),
        render(if c.federated { WEBSSO_CLOUDS_YAML } else { CLOUDS_YAML }, c, None),
    )
}

/// A new application credential's `[(file name, content)]`: `openrc` and `clouds.yaml`.
pub fn app_credential(c: &Credentials, id: &str, name: &str, secret: &str) -> Vec<(String, String)> {
    let app = Some((id, name, secret));
    vec![
        (format!("app-cred-{name}-openrc.sh"), render(APP_OPENRC, c, app)),
        ("clouds.yaml".into(), render(APP_CLOUDS_YAML, c, app)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Credentials {
        Credentials {
            user_name: "ana".into(),
            user_id: "u1".into(),
            user_domain: "Default".into(),
            project_name: "it's \"demo\"".into(),
            project_id: "p1".into(),
            region: "es-central-1".into(),
            auth_url: "https://keystone.example/v3".into(),
            interface: "public".into(),
            cloud_name: "openstack".into(),
            profile: None,
            ..Default::default()
        }
    }

    #[test]
    fn downloads_use_the_configured_names_and_interface() {
        let (_, y) = api_access(&sample());
        assert!(
            y.starts_with("clouds:\n  \"openstack\":\n    auth:\n") && y.contains("    interface: \"public\"\n"),
            "defaults:\n{y}"
        );
        let c = Credentials {
            interface: "internal".into(),
            cloud_name: "acme-prod".into(),
            profile: Some("acme".into()),
            ..sample()
        };
        let (rc, y) = api_access(&c);
        assert!(
            y.starts_with("clouds:\n  \"acme-prod\":\n    profile: \"acme\"\n    auth:\n"),
            "{y}"
        );
        assert!(y.contains("    interface: \"internal\"\n"));
        assert!(rc.contains("export OS_INTERFACE='internal'\n"));
    }

    #[test]
    fn websso_sessions_get_a_v3websso_clouds_yaml_without_a_user() {
        let c = Credentials {
            federated: true,
            identity_provider: Some("tef-adfs".into()),
            protocol: Some("openid".into()),
            ..sample()
        };
        let (rc, y) = api_access(&c);
        assert!(
            y.contains("clouds:\n  \"openstack\":\n    auth_type: \"v3websso\"\n    auth:\n"),
            "{y}"
        );
        assert!(
            y.contains("      identity_provider: \"tef-adfs\"\n      protocol: \"openid\"\n      project_id: \"p1\"\n"),
            "{y}"
        );
        assert!(y.contains("keystoneauth-websso"), "says which plugin it needs");
        assert!(
            !y.contains("username") && !y.contains("password"),
            "no Keystone user: the browser signs in"
        );
        assert!(
            rc.contains("read -sr OS_PASSWORD_INPUT"),
            "openrc stays the password one (the page says so)"
        );
        let unmapped = Credentials {
            identity_provider: None,
            ..c
        };
        let (_, y) = api_access(&unmapped);
        assert!(
            !y.contains("identity_provider") && y.contains("      protocol: \"openid\"\n"),
            "no idp_mapping: Keystone picks the provider\n{y}"
        );
        assert!(
            !api_access(&sample()).1.contains("v3websso"),
            "password sessions keep the password clouds.yaml"
        );
    }

    #[test]
    fn openrc_quotes_values_and_asks_for_the_password() {
        let (rc, _) = api_access(&sample());
        assert!(rc.contains("export OS_PROJECT_NAME='it'\\''s \"demo\"'\n"), "{rc}");
        assert!(rc.contains("export OS_AUTH_URL='https://keystone.example/v3'\n"));
        assert!(rc.contains("read -sr OS_PASSWORD_INPUT"));
        assert!(rc.contains("export OS_REGION_NAME='es-central-1'\n"));
        assert!(rc.ends_with("export OS_IDENTITY_API_VERSION=3\n"));
    }

    #[test]
    fn clouds_yaml_escapes_and_has_no_password() {
        let (_, y) = api_access(&sample());
        assert!(y.contains("      project_name: \"it's \\\"demo\\\"\"\n"), "{y}");
        assert!(y.contains("    region_name: \"es-central-1\"\n"));
        assert!(!y.contains("password"));
    }

    #[test]
    fn empty_region_is_left_out() {
        let (rc, y) = api_access(&Credentials {
            region: String::new(),
            ..sample()
        });
        assert!(rc.contains("unset OS_REGION_NAME\n"));
        assert!(!y.contains("region_name"));
    }

    #[test]
    fn application_credential_files_follow_horizon() {
        let files = app_credential(&sample(), "a1", "ci", "s'3\"cret");
        assert_eq!(
            (files[0].0.as_str(), files[1].0.as_str()),
            ("app-cred-ci-openrc.sh", "clouds.yaml")
        );
        assert!(files[0].1.contains("export OS_AUTH_TYPE=v3applicationcredential\n"));
        assert!(
            files[0]
                .1
                .contains("export OS_APPLICATION_CREDENTIAL_SECRET='s'\\''3\"cret'\n"),
            "{}",
            files[0].1
        );
        assert!(files[1].1.contains("    auth_type: \"v3applicationcredential\"\n"));
        assert!(
            files[1]
                .1
                .contains("      application_credential_secret: \"s'3\\\"cret\"\n"),
            "{}",
            files[1].1
        );
        let profiled = app_credential(
            &Credentials {
                profile: Some("acme".into()),
                ..sample()
            },
            "a1",
            "ci",
            "s",
        );
        assert!(
            !profiled[1].1.contains("auth_url") && !profiled[1].1.contains("interface"),
            "a profile supplies them, as Horizon's"
        );
    }

    #[test]
    fn horizon_templates_work_unchanged_and_broken_ones_are_refused() {
        let horizon = "{% load shellfilter %}#!/usr/bin/env bash\nexport OS_PROJECT_NAME=\"{{ tenant_name|shellfilter }}\"\nexport OS_USERNAME=\"{{ user.username|shellfilter }}\"\n";
        let env = build(&[(OPENRC, horizon.to_string())]).unwrap();
        let rc = env
            .get_template(OPENRC)
            .unwrap()
            .render(context(&sample(), None))
            .unwrap();
        assert_eq!(
            rc,
            "#!/usr/bin/env bash\nexport OS_PROJECT_NAME=\"it\\'s \\\"demo\\\"\"\nexport OS_USERNAME=\"ana\"\n"
        );
        assert!(
            build(&[(CLOUDS_YAML, "{% if %}".into())])
                .unwrap_err()
                .starts_with("template clouds.yaml"),
            "a syntax error stops start-up"
        );
        assert!(
            init(&[(OPENRC, Some("/nonexistent/openrc.j2".into()))])
                .unwrap_err()
                .contains("/nonexistent/openrc.j2")
        );
    }
}
