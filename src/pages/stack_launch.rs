//! *Launch Stack* and *Change Stack Template* (Heat). A page rather
//! than a dialog: the parameter fields come from the template, which Heat reads
//! (`run_stack_validate`) once the user has given it. Step one takes the template
//! and environment; step two the name, timeout, rollback and parameters. *Preview*
//! asks Heat what would be created; *Launch*/*Update* opens the usual
//! confirmation dialog, which runs the `OsAction` (and shows it in dry-run).

use std::cell::RefCell;
use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::details::param;
use crate::api::*;
use crate::components::dialog::{Dialog, FileLoad, RequestList, use_dialogs};
use crate::components::widgets::{PageHeader, Skeleton, render};
use crate::i18n::{K, Lang, t, use_i18n};
use crate::model::*;

/// What the form holds when *Launch*/*Preview* is pressed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackForm {
    /// The stack name, checked with `valid_stack_name` (fixed on an update).
    pub name: String,
    pub source: TemplateSource,
    /// The creation timeout in minutes, as typed; blank leaves Heat's default.
    pub timeout: String,
    /// Roll back on failure; sent inverted as Heat's `disable_rollback`.
    pub rollback: bool,
    /// Parameter name → the text typed for it.
    pub values: HashMap<String, String>,
    /// An update: blank parameters keep the stack's value instead of being required.
    pub update: bool,
}

/// The form as a [`StackSpec`], or what is wrong with it. Parameters are sent
/// in the template's order; blank ones are left out (Heat applies the default,
/// or keeps the current value in an update), so one without a default is
/// required only on launch.
pub fn stack_spec(lang: Lang, f: &StackForm, params: &[TemplateParam]) -> Result<StackSpec, String> {
    let name = f.name.trim().to_string();
    if !valid_stack_name(&name) {
        return Err(format!("{}: {}", t(lang, K::StackName), t(lang, K::StackNameHelp)));
    }
    let timeout_mins = match f.timeout.trim() {
        "" => None,
        x => Some(
            x.parse::<u32>()
                .ok()
                .filter(|m| *m > 0)
                .ok_or_else(|| format!("{}: ?", t(lang, K::StackTimeout)))?,
        ),
    };
    let mut parameters = vec![];
    for p in params {
        match f.values.get(&p.name).map(|v| v.trim()).filter(|v| !v.is_empty()) {
            Some(v) => parameters.push((p.name.clone(), v.to_string())),
            None if p.default.is_none() && !f.update => {
                return Err(format!(
                    "{}: {}",
                    p.label.as_deref().unwrap_or(&p.name),
                    t(lang, K::RequiredField)
                ));
            }
            None => {}
        }
    }
    let hidden = params.iter().filter(|p| p.hidden).map(|p| p.name.clone()).collect();
    Ok(StackSpec {
        name,
        source: f.source.clone(),
        parameters,
        hidden,
        timeout_mins,
        disable_rollback: !f.rollback,
    })
}

/// The values the form starts with: an update's current values (not Heat's
/// `******` for hidden ones), else the defaults (never for hidden parameters).
pub fn initial_values(params: &[TemplateParam], current: &[(String, String)]) -> HashMap<String, String> {
    params
        .iter()
        .filter_map(|p| {
            let now = current
                .iter()
                .find(|(k, _)| *k == p.name)
                .map(|(_, v)| v.clone())
                .filter(|v| v != "******");
            let v = if current.is_empty() {
                p.default.clone().filter(|_| !p.hidden)
            } else {
                now
            };
            Some((p.name.clone(), v?))
        })
        .collect()
}

thread_local! {
    /// A template the Template Generator hands to the next *Launch Stack*
    ///. Browser only: set by a click there, taken by the page it
    /// navigates to; the server never sets it, so SSR always starts empty.
    static HANDED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Opens the next *Launch Stack* with `template` as its direct input.
pub fn hand_template(template: String) {
    HANDED.with(|h| *h.borrow_mut() = Some(template));
}

/// Takes (and clears) the template [`hand_template`] left, so only the first *Launch Stack*
/// after the Template Generator is prefilled.
fn take_handed() -> Option<String> {
    HANDED.with(|h| h.borrow_mut().take())
}

/// Orchestration › Stacks › *Launch Stack*.
#[component]
pub fn StackLaunchPage() -> impl IntoView {
    view! { <StackFormView existing=None/> }
}

/// A stack's *Change Stack Template*: the form prefilled with its template and values.
#[component]
pub fn StackChangePage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(id, get_stack);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), |td| view! { <StackFormView existing=Some(td.data)/> })}
        </Transition>
    }
}

/// Both steps of the form, shared by [`StackLaunchPage`] (`existing` is `None`) and
/// [`StackChangePage`] (`existing` is the stack, whose template, timeout, rollback and parameter
/// values prefill it; the name is fixed and blank parameters keep their value). *Next* asks Heat to
/// validate the template and read its parameters; the buttons stay disabled while a request runs,
/// and *Launch*/*Change* also with writes off.
#[component]
fn StackFormView(existing: Option<StackDetail>) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    let update = existing.is_some();
    let current = StoredValue::new(existing.as_ref().map(|d| d.parameters.clone()).unwrap_or_default());
    let stack_id = existing.as_ref().map(|d| d.stack.id.clone()).unwrap_or_default();
    let use_url = RwSignal::new(false);
    let handed = if update { None } else { take_handed() };
    let template = RwSignal::new(
        existing
            .as_ref()
            .map(|d| d.template.clone())
            .or(handed)
            .unwrap_or_default(),
    );
    let url = RwSignal::new(String::new());
    let env = RwSignal::new(String::new());
    let name = RwSignal::new(existing.as_ref().map(|d| d.stack.name.clone()).unwrap_or_default());
    let timeout = RwSignal::new(
        existing
            .as_ref()
            .and_then(|d| d.timeout_mins)
            .map(|m| m.to_string())
            .unwrap_or_else(|| "60".into()),
    );
    let rollback = RwSignal::new(existing.as_ref().is_some_and(|d| !d.disable_rollback));
    let info = RwSignal::new(None::<TemplateInfo>);
    let values = RwSignal::new(HashMap::<String, String>::new());
    let error = RwSignal::new(None::<String>);
    let working = RwSignal::new(false);
    let preview = RwSignal::new(None::<ConsoleReply<Vec<PreviewResource>>>);

    let source = move || TemplateSource {
        template: if use_url.get_untracked() {
            String::new()
        } else {
            template.get_untracked()
        },
        url: use_url.get_untracked().then(|| url.get_untracked()),
        environment: Some(env.get_untracked()).filter(|e| !e.trim().is_empty()),
    };
    let form = move || StackForm {
        name: name.get_untracked(),
        source: source(),
        timeout: timeout.get_untracked(),
        rollback: rollback.get_untracked(),
        values: values.get_untracked(),
        update,
    };
    let next = move |_| {
        error.set(None);
        working.set(true);
        let src = source();
        spawn_local(async move {
            match run_stack_validate(src).await {
                Ok(reply) => {
                    let data = reply.data.unwrap_or_default();
                    values.set(initial_values(&data.params, &current.get_value()));
                    preview.set(None);
                    info.set(Some(data));
                }
                Err(e) => error.set(Some(e.to_string())),
            }
            working.set(false);
        });
    };
    let spec = move || {
        let params = info.with_untracked(|i| i.as_ref().map(|i| i.params.clone()).unwrap_or_default());
        stack_spec(i18n.lang(), &form(), &params)
    };
    let run_preview = move |_| {
        error.set(None);
        let s = match spec() {
            Ok(s) => s,
            Err(e) => return error.set(Some(e)),
        };
        working.set(true);
        spawn_local(async move {
            match run_stack_preview(s).await {
                Ok(r) => preview.set(Some(r)),
                Err(e) => error.set(Some(e.to_string())),
            }
            working.set(false);
        });
    };
    let sid = StoredValue::new(stack_id);
    let launch = move |_| {
        error.set(None);
        let lang = i18n.lang();
        let s = match spec() {
            Ok(s) => s,
            Err(e) => return error.set(Some(e)),
        };
        let (title, intro, then) = if update {
            (
                K::ChangeStackTemplate,
                K::UpdateStackIntro,
                format!("/project/stacks/{}", sid.get_value()),
            )
        } else {
            (K::LaunchStack, K::LaunchStackIntro, "/project/stacks".to_string())
        };
        let action = if update {
            OsAction::StackUpdate {
                id: sid.get_value(),
                spec: s.clone(),
            }
        } else {
            OsAction::StackCreate(s.clone())
        };
        let mut d = Dialog::form(t(lang, title), t(lang, title), vec![], move |_| Ok(action.clone())).then_go(then);
        d.intro = vec![t(lang, intro).to_string()];
        d.selection = vec![s.name];
        dialogs.open(d);
    };

    let template_step = move || {
        view! {
            <section class="card stack-form">
                <h2>{move || i18n.t(K::TemplateSourceLabel)}</h2>
                <div class="seg" role="radiogroup">
                    <button type="button" class="btn btn-sm" aria-pressed=move || (!use_url.get()).to_string()
                        on:click=move |_| use_url.set(false)>{move || i18n.t(K::TemplateDirect)}</button>
                    <button type="button" class="btn btn-sm" aria-pressed=move || use_url.get().to_string()
                        on:click=move |_| use_url.set(true)>{move || i18n.t(K::Url)}</button>
                </div>
                {move || if use_url.get() {
                    view! {
                        <div class="field">
                            <label for="stack-url">{move || i18n.t(K::Url)}<span class="req">" *"</span></label>
                            <input id="stack-url" type="text" placeholder="https://" prop:value=move || url.get()
                                on:input=move |ev| url.set(event_target_value(&ev))/>
                            <small class="muted">{move || i18n.t(K::TemplateUrlHelp)}</small>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="field">
                            <label for="stack-template">{move || i18n.t(K::TemplateData)}<span class="req">" *"</span></label>
                            <FileLoad limit=MAX_TEMPLATE_BYTES on_text=Callback::new(move |s: String| template.set(s))/>
                            <textarea id="stack-template" rows="16" spellcheck="false" prop:value=move || template.get()
                                on:input=move |ev| template.set(event_target_value(&ev))></textarea>
                        </div>
                    }.into_any()
                }}
                <div class="field">
                    <label for="stack-env">{move || i18n.t(K::EnvironmentData)}</label>
                    <FileLoad limit=MAX_TEMPLATE_BYTES on_text=Callback::new(move |s: String| env.set(s))/>
                    <textarea id="stack-env" rows="5" spellcheck="false" prop:value=move || env.get()
                        on:input=move |ev| env.set(event_target_value(&ev))></textarea>
                    {update.then(|| view! { <small class="muted">{move || i18n.t(K::EnvironmentKeep)}</small> })}
                </div>
                <div class="form-actions">
                    <button type="button" class="btn btn-primary" disabled=move || working.get() on:click=next>
                        {move || i18n.t(K::Next)}
                    </button>
                </div>
            </section>
        }
    };

    let param_field = move |p: TemplateParam| {
        let id = format!("stack-param-{}", p.name);
        let n = p.name.clone();
        let get = {
            let n = n.clone();
            move || values.with(|v| v.get(&n).cloned().unwrap_or_default())
        };
        let put = move |v: String| {
            values.update(|m| {
                m.insert(n.clone(), v);
            })
        };
        let label = p.label.clone().unwrap_or_else(|| p.name.clone());
        let required = p.default.is_none() && !update;
        let choices = if p.kind == "boolean" && p.allowed.is_empty() {
            vec!["true".to_string(), "false".to_string()]
        } else {
            p.allowed.clone()
        };
        let control = if !choices.is_empty() {
            view! {
                <select id=id.clone() prop:value=get.clone() on:change=move |ev| put(event_target_value(&ev))>
                    <option value="">"-"</option>
                    {choices.into_iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                </select>
            }
            .into_any()
        } else if p.hidden {
            view! { <input id=id.clone() type="password" autocomplete="new-password" prop:value=get.clone() on:input=move |ev| put(event_target_value(&ev))/> }.into_any()
        } else if p.kind == "json" {
            view! { <textarea id=id.clone() rows="4" prop:value=get.clone() on:input=move |ev| put(event_target_value(&ev))></textarea> }.into_any()
        } else {
            let ty = if p.kind == "number" { "number" } else { "text" };
            view! { <input id=id.clone() type=ty prop:value=get.clone() on:input=move |ev| put(event_target_value(&ev))/> }.into_any()
        };
        let mut help: Vec<String> = p.description.clone().into_iter().collect();
        if p.kind == "comma_delimited_list" {
            help.push(t(i18n.lang(), K::CommaSeparated).to_string());
        }
        view! {
            <div class="field">
                <label for=id>{label}{required.then(|| view! { <span class="req">" *"</span> })}</label>
                {control}
                {(!help.is_empty()).then(|| view! { <small class="muted">{help.join(" ")}</small> })}
            </div>
        }
    };

    let stack_step = move |i: TemplateInfo| {
        let params = i.params.clone();
        let empty = params.is_empty();
        view! {
            <section class="card stack-form">
                <h2>{move || i18n.t(if update { K::ChangeStackTemplate } else { K::LaunchStack })}</h2>
                {i.description.clone().map(|d| view! { <p class="muted">{d}</p> })}
                {i.local.then(|| view! { <p class="alert alert-info">{move || i18n.t(K::StackParamsLocal)}</p> })}
                <div class="field">
                    <label for="stack-name">{move || i18n.t(K::StackName)}<span class="req">" *"</span></label>
                    <input id="stack-name" type="text" prop:value=move || name.get() disabled=update
                        on:input=move |ev| name.set(event_target_value(&ev))/>
                    <small class="muted">{move || i18n.t(K::StackNameHelp)}</small>
                </div>
                <div class="field">
                    <label for="stack-timeout">{move || i18n.t(K::StackTimeoutMinutes)}</label>
                    <input id="stack-timeout" type="number" min="1" prop:value=move || timeout.get()
                        on:input=move |ev| timeout.set(event_target_value(&ev))/>
                </div>
                <div class="field check">
                    <label>
                        <input type="checkbox" prop:checked=move || rollback.get() on:change=move |ev| rollback.set(event_target_checked(&ev))/>
                        {move || i18n.t(K::StackRollback)}
                    </label>
                </div>
                <h2>{move || i18n.t(K::StackParameters)}</h2>
                {update.then(|| view! { <p class="muted">{move || i18n.t(K::StackParamKeep)}</p> })}
                {empty.then(|| view! { <p class="muted">{move || i18n.t(K::NoTemplateParams)}</p> })}
                {params.into_iter().map(param_field).collect_view()}
                <div class="form-actions">
                    <button type="button" class="btn" on:click=move |_| info.set(None)>{move || i18n.t(K::Back)}</button>
                    {(!update).then(|| view! {
                        <button type="button" class="btn" disabled=move || working.get() on:click=run_preview>{move || i18n.t(K::PreviewStack)}</button>
                    })}
                    <button type="button" class="btn btn-primary" disabled=move || working.get() || dialogs.mode.get() == WriteMode::Off
                        on:click=launch>{move || i18n.t(if update { K::ChangeStackTemplate } else { K::LaunchStack })}</button>
                </div>
            </section>
        }
    };

    let preview_view = move || {
        preview.get().map(|r| match r.data {
            None => view! {
                <section class="card stack-form">
                    <h2>{move || i18n.t(K::PreviewStack)}</h2>
                    <p class="muted">{move || i18n.t(K::PreviewDryRun)}</p>
                    <RequestList requests=r.requests/>
                </section>
            }
            .into_any(),
            Some(rows) => view! {
                <section class="card stack-form">
                    <h2>{move || i18n.t(K::PreviewResources)}</h2>
                    <ul class="plain preview-list">
                        {rows.into_iter().map(|x| view! {
                            <li style=format!("padding-left:{}em", x.depth as f64 * 1.5)>
                                <strong>{x.name}</strong>" "<span class="mono muted">{x.kind}</span>
                            </li>
                        }).collect_view()}
                    </ul>
                </section>
            }
            .into_any(),
        })
    };

    view! {
        <PageHeader/>
        {move || error.get().map(|e| view! { <p class="alert alert-error" role="alert">{e}</p> })}
        {move || match info.get() {
            None => template_step().into_any(),
            Some(i) => view! { {stack_step(i)} {preview_view} }.into_any(),
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, default: Option<&str>, hidden: bool) -> TemplateParam {
        TemplateParam {
            name: name.into(),
            label: None,
            description: None,
            kind: "string".into(),
            default: default.map(Into::into),
            hidden,
            allowed: vec![],
        }
    }

    fn form(values: &[(&str, &str)], update: bool) -> StackForm {
        StackForm {
            name: "web".into(),
            source: TemplateSource {
                template: "heat_template_version: 2018-08-31".into(),
                ..TemplateSource::default()
            },
            timeout: "60".into(),
            rollback: false,
            values: values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            update,
        }
    }

    #[test]
    fn launch_needs_parameters_without_defaults() {
        let params = [p("flavor", Some("m1.small"), false), p("db_password", None, true)];
        let l = Lang::EnUs;
        assert!(
            stack_spec(l, &form(&[], false), &params)
                .unwrap_err()
                .starts_with("db_password")
        );
        let s = stack_spec(l, &form(&[("db_password", "x"), ("flavor", " ")], false), &params).unwrap();
        assert_eq!(
            s.parameters,
            [("db_password".to_string(), "x".to_string())],
            "blank: Heat's default"
        );
        assert_eq!(
            (s.hidden.clone(), s.timeout_mins, s.disable_rollback),
            (vec!["db_password".to_string()], Some(60), true)
        );
        assert!(
            stack_spec(l, &form(&[], true), &params).is_ok(),
            "an update keeps what is left blank"
        );
        let mut bad = form(&[("db_password", "x")], false);
        bad.name = "9lives".into();
        assert!(stack_spec(l, &bad, &params).is_err());
        bad.name = "web".into();
        bad.timeout = "0".into();
        assert!(stack_spec(l, &bad, &params).is_err());
    }

    #[test]
    fn initial_values_use_defaults_or_the_current_ones() {
        let params = [p("flavor", Some("m1.small"), false), p("db_password", Some("x"), true)];
        let launch = initial_values(&params, &[]);
        assert_eq!(launch.get("flavor").map(String::as_str), Some("m1.small"));
        assert!(!launch.contains_key("db_password"), "hidden defaults stay hidden");
        let current = [
            ("flavor".to_string(), "m1.large".to_string()),
            ("db_password".to_string(), "******".to_string()),
        ];
        let change = initial_values(&params, &current);
        assert_eq!(change.get("flavor").map(String::as_str), Some("m1.large"));
        assert!(!change.contains_key("db_password"), "Heat's mask is not a value");
    }
}
