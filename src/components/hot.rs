//! Pure model of Orchestration › *Template Generator*: a draft
//! template (parameters, resources with their properties, outputs), what it
//! turns into as HOT YAML, and what is wrong with it. No DOM and no I/O, so it
//! is unit-tested; the page is `pages/template_generator.rs`.
//!
//! Horizon's generator (heat-dashboard) offers 18 resource types, each with a
//! hand-written form and fixed links between types. Here the form comes from the
//! type's schema (`GET resource_types/{type}`), so any type works, and a link is
//! a property whose `custom_constraint` names another resource of the draft
//! (`neutron.network` → `{get_resource: net}`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ResourceTypeDetail, TemplateVersion, TypeProperty};

/// The template version used when the cloud's list is unknown: Rocky's, which
/// every Heat Moonlight targets accepts.
pub const DEFAULT_VERSION: &str = "2018-08-31";

/// Parameter types HOT accepts (`parameters.<name>.type`).
pub const PARAM_TYPES: &[&str] = &["string", "number", "boolean", "json", "comma_delimited_list"];

/// The palette: Horizon's generator types (the ones with heat-dashboard's own
/// icon in [`hot_icons`](super::hot_icons)), by service, in Horizon's order of use.
pub const PALETTE: &[(&str, &[&str])] = &[
    ("Nova", &["OS::Nova::Server", "OS::Nova::KeyPair"]),
    (
        "Neutron",
        &[
            "OS::Neutron::Net",
            "OS::Neutron::Subnet",
            "OS::Neutron::Port",
            "OS::Neutron::Router",
            "OS::Neutron::RouterInterface",
            "OS::Neutron::SecurityGroup",
            "OS::Neutron::FloatingIP",
            "OS::Neutron::FloatingIPAssociation",
        ],
    ),
    ("Cinder", &["OS::Cinder::Volume", "OS::Cinder::VolumeAttachment"]),
    (
        "Heat",
        &[
            "OS::Heat::AutoScalingGroup",
            "OS::Heat::ResourceGroup",
            "OS::Heat::ScalingPolicy",
        ],
    ),
    ("Designate", &["OS::Designate::Zone", "OS::Designate::RecordSet"]),
    ("Swift", &["OS::Swift::Container"]),
];

/// A template being built. Kept in the browser's `localStorage` as JSON, so
/// new fields need `#[serde(default)]`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    /// `heat_template_version`, as a date (`2018-08-31`); empty means [`DEFAULT_VERSION`].
    pub version: String,
    pub description: String,
    pub parameters: Vec<DraftParam>,
    pub resources: Vec<DraftResource>,
    pub outputs: Vec<DraftOutput>,
}

/// A template parameter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DraftParam {
    pub name: String,
    /// One of [`PARAM_TYPES`].
    pub kind: String,
    /// As typed; empty means no default (the user must give one at launch).
    pub default: String,
    pub description: String,
    /// `hidden: true`: Heat shows `******` instead of the value.
    pub hidden: bool,
}

/// A resource of the draft.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DraftResource {
    /// The resource's name in the template, unique in the draft.
    pub name: String,
    /// Its Heat type (`OS::Nova::Server`).
    pub kind: String,
    /// The properties that have a value; the others are left out (Heat's default).
    pub props: Vec<DraftProp>,
    /// Explicit `depends_on`; a `get_resource` already implies one.
    pub depends_on: Vec<String>,
    /// Where the node sits on the canvas; `None` until placed.
    pub pos: Option<(f64, f64)>,
}

/// A property with a value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftProp {
    pub name: String,
    /// The schema's type (`string`, `integer`, `number`, `boolean`, `list`, `map`):
    /// how [`PropValue::Text`] is written out.
    pub kind: String,
    pub value: PropValue,
}

/// Where a property's value comes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PropValue {
    /// Typed by the user: a scalar, or JSON for `list` and `map`.
    Text(String),
    /// `{get_param: <name>}`.
    Param(String),
    /// `{get_resource: <name>}`: the other resource's ID, and an implicit dependency.
    Resource(String),
}

/// A template output: one resource attribute (`{get_attr: [resource, attribute]}`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DraftOutput {
    pub name: String,
    pub description: String,
    pub resource: String,
    pub attribute: String,
}

/// Something that makes Heat refuse the template, shown under the preview.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// A template needs at least one resource.
    NoResources,
    /// A resource, parameter or output without a name.
    EmptyName,
    /// Two resources, two parameters or two outputs with this name.
    Duplicate(String),
    /// `(resource, property)`: required by the type and not given.
    MissingRequired(String, String),
    /// `(resource, property, type)`: the text is not a valid value of the type.
    BadValue(String, String, String),
    /// Resources that depend on each other in a circle, first one repeated at the end.
    Cycle(Vec<String>),
    /// An output without a resource or an attribute.
    IncompleteOutput(String),
}

/// The template versions to offer: the cloud's HOT versions as dates, newest
/// first (the list comes sorted so), or [`DEFAULT_VERSION`] when there are none.
pub fn hot_versions(all: &[TemplateVersion]) -> Vec<String> {
    let out: Vec<String> = all
        .iter()
        .filter(|v| v.kind == "hot")
        .filter_map(|v| v.version.strip_prefix("heat_template_version."))
        .map(str::to_string)
        .collect();
    if out.is_empty() {
        vec![DEFAULT_VERSION.to_string()]
    } else {
        out
    }
}

/// `FloatingIPAssociation` → `floating_ip_association`: a name base for new
/// resources of a type.
pub fn snake(resource_type: &str) -> String {
    let last: Vec<char> = resource_type.rsplit("::").next().unwrap_or_default().chars().collect();
    let mut out = String::new();
    for (i, c) in last.iter().enumerate() {
        let prev_lower = i > 0 && (last[i - 1].is_ascii_lowercase() || last[i - 1].is_ascii_digit());
        let next_lower = last.get(i + 1).is_some_and(char::is_ascii_lowercase);
        if c.is_ascii_uppercase() && i > 0 && (prev_lower || (next_lower && last[i - 1].is_ascii_uppercase())) {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    if out.is_empty() { "resource".into() } else { out }
}

impl Draft {
    /// The version written out.
    pub fn version(&self) -> &str {
        if self.version.trim().is_empty() {
            DEFAULT_VERSION
        } else {
            self.version.trim()
        }
    }

    /// Adds a resource of `kind`, named after the type (`server_1`, `server_2`…),
    /// and returns its name.
    pub fn add(&mut self, kind: &str) -> String {
        let base = snake(kind);
        let name = (1..)
            .map(|n| format!("{base}_{n}"))
            .find(|n| self.resource(n).is_none())
            .unwrap_or_default();
        let pos = Some(self.free_spot());
        self.resources.push(DraftResource {
            name: name.clone(),
            kind: kind.to_string(),
            pos,
            ..Default::default()
        });
        name
    }

    /// The resource called `name`, if the draft has one.
    pub fn resource(&self, name: &str) -> Option<&DraftResource> {
        self.resources.iter().find(|r| r.name == name)
    }

    /// The resource called `name`, to edit in place.
    pub fn resource_mut(&mut self, name: &str) -> Option<&mut DraftResource> {
        self.resources.iter_mut().find(|r| r.name == name)
    }

    /// Renames a resource and every reference to it (`get_resource`,
    /// `depends_on`, outputs). Refused (`false`) when `new` is taken or empty.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        let new = new.trim();
        if new.is_empty() || (new != old && self.resource(new).is_some()) || self.resource(old).is_none() {
            return false;
        }
        for r in &mut self.resources {
            if r.name == old {
                r.name = new.to_string();
            }
            for d in &mut r.depends_on {
                if d == old {
                    *d = new.to_string();
                }
            }
            for p in &mut r.props {
                if p.value == PropValue::Resource(old.to_string()) {
                    p.value = PropValue::Resource(new.to_string());
                }
                if let Some(mut v) = p.json() {
                    let mut changed = false;
                    walk_refs_mut(&mut v, &mut |x| {
                        if x == old {
                            *x = new.to_string();
                            changed = true;
                        }
                    });
                    if changed {
                        p.value = PropValue::Text(v.to_string());
                    }
                }
            }
        }
        for o in &mut self.outputs {
            if o.resource == old {
                o.resource = new.to_string();
            }
        }
        true
    }

    /// Removes a resource and every reference to it: properties that pointed
    /// at it go back to unset, list items that did are dropped, outputs on it
    /// lose their resource.
    pub fn remove(&mut self, name: &str) {
        self.resources.retain(|r| r.name != name);
        let others: Vec<String> = self.resources.iter().map(|r| r.name.clone()).collect();
        for from in others {
            self.unlink(&from, name);
        }
        for o in &mut self.outputs {
            if o.resource == name {
                o.resource.clear();
                o.attribute.clear();
            }
        }
    }

    /// Renames parameter `i` and the `get_param`s that use it.
    pub fn rename_param(&mut self, i: usize, new: &str) {
        let Some(old) = self.parameters.get(i).map(|p| p.name.clone()) else {
            return;
        };
        self.parameters[i].name = new.to_string();
        for p in self.resources.iter_mut().flat_map(|r| r.props.iter_mut()) {
            if p.value == PropValue::Param(old.clone()) {
                p.value = PropValue::Param(new.to_string());
            }
        }
    }

    /// Removes parameter `i`; properties that used it go back to unset.
    pub fn remove_param(&mut self, i: usize) {
        if i >= self.parameters.len() {
            return;
        }
        let gone = PropValue::Param(self.parameters.remove(i).name);
        for r in &mut self.resources {
            r.props.retain(|p| p.value != gone);
        }
    }

    /// Resources `name` refers to: `depends_on` plus `get_resource`s (also those
    /// inside `list`/`map` JSON), without repeats.
    pub fn needs(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for e in self.edges().into_iter().filter(|e| e.from == name) {
            if !out.contains(&e.to) {
                out.push(e.to);
            }
        }
        out
    }

    /// Every link between resources, as the canvas draws them: `depends_on`
    /// (dashed), then each property's `get_resource`, labelled with where it
    /// sits (`network`, `networks[].port`). Links to unknown names are kept:
    /// the YAML has them too.
    pub fn edges(&self) -> Vec<Edge> {
        let mut out = vec![];
        for r in &self.resources {
            for d in &r.depends_on {
                out.push(Edge {
                    from: r.name.clone(),
                    to: d.clone(),
                    label: "depends_on".into(),
                    dashed: true,
                });
            }
            for p in &r.props {
                let edge = |to: &str, label: String| Edge {
                    from: r.name.clone(),
                    to: to.to_string(),
                    label,
                    dashed: false,
                };
                match &p.value {
                    PropValue::Resource(x) => out.push(edge(x, p.name.clone())),
                    PropValue::Text(_) => {
                        if let Some(v) = p.json() {
                            let mut found = vec![];
                            json_refs(&v, p.name.clone(), &mut found);
                            out.extend(found.into_iter().map(|(label, to)| edge(&to, label)));
                        }
                    }
                    PropValue::Param(_) => {}
                }
            }
        }
        out
    }

    /// What a link drawn from `from` to `to` can set, most specific first:
    /// string properties whose constraint names `to`'s type (or, without one,
    /// whose name is the type's, `_id` or not: `floatingip_id`), items of the [`NESTED`] lists,
    /// then `depends_on`. Those already pointing at `to` are left out. Uses `from`'s
    /// schema when loaded; without it only the last two kinds.
    pub fn link_options(&self, schemas: &HashMap<String, ResourceTypeDetail>, from: &str, to: &str) -> Vec<Link> {
        let (Some(a), Some(b)) = (self.resource(from), self.resource(to)) else {
            return vec![];
        };
        if from == to {
            return vec![];
        }
        let target = PropValue::Resource(to.to_string());
        let mut out = vec![];
        if let Some(s) = schemas.get(&a.kind) {
            let mut props: Vec<&TypeProperty> = s
                .properties
                .iter()
                .filter(|p| p.kind == "string" && a.prop(&p.name) != Some(&target))
                .filter(|p| match p.custom.as_deref() {
                    Some(c) => constraint_types(c).contains(&b.kind.as_str()),
                    None => names_type(&p.name, &b.kind),
                })
                .collect();
            props.sort_by(|x, y| y.required.cmp(&x.required).then_with(|| x.name.cmp(&y.name)));
            out.extend(props.into_iter().map(|p| Link::Prop(p.name.clone())));
        }
        for (kind, prop, key, types) in NESTED {
            let current = a.props.iter().find(|p| p.name == *prop);
            let points_there = current.and_then(DraftProp::json).is_some_and(|v| {
                let mut found = vec![];
                json_refs(&v, String::new(), &mut found);
                found.iter().any(|(_, x)| x == to)
            });
            if *kind == a.kind
                && types.contains(&b.kind.as_str())
                && !points_there
                && matches!(current.map(|p| &p.value), None | Some(PropValue::Text(_)))
            {
                out.push(Link::Item(prop.to_string(), key.map(str::to_string)));
            }
        }
        if !a.depends_on.iter().any(|d| d == to) {
            out.push(Link::DependsOn);
        }
        out
    }

    /// What a link drawn between `a` and `b` on the canvas should do. One
    /// reference that fits (either way: a link drawn from a port to its server
    /// still means `networks[].port`) is applied without asking; none falls
    /// back to `a` `depends_on` `b`; several are offered, with `depends_on` last.
    pub fn link_choices(
        &self,
        schemas: &HashMap<String, ResourceTypeDetail>,
        a: &str,
        b: &str,
    ) -> Vec<(String, String, Link)> {
        let forward = self.link_options(schemas, a, b);
        let mut refs: Vec<(String, String, Link)> = forward
            .iter()
            .filter(|l| **l != Link::DependsOn)
            .map(|l| (a.to_string(), b.to_string(), l.clone()))
            .collect();
        refs.extend(
            self.link_options(schemas, b, a)
                .into_iter()
                .filter(|l| *l != Link::DependsOn)
                .map(|l| (b.to_string(), a.to_string(), l)),
        );
        if refs.len() != 1 && forward.contains(&Link::DependsOn) {
            refs.push((a.to_string(), b.to_string(), Link::DependsOn));
        }
        refs
    }

    /// Makes `from` refer to `to` as `link` says. `false`, changing nothing, when
    /// it can't: no such resource, or a list property whose text isn't a JSON list.
    pub fn link(&mut self, from: &str, to: &str, link: &Link) -> bool {
        if self.resource(to).is_none() || from == to {
            return false;
        }
        let Some(r) = self.resource_mut(from) else { return false };
        let reference = || serde_json::json!({ "get_resource": to });
        match link {
            Link::Prop(name) => r.set_prop(name, "string", Some(PropValue::Resource(to.to_string()))),
            Link::Item(prop, key) => {
                let mut list = match r.prop(prop) {
                    None => vec![],
                    Some(PropValue::Text(t)) => match serde_json::from_str::<Value>(t) {
                        Ok(Value::Array(a)) => a,
                        _ => return false,
                    },
                    Some(_) => return false,
                };
                list.push(match key {
                    Some(k) => serde_json::json!({ k.as_str(): reference() }),
                    None => reference(),
                });
                r.set_prop(prop, "list", Some(PropValue::Text(Value::Array(list).to_string())));
            }
            Link::DependsOn => {
                if !r.depends_on.iter().any(|d| d == to) {
                    r.depends_on.push(to.to_string());
                }
            }
        }
        true
    }

    /// Removes every link from `from` to `to`: `depends_on`, properties set to
    /// it (unset), list items and map keys that point at it (dropped; a list or
    /// map left empty is unset).
    pub fn unlink(&mut self, from: &str, to: &str) {
        let Some(r) = self.resource_mut(from) else { return };
        r.depends_on.retain(|d| d != to);
        r.props.retain(|p| p.value != PropValue::Resource(to.to_string()));
        for p in &mut r.props {
            if let Some(mut v) = p.json()
                && strip_refs(&mut v, to)
            {
                p.value = PropValue::Text(v.to_string());
            }
        }
        r.props
            .retain(|p| !matches!(&p.value, PropValue::Text(t) if t == "[]" || t == "{}"));
    }

    /// Where a new node goes: the first cell of a grid ([`GRID`] apart) with no
    /// node near it, row by row, [`GRID_COLS`] to a row.
    pub fn free_spot(&self) -> (f64, f64) {
        let taken: Vec<(f64, f64)> = self.positions().into_iter().map(|(_, p)| p).collect();
        free_cell(&taken)
    }

    /// Every resource's place on the canvas: its own, or, for drafts saved
    /// before it had one, a free grid cell in list order.
    pub fn positions(&self) -> Vec<(String, (f64, f64))> {
        let mut taken: Vec<(f64, f64)> = self.resources.iter().filter_map(|r| r.pos).collect();
        self.resources
            .iter()
            .map(|r| {
                let p = r.pos.unwrap_or_else(|| {
                    let p = free_cell(&taken);
                    taken.push(p);
                    p
                });
                (r.name.clone(), p)
            })
            .collect()
    }

    /// A dependency circle, if any: Heat refuses those ("Circular Dependency Found").
    pub fn cycle(&self) -> Option<Vec<String>> {
        // 0 = unvisited, 1 = on the current path, 2 = done.
        fn visit(d: &Draft, n: &str, state: &mut HashMap<String, u8>, path: &mut Vec<String>) -> Option<Vec<String>> {
            match state.get(n) {
                Some(2) => return None,
                Some(1) => {
                    let start = path.iter().position(|p| p == n).unwrap_or_default();
                    let mut c = path[start..].to_vec();
                    c.push(n.to_string());
                    return Some(c);
                }
                _ => {}
            }
            state.insert(n.to_string(), 1);
            path.push(n.to_string());
            for m in d.needs(n).iter().filter(|m| d.resource(m).is_some()) {
                if let Some(c) = visit(d, m, state, path) {
                    return Some(c);
                }
            }
            path.pop();
            state.insert(n.to_string(), 2);
            None
        }
        let mut state = HashMap::new();
        self.resources
            .iter()
            .find_map(|r| visit(self, &r.name, &mut state, &mut vec![]))
    }

    /// What Heat would refuse. `schemas` holds the loaded type schemas; a type
    /// not loaded yet is not checked for required properties.
    pub fn problems(&self, schemas: &HashMap<String, ResourceTypeDetail>) -> Vec<Problem> {
        let mut out = vec![];
        if self.resources.is_empty() {
            out.push(Problem::NoResources);
        }
        let names = [
            self.parameters
                .iter()
                .map(|p| p.name.trim().to_string())
                .collect::<Vec<_>>(),
            self.resources.iter().map(|r| r.name.clone()).collect(),
            self.outputs.iter().map(|o| o.name.trim().to_string()).collect(),
        ];
        for list in names {
            if list.iter().any(String::is_empty) && !out.contains(&Problem::EmptyName) {
                out.push(Problem::EmptyName);
            }
            for (i, n) in list.iter().enumerate() {
                if !n.is_empty() && list[..i].contains(n) && !out.contains(&Problem::Duplicate(n.clone())) {
                    out.push(Problem::Duplicate(n.clone()));
                }
            }
        }
        for r in &self.resources {
            if let Some(schema) = schemas.get(&r.kind) {
                for p in schema.properties.iter().filter(|p| p.required && p.default.is_none()) {
                    if !r.props.iter().any(|x| x.name == p.name) {
                        out.push(Problem::MissingRequired(r.name.clone(), p.name.clone()));
                    }
                }
            }
            for p in &r.props {
                if let PropValue::Text(t) = &p.value
                    && typed(&p.kind, t).is_none()
                {
                    out.push(Problem::BadValue(r.name.clone(), p.name.clone(), p.kind.clone()));
                }
            }
        }
        if let Some(c) = self.cycle() {
            out.push(Problem::Cycle(c));
        }
        for o in &self.outputs {
            if o.resource.is_empty() || o.attribute.trim().is_empty() {
                out.push(Problem::IncompleteOutput(o.name.clone()));
            }
        }
        out
    }

    /// The draft as a HOT template. Written even with [`Problem`]s: a value
    /// that doesn't fit its type goes out as a string, for Heat to name.
    pub fn to_yaml(&self) -> String {
        let mut top = vec![("heat_template_version".to_string(), Y::Raw(self.version().to_string()))];
        if !self.description.trim().is_empty() {
            top.push(("description".into(), Y::S(self.description.trim_end().to_string())));
        }
        if !self.parameters.is_empty() {
            let params = self
                .parameters
                .iter()
                .map(|p| {
                    let kind = if PARAM_TYPES.contains(&p.kind.as_str()) {
                        p.kind.clone()
                    } else {
                        "string".into()
                    };
                    let mut m = vec![("type".to_string(), Y::S(kind.clone()))];
                    if !p.description.trim().is_empty() {
                        m.push(("description".into(), Y::S(p.description.trim().to_string())));
                    }
                    if !p.default.is_empty() {
                        let heat_kind = match kind.as_str() {
                            "json" => "map_or_list",
                            "comma_delimited_list" => "string",
                            k => k,
                        };
                        m.push((
                            "default".into(),
                            typed(heat_kind, &p.default).unwrap_or_else(|| Y::S(p.default.clone())),
                        ));
                    }
                    if p.hidden {
                        m.push(("hidden".into(), Y::Raw("true".into())));
                    }
                    (p.name.trim().to_string(), Y::M(m))
                })
                .collect();
            top.push(("parameters".into(), Y::M(params)));
        }
        let resources = self
            .resources
            .iter()
            .map(|r| {
                let mut m = vec![("type".to_string(), Y::S(r.kind.clone()))];
                if !r.depends_on.is_empty() {
                    m.push((
                        "depends_on".into(),
                        Y::L(r.depends_on.iter().map(|d| Y::S(d.clone())).collect()),
                    ));
                }
                if !r.props.is_empty() {
                    let props = r
                        .props
                        .iter()
                        .map(|p| {
                            let v = match &p.value {
                                PropValue::Text(t) => typed(&p.kind, t).unwrap_or_else(|| Y::S(t.clone())),
                                PropValue::Param(x) => Y::M(vec![("get_param".into(), Y::S(x.clone()))]),
                                PropValue::Resource(x) => Y::M(vec![("get_resource".into(), Y::S(x.clone()))]),
                            };
                            (p.name.clone(), v)
                        })
                        .collect();
                    m.push(("properties".into(), Y::M(props)));
                }
                (r.name.clone(), Y::M(m))
            })
            .collect();
        top.push(("resources".into(), Y::M(resources)));
        if !self.outputs.is_empty() {
            let outputs = self
                .outputs
                .iter()
                .map(|o| {
                    let mut m = vec![];
                    if !o.description.trim().is_empty() {
                        m.push(("description".to_string(), Y::S(o.description.trim().to_string())));
                    }
                    let at = Y::L(vec![Y::S(o.resource.clone()), Y::S(o.attribute.trim().to_string())]);
                    m.push(("value".into(), Y::M(vec![("get_attr".into(), at)])));
                    (o.name.trim().to_string(), Y::M(m))
                })
                .collect();
            top.push(("outputs".into(), Y::M(outputs)));
        }
        let mut out = String::new();
        write_map(&top, 0, &mut out);
        out
    }
}

/// A link between two resources on the canvas: `from` refers to `to`.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// Where the reference sits: `depends_on`, a property (`network`) or a
    /// path inside one (`networks[].port`).
    pub label: String,
    /// `depends_on`: drawn dashed, as an ordering rather than a value.
    pub dashed: bool,
}

/// What drawing a link from one resource to another sets (see [`Draft::link_options`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// A string property, to `{get_resource: to}`.
    Prop(String),
    /// A new item of a list property: `{key: {get_resource: to}}`, or the bare
    /// reference when `key` is `None` (`security_groups`).
    Item(String, Option<String>),
    /// `depends_on: [to]`.
    DependsOn,
}

impl Link {
    /// As the canvas names it: `network`, `networks[].port`, `security_groups[]`, `depends_on`.
    pub fn label(&self) -> String {
        match self {
            Link::Prop(p) => p.clone(),
            Link::Item(p, Some(k)) => format!("{p}[].{k}"),
            Link::Item(p, None) => format!("{p}[]"),
            Link::DependsOn => "depends_on".into(),
        }
    }
}

/// The edges merged per pair of resources, labels joined (`depends_on,
/// networks[].port`), dashed only when every one is `depends_on`: one arrow
/// each on the canvas.
pub fn edge_pairs(edges: &[Edge]) -> Vec<Edge> {
    let mut out: Vec<Edge> = vec![];
    for e in edges {
        match out.iter_mut().find(|o| o.from == e.from && o.to == e.to) {
            Some(o) => {
                o.label = format!("{}, {}", o.label, e.label);
                o.dashed &= e.dashed;
            }
            None => out.push(e.clone()),
        }
    }
    out
}

/// Whether an unconstrained property's name says it holds a resource of
/// `kind`: the type's name, underscores aside, maybe followed by `id`
/// (`floatingip_id` for `OS::Neutron::FloatingIP`, `port` or `port_id` for a
/// port), and not just any name containing it (`software_config_transport`).
fn names_type(prop: &str, kind: &str) -> bool {
    let (name, ty) = (prop.replace('_', ""), snake(kind).replace('_', ""));
    name == ty || name == format!("{ty}id")
}

/// Where an edge's line and label go, given its trimmed segment. A pair that
/// also has an edge the other way (`reverse`) is moved `EDGE_GAP` to its right,
/// so the two lines don't overlap: the other one, pointing back, goes to its
/// own right, the opposite side. Its label goes further out on that side,
/// starting or ending there when the line is rather vertical (`text-anchor`);
/// otherwise the label sits just above the middle of the line.
pub fn edge_place(seg: ((f64, f64), (f64, f64)), reverse: bool) -> EdgePlace {
    let ((x1, y1), (x2, y2)) = seg;
    let mid = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
    if !reverse {
        return EdgePlace {
            line: seg,
            label: (mid.0, mid.1 - 5.0),
            anchor: "middle",
        };
    }
    let d = (x2 - x1).hypot(y2 - y1).max(f64::EPSILON);
    // Screen y grows downwards, so (-dy, dx) is to the right of the direction.
    let (px, py) = (-(y2 - y1) / d, (x2 - x1) / d);
    let (ox, oy) = (px * EDGE_GAP, py * EDGE_GAP);
    let line = ((x1 + ox, y1 + oy), (x2 + ox, y2 + oy));
    let (lx, ly) = (mid.0 + px * EDGE_GAP * 2.0, mid.1 + py * EDGE_GAP * 2.0);
    if px.abs() > py.abs() {
        // Beside a rather vertical line: the text runs away from it, centred on it.
        EdgePlace {
            line,
            label: (lx, ly + 3.5),
            anchor: if px > 0.0 { "start" } else { "end" },
        }
    } else {
        // Above or below a rather horizontal one; the baseline is the text's bottom.
        EdgePlace {
            line,
            label: (lx, ly + if py > 0.0 { 9.0 } else { -2.0 }),
            anchor: "middle",
        }
    }
}

/// Where [`edge_place`] puts an edge: its line, its label and the label's
/// SVG `text-anchor`.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgePlace {
    /// From and to: the segment given, shifted [`EDGE_GAP`] to its right when the edge has a
    /// reverse twin, so the two don't overlap.
    pub line: ((f64, f64), (f64, f64)),
    /// Where the label's text anchor sits: above the middle of the line, or beside it, away from
    /// the twin, for a shifted one.
    pub label: (f64, f64),
    pub anchor: &'static str,
}

/// How far apart two edges between the same nodes, one each way, are drawn.
pub const EDGE_GAP: f64 = 5.0;

/// The node within `r` of `at`, the last drawn (topmost) when they overlap.
pub fn node_at(positions: &[(String, (f64, f64))], at: (f64, f64), r: f64) -> Option<String> {
    positions
        .iter()
        .rev()
        .find(|(_, (x, y))| (x - at.0).hypot(y - at.1) <= r)
        .map(|(n, _)| n.clone())
}

/// List properties whose items refer to other resources: `(type, property,
/// item key, target types)`. Heat's schema doesn't say what goes inside a
/// list, so these are the links Horizon's generator draws for them.
pub const NESTED: &[(&str, &str, Option<&str>, &[&str])] = &[
    ("OS::Nova::Server", "networks", Some("port"), &["OS::Neutron::Port"]),
    (
        "OS::Nova::Server",
        "networks",
        Some("network"),
        &["OS::Neutron::Net", "OS::Neutron::ProviderNet"],
    ),
    ("OS::Nova::Server", "networks", Some("subnet"), &["OS::Neutron::Subnet"]),
    (
        "OS::Nova::Server",
        "networks",
        Some("floating_ip"),
        &["OS::Neutron::FloatingIP"],
    ),
    (
        "OS::Nova::Server",
        "block_device_mapping_v2",
        Some("volume_id"),
        &["OS::Cinder::Volume"],
    ),
    (
        "OS::Nova::Server",
        "security_groups",
        None,
        &["OS::Neutron::SecurityGroup"],
    ),
    (
        "OS::Neutron::Port",
        "fixed_ips",
        Some("subnet"),
        &["OS::Neutron::Subnet"],
    ),
    (
        "OS::Neutron::Port",
        "security_groups",
        None,
        &["OS::Neutron::SecurityGroup"],
    ),
];

/// Distance between grid cells for new nodes, in canvas units.
pub const GRID: f64 = 130.0;
/// Grid cells per row.
pub const GRID_COLS: usize = 6;

/// The first grid cell, row by row, with no point of `taken` within half a cell of it (see
/// [`Draft::free_spot`]).
fn free_cell(taken: &[(f64, f64)]) -> (f64, f64) {
    (0..)
        .map(|i| {
            (
                GRID * (0.6 + (i % GRID_COLS) as f64),
                GRID * (0.6 + (i / GRID_COLS) as f64),
            )
        })
        .find(|(x, y)| {
            !taken
                .iter()
                .any(|(a, b)| (a - x).abs() < GRID / 2.0 && (b - y).abs() < GRID / 2.0)
        })
        .unwrap_or_default()
}

/// The name a `{"get_resource": name}` object refers to.
fn reference(v: &Value) -> Option<&str> {
    match v {
        Value::Object(o) if o.len() == 1 => o.get("get_resource")?.as_str(),
        _ => None,
    }
}

/// The `get_resource`s inside a JSON value, each with its path (`networks[].port`).
fn json_refs(v: &Value, path: String, out: &mut Vec<(String, String)>) {
    if let Some(to) = reference(v) {
        out.push((path, to.to_string()));
        return;
    }
    match v {
        Value::Array(a) => a.iter().for_each(|x| json_refs(x, format!("{path}[]"), out)),
        Value::Object(o) => o.iter().for_each(|(k, x)| json_refs(x, format!("{path}.{k}"), out)),
        _ => {}
    }
}

/// Calls `f` on the name of every `get_resource` inside a JSON value.
fn walk_refs_mut(v: &mut Value, f: &mut impl FnMut(&mut String)) {
    if let Value::Object(o) = v
        && o.len() == 1
        && let Some(Value::String(name)) = o.get_mut("get_resource")
    {
        return f(name);
    }
    match v {
        Value::Array(a) => a.iter_mut().for_each(|x| walk_refs_mut(x, f)),
        Value::Object(o) => o.values_mut().for_each(|x| walk_refs_mut(x, f)),
        _ => {}
    }
}

/// Drops the references to `name` inside `v`: list items that are or hold one
/// (`{port: {get_resource: name}}` goes whole), and map keys whose value is one.
/// Returns whether anything changed.
fn strip_refs(v: &mut Value, name: &str) -> bool {
    let points = |x: &Value| reference(x) == Some(name);
    let holds = |x: &Value| points(x) || matches!(x, Value::Object(o) if o.values().any(points));
    match v {
        Value::Array(a) => {
            let before = a.len();
            a.retain(|x| !holds(x));
            let mut changed = a.len() != before;
            for x in a.iter_mut() {
                changed |= strip_refs(x, name);
            }
            changed
        }
        Value::Object(o) => {
            let before = o.len();
            o.retain(|_, x| !points(x));
            let mut changed = o.len() != before;
            for x in o.values_mut() {
                changed |= strip_refs(x, name);
            }
            changed
        }
        _ => false,
    }
}

impl DraftProp {
    /// The value as JSON, for `list` and `map` text that parses; `None` otherwise.
    fn json(&self) -> Option<Value> {
        match &self.value {
            PropValue::Text(t) if matches!(self.kind.as_str(), "list" | "map") => serde_json::from_str(t).ok(),
            _ => None,
        }
    }
}

impl DraftResource {
    /// The value of property `name`; `None` when it is unset (Heat's default applies).
    pub fn prop(&self, name: &str) -> Option<&PropValue> {
        self.props.iter().find(|p| p.name == name).map(|p| &p.value)
    }

    /// Sets property `name` (of schema type `kind`); `None`, or empty text, unsets it.
    pub fn set_prop(&mut self, name: &str, kind: &str, value: Option<PropValue>) {
        let value = value.filter(|v| !matches!(v, PropValue::Text(t) if t.is_empty()));
        match (self.props.iter().position(|p| p.name == name), value) {
            (Some(i), Some(v)) => self.props[i].value = v,
            (Some(i), None) => {
                self.props.remove(i);
            }
            (None, Some(v)) => self.props.push(DraftProp {
                name: name.to_string(),
                kind: kind.to_string(),
                value: v,
            }),
            (None, None) => {}
        }
    }
}

/// The resource types a `custom_constraint` names, for `get_resource`
/// candidates; empty when it names none a template creates (`nova.flavor`).
pub fn constraint_types(custom: &str) -> &'static [&'static str] {
    match custom {
        "neutron.network" => &["OS::Neutron::Net", "OS::Neutron::ProviderNet"],
        "neutron.subnet" => &["OS::Neutron::Subnet"],
        "neutron.port" => &["OS::Neutron::Port"],
        "neutron.router" => &["OS::Neutron::Router"],
        "neutron.security_group" => &["OS::Neutron::SecurityGroup"],
        "neutron.qos_policy" => &["OS::Neutron::QoSPolicy"],
        "neutron.address_scope" => &["OS::Neutron::AddressScope"],
        "neutron.subnetpool" => &["OS::Neutron::SubnetPool"],
        "nova.server" => &["OS::Nova::Server"],
        "nova.keypair" => &["OS::Nova::KeyPair"],
        "nova.flavor" => &["OS::Nova::Flavor"],
        "cinder.volume" => &["OS::Cinder::Volume"],
        "cinder.snapshot" => &["OS::Cinder::VolumeSnapshot"],
        "cinder.vtype" => &["OS::Cinder::VolumeType"],
        "glance.image" => &["OS::Glance::WebImage"],
        "designate.zone" => &["OS::Designate::Zone"],
        "octavia.loadbalancer" => &["OS::Octavia::LoadBalancer"],
        "octavia.listener" => &["OS::Octavia::Listener"],
        "octavia.pool" => &["OS::Octavia::Pool"],
        "keystone.project" => &["OS::Keystone::Project"],
        "keystone.user" => &["OS::Keystone::User"],
        "keystone.group" => &["OS::Keystone::Group"],
        "keystone.role" => &["OS::Keystone::Role"],
        "keystone.domain" => &["OS::Keystone::Domain"],
        _ => &[],
    }
}

/// The resources a string property of `from` may point at with `get_resource`:
/// those of the types its constraint names, or, for a property without one,
/// every other resource (Heat doesn't check, as with
/// `FloatingIPAssociation.floatingip_id`). Empty for non-string properties.
pub fn ref_candidates(draft: &Draft, from: &str, p: &TypeProperty) -> Vec<String> {
    if p.kind != "string" {
        return vec![];
    }
    let types = p.custom.as_deref().map(constraint_types);
    if matches!(types, Some([])) {
        return vec![];
    }
    draft
        .resources
        .iter()
        .filter(|r| r.name != from && types.is_none_or(|t| t.contains(&r.kind.as_str())))
        .map(|r| r.name.clone())
        .collect()
}

/// `text` as a value of Heat type `kind`, or `None` when it isn't one. JSON for
/// `list`/`map` (`map_or_list` takes either, for `json` parameters).
fn typed(kind: &str, text: &str) -> Option<Y> {
    let t = text.trim();
    match kind {
        "integer" => t.parse::<i64>().ok().map(|_| Y::Raw(t.to_string())),
        "number" => t
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|_| Y::Raw(t.to_string())),
        "boolean" => matches!(t, "true" | "false").then(|| Y::Raw(t.to_string())),
        "list" => serde_json::from_str::<Value>(t)
            .ok()
            .filter(Value::is_array)
            .map(|v| from_json(&v)),
        "map" => serde_json::from_str::<Value>(t)
            .ok()
            .filter(Value::is_object)
            .map(|v| from_json(&v)),
        "map_or_list" => serde_json::from_str::<Value>(t)
            .ok()
            .filter(|v| v.is_array() || v.is_object())
            .map(|v| from_json(&v)),
        _ => Some(Y::S(text.to_string())),
    }
}

// ---------------------------------------------------------------------------
// A small YAML writer: block style, flow for intrinsic functions and short lists
// ---------------------------------------------------------------------------

/// A YAML node, keys in the order given (the template's order, not sorted).
#[derive(Clone, Debug, PartialEq)]
enum Y {
    /// A string, quoted when YAML would read it as something else.
    S(String),
    /// Written as is: numbers, booleans, the version date.
    Raw(String),
    /// A list, flow style (`[a, b]`) when short and flat, else one `- ` item per line.
    L(Vec<Y>),
    /// A map, keys in the order given.
    M(Vec<(String, Y)>),
}

/// A JSON value as a YAML node: strings stay strings (quoted only if needed), numbers and
/// booleans are written as they are.
fn from_json(v: &Value) -> Y {
    match v {
        Value::String(s) => Y::S(s.clone()),
        Value::Array(a) => Y::L(a.iter().map(from_json).collect()),
        Value::Object(o) => Y::M(o.iter().map(|(k, v)| (k.clone(), from_json(v))).collect()),
        other => Y::Raw(other.to_string()),
    }
}

/// Whether `s` reads back as the same string without quotes. Errs on the side
/// of quoting: anything that YAML 1.1 (PyYAML, Heat's parser) could take for a
/// number, boolean, null, date or syntax is quoted.
fn plain(s: &str) -> bool {
    let Some(first) = s.chars().next() else { return false };
    let lower = s.to_ascii_lowercase();
    // From a digit, `+` or `.`, YAML 1.1 may read a number (`1e3`, `0x1F`,
    // `1_000`), a date (`2024-01-01`) or a base-60 time (`1:20`): quoted unless
    // some character rules all of those out (`10.0.0.0/24`, `m1` is not affected).
    let numberish = (first.is_ascii_digit() || "+.".contains(first))
        && s.chars().all(|c| c.is_ascii_hexdigit() || "xXoO._:+- ".contains(c));
    s.trim() == s
        && !numberish
        && !s.contains(['\n', '\r', '\t', ',', '[', ']', '{', '}', '#', '"', '\''])
        && !s.contains(": ")
        && !s.ends_with(':')
        && !"-?:&*!|>%@`".contains(first)
        && !matches!(
            lower.as_str(),
            "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n" | "null" | "~"
        )
}

/// `s` as a YAML scalar: plain when [`plain`] allows it, else a JSON string (valid YAML).
fn scalar(s: &str) -> String {
    if plain(s) {
        s.to_string()
    } else {
        serde_json::to_string(s).unwrap_or_default()
    }
}

/// One line per item: `{get_resource: x}`, `[a, b]`.
fn flow(y: &Y) -> String {
    match y {
        Y::S(s) => scalar(s),
        Y::Raw(r) => r.clone(),
        Y::L(l) => format!("[{}]", l.iter().map(flow).collect::<Vec<_>>().join(", ")),
        Y::M(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{}: {}", scalar(k), flow(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Written on one line: intrinsic functions (`get_*`), lists of scalars and empty collections.
fn is_flow(y: &Y) -> bool {
    match y {
        Y::S(_) | Y::Raw(_) => true,
        Y::L(l) => l.iter().all(|x| {
            matches!(x, Y::S(s) if !s.contains('\n')) || matches!(x, Y::Raw(_)) || (matches!(x, Y::M(_)) && is_flow(x))
        }),
        Y::M(m) => m.is_empty() || (m.len() == 1 && m[0].0.starts_with("get_") && is_flow(&m[0].1)),
    }
}

/// A multi-line string as a literal block (`|`, `|-` without a final newline),
/// when it can be one: not starting with a space, at most one final newline.
fn literal(s: &str, indent: usize) -> Option<String> {
    if !s.contains('\n') || s.starts_with([' ', '\t']) || s.ends_with("\n\n") || s.contains('\r') {
        return None;
    }
    let body = s.strip_suffix('\n');
    let mut out = String::from(if body.is_some() { "|" } else { "|-" });
    for line in body.unwrap_or(s).split('\n') {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&" ".repeat(indent));
            out.push_str(line);
        }
    }
    Some(out)
}

/// Appends the entries of `m` at `indent` spaces, one `key:` per line.
fn write_map(m: &[(String, Y)], indent: usize, out: &mut String) {
    for (k, v) in m {
        out.push_str(&" ".repeat(indent));
        out.push_str(&scalar(k));
        out.push(':');
        write_value(v, indent, out);
    }
}

/// Appends what follows a `key:` (or a `-`): a block literal for multi-line text, flow style for
/// intrinsic functions and short lists, else a nested block two spaces in.
fn write_value(v: &Y, indent: usize, out: &mut String) {
    match v {
        Y::S(s) if s.contains('\n') => {
            out.push(' ');
            out.push_str(&literal(s, indent + 2).unwrap_or_else(|| scalar(s)));
            out.push('\n');
        }
        _ if is_flow(v) => {
            out.push(' ');
            out.push_str(&flow(v));
            out.push('\n');
        }
        Y::M(m) => {
            out.push('\n');
            write_map(m, indent + 2, out);
        }
        Y::L(l) => {
            out.push('\n');
            for item in l {
                let dash = format!("{}- ", " ".repeat(indent + 2));
                match item {
                    Y::M(m) if !is_flow(item) => {
                        // Written at the item's indentation, then its first line's
                        // indent becomes the dash.
                        let mut tmp = String::new();
                        write_map(m, indent + 4, &mut tmp);
                        out.push_str(&dash);
                        out.push_str(&tmp[indent + 4..]);
                    }
                    Y::S(s) if s.contains('\n') => {
                        out.push_str(&dash);
                        out.push_str(&literal(s, indent + 4).unwrap_or_else(|| scalar(s)));
                        out.push('\n');
                    }
                    _ => {
                        out.push_str(&dash);
                        out.push_str(&flow(item));
                        out.push('\n');
                    }
                }
            }
        }
        Y::S(_) | Y::Raw(_) => unreachable!("scalars are flow"),
    }
}

/// `text` as a `data:` URL, for a download link that needs no server round trip.
pub fn data_url(mime: &str, text: &str) -> String {
    let mut out = format!("data:{mime};charset=utf-8,");
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(name: &str, kind: &str, required: bool, custom: Option<&str>) -> TypeProperty {
        TypeProperty {
            name: name.into(),
            kind: kind.into(),
            required,
            update_allowed: true,
            default: None,
            description: None,
            constraints: vec![],
            allowed: vec![],
            custom: custom.map(str::to_string),
        }
    }

    fn schema(name: &str, props: Vec<TypeProperty>) -> ResourceTypeDetail {
        ResourceTypeDetail {
            name: name.into(),
            description: None,
            support: None,
            properties: props,
            attributes: vec![],
            raw: String::new(),
        }
    }

    /// A network, a subnet on it, a server with a port: the usual first template.
    fn sample() -> Draft {
        let mut d = Draft {
            description: "Demo".into(),
            ..Default::default()
        };
        let net = d.add("OS::Neutron::Net");
        let sub = d.add("OS::Neutron::Subnet");
        let srv = d.add("OS::Nova::Server");
        d.parameters.push(DraftParam {
            name: "image".into(),
            kind: "string".into(),
            default: "cirros".into(),
            ..Default::default()
        });
        let s = d.resource_mut(&sub).unwrap();
        s.set_prop("network", "string", Some(PropValue::Resource(net.clone())));
        s.set_prop("cidr", "string", Some(PropValue::Text("10.0.0.0/24".into())));
        s.set_prop("enable_dhcp", "boolean", Some(PropValue::Text("true".into())));
        let v = d.resource_mut(&srv).unwrap();
        v.set_prop("image", "string", Some(PropValue::Param("image".into())));
        v.set_prop("flavor", "string", Some(PropValue::Text("m1.small".into())));
        v.set_prop(
            "networks",
            "list",
            Some(PropValue::Text(r#"[{"network": {"get_resource": "net_1"}}]"#.into())),
        );
        v.depends_on.push(sub);
        d.outputs.push(DraftOutput {
            name: "ip".into(),
            description: "Address".into(),
            resource: srv,
            attribute: "first_address".into(),
        });
        d
    }

    #[test]
    fn names_come_from_the_type_and_stay_unique() {
        assert_eq!(snake("OS::Neutron::FloatingIPAssociation"), "floating_ip_association");
        assert_eq!(snake("OS::Nova::KeyPair"), "key_pair");
        assert_eq!(snake("OS::Neutron::Net"), "net");
        assert_eq!(snake("OS::Heat::SoftwareConfig"), "software_config");
        let mut d = Draft::default();
        assert_eq!(
            (d.add("OS::Nova::Server"), d.add("OS::Nova::Server")),
            ("server_1".into(), "server_2".into())
        );
        d.remove("server_1");
        assert_eq!(d.add("OS::Nova::Server"), "server_1", "a freed name is reused");
    }

    #[test]
    fn writes_a_hot_template_in_order() {
        let y = sample().to_yaml();
        let expected = "\
heat_template_version: 2018-08-31
description: Demo
parameters:
  image:
    type: string
    default: cirros
resources:
  net_1:
    type: OS::Neutron::Net
  subnet_1:
    type: OS::Neutron::Subnet
    properties:
      network: {get_resource: net_1}
      cidr: 10.0.0.0/24
      enable_dhcp: true
  server_1:
    type: OS::Nova::Server
    depends_on: [subnet_1]
    properties:
      image: {get_param: image}
      flavor: m1.small
      networks:
        - network: {get_resource: net_1}
outputs:
  ip:
    description: Address
    value: {get_attr: [server_1, first_address]}
";
        assert_eq!(y, expected);
    }

    #[test]
    fn quotes_strings_yaml_would_read_as_something_else() {
        for s in [
            "true",
            "No",
            "n",
            "null",
            "~",
            "8080",
            "1e3",
            "0x1F",
            "2024-01-01",
            "1:20",
            "10.0.0.1",
            "-x",
            "a: b",
            "x #y",
            "*a",
            "",
            " lead",
            "[a]",
            "it's",
        ] {
            assert!(!plain(s), "{s:?} must be quoted");
        }
        for s in [
            "m1.small",
            "OS::Nova::Server",
            "10.0.0.0/24",
            "1st",
            "a-b_c",
            "https://x/y?z=1",
            "nombre-español",
        ] {
            assert!(plain(s), "{s:?} can be plain");
        }
        assert_eq!(scalar("8080"), "\"8080\"");
        assert_eq!(scalar("say \"hi\""), r#""say \"hi\"""#);
    }

    #[test]
    fn multi_line_strings_are_literal_blocks() {
        let mut d = Draft::default();
        let s = d.add("OS::Nova::Server");
        let r = d.resource_mut(&s).unwrap();
        r.set_prop(
            "user_data",
            "string",
            Some(PropValue::Text("#!/bin/sh\necho hi\n\nexit 0\n".into())),
        );
        r.set_prop("name", "string", Some(PropValue::Text("a\n b".into())));
        let y = d.to_yaml();
        assert!(
            y.contains("      user_data: |\n        #!/bin/sh\n        echo hi\n\n        exit 0\n"),
            "{y}"
        );
        assert!(
            y.contains("      name: |-\n        a\n         b\n"),
            "no final newline → |-; {y}"
        );
        let lead = Draft {
            description: " indented\nsecond".into(),
            resources: d.resources.clone(),
            ..Default::default()
        };
        assert!(
            lead.to_yaml().contains("description: \" indented\\nsecond\"\n"),
            "a leading space can't start a block"
        );
    }

    #[test]
    fn typed_values_and_bad_ones() {
        assert_eq!(typed("integer", " 3 "), Some(Y::Raw("3".into())));
        assert_eq!(typed("integer", "3.5"), None);
        assert_eq!(typed("number", "3.5"), Some(Y::Raw("3.5".into())));
        assert_eq!(typed("number", "NaN"), None);
        assert_eq!(typed("boolean", "yes"), None, "only true/false");
        assert_eq!(typed("list", r#"{"a": 1}"#), None, "a map is not a list");
        assert_eq!(
            typed("map", r#"{"a": 1}"#),
            Some(Y::M(vec![("a".into(), Y::Raw("1".into()))]))
        );
        assert_eq!(typed("string", "8080"), Some(Y::S("8080".into())));
        let mut d = sample();
        d.resource_mut("server_1")
            .unwrap()
            .set_prop("networks", "list", Some(PropValue::Text("[oops".into())));
        let problems = d.problems(&HashMap::new());
        assert_eq!(
            problems,
            [Problem::BadValue("server_1".into(), "networks".into(), "list".into())]
        );
        assert!(
            d.to_yaml().contains("      networks: \"[oops\"\n"),
            "written as a string for Heat to name"
        );
    }

    #[test]
    fn json_parameter_defaults_and_hidden() {
        let mut d = sample();
        d.parameters = vec![
            DraftParam {
                name: "cfg".into(),
                kind: "json".into(),
                default: r#"{"k": [1, 2]}"#.into(),
                ..Default::default()
            },
            DraftParam {
                name: "pw".into(),
                kind: "string".into(),
                hidden: true,
                description: "Secret".into(),
                ..Default::default()
            },
            DraftParam {
                name: "count".into(),
                kind: "number".into(),
                default: "2".into(),
                ..Default::default()
            },
        ];
        let y = d.to_yaml();
        assert!(
            y.contains("  cfg:\n    type: json\n    default:\n      k: [1, 2]\n"),
            "{y}"
        );
        assert!(
            y.contains("  pw:\n    type: string\n    description: Secret\n    hidden: true\n"),
            "{y}"
        );
        assert!(y.contains("  count:\n    type: number\n    default: 2\n"), "{y}");
    }

    #[test]
    fn rename_and_remove_follow_references() {
        let mut d = sample();
        assert!(!d.rename("net_1", "subnet_1"), "taken");
        assert!(!d.rename("net_1", " "), "empty");
        assert!(d.rename("server_1", "web"));
        assert!(d.rename("subnet_1", "sub"));
        assert_eq!(d.resource("web").unwrap().depends_on, ["sub"]);
        assert_eq!(d.outputs[0].resource, "web");
        d.remove("net_1");
        assert_eq!(
            d.resource("sub").unwrap().prop("network"),
            None,
            "get_resource to it is unset"
        );
        d.remove("sub");
        assert!(d.resource("web").unwrap().depends_on.is_empty());
        d.rename_param(0, "img");
        assert_eq!(
            d.resource("web").unwrap().prop("image"),
            Some(&PropValue::Param("img".into()))
        );
        d.remove_param(0);
        assert_eq!(d.resource("web").unwrap().prop("image"), None);
        d.remove("web");
        assert_eq!(
            (d.outputs[0].resource.as_str(), d.outputs[0].attribute.as_str()),
            ("", "")
        );
        assert!(
            d.problems(&HashMap::new())
                .contains(&Problem::IncompleteOutput("ip".into()))
        );
    }

    #[test]
    fn finds_dependency_cycles_through_both_kinds_of_link() {
        let mut d = sample();
        assert_eq!(d.cycle(), None);
        // net_1 → server_1 (depends_on) → subnet_1 (depends_on) → net_1 (get_resource)
        d.resource_mut("net_1").unwrap().depends_on.push("server_1".into());
        assert_eq!(
            d.cycle(),
            Some(vec![
                "net_1".into(),
                "server_1".into(),
                "subnet_1".into(),
                "net_1".into()
            ])
        );
        assert!(matches!(d.problems(&HashMap::new()).last(), Some(Problem::Cycle(_))));
    }

    #[test]
    fn problems_name_what_heat_would_refuse() {
        assert_eq!(Draft::default().problems(&HashMap::new()), [Problem::NoResources]);
        let mut d = sample();
        let schemas = HashMap::from([(
            "OS::Nova::Server".to_string(),
            schema(
                "OS::Nova::Server",
                vec![
                    prop("flavor", "string", true, Some("nova.flavor")),
                    prop("key_name", "string", true, None),
                ],
            ),
        )]);
        assert_eq!(
            d.problems(&schemas),
            [Problem::MissingRequired("server_1".into(), "key_name".into())]
        );
        d.parameters.push(DraftParam {
            name: "image".into(),
            ..Default::default()
        });
        d.outputs.push(DraftOutput {
            name: " ".into(),
            resource: "net_1".into(),
            attribute: "name".into(),
            ..Default::default()
        });
        let p = d.problems(&schemas);
        assert!(
            p.contains(&Problem::Duplicate("image".into())) && p.contains(&Problem::EmptyName),
            "{p:?}"
        );
    }

    #[test]
    fn references_offer_the_types_the_constraint_names() {
        let mut d = sample();
        d.add("OS::Neutron::ProviderNet");
        let network = prop("network", "string", false, Some("neutron.network"));
        assert_eq!(ref_candidates(&d, "subnet_1", &network), ["net_1", "provider_net_1"]);
        assert!(
            ref_candidates(&d, "server_1", &prop("flavor", "string", true, Some("nova.flavor"))).is_empty(),
            "no flavor in the draft"
        );
        assert!(ref_candidates(&d, "x", &prop("image", "string", false, Some("glance.image"))).is_empty());
        assert_eq!(
            ref_candidates(&d, "net_1", &prop("floatingip_id", "string", true, None)).len(),
            3,
            "unconstrained: any other"
        );
        assert!(
            ref_candidates(&d, "net_1", &prop("networks", "list", false, None)).is_empty(),
            "only strings"
        );
    }

    #[test]
    fn edges_include_references_inside_lists() {
        let d = sample();
        let e = d.edges();
        let pairs: Vec<(&str, &str, &str, bool)> = e
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.label.as_str(), e.dashed))
            .collect();
        assert_eq!(
            pairs,
            [
                ("subnet_1", "net_1", "network", false),
                ("server_1", "subnet_1", "depends_on", true),
                ("server_1", "net_1", "networks[].network", false)
            ]
        );
        assert_eq!(
            d.needs("server_1"),
            ["subnet_1", "net_1"],
            "the JSON reference is a dependency too"
        );
    }

    #[test]
    fn drawing_a_link_offers_what_fits_the_target() {
        let mut d = sample();
        let port = d.add("OS::Neutron::Port");
        let schemas = HashMap::from([
            (
                "OS::Neutron::Port".to_string(),
                schema(
                    "OS::Neutron::Port",
                    vec![
                        prop("network", "string", true, Some("neutron.network")),
                        prop("name", "string", false, None),
                    ],
                ),
            ),
            (
                "OS::Neutron::FloatingIPAssociation".to_string(),
                schema(
                    "x",
                    vec![
                        prop("floatingip_id", "string", true, None),
                        prop("port_id", "string", true, Some("neutron.port")),
                    ],
                ),
            ),
        ]);
        assert_eq!(
            d.link_options(&schemas, &port, "net_1"),
            [Link::Prop("network".into()), Link::DependsOn]
        );
        assert!(d.link(&port, "net_1", &Link::Prop("network".into())));
        assert_eq!(
            d.link_options(&schemas, &port, "net_1"),
            [Link::DependsOn],
            "already set"
        );
        // Server → Port: an item of `networks`, which already holds a network.
        assert_eq!(
            d.link_options(&schemas, "server_1", &port),
            [Link::Item("networks".into(), Some("port".into())), Link::DependsOn]
        );
        assert!(d.link("server_1", &port, &Link::Item("networks".into(), Some("port".into()))));
        assert_eq!(
            d.resource("server_1").unwrap().prop("networks"),
            Some(&PropValue::Text(
                r#"[{"network":{"get_resource":"net_1"}},{"port":{"get_resource":"port_1"}}]"#.into()
            ))
        );
        assert!(
            d.to_yaml().contains(
                "      networks:\n        - network: {get_resource: net_1}\n        - port: {get_resource: port_1}\n"
            ),
            "{}",
            d.to_yaml()
        );
        // Unconstrained properties match by name; the type's own name is enough.
        let fip = d.add("OS::Neutron::FloatingIP");
        let assoc = d.add("OS::Neutron::FloatingIPAssociation");
        assert_eq!(
            d.link_options(&schemas, &assoc, &fip),
            [Link::Prop("floatingip_id".into()), Link::DependsOn]
        );
        assert!(names_type("port_id", "OS::Neutron::Port") && names_type("port", "OS::Neutron::Port"));
        assert!(
            !names_type("software_config_transport", "OS::Neutron::Port"),
            "a name merely containing the type's"
        );
        assert_eq!(
            d.link_options(&schemas, &assoc, &port),
            [Link::Prop("port_id".into()), Link::DependsOn]
        );
        // Without a schema: only the fixed list items and depends_on.
        assert_eq!(d.link_options(&HashMap::new(), &port, "net_1"), [Link::DependsOn]);
        assert!(d.link_options(&schemas, &port, &port).is_empty(), "not to itself");
        // A list that isn't JSON can't take an item.
        d.resource_mut("server_1")
            .unwrap()
            .set_prop("networks", "list", Some(PropValue::Text("[oops".into())));
        assert!(!d.link("server_1", &port, &Link::Item("networks".into(), Some("port".into()))));
    }

    #[test]
    fn unlink_rename_and_remove_reach_inside_lists() {
        let mut d = sample();
        let port = d.add("OS::Neutron::Port");
        d.link("server_1", &port, &Link::Item("networks".into(), Some("port".into())));
        d.link("server_1", &port, &Link::DependsOn);
        assert!(d.rename(&port, "web_port"));
        let nets = |d: &Draft| d.resource("server_1").unwrap().prop("networks").cloned();
        assert_eq!(
            nets(&d),
            Some(PropValue::Text(
                r#"[{"network":{"get_resource":"net_1"}},{"port":{"get_resource":"web_port"}}]"#.into()
            ))
        );
        d.unlink("server_1", "web_port");
        assert_eq!(
            nets(&d),
            Some(PropValue::Text(r#"[{"network":{"get_resource":"net_1"}}]"#.into()))
        );
        assert_eq!(
            d.resource("server_1").unwrap().depends_on,
            ["subnet_1"],
            "depends_on on it went too"
        );
        d.remove("net_1");
        assert_eq!(nets(&d), None, "a list left empty is unset");
        assert_eq!(d.resource("subnet_1").unwrap().prop("network"), None);
    }

    #[test]
    fn a_drawn_link_applies_the_only_fit_or_asks() {
        let mut d = sample();
        let port = d.add("OS::Neutron::Port");
        let sg = d.add("OS::Neutron::SecurityGroup");
        let schemas = HashMap::from([(
            "OS::Neutron::Port".to_string(),
            schema(
                "OS::Neutron::Port",
                vec![prop("network", "string", true, Some("neutron.network"))],
            ),
        )]);
        let one = |from: &str, to: &str, l: Link| vec![(from.to_string(), to.to_string(), l)];
        assert_eq!(
            d.link_choices(&schemas, &port, "net_1"),
            one(&port, "net_1", Link::Prop("network".into()))
        );
        // Drawn from the port to the server, it is still the server's reference.
        assert_eq!(
            d.link_choices(&schemas, &port, "server_1"),
            one("server_1", &port, Link::Item("networks".into(), Some("port".into())))
        );
        assert_eq!(
            d.link_choices(&schemas, "net_1", &sg),
            one("net_1", &sg, Link::DependsOn),
            "nothing fits: depends_on"
        );
        // A server and a security group: the port's list and the server's, both ways round.
        let c = d.link_choices(&schemas, &sg, &port);
        assert_eq!(c, one(&port, &sg, Link::Item("security_groups".into(), None)));
        d.link("server_1", &port, &Link::DependsOn);
        assert!(
            d.link_choices(&HashMap::new(), "server_1", &port).len() == 1,
            "depends_on only offered once"
        );
        assert_eq!(
            d.link_choices(&HashMap::new(), "server_1", "net_1"),
            one("server_1", "net_1", Link::DependsOn),
            "its networks already hold net_1"
        );
    }

    #[test]
    fn the_canvas_draws_one_arrow_per_pair_and_finds_nodes() {
        let mut d = sample();
        d.link("server_1", "net_1", &Link::DependsOn);
        let pairs = edge_pairs(&d.edges());
        let server_net = pairs.iter().find(|e| e.from == "server_1" && e.to == "net_1").unwrap();
        assert_eq!(
            (server_net.label.as_str(), server_net.dashed),
            ("depends_on, networks[].network", false)
        );
        assert!(
            pairs
                .iter()
                .any(|e| e.from == "server_1" && e.to == "subnet_1" && e.dashed)
        );
        assert_eq!(pairs.len(), 3);
        let pos = d.positions();
        assert_eq!(
            node_at(&pos, (GRID * 0.6 + 10.0, GRID * 0.6 - 10.0), 28.0),
            Some("net_1".into())
        );
        assert_eq!(node_at(&pos, (GRID * 1.1, GRID * 0.6), 28.0), None, "between two nodes");
    }

    #[test]
    fn edges_both_ways_are_drawn_apart() {
        let seg = ((0.0, 0.0), (100.0, 0.0));
        let back = ((100.0, 0.0), (0.0, 0.0));
        // Alone: the line as it is, the label just above its middle.
        assert_eq!(
            edge_place(seg, false),
            EdgePlace {
                line: seg,
                label: (50.0, -5.0),
                anchor: "middle"
            }
        );
        // Both ways: each moves to its own right, so they end up on opposite sides.
        let a = edge_place(seg, true);
        let b = edge_place(back, true);
        assert_eq!(a.line, ((0.0, EDGE_GAP), (100.0, EDGE_GAP)));
        assert_eq!(b.line, ((100.0, -EDGE_GAP), (0.0, -EDGE_GAP)));
        assert!(
            a.label.1 > a.line.0.1 && b.label.1 < b.line.0.1,
            "labels on the outer sides: {a:?} {b:?}"
        );
        assert_eq!((a.label.0, a.anchor), (50.0, "middle"));
        // Vertical: the labels start right of the right line and end left of the left one.
        let (down, up) = (
            edge_place(((0.0, 0.0), (0.0, 100.0)), true),
            edge_place(((0.0, 100.0), (0.0, 0.0)), true),
        );
        assert_eq!((down.line.0.0, down.anchor), (-EDGE_GAP, "end"));
        assert_eq!((up.line.0.0, up.anchor), (EDGE_GAP, "start"));
        assert!(down.label.0 < down.line.0.0 && up.label.0 > up.line.0.0);
    }

    #[test]
    fn new_nodes_take_free_grid_cells() {
        let mut d = Draft::default();
        d.add("OS::Neutron::Net");
        d.add("OS::Neutron::Net");
        let at = |d: &Draft, i: usize| d.resources[i].pos.unwrap();
        assert_eq!(at(&d, 0), (GRID * 0.6, GRID * 0.6));
        assert_eq!(at(&d, 1), (GRID * 1.6, GRID * 0.6));
        // Moved away, its cell is free again.
        d.resources[0].pos = Some((900.0, 900.0));
        d.add("OS::Neutron::Net");
        assert_eq!(at(&d, 2), (GRID * 0.6, GRID * 0.6));
        // A draft saved before positions existed gets cells in list order.
        d.resources[1].pos = None;
        assert_eq!(d.positions()[1].1, (GRID * 1.6, GRID * 0.6));
    }

    #[test]
    fn versions_are_the_clouds_hot_dates() {
        let v = |version: &str, kind: &str| TemplateVersion {
            version: version.into(),
            kind: kind.into(),
            aliases: vec![],
        };
        assert_eq!(
            hot_versions(&[
                v("heat_template_version.2021-04-16", "hot"),
                v("AWSTemplateFormatVersion.2010-09-09", "cfn")
            ]),
            ["2021-04-16"]
        );
        assert_eq!(hot_versions(&[]), [DEFAULT_VERSION]);
        assert_eq!(
            Draft {
                version: " ".into(),
                ..Default::default()
            }
            .version(),
            DEFAULT_VERSION
        );
    }

    #[test]
    fn palette_types_all_have_an_icon() {
        for t in PALETTE.iter().flat_map(|(_, ts)| ts.iter()) {
            assert!(super::super::hot_icons::markup(t).is_some(), "{t}");
        }
        assert_eq!(
            PALETTE.iter().map(|(_, ts)| ts.len()).sum::<usize>(),
            18,
            "Horizon's generator types"
        );
    }

    #[test]
    fn drafts_survive_local_storage() {
        let d = sample();
        assert_eq!(
            serde_json::from_str::<Draft>(&serde_json::to_string(&d).unwrap()).unwrap(),
            d
        );
        let old: Draft = serde_json::from_str(r#"{"resources": [{"name": "a", "kind": "OS::Heat::None"}]}"#).unwrap();
        assert_eq!(old.resources[0].pos, None, "fields added later default");
    }

    #[test]
    fn data_urls_escape_everything_else() {
        assert_eq!(
            data_url("application/x-yaml", "a: b\n#ñ"),
            "data:application/x-yaml;charset=utf-8,a%3A%20b%0A%23%C3%B1"
        );
    }
}
