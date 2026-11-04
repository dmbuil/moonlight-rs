//! Orchestration › Resource Types and Template Versions (Heat, roadmap 62):
//! what the cloud's Heat accepts in templates. Read-only, GETs only; the type
//! names and versions are the detail pages' path segments.

use leptos::prelude::*;

use super::details::{mono, param, yes_no, Card, DetailHeader, Kv};
use super::lists::link;
use crate::api::*;
use crate::components::dialog::data_version;
use crate::components::table::{col, Col, DataTable};
use crate::components::widgets::{dash, render, PageHeader, Skeleton};
use crate::i18n::{use_i18n, K};
use crate::model::*;

fn type_columns() -> Vec<Col<ResourceTypeRow>> {
    vec![
        col(K::Type, |x: &ResourceTypeRow, _| link(format!("/project/resource_types/{}", x.name), x.name.clone()))
            .sort_text(|x| x.name.clone()),
        col(K::Description, |x: &ResourceTypeRow, _| dash(x.description.clone())),
    ]
}

/// Orchestration › Resource Types: every type this Heat offers.
#[component]
pub fn ResourceTypesPage() -> impl IntoView {
    let data = Resource::new(data_version, |_| list_resource_types());
    let columns = StoredValue::new(type_columns());
    view! {
        <PageHeader/>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable
                    rows=td.data
                    columns=columns.get_value()
                    search=|x: &ResourceTypeRow| format!("{} {}", x.name, x.description.as_deref().unwrap_or(""))
                    timing=(td.millis, td.calls)
                    on_refresh=move |_| data.refetch()
                />
            })}
        </Transition>
    }
}

fn property_columns() -> Vec<Col<TypeProperty>> {
    vec![
        col(K::Name, |p: &TypeProperty, _| mono(p.name.clone())).sort_text(|p| p.name.clone()),
        col(K::Type, |p: &TypeProperty, _| p.kind.clone().into_any()).sort_text(|p| p.kind.clone()),
        col(K::RequiredProp, |p: &TypeProperty, l| yes_no(l, p.required).into_any()).sort_num(|p| p.required as i64),
        col(K::UpdateAllowed, |p: &TypeProperty, l| yes_no(l, p.update_allowed).into_any()).sort_num(|p| p.update_allowed as i64),
        col(K::DefaultValue, |p: &TypeProperty, _| match p.default.clone() {
            Some(d) => mono(d),
            None => "-".into_any(),
        }),
        col(K::Constraints, |p: &TypeProperty, _| {
            if p.constraints.is_empty() {
                "-".into_any()
            } else {
                let items = p.constraints.iter().map(|c| view! { <li class="mono">{c.clone()}</li> }).collect_view();
                view! { <ul class="plain">{items}</ul> }.into_any()
            }
        }),
        col(K::Description, |p: &TypeProperty, _| dash(p.description.clone())),
    ]
}

fn attribute_columns() -> Vec<Col<TypeAttribute>> {
    vec![
        col(K::Name, |a: &TypeAttribute, _| mono(a.name.clone())).sort_text(|a| a.name.clone()),
        col(K::Type, |a: &TypeAttribute, _| dash(a.kind.clone())),
        col(K::Description, |a: &TypeAttribute, _| dash(a.description.clone())),
    ]
}

/// A resource type's schema: properties and attributes as tables, plus the raw JSON.
#[component]
pub fn ResourceTypeDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_resource_type(id));
    let (props, attrs) = (StoredValue::new(property_columns()), StoredValue::new(attribute_columns()));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                view! {
                    <DetailHeader group=K::Orchestration panel=K::ResourceTypes href="/project/resource_types"
                        name=d.name.clone() timing=(td.millis, td.calls) items=Signal::derive(Vec::new)/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Type><span class="mono">{d.name.clone()}</span></Kv>
                            <Kv label=K::Description>{dash(d.description.clone())}</Kv>
                            <Kv label=K::SupportStatus>{dash(d.support.clone())}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Properties)}</h2>
                    <DataTable rows=d.properties columns=props.get_value()
                        search=|p: &TypeProperty| format!("{} {} {}", p.name, p.kind, p.description.as_deref().unwrap_or(""))/>
                    <h2 class="section-title">{move || i18n.t(K::Attributes)}</h2>
                    <DataTable rows=d.attributes columns=attrs.get_value()
                        search=|a: &TypeAttribute| format!("{} {}", a.name, a.description.as_deref().unwrap_or(""))/>
                    <details class="raw-schema">
                        <summary>{move || i18n.t(K::Schema)}</summary>
                        <pre class="console-log">{d.raw}</pre>
                    </details>
                }
            })}
        </Transition>
    }
}

fn version_columns() -> Vec<Col<TemplateVersion>> {
    vec![
        col(K::Version, |v: &TemplateVersion, _| link(format!("/project/template_versions/{}", v.version), v.version.clone()))
            .sort_text(|v| v.version.clone()),
        col(K::Type, |v: &TemplateVersion, _| mono(v.kind.to_uppercase())).sort_text(|v| v.kind.clone()),
        col(K::Aliases, |v: &TemplateVersion, _| dash((!v.aliases.is_empty()).then(|| v.aliases.join(", ")))),
    ]
}

/// Orchestration › Template Versions.
#[component]
pub fn TemplateVersionsPage() -> impl IntoView {
    let data = Resource::new(data_version, |_| list_template_versions());
    let columns = StoredValue::new(version_columns());
    view! {
        <PageHeader/>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable
                    rows=td.data
                    columns=columns.get_value()
                    search=|v: &TemplateVersion| format!("{} {} {}", v.version, v.kind, v.aliases.join(" "))
                    timing=(td.millis, td.calls)
                    on_refresh=move |_| data.refetch()
                />
            })}
        </Transition>
    }
}

fn function_columns() -> Vec<Col<TemplateFunction>> {
    vec![
        col(K::Function, |f: &TemplateFunction, _| mono(f.name.clone())).sort_text(|f| f.name.clone()),
        col(K::Description, |f: &TemplateFunction, _| dash(f.description.clone())),
    ]
}

/// The functions a template version supports.
#[component]
pub fn TemplateVersionDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_template_functions(id));
    let columns = StoredValue::new(function_columns());
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| view! {
                <DetailHeader group=K::Orchestration panel=K::TemplateVersions href="/project/template_versions"
                    name=id() timing=(td.millis, td.calls) items=Signal::derive(Vec::new)/>
                <DataTable rows=td.data columns=columns.get_value()
                    search=|f: &TemplateFunction| format!("{} {}", f.name, f.description.as_deref().unwrap_or(""))/>
            })}
        </Transition>
    }
}
