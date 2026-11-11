use leptos::prelude::*;
use leptos_meta::Title;

use crate::api::{Login, get_login_prefs};
use crate::components::layout::{LanguageMenu, MoonMark, ThemeMenu};
use crate::components::select::Combo;
use crate::i18n::Lang;
use crate::i18n::{K, use_i18n};
use crate::model::Choice;
use crate::model::ErrorKind;
use crate::prefs::{LoginPrefs, SsoChoice, WEBSSO_START};

#[component]
pub fn LoginPage() -> impl IntoView {
    let i18n = use_i18n();
    let action = ServerAction::<Login>::new();
    // `[login]` settings; the defaults render while (or if) they cannot load.
    let prefs = Resource::new(|| (), |_| get_login_prefs());
    // `?sso=…`: why the WebSSO callback sent the browser back
    // (`server::websso::Refusal::code`, roadmap 80).
    let query = leptos_router::hooks::use_query_map();
    let sso_error = move || {
        query.with(|q| match q.get("sso").as_deref() {
            Some("expired") => Some(i18n.t(K::SsoNotStarted)),
            Some("noprojects") => Some(i18n.t(K::NoProjects)),
            Some(_) => Some(i18n.t(K::LoginFailed)),
            None => None,
        })
    };
    let error = move || {
        action
            .value()
            .get()
            .and_then(Result::err)
            .map(|e| {
                match e.kind {
                    // Credentials were accepted but there is no project to scope to.
                    ErrorKind::Forbidden => i18n.t(K::NoProjects),
                    // `[login] max_failures` reached (roadmap 50).
                    ErrorKind::TooManyAttempts => i18n.t(K::LoginThrottled),
                    _ => i18n.t(K::LoginFailed),
                }
            })
            .or_else(sso_error)
    };

    view! {
        <Title text=move || i18n.t(K::LoginTitle).to_string()/>
        <div class="login-page">
            <div class="login-top">
                <ThemeMenu/>
                <LanguageMenu/>
            </div>
            <div class="login-card">
                <div class="login-brand">
                    <MoonMark/>
                    <span>"Moonlight"</span>
                </div>
                <h1>{move || i18n.t(K::LoginTitle)}</h1>
                <p class="muted">{move || i18n.t(K::LoginSubtitle)}</p>
                {move || error().map(|e| view! { <div class="alert alert-error" role="alert">{e}</div> })}
                <Transition fallback=move || view! { <LoginForm action=action prefs=LoginPrefs::default()/> }>
                    {move || prefs.get().map(|p| view! { <LoginForm action=action prefs=p.unwrap_or_default()/> })}
                </Transition>
            </div>
        </div>
    }
}

/// The form, shaped by `[login]`: a domain text field, a dropdown of
/// `domain_choices`, or no domain at all; password autocomplete and reveal.
/// With `[websso]`, an *Authenticate using* select first: a federated choice
/// hides the credentials and turns the button into a link to Keystone's WebSSO
/// (through `WEBSSO_START`), as Horizon does (roadmap 80).
#[component]
fn LoginForm(action: ServerAction<Login>, prefs: LoginPrefs) -> impl IntoView {
    let i18n = use_i18n();
    let pending = action.pending();
    let reveal = RwSignal::new(false);
    let choice = RwSignal::new(prefs.websso_initial.clone());
    let federated = move || choice.with(|c| !c.is_empty() && c != SsoChoice::CREDENTIALS);
    // One choice needs no select (validation leaves it federated). The themed
    // `Combo` has no form value, so the password sign-in never submits it.
    let method = (prefs.websso_choices.len() > 1).then(|| {
        let choices = StoredValue::new(prefs.websso_choices.clone());
        let options = Signal::derive(move || {
            Some(choices.with_value(|cs| cs.iter().map(|c| Choice { value: c.id.clone(), label: sso_label(c, i18n.lang()), ..Default::default() }).collect()))
        });
        let name = Signal::derive(move || i18n.t(K::AuthenticateUsing).to_string());
        view! {
            <div class="login-field">
                <span>{move || i18n.t(K::AuthenticateUsing)}</span>
                <Combo id="websso-choice" options=options value=choice on_change=Callback::new(move |v| choice.set(v)) required=true aria_label=name/>
            </div>
        }
    });
    let domain = if prefs.domain_hidden {
        view! { <input name="domain" type="hidden" value=prefs.default_domain.clone()/> }.into_any()
    } else if prefs.domain_choices.is_empty() {
        view! {
            <label>
                <span>{move || i18n.t(K::Domain)}</span>
                <input name="domain" type="text" autocomplete="organization" value=prefs.default_domain.clone()/>
            </label>
        }
        .into_any()
    } else {
        // The themed `Combo`; a hidden input carries its value into the form.
        let picked = RwSignal::new(if prefs.domain_choices.contains(&prefs.default_domain) {
            prefs.default_domain.clone()
        } else {
            prefs.domain_choices[0].clone()
        });
        let options: Vec<Choice> = prefs
            .domain_choices
            .iter()
            .map(|d| Choice {
                value: d.clone(),
                label: d.clone(),
                ..Default::default()
            })
            .collect();
        let name = Signal::derive(move || i18n.t(K::Domain).to_string());
        view! {
            <div class="login-field">
                <span>{move || i18n.t(K::Domain)}</span>
                <Combo id="login-domain" options=Signal::stored(Some(options)) value=picked on_change=Callback::new(move |v| picked.set(v)) required=true aria_label=name/>
                <input name="domain" type="hidden" value=move || picked.get()/>
            </div>
        }
        .into_any()
    };
    let autocomplete = if prefs.password_autocomplete {
        "current-password"
    } else {
        "off"
    };
    view! {
        {prefs.message.clone().map(|m| view! { <div class="alert alert-info login-message" role="note">{m}</div> })}
        <ActionForm action=action attr:class="login-form">
            {method}
            <div class="login-fields" style:display=move || federated().then_some("none")>
                {domain}
                <label>
                    <span>{move || i18n.t(K::Username)}</span>
                    <input name="username" type="text" autocomplete="username" required autofocus/>
                </label>
                <label>
                    <span>{move || i18n.t(K::Password)}</span>
                    <span class="password-field">
                        <input name="password" type=move || if reveal.get() { "text" } else { "password" } autocomplete=autocomplete required/>
                        {prefs.password_reveal.then(|| view! {
                            <button type="button" class="btn-reveal" aria-pressed=move || reveal.get().to_string()
                                aria-label=move || i18n.t(if reveal.get() { K::HidePassword } else { K::ShowPassword })
                                on:click=move |_| reveal.update(|r| *r = !*r)>
                                {move || i18n.t(if reveal.get() { K::HidePassword } else { K::ShowPassword })}
                            </button>
                        })}
                    </span>
                </label>
            </div>
            {move || if federated() {
                // `rel="external"`: a server route, not a page. Without it the
                // router catches the click and renders its own 404.
                view! {
                    <a class="btn btn-primary" rel="external" href=format!("{WEBSSO_START}?choice={}", choice.get())>{move || i18n.t(K::SignIn)}</a>
                }.into_any()
            } else {
                view! {
                    <button class="btn btn-primary" type="submit" disabled=pending>
                        {move || if pending.get() { i18n.t(K::SigningIn) } else { i18n.t(K::SignIn) }}
                    </button>
                }.into_any()
            }}
        </ActionForm>
    }
}

/// A sign-in choice's text: its label, else *Keystone Credentials* for the
/// password form, else its ID.
fn sso_label(c: &SsoChoice, lang: Lang) -> String {
    match (c.label.is_empty(), c.id == SsoChoice::CREDENTIALS) {
        (false, _) => c.label.clone(),
        (true, true) => crate::i18n::t(lang, K::KeystoneCredentials).to_string(),
        (true, false) => c.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_in_choices_show_their_label_or_a_translated_default() {
        let c = |id: &str, label: &str| SsoChoice {
            id: id.into(),
            label: label.into(),
        };
        assert_eq!(sso_label(&c("oidc", "ACME SSO"), Lang::EnUs), "ACME SSO");
        assert_eq!(sso_label(&c("credentials", ""), Lang::EnUs), "Keystone Credentials");
        assert_eq!(sso_label(&c("credentials", ""), Lang::EsEs), "Credenciales de Keystone");
        assert_eq!(sso_label(&c("saml2", ""), Lang::EnUs), "saml2", "no label: the ID");
    }
}
