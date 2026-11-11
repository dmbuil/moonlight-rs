//! Heat: stacks, resource types, templates.

use super::*;

/// A Heat stack, as the *Stacks* list shows it. Heat's times are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackRow {
    pub id: String,
    /// `stack_name` (Heat requires one).
    pub name: String,
    pub description: Option<String>,
    /// Heat's `stack_status`: action and state, e.g. `CREATE_COMPLETE`.
    pub status: String,
    pub status_reason: Option<String>,
    pub created: String,
    /// `None` until the stack is first updated.
    pub updated: Option<String>,
}

/// Whether Heat is still working on the stack or resource (`*_IN_PROGRESS`):
/// pages poll while it is (roadmap 46).
pub fn stack_busy(status: &str) -> bool {
    status.ends_with("_IN_PROGRESS")
}

/// Whether *Resume Stack* applies: the stack is suspended, or a resume failed.
pub fn stack_suspended(status: &str) -> bool {
    matches!(status, "SUSPEND_COMPLETE" | "RESUME_FAILED")
}

/// A stack as one string, `name/id`, for the list's selection (whose key is a
/// single string) and back: Heat names can't contain `/`.
pub fn stack_key(name: &str, id: &str) -> String {
    format!("{name}/{id}")
}

/// [`stack_key`]'s parts; empty ones (refused by the server) when malformed.
pub fn split_stack_key(key: &str) -> (String, String) {
    let (name, id) = key.split_once('/').unwrap_or_default();
    (name.to_string(), id.to_string())
}

/// A row of Orchestration › Resource Types (`GET resource_types?with_description=true`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceTypeRow {
    /// `OS::Nova::Server`, also the detail page's path segment.
    pub name: String,
    /// `None` when Heat lists bare names (before `with_description`, Pike).
    pub description: Option<String>,
}

/// A resource type's schema (`GET resource_types/{type}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceTypeDetail {
    pub name: String,
    pub description: Option<String>,
    /// `support_status.status` (`SUPPORTED`, `DEPRECATED`…) with the version it changed in.
    pub support: Option<String>,
    /// Top-level properties, by name; nested schemas are in `raw`.
    pub properties: Vec<TypeProperty>,
    /// Attributes (`get_attr`), by name.
    pub attributes: Vec<TypeAttribute>,
    /// The whole schema, pretty JSON, for what the tables leave out.
    pub raw: String,
}

/// One property of a resource type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeProperty {
    pub name: String,
    /// Heat's `type`: `string`, `number`, `map`, `list`, `boolean`, `integer`.
    pub kind: String,
    pub required: bool,
    /// `update_allowed`: changed in place by a stack update (else replaced).
    pub update_allowed: bool,
    /// The default as text; `None` when there is none.
    pub default: Option<String>,
    pub description: Option<String>,
    /// Each constraint compact (`allowed_values: [a, b]`, `custom_constraint: nova.flavor`).
    pub constraints: Vec<String>,
    /// The `allowed_values` constraint's values as text, for a select in the
    /// Template Generator (roadmap 92); empty when unconstrained.
    #[serde(default)]
    pub allowed: Vec<String>,
    /// The `custom_constraint` (`neutron.network`, `nova.flavor`): what the value
    /// names, so the Template Generator can offer a `get_resource` to a matching resource.
    #[serde(default)]
    pub custom: Option<String>,
}

/// One attribute of a resource type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeAttribute {
    pub name: String,
    /// `None` on Heat releases that don't report it.
    pub kind: Option<String>,
    pub description: Option<String>,
}

/// A row of Orchestration › Template Versions (`GET template_versions`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateVersion {
    /// `heat_template_version.2018-08-31`, also the detail page's path segment.
    pub version: String,
    /// `hot` or `cfn`.
    pub kind: String,
    /// Other names for it (`heat_template_version.rocky`).
    pub aliases: Vec<String>,
}

/// An intrinsic function of a template version (`GET template_versions/{v}/functions`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateFunction {
    pub name: String,
    pub description: Option<String>,
}

/// Where a stack's template comes from (*Launch Stack*, *Change Stack Template*,
/// validation, preview).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateSource {
    /// Template text, YAML or JSON (Heat parses either); unused when `url` is set.
    pub template: String,
    /// `template_url`: Heat downloads the template itself.
    pub url: Option<String>,
    /// Environment text (YAML), if any. In an update, none keeps the stack's.
    pub environment: Option<String>,
}

/// The largest template plus environment Moonlight sends, in bytes: Heat's
/// default `max_template_size` (512 KiB).
pub const MAX_TEMPLATE_BYTES: usize = 512 * 1024;

/// A stack to create, or the new template and values of one (*Change Stack Template*).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StackSpec {
    /// `stack_name`: a letter, then letters, digits, `_`, `-` or `.`; part of
    /// Heat's paths. Unchanged by an update.
    pub name: String,
    pub source: TemplateSource,
    /// Parameter values as text (Heat converts them). Left out: the template's
    /// default, or in an update the stack's current value.
    pub parameters: Vec<(String, String)>,
    /// Hidden (`NoEcho`) parameters, masked in request previews.
    pub hidden: Vec<String>,
    /// `timeout_mins`; `None` is Heat's default (60).
    pub timeout_mins: Option<u32>,
    pub disable_rollback: bool,
}

/// What Heat's `POST validate` says about a template: its description and
/// parameters. `local` when Moonlight read them from the template itself
/// (dry-run, where the request is only previewed).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub description: Option<String>,
    pub params: Vec<TemplateParam>,
    pub local: bool,
}

/// One template parameter, as the launch form shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateParam {
    pub name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    /// `string`, `number`, `json`, `comma_delimited_list` or `boolean`.
    pub kind: String,
    /// The default as text; `None` when the parameter has none (then required).
    pub default: Option<String>,
    /// `hidden` (`NoEcho`): a password field, masked in previews.
    pub hidden: bool,
    /// `allowed_values`: the form offers a list instead of a text box.
    pub allowed: Vec<String>,
}

/// A resource *Preview Stack* says the template would create; `depth` > 0
/// inside nested stacks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewResource {
    pub name: String,
    pub kind: String,
    pub depth: u32,
}

/// Whether `name` is a valid Heat stack name (Heat's own rule).
pub fn valid_stack_name(name: &str) -> bool {
    let mut c = name.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && name.len() <= 255
        && c.all(|x| x.is_ascii_alphanumeric() || matches!(x, '_' | '-' | '.'))
}

/// One output of a stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackOutput {
    pub key: String,
    /// Strings as they are, other values as JSON; `-` when Heat sends none.
    pub value: String,
    pub description: Option<String>,
    /// `output_error`: why Heat could not resolve the value.
    pub error: Option<String>,
}

/// A resource of a stack (*Resources* tab; top level only).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackResource {
    /// `resource_name`, the template's logical name.
    pub name: String,
    /// The ID in its own service; `None` before it is created.
    pub physical_id: Option<String>,
    /// `OS::Nova::Server`, a nested template's file name…
    pub resource_type: String,
    pub status: String,
    pub status_reason: Option<String>,
    pub updated: String,
    /// The resources that depend on this one (`required_by`).
    pub required_by: Vec<String>,
    /// The resources this one depends on: the inverse of `required_by`.
    pub depends_on: Vec<String>,
    /// Moonlight's page for the physical resource, when there is one.
    pub href: Option<String>,
}

/// One stack event (*Events* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackEvent {
    pub id: String,
    pub time: String,
    pub resource_name: String,
    pub physical_id: Option<String>,
    pub status: String,
    pub status_reason: Option<String>,
}

/// The stack detail page: Horizon's *Topology*, *Overview*, *Resources*,
/// *Events* and *Template* tabs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackDetail {
    pub stack: StackRow,
    /// The stack's parameters, sorted; Heat's own `OS::*` ones left out.
    /// Heat masks hidden parameters itself (`******`).
    pub parameters: Vec<(String, String)>,
    pub outputs: Vec<StackOutput>,
    pub timeout_mins: Option<i64>,
    pub disable_rollback: bool,
    /// The parent stack's ID, for a nested stack.
    pub parent: Option<String>,
    pub tags: Vec<String>,
    pub resources: Vec<StackResource>,
    /// Newest first.
    pub events: Vec<StackEvent>,
    /// The template as Heat returns it (JSON), pretty-printed.
    pub template: String,
}

// --- Alarming (Aodh, roadmap 79) ----------------------------------------------
