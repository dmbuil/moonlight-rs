//! Modal dialogs for every write action: a small declarative form model
//! (fields, dynamic choices, conditional visibility), a confirmation variant
//! for destructive actions, and a result view that shows the dry-run requests.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_choices, get_image_upload_ticket, list_metadata_options, run_action};
use crate::components::pick::PickTable;
use crate::components::select::Combo;
use crate::components::widgets::{data_url, Copyable, InfoTip};
use crate::components::upload::{self, Picked, Upload};
use crate::i18n::{bytes, t, use_i18n, with_count, Lang, K};
use crate::model::{ActionOutcome, AssignmentRow, Capabilities, Choice, ChoiceSet, ErrorKind, LbMember, MemberRow, MetadataKind, OsAction, SecretKind, WriteMode};

// ---------------------------------------------------------------------------
// Form model
// ---------------------------------------------------------------------------

/// Where a select's (or picker's) options come from.
#[derive(Clone, Debug)]
pub enum Source {
    Static(Vec<(String, String)>),
    Set(ChoiceSet),
    /// The choices of a set that belong to one resource: those whose first
    /// cell is this ID (e.g. the ports of one instance).
    Of(ChoiceSet, String),
    /// Choices depend on the value of another field: `(field, [(value, set)])`.
    Switch(&'static str, Vec<(&'static str, ChoiceSet)>),
    /// A set without the choice whose value is this one: a migration's
    /// targets without the current host or pool, which Nova and Cinder refuse
    /// (Horizon leaves it out too).
    Except(ChoiceSet, String),
    /// A set without these values: a firewall policy's *Insert Rule* leaves out
    /// the rules it already has (roadmap 93).
    Without(ChoiceSet, Vec<String>),
}

impl Source {
    /// Whether choice `c` of this source's set is offered (`Of` keeps one
    /// resource's, `Except` drops one value; the others keep all).
    fn admits(&self, c: &Choice) -> bool {
        match self {
            Source::Of(_, owner) => c.cells.first() == Some(owner),
            Source::Except(_, value) => c.value != *value,
            Source::Without(_, values) => !values.contains(&c.value),
            _ => true,
        }
    }
}

/// How a field is edited; every kind submits a string (see [`Values`]).
#[derive(Clone, Debug)]
pub enum Kind {
    Text,
    /// Masked text (an admin password); never prefilled.
    Password,
    TextArea,
    Number { min: i64 },
    Check,
    /// Yes / No buttons (Horizon's toggles); the value is `true`/`false`.
    YesNo,
    Select(Source),
    /// Checkbox list; the value is a comma-separated list.
    Multi(Source),
    /// Horizon's *Allocated / Available* tables; the value is a
    /// comma-separated list, in allocation order.
    Pick { src: Source, multi: bool },
    /// Key/value rows (metadata); the value is a JSON list of `[key, value]`.
    KeyValue,
    /// Load balancer members, one row each (roadmap 54); the value is a JSON
    /// list of `model::MemberRow`. New rows take their port from the field
    /// `port_from` when set; `existing` are the pool's members when editing
    /// them, to show which ones the save deletes.
    MemberRows { port_from: Option<&'static str>, existing: Vec<LbMember> },
    /// `YYYY-MM-DD` with a calendar (`datetime::DatePicker`); past days
    /// disabled when `future_only`.
    Date { future_only: bool },
    /// `HH:MM` with hour/minute columns (`datetime::TimePicker`).
    Time,
    /// A local file to upload (roadmap 45). The value is a [`Picked`] (name and size); the
    /// `File` itself waits in `components::upload` until the action has run.
    File,
    /// Read-only: the first choice of `src`, its label with its value under it
    /// (Horizon's greyed *Domain Name* / *Domain ID* in *Create Project*).
    Info(ChoiceSet),
    /// Users or groups (`actors`) with their roles on a project
    /// (`assignment_rows::AssignmentRows`); the value is a JSON list of
    /// `model::AssignmentRow`.
    Assignments { actors: ChoiceSet },
}

/// Current form values, and the loaded choice lists.
pub type Vals = HashMap<String, String>;
/// Choice lists loaded so far, by set (for `on_change`, `check` and `block`).
pub type Sets = HashMap<ChoiceSet, Vec<Choice>>;

/// One form field: a name (the key in [`Values`]), a label and how it is edited,
/// plus the rules for when it shows and what it starts with.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: &'static str,
    pub label: String,
    pub kind: Kind,
    pub required: bool,
    pub default: String,
    pub help: Option<String>,
    /// Only shown (and submitted) when another field has this value.
    pub when: Option<(&'static str, &'static str)>,
    /// Extra condition over all values (for "a or b" rules).
    pub show_if: Option<fn(&Vals) -> bool>,
    /// Label of an optional select's empty choice, instead of *— None —*. On a
    /// required select it is a placeholder, and the first choice is not preselected.
    pub empty_label: Option<String>,
    /// A text area that can be filled from a local file of at most this many
    /// bytes, with a size counter (the customization script, roadmap 41).
    pub from_file: Option<usize>,
    /// A monospace example shown under the help (e.g. Access Rules' JSON/YAML).
    pub help_code: Option<String>,
    /// A longer explanation behind an (i) beside the label ([`InfoTip`] text:
    /// lines, `- ` for list items), for fields that need examples (roadmap 79).
    pub tip: Option<String>,
    /// A risky option: its label is red (e.g. *Unrestricted (dangerous)*).
    pub danger: bool,
    /// A narrow field (a number, a short code): up to three consecutive short
    /// fields share a line (`.wizard-pane` is a grid), as Create Flavor's sizes.
    pub short: bool,
    /// Half the row: two consecutive half fields share a line (Create Image's format and
    /// architecture, minimum disk and RAM).
    pub half: bool,
    /// Starts a new line even when the field before it left room (a hidden field
    /// must not pull the next one up beside it).
    pub new_line: bool,
    /// Visible lines of a text area (4 when `None`).
    pub rows: Option<u8>,
    /// Hidden once its choices load empty (Horizon drops *Availability Zone
    /// Hints* when Neutron reports no zones of that kind).
    pub hide_empty: bool,
    /// A [`Field::key_values`] only: the Glance metadata catalog resource type
    /// (`OS::Nova::Server`…) whose properties it suggests (roadmap 25).
    pub catalog: Option<&'static str>,
    /// Keys the catalog picker leaves out because another field of the dialog sets them
    /// (Create Image's `architecture`…); a repeated key is refused.
    pub catalog_skip: &'static [&'static str],
}

impl Field {
    fn new(name: &'static str, label: impl Into<String>, kind: Kind) -> Self {
        Field { name, label: label.into(), kind, required: false, default: String::new(), help: None, when: None, show_if: None, empty_label: None, from_file: None, help_code: None, tip: None, danger: false, short: false, half: false, new_line: false, rows: None, hide_empty: false, catalog: None, catalog_skip: &[] }
    }
    /// One-line text.
    pub fn text(name: &'static str, label: impl Into<String>) -> Self {
        Self::new(name, label, Kind::Text)
    }
    /// Masked text (an admin password); never prefilled.
    pub fn password(name: &'static str, label: impl Into<String>) -> Self {
        Self::new(name, label, Kind::Password)
    }
    /// Multi-line text.
    pub fn area(name: &'static str, label: impl Into<String>) -> Self {
        Self::new(name, label, Kind::TextArea)
    }
    /// A whole number, at least `min`.
    pub fn number(name: &'static str, label: impl Into<String>, min: i64) -> Self {
        Self::new(name, label, Kind::Number { min })
    }
    /// A checkbox, ticked when `on`; submits `true`/`false`.
    pub fn check(name: &'static str, label: impl Into<String>, on: bool) -> Self {
        Self::new(name, label, Kind::Check).value(if on { "true" } else { "false" })
    }
    /// One choice from `src` (the custom select, with a filter for long lists).
    pub fn select(name: &'static str, label: impl Into<String>, src: Source) -> Self {
        Self::new(name, label, Kind::Select(src))
    }
    /// A checkbox list; submits the ticked values as a comma list.
    pub fn multi(name: &'static str, label: impl Into<String>, src: Source) -> Self {
        Self::new(name, label, Kind::Multi(src))
    }
    /// Horizon's Yes / No toggle; submits `true`/`false`.
    pub fn yes_no(name: &'static str, label: impl Into<String>, on: bool) -> Self {
        Self::new(name, label, Kind::YesNo).value(if on { "true" } else { "false" })
    }
    /// One row of an *Allocated / Available* table.
    pub fn pick(name: &'static str, label: impl Into<String>, src: Source) -> Self {
        Self::new(name, label, Kind::Pick { src, multi: false })
    }
    /// Several rows, in order.
    pub fn pick_many(name: &'static str, label: impl Into<String>, src: Source) -> Self {
        Self::new(name, label, Kind::Pick { src, multi: true })
    }
    /// Editable key/value rows, starting with `pairs`.
    pub fn key_values(name: &'static str, label: impl Into<String>, pairs: &[(String, String)]) -> Self {
        Self::new(name, label, Kind::KeyValue).value(serde_json::to_string(pairs).unwrap_or_default())
    }
    /// A file to upload (the value is a [`Picked`]).
    pub fn file(name: &'static str, label: impl Into<String>) -> Self {
        Self::new(name, label, Kind::File)
    }
    /// A date (`YYYY-MM-DD`) with a calendar; `future_only` disables past days.
    pub fn date(name: &'static str, label: impl Into<String>, future_only: bool) -> Self {
        Self::new(name, label, Kind::Date { future_only })
    }
    /// A time (`HH:MM`) with hour and minute columns.
    pub fn time(name: &'static str, label: impl Into<String>) -> Self {
        Self::new(name, label, Kind::Time)
    }
    /// Marks a risky option: its label is shown in red.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    /// A read-only line showing the first choice of `src` (label and value).
    pub fn info(name: &'static str, label: impl Into<String>, src: ChoiceSet) -> Self {
        Self::new(name, label, Kind::Info(src))
    }
    /// The role assignments editor, starting with `rows`.
    pub fn assignments(name: &'static str, label: impl Into<String>, rows: &[AssignmentRow], actors: ChoiceSet) -> Self {
        Self::new(name, label, Kind::Assignments { actors }).value(serde_json::to_string(rows).unwrap_or_default())
    }
    /// An (i) beside the label that shows `text` on hover or focus.
    pub fn tip(mut self, text: impl Into<String>) -> Self {
        self.tip = Some(text.into());
        self
    }
    /// A monospace example under the help text.
    pub fn help_code(mut self, code: impl Into<String>) -> Self {
        self.help_code = Some(code.into());
        self
    }
    /// The members editor (`member_rows::MemberRows`), starting with `rows`.
    pub fn member_rows(name: &'static str, label: impl Into<String>, rows: &[MemberRow], port_from: Option<&'static str>, existing: Vec<LbMember>) -> Self {
        Self::new(name, label, Kind::MemberRows { port_from, existing }).value(serde_json::to_string(rows).unwrap_or_default())
    }
    /// Shows the field only when `f` holds over the current values (for "a or b"
    /// rules that `when` can't say).
    pub fn show_if(mut self, f: fn(&Vals) -> bool) -> Self {
        self.show_if = Some(f);
        self
    }
    /// Lines a text area shows (default 4): short descriptions don't need more.
    pub fn rows(mut self, n: u8) -> Self {
        self.rows = Some(n);
        self
    }
    /// Lays the field out at half the row, beside the half field next to it.
    pub fn half(mut self) -> Self {
        self.half = true;
        self
    }
    /// Starts the field on a new line (see [`Field::new_line`]).
    pub fn new_line(mut self) -> Self {
        self.new_line = true;
        self
    }
    /// Lays the field out narrow, beside the short fields next to it.
    pub fn short(mut self) -> Self {
        self.short = true;
        self
    }
    /// Must not be empty to submit (marked `*`).
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    /// The initial value.
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.default = v.into();
        self
    }
    /// *Load from a file* next to a text area, and a `n of limit` counter.
    pub fn from_file(mut self, limit: usize) -> Self {
        self.from_file = Some(limit);
        self
    }
    /// Suggests the Glance metadata catalog's properties for `resource_type` in a
    /// [`Field::key_values`] (an *Add from catalog* picker; typed value inputs). The
    /// catalog is read when the dialog opens; if it fails or is empty the editor stays plain.
    pub fn catalog(mut self, resource_type: &'static str) -> Self {
        self.catalog = Some(resource_type);
        self
    }
    /// Keeps `keys` out of the catalog picker (see [`Field::catalog`]).
    pub fn catalog_skip(mut self, keys: &'static [&'static str]) -> Self {
        self.catalog_skip = keys;
        self
    }
    /// Hide the field when its choice list loads empty (nothing to pick).
    pub fn hide_when_empty(mut self) -> Self {
        self.hide_empty = true;
        self
    }
    /// What an empty choice means here (e.g. *Any Availability Zone*); on a required
    /// select, the placeholder shown until the user picks (no preselection).
    pub fn empty_label(mut self, l: impl Into<String>) -> Self {
        self.empty_label = Some(l.into());
        self
    }
    /// Help text under the field.
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
    /// Shows (and submits) the field only while `field` has `value`. Transitive:
    /// hiding a field hides those that depend on it.
    pub fn when(mut self, field: &'static str, value: &'static str) -> Self {
        self.when = Some((field, value));
        self
    }
}

/// Fixed options with translated labels: `(value, label key)`.
pub fn options(items: &[(&str, K)], lang: Lang) -> Source {
    Source::Static(items.iter().map(|(v, k)| (v.to_string(), t(lang, *k).to_string())).collect())
}

/// Fixed options whose label is the value itself (API enums like `ROUND_ROBIN`).
pub fn options_raw(items: &[&str]) -> Source {
    Source::Static(items.iter().map(|v| (v.to_string(), v.to_string())).collect())
}

/// Submitted values, with typed accessors used by the action builders.
#[derive(Clone, Debug, Default)]
pub struct Values(pub HashMap<String, String>);

impl Values {
    /// The trimmed value; empty when the field is missing or hidden.
    pub fn get(&self, k: &str) -> String {
        self.0.get(k).map(|s| s.trim().to_string()).unwrap_or_default()
    }
    /// The trimmed value, `None` when empty.
    pub fn opt(&self, k: &str) -> Option<String> {
        Some(self.get(k)).filter(|s| !s.is_empty())
    }
    /// A check / yes-no field: `true` only for `"true"`.
    pub fn flag(&self, k: &str) -> bool {
        self.get(k) == "true"
    }
    /// A number field: `None` when empty, an error naming the field when not a number.
    pub fn num(&self, k: &str) -> Result<Option<i64>, String> {
        match self.opt(k) {
            None => Ok(None),
            Some(v) => v.parse().map(Some).map_err(|_| format!("{k}: not a number")),
        }
    }
    /// A comma- or newline-separated list (multi selects, pickers), empty items dropped.
    pub fn list(&self, k: &str) -> Vec<String> {
        self.get(k)
            .split([',', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
    /// The rows of a [`Field::key_values`], untrimmed (validation trims).
    pub fn pairs(&self, k: &str) -> Vec<(String, String)> {
        self.0.get(k).and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
    }
    /// Non-empty trimmed lines of a text area.
    pub fn lines(&self, k: &str) -> Vec<String> {
        self.get(k).lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
    }
}

/// Turns the submitted values into the action to run, or a message for the user.
pub type BuildFn = Arc<dyn Fn(&Values) -> Result<OsAction, String> + Send + Sync>;
/// Runs after a field changes (`name`), and may adjust other values.
pub type ChangeFn = Arc<dyn Fn(&str, &mut Vals, &Sets) + Send + Sync>;
/// Field-level problems beyond "required" (e.g. a volume that is too small).
pub type CheckFn = Arc<dyn Fn(&Vals, &Sets) -> Vec<(&'static str, String)> + Send + Sync>;
/// Why `choice` cannot be picked in field `name`, if it cannot.
pub type BlockFn = Arc<dyn Fn(&str, &Choice, &Vals, &Sets) -> Option<String> + Send + Sync>;

/// One page of a wizard: a title, a help text and the fields it shows.
#[derive(Clone, Debug)]
pub struct Step {
    pub title: String,
    pub help: Option<String>,
    pub fields: Vec<&'static str>,
    /// Marked with `*` in the step list, like Horizon.
    pub required: bool,
    /// Steps can be switched off (e.g. *Subnet* when no subnet is created);
    /// their fields are then neither validated nor submitted.
    pub enabled: Option<fn(&Vals) -> bool>,
}

impl Step {
    /// A step titled `title` showing these fields, in order.
    pub fn new(title: impl Into<String>, fields: &[&'static str]) -> Self {
        Step { title: title.into(), help: None, fields: fields.to_vec(), required: false, enabled: None }
    }
    /// Help text shown at the top of the step.
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
    /// Marks the step as required (`*` in the step list).
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    /// Switches the step off while `f` doesn't hold (its fields are neither
    /// validated nor submitted).
    pub fn enabled_if(mut self, f: fn(&Vals) -> bool) -> Self {
        self.enabled = Some(f);
        self
    }
    fn is_enabled(&self, v: &Vals) -> bool {
        self.enabled.is_none_or(|f| f(v))
    }
}

/// Modal widths: `Small` for confirmations and short forms, `Large` for wizards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DialogSize {
    Small,
    Medium,
    Large,
}

impl DialogSize {
    fn class(self) -> &'static str {
        match self {
            DialogSize::Small => "modal s",
            DialogSize::Medium => "modal m",
            DialogSize::Large => "modal l",
        }
    }
}

/// A modal form: fields (or wizard steps), the action they build, and the
/// hooks that react to changes. Built by `ops.rs`, shown by [`DialogHost`].
#[derive(Clone)]
pub struct Dialog {
    pub title: String,
    /// Paragraphs above the fields: the confirmation text of destructive
    /// actions, or read-only context (Admin *Edit Network*'s provider attributes).
    pub intro: Vec<String>,
    /// The resources the action applies to, listed as bullets after the
    /// first intro line ("You have selected:").
    pub selection: Vec<String>,
    pub fields: Vec<Field>,
    pub submit: String,
    pub danger: bool,
    pub build: BuildFn,
    /// Empty for single-page dialogs.
    pub steps: Vec<Step>,
    pub on_change: Option<ChangeFn>,
    pub check: Option<CheckFn>,
    pub block: Option<BlockFn>,
    /// Overrides the width picked by `Dialog::size`.
    pub fixed_size: Option<DialogSize>,
    /// The wizard step shown first (0 unless `starting_at`).
    pub first_step: usize,
    /// Where to go once the action really ran (not in dry-run), e.g. the list
    /// after deleting from a detail page, which would otherwise show "not found".
    pub then_go: Option<String>,
    /// Shown instead of the service's error when it refuses with 403: for
    /// actions whose permission can't be known beforehand (roadmap 82; item 35
    /// would hide them instead).
    pub forbidden: Option<String>,
}

impl Dialog {
    /// A plain form: `build` turns the submitted values into the action.
    pub fn form(title: impl Into<String>, submit: impl Into<String>, fields: Vec<Field>, build: impl Fn(&Values) -> Result<OsAction, String> + Send + Sync + 'static) -> Self {
        Dialog {
            title: title.into(),
            intro: vec![],
            selection: vec![],
            fields,
            submit: submit.into(),
            danger: false,
            build: Arc::new(build),
            steps: vec![],
            on_change: None,
            check: None,
            block: None,
            fixed_size: None,
            first_step: 0,
            then_go: None,
            forbidden: None,
        }
    }

    /// The width that fits the content: wizards are large, confirmations and
    /// forms of up to three one-line fields are small, the rest medium.
    pub fn size(&self) -> DialogSize {
        let tall = |f: &Field| matches!(f.kind, Kind::TextArea | Kind::Multi(_) | Kind::Pick { .. } | Kind::KeyValue | Kind::MemberRows { .. } | Kind::Assignments { .. });
        self.fixed_size.unwrap_or(if !self.steps.is_empty() {
            DialogSize::Large
        } else if self.fields.len() <= 3 && !self.fields.iter().any(tall) {
            DialogSize::Small
        } else {
            DialogSize::Medium
        })
    }
    /// Forces a width instead of the one [`Dialog::size`] picks.
    pub fn sized(mut self, size: DialogSize) -> Self {
        self.fixed_size = Some(size);
        self
    }

    /// Turns the form into a wizard (every field must belong to one step).
    pub fn wizard(mut self, steps: Vec<Step>) -> Self {
        self.steps = steps;
        self
    }
    /// Opens the wizard on step `i` instead of the first, e.g. *Edit Port* on its
    /// *Security Groups* tab for *Edit Security Groups*. Out of range means 0.
    pub fn starting_at(mut self, i: usize) -> Self {
        self.first_step = i;
        self
    }
    /// The message for a 403 (see [`Dialog::forbidden`]).
    pub fn forbidden(mut self, message: impl Into<String>) -> Self {
        self.forbidden = Some(message.into());
        self
    }
    /// Navigates to `href` after the action ran for real (see `then_go`).
    pub fn then_go(mut self, href: impl Into<String>) -> Self {
        self.then_go = Some(href.into());
        self
    }
    /// Runs after a field changes (also after a preset value's set loads), and may
    /// update other values, e.g. preset a port's groups when the port is picked.
    pub fn on_change(mut self, f: impl Fn(&str, &mut Vals, &Sets) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }
    /// Cross-field validation before submit: `(field, message)` errors shown under
    /// the fields.
    pub fn check(mut self, f: impl Fn(&Vals, &Sets) -> Vec<(&'static str, String)> + Send + Sync + 'static) -> Self {
        self.check = Some(Arc::new(f));
        self
    }
    /// Greys out a choice of a picker with a reason (e.g. a flavor too small for the
    /// image), instead of hiding it.
    pub fn block(mut self, f: impl Fn(&str, &Choice, &Vals, &Sets) -> Option<String> + Send + Sync + 'static) -> Self {
        self.block = Some(Arc::new(f));
        self
    }

    /// "You have selected: • X. This action cannot be undone." Like every
    /// destructive dialog, it needs the "I understand" box ticked to submit.
    pub fn confirm(lang: Lang, title: impl Into<String>, what: impl Into<String>, action: OsAction) -> Self {
        let title = title.into();
        let mut d = Dialog::form(title.clone(), title, vec![], move |_| Ok(action.clone()));
        d.intro = vec![t(lang, K::YouSelected).into(), t(lang, K::CannotBeUndone).into()];
        d.selection = vec![what.into()];
        d.danger = true;
        d
    }

    /// One-click actions that are not destructive (pause, lock…) still get a
    /// confirmation, like Horizon, but not the red styling.
    pub fn simple(lang: Lang, title: impl Into<String>, what: impl Into<String>, action: OsAction) -> Self {
        let mut d = Self::confirm(lang, title, what, action);
        d.intro.truncate(1);
        d.danger = false;
        d
    }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Global dialog state + a version counter pages use to refetch after writes.
#[derive(Clone, Copy)]
pub struct Dialogs {
    current: RwSignal<Option<Arc<Dialog>>>,
    pub version: RwSignal<u64>,
    pub mode: RwSignal<WriteMode>,
    /// What the project + region offers (from the session info).
    pub caps: RwSignal<Capabilities>,
    /// `moonlight.conf` preferences (from the session info).
    pub prefs: RwSignal<crate::prefs::Prefs>,
}

impl Dialogs {
    /// Creates the dialog state and provides it as context (once, in the layout).
    pub fn provide() -> Self {
        let d = Dialogs {
            current: RwSignal::new(None),
            version: RwSignal::new(0),
            mode: RwSignal::new(WriteMode::DryRun),
            caps: RwSignal::new(Capabilities::default()),
            prefs: RwSignal::new(crate::prefs::Prefs::default()),
        };
        provide_context(d);
        d
    }
    /// Opens `d`, replacing any dialog already open.
    pub fn open(&self, d: Dialog) {
        self.current.set(Some(Arc::new(d)));
    }
}

/// The dialog state provided by [`Dialogs::provide`].
pub fn use_dialogs() -> Dialogs {
    expect_context::<Dialogs>()
}

/// The current capabilities (tracked). Outside the app (tests) everything is
/// unknown, so every requirement passes.
pub fn caps() -> Capabilities {
    use_context::<Dialogs>().map(|d| d.caps.get()).unwrap_or_default()
}

/// The deployment's preferences; defaults until the session info loads.
pub fn prefs() -> crate::prefs::Prefs {
    use_context::<Dialogs>().map(|d| d.prefs.get_untracked()).unwrap_or_default()
}

/// [`prefs`], tracked: for what a page builds before the session info loads
/// (a server-rendered table's row menus) and must rebuild once it has.
pub fn prefs_tracked() -> crate::prefs::Prefs {
    use_context::<Dialogs>().map(|d| d.prefs.get()).unwrap_or_default()
}

/// Tracks the data version: put it in a `Resource` source to refetch after writes.
pub fn data_version() -> u64 {
    use_context::<Dialogs>().map(|d| d.version.get()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Editing,
    Running,
    Done(ActionOutcome),
    Failed(String),
    /// The record exists and its data is on the way (roadmap 45): progress is in a signal.
    Uploading,
    /// The upload did not finish: the image `.0` stays `queued`, and the user may delete it.
    UploadFailed(String, String),
    /// The session expired before the action ran: nothing was sent.
    Expired,
}

/// A field is visible when its condition holds *and* the field it depends on
/// is itself visible (so hiding a checkbox also hides what it controls).
fn visible(f: &Field, values: &HashMap<String, String>, all: &[Field]) -> bool {
    if f.show_if.is_some_and(|cond| !cond(values)) {
        return false;
    }
    match f.when {
        Some((other, want)) => {
            values.get(other).map(String::as_str).unwrap_or("") == want
                && all.iter().find(|p| p.name == other).is_none_or(|p| visible(p, values, all))
        }
        None => true,
    }
}

fn choice_set_for(src: &Source, values: &HashMap<String, String>) -> Option<ChoiceSet> {
    match src {
        Source::Static(_) => None,
        Source::Set(s) | Source::Of(s, _) | Source::Except(s, _) | Source::Without(s, _) => Some(*s),
        Source::Switch(field, map) => {
            let v = values.get(*field).cloned().unwrap_or_default();
            map.iter().find(|(k, _)| *k == v).map(|(_, s)| *s)
        }
    }
}

fn all_sets(fields: &[Field]) -> Vec<ChoiceSet> {
    let mut out = Vec::new();
    for f in fields {
        if let Kind::Select(src) | Kind::Multi(src) | Kind::Pick { src, .. } = &f.kind {
            match src {
                Source::Set(s) | Source::Of(s, _) | Source::Except(s, _) | Source::Without(s, _) => out.push(*s),
                Source::Switch(_, m) => out.extend(m.iter().map(|(_, s)| *s)),
                Source::Static(_) => {}
            }
        }
        if let Kind::MemberRows { .. } = &f.kind {
            out.extend([ChoiceSet::LbMemberTargets, ChoiceSet::LbMemberSubnets]);
        }
        if let Kind::Info(set) = &f.kind {
            out.push(*set);
        }
        if let Kind::Assignments { actors } = &f.kind {
            out.extend([*actors, ChoiceSet::IdentityRoles]);
        }
    }
    out.sort_by_key(|s| format!("{s:?}"));
    out.dedup();
    out
}

/// The wizard's steps, or one implicit step with every field.
fn steps_of(d: &Dialog) -> Vec<Step> {
    if d.steps.is_empty() {
        vec![Step::new(d.title.clone(), &d.fields.iter().map(|f| f.name).collect::<Vec<_>>())]
    } else {
        d.steps.clone()
    }
}

/// Submitted fields: visible, and in a step that is switched on.
fn active(f: &Field, d: &Dialog, steps: &[Step], v: &Vals) -> bool {
    visible(f, v, &d.fields) && steps.iter().find(|s| s.fields.contains(&f.name)).is_none_or(|s| s.is_enabled(v))
}

/// Problems per field: `(message, shown only after Next/submit)`. Problems
/// of empty fields (missing values, "pick a network or a port") wait for an
/// attempt, like Horizon; a wrong value (a too small volume) shows at once.
fn problems(d: &Dialog, steps: &[Step], v: &Vals, sets: &Sets, lang: Lang) -> HashMap<&'static str, (String, bool)> {
    let mut out = HashMap::new();
    for f in d.fields.iter().filter(|f| f.required && active(f, d, steps, v)) {
        if v.get(f.name).is_none_or(|x| x.trim().is_empty()) {
            out.insert(f.name, (t(lang, K::FieldRequired).to_string(), true));
        }
    }
    if let Some(check) = &d.check {
        for (name, msg) in check(v, sets) {
            if d.fields.iter().any(|f| f.name == name && active(f, d, steps, v)) {
                let empty = v.get(name).is_none_or(|x| x.trim().is_empty());
                out.insert(name, (msg, empty));
            }
        }
    }
    out
}

/// Renders the open dialog, if any (once, in the layout).
#[component]
pub fn DialogHost() -> impl IntoView {
    let dialogs = use_dialogs();
    move || dialogs.current.get().map(|d| view! { <DialogView dialog=d/> })
}

#[component]
fn DialogView(dialog: Arc<Dialog>) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    let jobs = use_context::<crate::components::jobs::Jobs>();
    let initial: Vals = dialog.fields.iter().map(|f| (f.name.to_string(), f.default.clone())).collect();
    let values = RwSignal::new(initial);
    let phase = RwSignal::new(Phase::Editing);
    let choices: RwSignal<Sets> = RwSignal::new(HashMap::new());
    let errors: RwSignal<Option<String>> = RwSignal::new(None);
    let wizard = !dialog.steps.is_empty();
    let steps = StoredValue::new(steps_of(&dialog));
    let n_steps = steps.with_value(Vec::len);
    let step = RwSignal::new(if dialog.first_step < n_steps { dialog.first_step } else { 0 });
    // Steps the user tried to leave or submit: their missing fields show.
    let attempted: RwSignal<HashSet<usize>> = RwSignal::new(HashSet::new());

    // Load every dynamic choice list once, in parallel.
    for set in all_sets(&dialog.fields) {
        let fields = dialog.fields.clone();
        let hook = dialog.on_change.clone();
        spawn_local(async move {
            let list = get_choices(set).await.unwrap_or_default();
            choices.update(|m| {
                m.insert(set, list);
            });
            // Pre-select the first option of required selects that are still
            // empty (of the owner's choices for `Source::Of`), unless the field
            // has a placeholder (`empty_label`): there the user must choose, as
            // Horizon's admin *Select a project* (an admin creating a network
            // in the first project by accident, roadmap 85). Either way, a
            // field that now has a value from this set (preselected, or preset
            // by the caller, e.g. *Launch* from an image row) runs the hook as
            // if the user had picked it, so dependent fields follow.
            // The dialog may have closed while the list loaded: its signals are gone
            // (`update` is then a no-op, but a read panics and kills the app).
            let Some(sets) = choices.try_get_untracked() else { return };
            values.update(|vals| {
                for f in &fields {
                    let (Kind::Select(src) | Kind::Multi(src) | Kind::Pick { src, .. }) = &f.kind else { continue };
                    if choice_set_for(src, vals) != Some(set) {
                        continue;
                    }
                    let empty = vals.get(f.name).is_none_or(|v| v.is_empty());
                    if empty
                        && preselects(f)
                        && let Some(first) = sets[&set].iter().find(|c| src.admits(c))
                    {
                        vals.insert(f.name.to_string(), first.value.clone());
                    }
                    if !vals.get(f.name).is_none_or(|v| v.is_empty())
                        && let Some(hook) = &hook
                    {
                        hook(f.name, vals, &sets);
                    }
                }
            });
        });
    }

    let d = dialog.clone();
    let set_value = Callback::new(move |(name, v): (&'static str, String)| {
        let sets = choices.get_untracked();
        values.update(|m| {
            m.insert(name.to_string(), v);
            if let Some(hook) = &d.on_change {
                hook(name, m, &sets);
            }
        });
    });
    let d = dialog.clone();
    let issues = Memo::new(move |_| {
        let (v, sets) = (values.get(), choices.get());
        steps.with_value(|st| problems(&d, st, &v, &sets, i18n.lang()))
    });
    let step_of = move |name: &str| steps.with_value(|st| st.iter().position(|s| s.fields.contains(&name)));
    let step_bad = move |i: usize| {
        let iss = issues.get();
        steps.with_value(|st| st[i].fields.iter().any(|f| iss.contains_key(f)))
    };
    let enabled = move |i: usize| values.with(|v| steps.with_value(|st| st[i].is_enabled(v)));
    let neighbour = move |from: usize, forward: bool| -> Option<usize> {
        let mut i = from;
        loop {
            i = if forward { i.checked_add(1).filter(|&n| n < n_steps)? } else { i.checked_sub(1)? };
            if enabled(i) {
                return Some(i);
            }
        }
    };
    let field_error = move |name: &'static str| {
        Signal::derive(move || {
            issues.get().get(name).and_then(|(msg, after)| {
                let show = !*after || step_of(name).is_some_and(|i| attempted.with(|a| a.contains(&i)));
                show.then(|| msg.clone())
            })
        })
    };

    // Upload progress `(sent, total)` and the transfer, to cancel it (roadmap 45).
    let progress = RwSignal::new((0.0_f64, 0.0_f64));
    let uploading = StoredValue::new_local(Upload::default());
    let close = move || dialogs.current.set(None);
    let go = move |forward: bool| {
        let i = step.get_untracked();
        attempted.update(|a| {
            a.insert(i);
        });
        if let Some(n) = neighbour(i, forward) {
            step.set(n);
        }
    };
    let navigate = leptos_router::hooks::use_navigate();
    let then_go = dialog.then_go.clone();
    let d = dialog.clone();
    let submit = move || {
        attempted.set((0..n_steps).collect());
        let iss = issues.get_untracked();
        if !iss.is_empty() {
            if let Some(first) = steps.with_value(|st| st.iter().position(|s| s.fields.iter().any(|f| iss.contains_key(f)))) {
                step.set(first);
            }
            return;
        }
        let vals = values.get_untracked();
        let shown: HashMap<String, String> = steps.with_value(|st| {
            vals.iter().filter(|(k, _)| d.fields.iter().any(|f| f.name == *k && active(f, &d, st, &vals))).map(|(k, v)| (k.clone(), v.clone())).collect()
        });
        match (d.build)(&Values(shown)) {
            Err(e) => errors.set(Some(e)),
            Ok(action) => {
                errors.set(None);
                // Long batches run in the background with a progress card (roadmap 87).
                let from = dialogs.prefs.get_untracked().background_batch_from;
                let action = match (jobs.filter(|_| crate::components::jobs::in_background(&action, from)), action) {
                    (Some(jobs), OsAction::Batch(items)) => {
                        close();
                        jobs.start(d.title.clone(), items, dialogs.version);
                        return;
                    }
                    (_, action) => action,
                };
                phase.set(Phase::Running);
                let (navigate, then_go, forbidden) = (navigate.clone(), then_go.clone(), d.forbidden.clone());
                let file_bytes = action.image_upload_bytes();
                let upload_mode = dialogs.prefs.get_untracked().images.upload;
                spawn_local(async move {
                    match run_action(action).await {
                        Ok(out) => {
                            if out.mode == WriteMode::On {
                                // Leave first, so the deleted resource's page isn't refetched.
                                if let Some(href) = &then_go {
                                    navigate(href, Default::default());
                                }
                                dialogs.version.update(|v| *v += 1);
                            }
                            // *Create Image* › *File*: the record exists, now its data (dry-run
                            // creates nothing, so it only shows the preview).
                            match (out.mode, out.created.clone(), file_bytes) {
                                (WriteMode::On, Some(id), Some(size)) => {
                                    upload_image(id, size, upload_mode, out, phase, progress, uploading, dialogs.version).await;
                                }
                                _ => phase.set(Phase::Done(out)),
                            }
                        }
                        Err(e) if e.kind == ErrorKind::Unauthenticated => phase.set(Phase::Expired),
                        Err(e) if e.kind == ErrorKind::Forbidden && forbidden.is_some() => phase.set(Phase::Failed(forbidden.unwrap_or_default())),
                        Err(e) => phase.set(Phase::Failed(e.to_string())),
                    }
                });
            }
        }
    };

    let title = dialog.title.clone();
    let size = dialog.size();
    let danger = dialog.danger;
    let submit_label = dialog.submit.clone();
    let intro = dialog.intro.clone();
    let selection = dialog.selection.clone();
    // Destructive dialogs submit only once "Yes, I understand" is ticked.
    let ack = RwSignal::new(!danger);
    let fields = dialog.fields.clone();
    let block = dialog.block.clone();

    view! {
        <div class="modal-backdrop" on:click=move |_| close()>
            // The request preview needs room, so results open at least medium.
            <div class=move || if matches!(phase.get(), Phase::Done(_)) { size.max(DialogSize::Medium) } else { size }.class()
                role="dialog" aria-modal="true" aria-label=title.clone()
                on:click=|ev| ev.stop_propagation()
                on:keydown=move |ev| if ev.key() == "Escape" { close() }>
                <header class="modal-head">
                    <h2>{title.clone()}</h2>
                    <button class="btn-close" type="button" aria-label=move || i18n.t(K::Close) on:click=move |_| close()>"×"</button>
                </header>
                {move || match phase.get() {
                    Phase::Done(out) => view! { <Outcome out=out/> }.into_any(),
                    Phase::Expired => view! {
                        <div class="modal-body">
                            <div class="alert alert-error" role="alert">
                                <strong>{move || i18n.t(K::ErrUnauthTitle)}</strong>
                                <span>{move || i18n.t(K::ErrUnauthText)}</span>
                            </div>
                            <footer class="modal-foot">
                                <button type="button" class="btn" on:click=move |_| dialogs.current.set(None)>{move || i18n.t(K::Close)}</button>
                                // A full load, like the error page's: the login page starts a fresh app.
                                <a class="btn btn-primary" href="/login" rel="external">{move || i18n.t(K::SignIn)}</a>
                            </footer>
                        </div>
                    }.into_any(),
                    Phase::Uploading => view! {
                        <div class="modal-body">
                            <p>{move || i18n.t(K::Uploading)}</p>
                            <progress max="100" value=move || percent(progress.get())></progress>
                            <small class="muted">
                                {move || { let (sent, total) = progress.get(); format!("{:.0}%  ·  {} / {}", percent((sent, total)), bytes(sent as u64), bytes(total as u64)) }}
                            </small>
                            <footer class="modal-foot">
                                <button type="button" class="btn" on:click=move |_| uploading.with_value(|u| u.abort())>{move || i18n.t(K::CancelUpload)}</button>
                            </footer>
                        </div>
                    }.into_any(),
                    Phase::UploadFailed(id, why) => {
                        let delete = move |_| {
                            let id = id.clone();
                            spawn_local(async move {
                                if run_action(OsAction::ImageDelete { id }).await.is_ok() {
                                    dialogs.version.update(|v| *v += 1);
                                }
                                dialogs.current.set(None);
                            });
                        };
                        view! {
                            <div class="modal-body">
                                <div class="alert alert-error" role="alert">
                                    <strong>{move || i18n.t(K::UploadFailedTitle)}</strong>
                                    <span>{why}</span>
                                </div>
                                <p>{move || i18n.t(K::UploadFailedText)}</p>
                                <footer class="modal-foot">
                                    <button type="button" class="btn" on:click=move |_| dialogs.current.set(None)>{move || i18n.t(K::Close)}</button>
                                    <button type="button" class="btn btn-danger" on:click=delete>{move || i18n.t(K::DeleteImage)}</button>
                                </footer>
                            </div>
                        }.into_any()
                    }
                    Phase::Failed(e) => view! {
                        <div class="modal-body">
                            <div class="alert alert-error" role="alert">
                                {if e.contains("writes are disabled") { i18n.t(K::WritesOff).to_string() } else { e }}
                            </div>
                            // Back to the form with what was typed, to fix it (the values live outside the phase).
                            <footer class="modal-foot">
                                <button type="button" class="btn" on:click=move |_| phase.set(Phase::Editing)>{move || i18n.t(K::Back)}</button>
                                <button type="button" class="btn btn-primary" on:click=move |_| dialogs.current.set(None)>{move || i18n.t(K::Close)}</button>
                            </footer>
                        </div>
                    }.into_any(),
                    _ => {
                        let fields = fields.clone();
                        let intro = intro.clone();
                        let selection = selection.clone();
                        let submit = submit.clone();
                        let submit_label = submit_label.clone();
                        let block = block.clone();
                        view! {
                            <form class="modal-body" class:wizard=wizard on:submit=move |ev| {
                                ev.prevent_default();
                                if ack.get_untracked() { submit() }
                            }>
                                {wizard.then(|| view! {
                                    <nav class="wizard-nav" aria-label=move || i18n.t(K::Steps)>
                                        <ol>
                                            {steps.with_value(|st| st.iter().enumerate().map(|(i, s)| {
                                                let (label, req) = (s.title.clone(), s.required);
                                                view! {
                                                    <li>
                                                        <button type="button"
                                                            class:current=move || step.get() == i
                                                            class:bad=move || attempted.with(|a| a.contains(&i)) && step_bad(i)
                                                            class:done=move || attempted.with(|a| a.contains(&i)) && !step_bad(i)
                                                            disabled=move || !enabled(i)
                                                            aria-current=move || (step.get() == i).then_some("step")
                                                            on:click=move |_| {
                                                                attempted.update(|a| { a.insert(step.get_untracked()); });
                                                                step.set(i);
                                                            }>
                                                            {label}
                                                            {req.then(|| view! { <span class="req" aria-hidden="true">" *"</span> })}
                                                        </button>
                                                    </li>
                                                }
                                            }).collect_view())}
                                        </ol>
                                    </nav>
                                })}
                                <div class="wizard-pane">
                                    {intro.into_iter().enumerate().map(|(i, p)| view! {
                                        <p class=if danger { "intro danger" } else { "intro" }>{p}</p>
                                        {(i == 0 && !selection.is_empty()).then(|| view! {
                                            <ul class="selection">
                                                {selection.iter().map(|s| view! { <li>{s.clone()}</li> }).collect_view()}
                                            </ul>
                                        })}
                                    }).collect_view()}
                                    {move || wizard.then(|| steps.with_value(|st| st[step.get()].help.clone()).map(|h| view! { <p class="step-help">{h}</p> }))}
                                    {fields.iter().cloned().map(|f| {
                                        let name = f.name;
                                        let in_step = Signal::derive(move || !wizard || step_of(name) == Some(step.get()));
                                        view! {
                                            <FieldView field=f all=fields.clone() values=values choices=choices
                                                set=set_value error=field_error(name) in_step=in_step block=block.clone()/>
                                        }
                                    }).collect_view()}
                                    {move || errors.get().map(|e| view! { <div class="alert alert-error" role="alert">{e}</div> })}
                                    {danger.then(|| view! {
                                        <label class="ack">
                                            <input type="checkbox" prop:checked=ack on:change=move |ev| ack.set(event_target_checked(&ev))/>
                                            <span>{move || i18n.t(K::IUnderstand)}</span>
                                        </label>
                                    })}
                                </div>
                                <footer class="modal-foot">
                                    <button type="button" class="btn" on:click=move |_| close()>{move || i18n.t(K::Cancel)}</button>
                                    {wizard.then(|| view! {
                                        <span class="foot-spacer"></span>
                                        <button type="button" class="btn" disabled=move || neighbour(step.get(), false).is_none()
                                            on:click=move |_| go(false)>{move || format!("‹ {}", i18n.t(K::Back))}</button>
                                        <button type="button" class="btn" disabled=move || neighbour(step.get(), true).is_none()
                                            on:click=move |_| go(true)>{move || format!("{} ›", i18n.t(K::Next))}</button>
                                    })}
                                    // Like Horizon: the wizard can only be submitted once valid.
                                    <button type="submit" class=if danger { "btn btn-danger" } else { "btn btn-primary" }
                                        disabled=move || phase.get() == Phase::Running || !ack.get() || (wizard && !issues.get().is_empty())>
                                        {
                                            let label = submit_label.clone();
                                            move || if phase.get() == Phase::Running { i18n.t(K::Working).to_string() } else { label.clone() }
                                        }
                                    </button>
                                </footer>
                            </form>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

/// The requests an action built (method, path, service, body), unnumbered
/// (roadmap 78); nothing when the server left them out (`[ui] show_requests`).
#[component]
pub fn RequestList(requests: Vec<crate::model::RequestPreview>) -> impl IntoView {
    (!requests.is_empty()).then(|| view! {
        <ul class="requests">
            {requests.into_iter().map(|r| view! {
                <li>
                    <div class="req-line">
                        <span class=format!("method m-{}", r.method.to_lowercase())>{r.method.clone()}</span>
                        <span class="mono">{r.path.clone()}</span>
                        <span class="muted">{r.service.clone()}</span>
                    </div>
                    {r.body.map(|b| view! { <pre class="req-body">{b}</pre> })}
                </li>
            }).collect_view()}
        </ul>
    })
}

#[component]
fn Outcome(out: ActionOutcome) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    let dry = out.mode == WriteMode::DryRun;
    let failed: Vec<_> = out.results.iter().filter_map(|r| r.error.clone().map(|e| (r.label.clone(), e))).collect();
    let all_failed = !failed.is_empty() && failed.len() == out.results.len();
    let failures = (!failed.is_empty()).then(|| view! {
        <div class="alert alert-error batch-failures">
            <strong>{move || i18n.t(if all_failed { K::AllItemsFailed } else { K::SomeItemsFailed })}</strong>
            <ul>
                {failed.into_iter().map(|(label, e)| view! { <li><span class="strong">{label}</span>": "{e}</li> }).collect_view()}
            </ul>
        </div>
    });
    let secret = out.secret.map(|s| {
        let (title, explain) = match s.kind {
            SecretKind::PrivateKey => (K::SecretTitle, K::SecretExplain),
            SecretKind::Password => (K::PasswordTitle, K::SecretExplain),
            SecretKind::Transfer => (K::TransferTitle, K::SecretExplain),
            SecretKind::AppCredential => (K::AppCredTitle, K::AppCredExplain),
        };
        // Horizon's success page: the values to copy, then every download.
        let fields = (!s.fields.is_empty()).then(|| view! {
            <dl class="kv secret-fields">
                {s.fields.iter().map(|(k, v)| {
                    let label = match k.as_str() { "id" => K::Id, "name" => K::Name, _ => K::Secret };
                    view! { <dt>{move || i18n.t(label)}</dt><dd><Copyable value=v.clone() mono=true/></dd> }
                }).collect_view()}
            </dl>
        });
        let downloads = std::iter::once((s.file_name.clone(), s.content.clone())).chain(s.extra.iter().cloned()).map(|(name, content)| view! {
            <a class="btn btn-primary" href=data_url(&content) download=name.clone()>{move || i18n.t(K::DownloadFile)} " " {name.clone()}</a>
        }).collect_view();
        view! {
            <div class="alert alert-warn secret">
                <strong>{move || i18n.t(title)}</strong>
                <span>{move || i18n.t(explain)}</span>
                {fields}
                <div class="secret-downloads">{downloads}</div>
            </div>
        }
    });
    view! {
        <div class="modal-body">
            {secret}
            {if dry {
                view! {
                    <div class="alert alert-warn">
                        <strong>{move || i18n.t(K::DryRunTitle)}</strong>
                        <span>{move || i18n.t(K::DryRunExplain)}</span>
                    </div>
                }.into_any()
            } else if all_failed {
                ().into_any()
            } else {
                view! { <div class="alert alert-ok">{move || i18n.t(K::ActionDone)}</div> }.into_any()
            }}
            {failures}
            <RequestList requests=out.requests/>
            <footer class="modal-foot">
                <button type="button" class="btn btn-primary" on:click=move |_| dialogs.current.set(None)>{move || i18n.t(K::Close)}</button>
            </footer>
        </div>
    }
}

#[component]
fn FieldView(
    field: Field,
    all: Vec<Field>,
    values: RwSignal<Vals>,
    choices: RwSignal<Sets>,
    set: Callback<(&'static str, String)>,
    error: Signal<Option<String>>,
    /// False when the field belongs to another wizard step.
    in_step: Signal<bool>,
    block: Option<BlockFn>,
) -> impl IntoView {
    let i18n = use_i18n();
    let name = field.name;
    let id = format!("f-{name}");
    let label_id = format!("{id}-label");
    let err_id = format!("{id}-error");
    let get = move || values.with(|v| v.get(name).cloned().unwrap_or_default());
    let put = move |v: String| set.run((name, v));
    let f = field.clone();
    let shown = move || {
        in_step.get()
            && values.with(|v| visible(&f, v, &all))
            && !(f.hide_empty && matches!(&f.kind, Kind::Select(src) | Kind::Multi(src) | Kind::Pick { src, .. } if resolve(src, values, choices).is_some_and(|l| l.is_empty())))
    };
    let label = field.label.clone();
    let req = field.required;
    let help = field.help.clone();
    let help_code = field.help_code.clone();
    let tip = field.tip.clone().map(|text| {
        let aria = format!("{}: {}", field.label, i18n.t(K::MoreInfo));
        view! { <InfoTip text=text label=aria id=id.clone()/> }
    });
    let invalid = move || error.get().is_some().then_some("true");
    let err_id2 = err_id.clone();
    let described = move || error.get().is_some().then(|| err_id2.clone());
    let error_view = move || error.get().map(|e| view! { <small class="field-error" id=err_id.clone() role="alert">{e}</small> });
    // Groups (yes/no, tables, checkbox lists) are labelled, not `<label for>`.
    let grouped = matches!(field.kind, Kind::YesNo | Kind::Pick { .. } | Kind::Multi(_) | Kind::KeyValue | Kind::MemberRows { .. } | Kind::Assignments { .. } | Kind::Info(_));

    let control = match field.kind.clone() {
        Kind::Text => view! {
            <input id=id.clone() type="text" prop:value=get aria-invalid=invalid aria-describedby=described
                on:input=move |ev| put(event_target_value(&ev))/>
        }.into_any(),
        Kind::Info(set) => {
            let first = move || choices.with(|m| m.get(&set).and_then(|l| l.first().cloned()));
            view! {
                <div class="field-info" id=id.clone() aria-labelledby=label_id.clone()>
                    {move || match first() {
                        None => view! { <span class="muted">{i18n.t(K::LoadingChoices)}</span> }.into_any(),
                        Some(c) => view! { <strong>{c.label}</strong><small class="mono muted">{c.value}</small> }.into_any(),
                    }}
                </div>
            }.into_any()
        }
        Kind::Assignments { actors } => {
            let set = |s: ChoiceSet| Signal::derive(move || choices.with(|m| m.get(&s).cloned()));
            view! {
                <crate::components::assignment_rows::AssignmentRows id=id.clone() labelled_by=label_id.clone()
                    initial=get_untracked_value(values, name) actors=set(actors) roles=set(ChoiceSet::IdentityRoles)
                    on_change=Callback::new(move |v: String| put(v))/>
            }.into_any()
        }
        Kind::Date { future_only } => view! {
            <crate::components::datetime::DatePicker id=id.clone() value=Signal::derive(get) future_only=future_only
                on_change=Callback::new(move |v: String| put(v)) described_by=Signal::derive(described.clone())/>
        }.into_any(),
        Kind::Time => view! {
            <crate::components::datetime::TimePicker id=id.clone() value=Signal::derive(get)
                on_change=Callback::new(move |v: String| put(v)) described_by=Signal::derive(described.clone())/>
        }.into_any(),
        Kind::Password => view! {
            <input id=id.clone() type="password" autocomplete="new-password" prop:value=get aria-invalid=invalid aria-describedby=described
                on:input=move |ev| put(event_target_value(&ev))/>
        }.into_any(),
        Kind::TextArea => {
            let area = view! {
                <textarea id=id.clone() rows=field.rows.unwrap_or(4).to_string() prop:value=get aria-invalid=invalid aria-describedby=described
                    on:input=move |ev| put(event_target_value(&ev))></textarea>
            };
            match field.from_file {
                None => area.into_any(),
                Some(limit) => {
                    let size = Signal::derive(move || get().len());
                    // Horizon's "Script size" is the launch script's; other files (a
                    // metadata namespace's JSON) just say "Size".
                    let what = if field.name == "user_data" { K::ScriptSize } else { K::Size };
                    view! {
                        <FileLoad limit=limit on_text=Callback::new(move |s: String| put(s))/>
                        {area}
                        <small class="muted" class=("over", move || size.get() > limit)>
                            {move || script_size(i18n.lang(), what, size.get(), limit)}
                        </small>
                    }.into_any()
                }
            }
        }
        Kind::File => {
            let picked = move |ev: leptos::ev::Event| {
                #[cfg(feature = "hydrate")]
                {
                    use wasm_bindgen::JsCast;
                    let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
                    let file = input.files().and_then(|l| l.get(0));
                    put(file.as_ref().map(|f| Picked { name: f.name(), size: f.size() as u64 }.encode()).unwrap_or_default());
                    upload::remember(file);
                }
                #[cfg(not(feature = "hydrate"))]
                let _ = ev;
            };
            // A styled button, not the browser's own control (which looks different on every
            // platform and can't be translated); the real input stays for keyboard and screen readers.
            view! {
                <div class="file-load">
                    <label class="btn btn-sm">
                        {move || i18n.t(K::ChooseFile)}
                        <input id=id.clone() type="file" class="sr-only" aria-invalid=invalid aria-describedby=described on:change=picked/>
                    </label>
                    <small class="muted">
                        {move || Picked::decode(&get()).map(|p| format!("{} ({})", p.name, bytes(p.size)))}
                    </small>
                </div>
            }.into_any()
        }
        Kind::Number { min } => view! {
            <input id=id.clone() type="number" min=min prop:value=get aria-invalid=invalid aria-describedby=described
                on:input=move |ev| put(event_target_value(&ev))/>
        }.into_any(),
        Kind::Check => {
            return view! {
                <div class="field check" class:half=field.half class:danger=field.danger style:display=move || if shown() { "" } else { "none" }>
                    <label>
                        <input type="checkbox" prop:checked=move || get() == "true"
                            on:change=move |ev| put(event_target_checked(&ev).to_string())/>
                        <span>{label.clone()}</span>
                    </label>
                    {help.clone().map(|h| view! { <small class="muted">{h}</small> })}
                    {error_view}
                </div>
            }.into_any();
        }
        Kind::YesNo => view! {
            <div class="yes-no" role="radiogroup" aria-labelledby=label_id.clone()>
                <button type="button" role="radio" aria-checked=move || (get() == "true").to_string()
                    class:on=move || get() == "true" on:click=move |_| put("true".into())>{move || i18n.t(K::Yes)}</button>
                <button type="button" role="radio" aria-checked=move || (get() != "true").to_string()
                    class:on=move || get() != "true" on:click=move |_| put("false".into())>{move || i18n.t(K::No)}</button>
            </div>
        }.into_any(),
        Kind::Select(src) => {
            let options = Signal::derive(move || resolve(&src, values, choices));
            view! {
                <Combo id=id.clone() options=options value=Signal::derive(get) required=req empty_label=field.empty_label.clone()
                    on_change=Callback::new(move |v: String| put(v))/>
            }.into_any()
        }
        Kind::Pick { src, multi } => {
            let src2 = src.clone();
            let options = Signal::derive(move || resolve(&src, values, choices));
            let set_kind = Signal::derive(move || values.with(|v| choice_set_for(&src2, v)));
            let blocked = Callback::new(move |c: Choice| {
                block.as_ref().and_then(|b| values.with(|v| choices.with(|s| b(name, &c, v, s))))
            });
            view! {
                <PickTable id=id.clone() multi=multi options=options set=set_kind value=Signal::derive(get)
                    on_change=Callback::new(move |v: String| put(v)) blocked=blocked/>
            }.into_any()
        }
        Kind::KeyValue => view! {
            <KeyValueRows id=id.clone() labelled_by=label_id.clone() initial=get_untracked_value(values, name) catalog=field.catalog skip=field.catalog_skip
                on_change=Callback::new(move |v: String| put(v))/>
        }.into_any(),
        Kind::MemberRows { port_from, existing } => {
            let set = |s: ChoiceSet| Signal::derive(move || choices.with(|m| m.get(&s).cloned()));
            let default_port = Signal::derive(move || port_from.and_then(|f| values.with(|v| v.get(f).cloned())).unwrap_or_default());
            view! {
                <crate::components::member_rows::MemberRows id=id.clone() labelled_by=label_id.clone()
                    initial=get_untracked_value(values, name) targets=set(ChoiceSet::LbMemberTargets) subnets=set(ChoiceSet::LbMemberSubnets)
                    default_port=default_port existing=existing.clone() on_change=Callback::new(move |v: String| put(v))/>
            }.into_any()
        }
        Kind::Multi(src) => {
            let src2 = src.clone();
            // Values ticked when the dialog opened but missing from the set (a
            // project deleted since it was given flavor access) stay as rows of
            // their own, labelled by ID: otherwise the count says *1 selected*
            // with nothing ticked, and saving drops them unseen (roadmap 89).
            let initial = get_untracked_value(values, name);
            let opts = move || resolve(&src2, values, choices).map(|l| with_unknown(l, &initial));
            let src3 = src.clone();
            let long = move || resolve(&src3, values, choices).is_some_and(|l| l.len() > MULTI_FILTER_FROM);
            // Hidden rows keep their ticks: the filter only narrows what is shown.
            let filter = RwSignal::new(String::new());
            let picked = move || get().split(',').filter(|s| !s.is_empty()).count();
            view! {
                {move || long().then(|| view! {
                    <div class="multi-filter">
                        <input type="search" class="pick-filter" placeholder=move || i18n.t(K::FilterPlaceholder)
                            aria-label=move || i18n.t(K::FilterPlaceholder) prop:value=move || filter.get()
                            on:input=move |ev| filter.set(event_target_value(&ev))/>
                        <small class="muted">{move || format!("{} {}", picked(), i18n.t(K::NSelected))}</small>
                    </div>
                })}
                <div class="multi" id=id.clone() role="group" aria-labelledby=label_id.clone()>
                    {move || match opts() {
                        None => view! { <span class="muted">{i18n.t(K::LoadingChoices)}</span> }.into_any(),
                        Some(list) => list.into_iter().map(|c| {
                            let v = c.value.clone();
                            let v2 = c.value.clone();
                            let checked = move || get().split(',').any(|x| x == v);
                            let (lbl, val) = (c.label.clone(), c.value.clone());
                            let hidden = move || filter.with(|q| !filter_matches(q, &lbl, &val));
                            view! {
                                <label class="multi-item" style:display=move || if hidden() { "none" } else { "" }>
                                    <input type="checkbox" prop:checked=checked on:change=move |ev| {
                                        let on = event_target_checked(&ev);
                                        let mut cur: Vec<String> = get().split(',').filter(|s| !s.is_empty() && *s != v2).map(str::to_string).collect();
                                        if on { cur.push(v2.clone()); }
                                        put(cur.join(","));
                                    }/>
                                    <span>{c.label}</span>
                                </label>
                            }
                        }).collect_view().into_any(),
                    }}
                </div>
            }.into_any()
        }
    };

    let star = req.then(|| view! { <span class="req" aria-hidden="true">" *"</span> });
    let label_view = if grouped {
        view! { <span class="field-label" id=label_id>{label}{star}</span> }.into_any()
    } else {
        view! { <label for=id id=label_id>{label}{star}</label> }.into_any()
    };
    // The tip sits beside the label, not inside it: a click on it must not
    // move the focus to the control.
    let label_view = match tip {
        Some(tip) => view! { <div class="field-head">{label_view}{tip}</div> }.into_any(),
        None => label_view,
    };
    let (short, half, new_line) = (field.short, field.half, field.new_line);
    view! {
        <div class="field" class:short=short class:half=half class:new-line=new_line class:invalid=move || error.get().is_some() style:display=move || if shown() { "" } else { "none" }>
            {label_view}
            {control}
            {help.map(|h| view! { <small class="muted">{h}</small> })}
            {help_code.map(|c| view! { <pre class="help-code">{c}</pre> })}
            {error_view}
        </div>
    }.into_any()
}

/// A checkbox list longer than this gets a filter box (e.g. *Flavor Access*'s
/// projects, every domain's).
const MULTI_FILTER_FROM: usize = 8;

/// `list` plus a choice for every comma-separated value of `selected` it
/// lacks, labelled by the value itself, after the known ones.
fn with_unknown(mut list: Vec<Choice>, selected: &str) -> Vec<Choice> {
    for v in selected.split(',').filter(|v| !v.is_empty()) {
        if !list.iter().any(|c| c.value == v) {
            list.push(Choice::new(v, v));
        }
    }
    list
}

/// Whether a choice matches a filter: case-insensitive, on its label or value
/// (an ID pasted from elsewhere finds its project); every word must match.
fn filter_matches(query: &str, label: &str, value: &str) -> bool {
    let (label, value) = (label.to_lowercase(), value.to_lowercase());
    query.to_lowercase().split_whitespace().all(|w| label.contains(w) || value.contains(w))
}

/// `Script size: 120 bytes of 16.00 KB`, like Horizon; `what` names the size.
pub fn script_size(lang: Lang, what: K, bytes: usize, limit: usize) -> String {
    format!("{}: {bytes} {} {:.2} KB", t(lang, what), t(lang, K::BytesOf), limit as f64 / 1024.0)
}

/// Why a picked file can't fill the field, if it can't: too big (checked
/// before reading) or not text (decoded with replacement characters).
pub fn file_problem(lang: Lang, size: f64, text: Option<&str>, limit: usize) -> Option<String> {
    if size > limit as f64 {
        return Some(format!("{} ({:.2} KB)", t(lang, K::FileTooBig), limit as f64 / 1024.0));
    }
    text.filter(|s| s.contains('\u{FFFD}') || s.contains('\0')).map(|_| t(lang, K::FileNotText).to_string())
}

/// *Load from a file*: reads the picked file in the browser and hands its text
/// over; nothing is uploaded here (the text goes out with the form).
#[component]
pub fn FileLoad(limit: usize, on_text: Callback<String>) -> impl IntoView {
    let i18n = use_i18n();
    let problem = RwSignal::new(None::<String>);
    let picked = move |ev: leptos::ev::Event| {
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
            let Some(file) = input.files().and_then(|l| l.get(0)) else { return };
            // Picking the same file again must fire `change` again.
            input.set_value("");
            let lang = i18n.lang();
            if let Some(p) = file_problem(lang, file.size(), None, limit) {
                problem.set(Some(p));
                return;
            }
            spawn_local(async move {
                let text = wasm_bindgen_futures::JsFuture::from(file.text()).await.ok().and_then(|v| v.as_string());
                match text {
                    Some(s) if file_problem(lang, 0.0, Some(&s), limit).is_none() => {
                        problem.set(None);
                        on_text.run(s);
                    }
                    _ => problem.set(Some(t(lang, K::FileNotText).to_string())),
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (ev, limit, on_text, problem);
    };
    view! {
        <div class="file-load">
            <label class="btn btn-sm">
                {move || i18n.t(K::LoadFromFile)}
                <input type="file" class="sr-only" on:change=picked/>
            </label>
            {move || problem.get().map(|p| view! { <small class="field-error" role="alert">{p}</small> })}
        </div>
    }
}

/// `sent` of `total` as 0–100.
fn percent((sent, total): (f64, f64)) -> f64 {
    if total > 0.0 { (sent / total * 100.0).clamp(0.0, 100.0) } else { 0.0 }
}

/// Sends the file picked for *Create Image* to the image `id` that now exists (roadmap 45),
/// showing progress in `phase`. `proxy` goes through Moonlight's route; `direct` asks the
/// server for a ticket first and goes to Glance itself. `out` is the creation's outcome, shown
/// when the data arrived; on failure the dialog offers to delete the empty image.
#[allow(clippy::too_many_arguments)] // one call site; a struct would only rename the arguments
async fn upload_image(
    id: String,
    size: u64,
    mode: crate::prefs::ImageUpload,
    out: ActionOutcome,
    phase: RwSignal<Phase>,
    progress: RwSignal<(f64, f64)>,
    transfer: StoredValue<Upload, LocalStorage>,
    version: RwSignal<u64>,
) {
    use crate::prefs::ImageUpload;
    phase.set(Phase::Uploading);
    progress.set((0.0, size as f64));
    let (url, token) = match mode {
        ImageUpload::Direct => match get_image_upload_ticket(id.clone(), size).await {
            Ok(t) => (t.url, Some(t.token)),
            Err(e) => return phase.set(Phase::UploadFailed(id, e.to_string())),
        },
        _ => (crate::model::image_upload_path(&id), None),
    };
    let failed_id = id.clone();
    let done = Callback::new(move |r: upload::Outcome| match r {
        Ok(()) => {
            version.update(|v| *v += 1);
            phase.set(Phase::Done(out.clone()));
        }
        Err(e) => phase.set(Phase::UploadFailed(failed_id.clone(), e)),
    });
    transfer.set_value(upload::start(&url, token.as_deref(), Callback::new(move |p| progress.set(p)), done));
}

fn get_untracked_value(values: RwSignal<Vals>, name: &str) -> String {
    values.with_untracked(|v| v.get(name).cloned().unwrap_or_default())
}

/// The rows live in their own signals so typing doesn't rebuild them (and
/// lose focus); every change is written back to the form as JSON.
#[component]
fn KeyValueRows(id: String, labelled_by: String, initial: String, catalog: Option<&'static str>, skip: &'static [&'static str], on_change: Callback<String>) -> impl IntoView {
    let i18n = use_i18n();
    let start: Vec<(String, String)> = serde_json::from_str(&initial).unwrap_or_default();
    let next = StoredValue::new(start.len());
    let rows = RwSignal::new(
        start.into_iter().enumerate().map(|(i, (k, v))| (i, RwSignal::new(k), RwSignal::new(v))).collect::<Vec<_>>(),
    );
    let sync = move || {
        let pairs: Vec<(String, String)> = rows.with_untracked(|r| r.iter().map(|(_, k, v)| (k.get_untracked(), v.get_untracked())).collect());
        on_change.run(serde_json::to_string(&pairs).unwrap_or_default());
    };
    // Rows are added from the catalog combo's handler, whose owner is disposed when it closes:
    // their signals must belong to this component (AGENTS.md §6, roadmap 54).
    let owner = StoredValue::new(Owner::current());
    let push = move |k: String, v: String| {
        let i = next.get_value();
        next.set_value(i + 1);
        let make = || (i, RwSignal::new(k), RwSignal::new(v));
        let row = match owner.get_value() {
            Some(o) => o.with(make),
            None => make(),
        };
        rows.update(|r| r.push(row));
        sync();
    };
    let add = move |_| push(String::new(), String::new());
    let id2 = id.clone();
    // The metadata catalog (roadmap 25): read when the dialog opens. If it fails or is empty the
    // editor stays plain, so a cloud without Glance's metadefs loses nothing.
    let options = LocalResource::new(move || async move {
        match catalog {
            Some(rt) => list_metadata_options(rt.to_string()).await.map(|t| t.data).unwrap_or_default(),
            None => Vec::new(),
        }
    });
    let all = Signal::derive(move || options.get().map(|o| o.to_vec()).unwrap_or_default());
    // Properties not yet in the rows, grouped by namespace in the picker.
    let available = move || {
        let used: HashSet<String> = rows.with(|r| r.iter().map(|(_, k, _)| k.get().trim().to_string()).collect());
        all.with(|o| o.iter().filter(|p| !used.contains(&p.key) && !skip.contains(&p.key.as_str())).cloned().collect::<Vec<_>>())
    };
    view! {
        <div class="kv-rows" id=id role="group" aria-labelledby=labelled_by>
            {move || rows.with(Vec::is_empty).then(|| view! { <p class="muted">{i18n.t(K::NoMetadata)}</p> })}
            <For each=move || rows.get() key=|(i, _, _)| *i let:row>
                {
                    let (i, k, v) = row;
                    // The catalog's property for this row's key, which types the value input.
                    let known = move || {
                        let key = k.get();
                        all.with(|o| o.iter().find(|o| o.key == key.trim()).cloned())
                    };
                    let value_input = move || match known() {
                        Some(o) if !o.choices.is_empty() || o.kind == MetadataKind::Boolean => {
                            let mut list = if o.choices.is_empty() { vec!["true".to_string(), "false".to_string()] } else { o.choices.clone() };
                            let current = v.get_untracked();
                            if !current.is_empty() && !list.contains(&current) {
                                list.insert(0, current.clone());
                            }
                            view! {
                                <select aria-label=move || i18n.t(K::Value) title=o.description.clone().unwrap_or_default()
                                    on:change=move |ev| { v.set(event_target_value(&ev)); sync(); }>
                                    {list.into_iter().map(|c| { let (sel, text) = (c == current, c.clone()); view! { <option value=c selected=sel>{text}</option> } }).collect_view()}
                                </select>
                            }.into_any()
                        }
                        other => {
                            let numeric = other.as_ref().map(|o| o.kind);
                            let (kind, step) = match numeric {
                                Some(MetadataKind::Integer) => ("number", "1"),
                                Some(MetadataKind::Number) => ("number", "any"),
                                _ => ("text", ""),
                            };
                            view! {
                                <input type=kind step=step maxlength="255" prop:value=move || v.get()
                                    title=other.and_then(|o| o.description).unwrap_or_default()
                                    placeholder=move || i18n.t(K::Value) aria-label=move || i18n.t(K::Value)
                                    on:input=move |ev| { v.set(event_target_value(&ev)); sync(); }/>
                            }.into_any()
                        }
                    };
                    view! {
                        <div class="kv-row">
                            <input type="text" class="mono" maxlength="255" prop:value=move || k.get()
                                placeholder=move || i18n.t(K::Key) aria-label=move || i18n.t(K::Key)
                                on:input=move |ev| { k.set(event_target_value(&ev)); sync(); }/>
                            {move || value_input()}
                            <button type="button" class="btn btn-icon" title=move || i18n.t(K::Remove) aria-label=move || i18n.t(K::Remove)
                                on:click=move |_| { rows.update(|r| r.retain(|(j, _, _)| *j != i)); sync(); }>"×"</button>
                        </div>
                    }
                }
            </For>
            <div class="kv-add">
                // Built once the catalog has loaded; the options inside follow the rows.
                {move || all.with(|o| !o.is_empty()).then(|| {
                    // The Combo hands over the ticked keys as a JSON list.
                    let pick = Callback::new(move |keys: String| {
                        for key in serde_json::from_str::<Vec<String>>(&keys).unwrap_or_default() {
                            if let Some(o) = all.with(|o| o.iter().find(|o| o.key == key).cloned()) {
                                let first = o.default.clone().or_else(|| o.choices.first().cloned()).or_else(|| (o.kind == MetadataKind::Boolean).then(|| "true".to_string()));
                                push(o.key, first.unwrap_or_default());
                            }
                        }
                    });
                    let options = Signal::derive(move || Some(available().into_iter().map(|o| Choice::new(o.key.clone(), format!("{} ({})", o.title, o.key))).collect::<Vec<_>>()));
                    view! {
                        <div class="kv-catalog">
                            <Combo id=format!("{id2}-catalog") options=options value=Signal::derive(String::new)
                                required=true multi=true empty_label=Some(i18n.t(K::AddFromCatalog).to_string()) on_change=pick/>
                        </div>
                    }
                })}
                <button type="button" class="btn kv-custom" on:click=add>{move || i18n.t(K::AddCustom)}" +"</button>
            </div>
        </div>
    }
}

fn resolve(src: &Source, values: RwSignal<HashMap<String, String>>, choices: RwSignal<HashMap<ChoiceSet, Vec<Choice>>>) -> Option<Vec<Choice>> {
    match src {
        Source::Static(list) => Some(list.iter().map(|(v, l)| Choice::new(v.clone(), l.clone())).collect()),
        Source::Of(set, _) | Source::Except(set, _) | Source::Without(set, _) => choices.with(|m| m.get(set).map(|l| l.iter().filter(|c| src.admits(c)).cloned().collect())),
        other => {
            let set = values.with(|v| choice_set_for(other, v))?;
            choices.with(|m| m.get(&set).cloned())
        }
    }
}

// ---------------------------------------------------------------------------
// Buttons & row menus
// ---------------------------------------------------------------------------

/// One entry of a row's action menu.
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub danger: bool,
    pub kind: ItemKind,
}

/// What a menu entry does: open a dialog (built on click), follow a link, or
/// run something that only changes Moonlight's session (e.g. *Set as Active
/// Project*), so it stays enabled with `[writes] mode = off`.
#[derive(Clone)]
pub enum ItemKind {
    Dialog(Arc<dyn Fn() -> Dialog + Send + Sync>),
    Link(String),
    Run(Arc<dyn Fn() + Send + Sync>),
}

impl MenuItem {
    /// An entry that opens the dialog `f` builds when clicked.
    pub fn dialog(label: impl Into<String>, f: impl Fn() -> Dialog + Send + Sync + 'static) -> Self {
        MenuItem { label: label.into(), danger: false, kind: ItemKind::Dialog(Arc::new(f)) }
    }
    /// A destructive entry (shown in red, placed last), opening `f`'s dialog.
    pub fn danger(label: impl Into<String>, f: impl Fn() -> Dialog + Send + Sync + 'static) -> Self {
        MenuItem { label: label.into(), danger: true, kind: ItemKind::Dialog(Arc::new(f)) }
    }
    /// An entry that navigates to `href`.
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Self {
        MenuItem { label: label.into(), danger: false, kind: ItemKind::Link(href.into()) }
    }
    /// An entry that runs `f` at once (no dialog, no OpenStack write).
    pub fn action(label: impl Into<String>, f: impl Fn() + Send + Sync + 'static) -> Self {
        MenuItem { label: label.into(), danger: false, kind: ItemKind::Run(Arc::new(f)) }
    }
}

fn disabled_reason(mode: WriteMode, lang: Lang) -> Option<&'static str> {
    (mode == WriteMode::Off).then(|| t(lang, K::WritesOff))
}

/// Whether an empty field takes its set's first choice once loaded: required
/// selects do, except those with a placeholder (`empty_label`), where the user
/// must choose (roadmap 85).
fn preselects(f: &Field) -> bool {
    f.required && f.empty_label.is_none() && matches!(f.kind, Kind::Select(_))
}

/// Horizon-style split button: the first item as a button, the rest in a menu.
#[component]
pub fn RowActions(items: Vec<MenuItem>) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    let mut items = items.into_iter();
    let Some(first) = items.next() else { return ().into_any() };
    let rest: Vec<MenuItem> = items.collect();
    let run = move |it: &MenuItem| match &it.kind {
        ItemKind::Dialog(f) => dialogs.open(f()),
        ItemKind::Run(f) => f(),
        ItemKind::Link(_) => {}
    };
    let primary = match first.kind.clone() {
        ItemKind::Link(href) => view! { <a class="btn btn-sm" href=href>{first.label.clone()}</a> }.into_any(),
        ItemKind::Run(_) => {
            let it = first.clone();
            view! { <button type="button" class="btn btn-sm" on:click=move |_| run(&it)>{first.label.clone()}</button> }.into_any()
        }
        ItemKind::Dialog(_) => {
            let it = first.clone();
            view! {
                <button type="button" class=if first.danger { "btn btn-sm btn-danger-ghost" } else { "btn btn-sm" }
                    disabled=move || dialogs.mode.get() == WriteMode::Off
                    title=move || disabled_reason(dialogs.mode.get(), i18n.lang()).unwrap_or("")
                    on:click=move |_| run(&it)>
                    {first.label.clone()}
                </button>
            }.into_any()
        }
    };
    let menu = (!rest.is_empty()).then(|| {
        let details = NodeRef::<leptos::html::Details>::new();
        view! {
            <details class="row-menu" node_ref=details
                on:toggle=move |_| place_menu(details)
                on:keydown=move |ev| {
                    if ev.key() == "Escape"
                        && let Some(d) = details.get_untracked()
                    {
                        d.set_open(false);
                        focus_summary(&d);
                    }
                }>
                <summary class="btn btn-sm btn-caret" aria-label=move || i18n.t(K::More)>
                    <svg aria-hidden="true" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><path d="m6 9 6 6 6-6"></path></svg>
                </summary>
                <ul class="menu-list" role="menu">
                    {rest.into_iter().map(|it| {
                        let label = it.label.clone();
                        let danger = it.danger;
                        match it.kind.clone() {
                            ItemKind::Link(href) => view! { <li><a class="menu-item" href=href>{label}</a></li> }.into_any(),
                            ItemKind::Run(_) => view! {
                                <li>
                                    <button type="button" role="menuitem" on:click=move |_| {
                                        if let Some(d) = details.get_untracked() { d.set_open(false); }
                                        run(&it)
                                    }>{label}</button>
                                </li>
                            }.into_any(),
                            ItemKind::Dialog(_) => view! {
                                <li>
                                    <button type="button" role="menuitem" class:danger=danger
                                        disabled=move || dialogs.mode.get() == WriteMode::Off
                                        on:click=move |_| {
                                            if let Some(d) = details.get_untracked() { d.set_open(false); }
                                            run(&it)
                                        }>
                                        {label}
                                    </button>
                                </li>
                            }.into_any(),
                        }
                    }).collect_view()}
                </ul>
            </details>
        }
    });
    view! { <div class="row-actions">{primary}{menu}</div> }.into_any()
}

/// Where a `position: fixed` row menu opens, in viewport pixels: `(left, top,
/// max_height)`. `trigger` is the button's `(top, bottom, right)`, `menu` the
/// list's natural `(width, height)`, `viewport` its `(width, height)`. Below the
/// button when the list fits there, above when only that fits, else on the side
/// with more room with its height capped to it (the list scrolls inside; roadmap
/// 47: the instance menu, ~17 items, ran off the bottom of the window and its
/// last items couldn't be reached). Right-aligned with the button, kept inside
/// the viewport horizontally.
pub fn menu_box(trigger: (f64, f64, f64), menu: (f64, f64), viewport: (f64, f64)) -> (f64, f64, f64) {
    const GAP: f64 = 4.0;
    const EDGE: f64 = 8.0;
    let ((top, bottom, right), (mw, mh), (vw, vh)) = (trigger, menu, viewport);
    let below = (vh - EDGE - bottom - GAP).max(0.0);
    let above = (top - GAP - EDGE).max(0.0);
    let left = (right - mw).min(vw - EDGE - mw).max(EDGE);
    if mh <= below || (mh > above && below >= above) {
        (left, bottom + GAP, below)
    } else {
        (left, (top - GAP - mh).max(EDGE), above)
    }
}

/// Row menus live inside scrolling tables (`overflow-x: auto` clips anything
/// absolutely positioned), so the open menu is positioned with `position: fixed`
/// next to its trigger (see [`menu_box`]).
fn place_menu(details: NodeRef<leptos::html::Details>) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let Some(d) = details.get_untracked() else { return };
        if !d.open() {
            return;
        }
        let (Some(summary), Some(menu)) = (
            d.query_selector("summary").ok().flatten(),
            d.query_selector(".menu-list").ok().flatten().and_then(|m| m.dyn_into::<web_sys::HtmlElement>().ok()),
        ) else {
            return;
        };
        let r = summary.get_bounding_client_rect();
        // The natural height (`scroll_height`): `offset_height` is already capped by the CSS max-height.
        let (mw, mh) = (menu.offset_width() as f64, menu.scroll_height() as f64);
        let size = |v: Result<wasm_bindgen::JsValue, _>, d| v.ok().and_then(|v| v.as_f64()).unwrap_or(d);
        let viewport = (size(window().inner_width(), 1200.0), size(window().inner_height(), 800.0));
        let (left, top, max_h) = menu_box((r.top(), r.bottom(), r.right()), (mw, mh), viewport);
        let _ = menu.set_attribute("style", &format!("left:{left}px;top:{top}px;max-height:{max_h}px"));
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = details;
}

fn focus_summary(_d: &web_sys::HtmlDetailsElement) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(s) = _d.query_selector("summary").ok().flatten().and_then(|s| s.dyn_into::<web_sys::HtmlElement>().ok()) {
            let _ = s.focus();
        }
    }
}

/// Closes every open `<details>` menu that does not contain `target`
/// (outside click), and all of them on scroll/resize (their fixed position
/// would otherwise drift away from the trigger). Installed once by the layout.
pub fn install_menu_dismissal() {
    #[cfg(feature = "hydrate")]
    {
        use leptos::ev;
        use wasm_bindgen::JsCast;
        fn close_all(except: Option<&web_sys::Node>) {
            close_matching("details.row-menu[open], details.menu[open]", except);
        }
        fn close_matching(selector: &str, except: Option<&web_sys::Node>) {
            let Some(doc) = window().document() else { return };
            let Ok(list) = doc.query_selector_all(selector) else { return };
            for i in 0..list.length() {
                if let Some(d) = list.item(i).and_then(|n| n.dyn_into::<web_sys::HtmlDetailsElement>().ok())
                    && except.is_none_or(|t| !d.contains(Some(t)))
                {
                    d.set_open(false);
                }
            }
        }
        let _ = window_event_listener(ev::click, |e| {
            let target = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok());
            close_all(target.as_ref());
        });
        let _ = window_event_listener(ev::resize, |_| close_all(None));
        // Scroll does not bubble: listen in the capture phase to catch table scrolls too.
        let closure = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::Event)>::new(|e: web_sys::Event| {
            // Scrolling inside an open menu (long lists) must not close it.
            let inside_menu = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .is_some_and(|el| el.closest(".menu-list").ok().flatten().is_some());
            // Only row menus drift on scroll; header menus sit in the sticky header.
            if !inside_menu {
                close_matching("details.row-menu[open]", None);
            }
        });
        let _ = window().add_event_listener_with_callback_and_bool("scroll", closure.as_ref().unchecked_ref(), true);
        closure.forget();
    }
}

/// Page-level button that opens a dialog ("+ Create Network").
#[component]
pub fn CreateButton(#[prop(into)] label: Signal<String>, open: Arc<dyn Fn() -> Dialog + Send + Sync>, #[prop(optional)] danger: bool) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    view! {
        <button type="button" class=if danger { "btn btn-danger-ghost" } else { "btn btn-primary" }
            disabled=move || dialogs.mode.get() == WriteMode::Off
            title=move || disabled_reason(dialogs.mode.get(), i18n.lang()).unwrap_or("")
            on:click=move |_| dialogs.open(open())>
            {(!danger).then(|| view! { <span aria-hidden="true">"+ "</span> })}
            {label}
        </button>
    }
}

/// Opens the bulk dialog for the selected `(id, name)` pairs.
pub type BulkOpen = Arc<dyn Fn(Lang, Vec<(String, String)>) -> Dialog + Send + Sync>;

/// Page-level bulk button ("Delete Volumes (3)"), enabled once rows are
/// selected in the table; `open` gets the selection as `(id, name)` pairs.
#[component]
pub fn BulkButton(
    label: K,
    picked: RwSignal<Vec<(String, String)>>,
    open: BulkOpen,
) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    view! {
        <button type="button" class="btn btn-danger-ghost"
            disabled=move || dialogs.mode.get() == WriteMode::Off || picked.with(Vec::is_empty)
            title=move || disabled_reason(dialogs.mode.get(), i18n.lang()).unwrap_or("")
            on:click=move |_| dialogs.open(open(i18n.lang(), picked.get_untracked()))>
            {move || with_count(i18n.lang(), label, picked.with(Vec::len))}
        </button>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn multi_filter_matches_label_or_value_and_every_word() {
        use super::filter_matches;
        assert!(filter_matches("", "Pokemon", "p1"), "an empty filter shows everything");
        assert!(filter_matches("POKE", "pokemon (ttcp-spain)", "p1"), "case-insensitive");
        assert!(filter_matches("ab12", "pokemon", "AB12cd"), "an ID finds its project");
        assert!(filter_matches("poke spain", "pokemon (ttcp-spain)", "p1"));
        assert!(!filter_matches("poke admin", "pokemon (ttcp-spain)", "p1"), "every word must match");
    }

    use super::*;

    #[test]
    fn multi_keeps_selected_values_missing_from_the_set() {
        let known = vec![Choice::new("p1", "pokemon"), Choice::new("p2", "admin2")];
        let got = with_unknown(known.clone(), "p2,gone,,gone");
        let values: Vec<_> = got.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect();
        assert_eq!(values, [("p1", "pokemon"), ("p2", "admin2"), ("gone", "gone")], "a deleted project shows as its ID, once");
        assert_eq!(with_unknown(known.clone(), ""), known, "nothing selected adds nothing");
    }

    #[test]
    fn admin_project_pickers_make_the_user_choose() {
        let set = || Source::Set(ChoiceSet::AllProjects);
        assert!(preselects(&Field::select("p", "P", set()).required()), "a required select takes its first choice");
        assert!(!preselects(&Field::select("p", "P", set())), "an optional one stays on *None*");
        assert!(!preselects(&Field::select("p", "P", set()).required().empty_label("Select a project")), "a placeholder means: choose");
        use crate::{components::ops, i18n::Lang};
        for d in [ops::admin_create_network_dialog(Lang::EnUs), ops::admin_create_router_dialog(Lang::EnUs), ops::admin_allocate_ip_dialog(Lang::EnUs)] {
            let project = d.fields.iter().find(|f| f.name == "project_id").expect("a project field");
            assert!(project.required && !preselects(project), "{}: the owner project must be picked, not the first one", d.title);
        }
        for d in [ops::admin_create_network_dialog(Lang::EnUs), ops::admin_create_router_dialog(Lang::EnUs)] {
            let hints = d.fields.iter().find(|f| f.name == "az_hints").expect("zone hints");
            assert!(hints.hide_empty, "{}: no zones of that kind, no picker (as Horizon)", d.title);
        }
        // `*` first would share with every project by default.
        let rbac = ops::create_rbac_policy_dialog(Lang::EnUs);
        for name in ["target_project", "network_id", "qos_policy_id"] {
            let f = rbac.fields.iter().find(|f| f.name == name).expect("an RBAC field");
            assert!(f.required && !preselects(f), "Create RBAC Policy: {name} must be picked");
        }
    }

    #[test]
    fn except_drops_one_value_and_of_keeps_one_owner() {
        let c = |v: &str, owner: &str| Choice { cells: vec![owner.to_string()], ..Choice::new(v, v) };
        let except = Source::Except(ChoiceSet::ComputeHosts, "c1".into());
        assert!(!except.admits(&c("c1", "")) && except.admits(&c("c2", "")), "the current host is left out");
        let of = Source::Of(ChoiceSet::InstancePorts, "s1".into());
        assert!(of.admits(&c("p1", "s1")) && !of.admits(&c("p2", "s2")));
        assert!(Source::Set(ChoiceSet::ComputeHosts).admits(&c("c1", "")));
    }

    #[test]
    fn long_menus_stay_reachable() {
        // A 700 px high window; the instance menu (~17 items) is 600 px tall.
        let vp = (1200.0, 700.0);
        let (_, top, max_h) = menu_box((100.0, 130.0, 900.0), (220.0, 300.0), vp);
        assert_eq!((top, max_h), (134.0, 558.0), "fits below: opens below");
        let (_, top, _) = menu_box((600.0, 630.0, 900.0), (220.0, 300.0), vp);
        assert_eq!(top, 296.0, "only fits above: opens above, its bottom 4 px over the button");
        let (_, top, max_h) = menu_box((400.0, 430.0, 900.0), (220.0, 600.0), vp);
        assert_eq!((top, max_h), (8.0, 388.0), "fits neither way: the bigger side (above), capped and scrolling");
        let (_, top, max_h) = menu_box((200.0, 230.0, 900.0), (220.0, 600.0), vp);
        assert_eq!((top, max_h), (234.0, 458.0), "fits neither way: below has more room");
        let (left, ..) = menu_box((100.0, 130.0, 150.0), (220.0, 300.0), vp);
        assert_eq!(left, 8.0, "never off the left edge");
        let (left, ..) = menu_box((100.0, 130.0, 1300.0), (220.0, 300.0), vp);
        assert_eq!(left, 972.0, "nor off the right edge");
    }

    #[test]
    fn script_files_are_checked_before_filling_the_field() {
        let l = Lang::EnUs;
        assert_eq!(script_size(l, K::ScriptSize, 120, 16 * 1024), "Script size: 120 bytes of 16.00 KB");
        assert!(file_problem(l, 20_000.0, None, 16 * 1024).unwrap().contains("too large"));
        assert_eq!(file_problem(l, 100.0, None, 16 * 1024), None);
        assert_eq!(file_problem(l, 0.0, Some("#!/bin/sh\necho hi\n"), 16 * 1024), None);
        assert!(file_problem(l, 0.0, Some("PK\u{FFFD}\u{FFFD}"), 16 * 1024).is_some(), "binary file");
        assert!(Field::area("user_data", "Script").from_file(16 * 1024).from_file.is_some());
    }

    fn vals(pairs: &[(&str, &str)]) -> Values {
        Values(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn dialogs_get_the_width_their_content_needs() {
        let form = |fields: Vec<Field>| Dialog::form("t", "ok", fields, |_| Err("unused".into()));
        let text = |n| Field::text(n, n);
        let action = OsAction::FloatingIpRelease { id: "f1".into() };
        assert_eq!(Dialog::confirm(Lang::EnUs, "Delete", "x", action).size(), DialogSize::Small);
        assert_eq!(form(vec![text("a"), text("b")]).size(), DialogSize::Small, "e.g. Allocate IP");
        assert_eq!(form(vec![text("a"), text("b"), text("c"), text("d")]).size(), DialogSize::Medium);
        assert_eq!(form(vec![Field::area("a", "a")]).size(), DialogSize::Medium, "tall fields need room");
        let mut wizard = form(vec![text("a")]);
        wizard.steps = vec![Step::new("s", &["a"])];
        assert_eq!(wizard.size(), DialogSize::Large);
        assert_eq!(form(vec![text("a")]).sized(DialogSize::Large).size(), DialogSize::Large);
    }

    #[test]
    fn values_accessors() {
        let v = vals(&[("a", "  x "), ("n", "12"), ("bad", "1x"), ("f", "true"), ("l", "a, b,,c\nd")]);
        assert_eq!(v.get("a"), "x");
        assert_eq!(v.opt("missing"), None);
        assert_eq!(v.num("n"), Ok(Some(12)));
        assert!(v.num("bad").is_err());
        assert!(v.flag("f") && !v.flag("a"));
        assert_eq!(v.list("l"), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn conditional_visibility_and_switch_sources() {
        let all = vec![
            Field::select("source", "Source", options_raw(&["image", "volume"])),
            Field::check("create", "Create", true).when("source", "image"),
            Field::text("size", "Size").when("create", "true"),
        ];
        let f = &all[2];
        let mut m = HashMap::new();
        m.insert("source".to_string(), "image".to_string());
        assert!(!visible(f, &m, &all));
        m.insert("create".to_string(), "true".to_string());
        assert!(visible(f, &m, &all));
        // hiding the checkbox hides what depends on it, even if it is still "true"
        m.insert("source".to_string(), "volume".to_string());
        assert!(!visible(f, &m, &all));

        let src = Source::Switch("kind", vec![("image", ChoiceSet::Images), ("volume", ChoiceSet::AvailableVolumes)]);
        m.insert("kind".into(), "volume".into());
        assert_eq!(choice_set_for(&src, &m), Some(ChoiceSet::AvailableVolumes));
        m.insert("kind".into(), "other".into());
        assert_eq!(choice_set_for(&src, &m), None);
    }

    #[test]
    fn collects_each_choice_set_once() {
        let fields = vec![
            Field::select("a", "A", Source::Set(ChoiceSet::Flavors)),
            Field::multi("b", "B", Source::Set(ChoiceSet::Flavors)),
            Field::select("c", "C", Source::Switch("x", vec![("1", ChoiceSet::Images), ("2", ChoiceSet::Flavors)])),
        ];
        assert_eq!(all_sets(&fields).len(), 2);
    }

    fn vals_of(pairs: &[(&str, &str)]) -> Vals {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn problems_ignore_hidden_fields_and_switched_off_steps() {
        let fields = vec![
            Field::text("name", "Name").required(),
            Field::check("sub", "Subnet", false),
            Field::text("cidr", "CIDR").required(),
        ];
        let sub_on: fn(&Vals) -> bool = |v| v.get("sub").map(String::as_str) == Some("true");
        let d = Dialog::form("t", "s", fields, |_| Err("unused".into()))
            .wizard(vec![Step::new("A", &["name", "sub"]), Step::new("B", &["cidr"]).enabled_if(sub_on)])
            .check(|v, _| if v.get("name").is_some_and(|n| n == "bad") { vec![("name", "no".into())] } else { vec![] });
        let st = steps_of(&d);
        let mut v = vals_of(&[("name", ""), ("sub", "false"), ("cidr", "")]);
        let p = problems(&d, &st, &v, &Sets::new(), Lang::EnUs);
        assert_eq!(p.get("name").map(|x| x.1), Some(true), "missing values wait for an attempt");
        assert!(!p.contains_key("cidr"), "the subnet step is off");
        v.insert("sub".into(), "true".into());
        v.insert("name".into(), "bad".into());
        let p = problems(&d, &st, &v, &Sets::new(), Lang::EnUs);
        assert!(p.contains_key("cidr"));
        assert_eq!(p.get("name"), Some(&("no".to_string(), false)), "a wrong value shows at once");
    }

    #[test]
    fn rule_errors_on_empty_fields_wait_for_an_attempt() {
        let d = Dialog::form("t", "s", vec![Field::text("nets", "Networks")], |_| Err("unused".into()))
            .check(|_, _| vec![("nets", "pick one".into())]);
        let p = problems(&d, &steps_of(&d), &vals_of(&[("nets", "")]), &Sets::new(), Lang::EnUs);
        assert_eq!(p.get("nets"), Some(&("pick one".to_string(), true)));
    }

    #[test]
    fn destructive_confirms_list_the_selection_and_simple_ones_are_not_red() {
        let d = Dialog::confirm(Lang::EnUs, "Delete Volume", "data", OsAction::VolumeDelete { id: "v".into() });
        assert!(d.danger, "red dialogs ask for the 'I understand' box");
        assert_eq!((d.intro[0].as_str(), d.selection.as_slice()), ("You have selected:", &["data".to_string()][..]));
        let s = Dialog::simple(Lang::EnUs, "Pause Instance", "web", OsAction::ServerPause { id: "s".into() });
        assert!(!s.danger);
        assert_eq!(s.intro.len(), 1, "no 'cannot be undone' line");
    }

    #[test]
    fn single_page_dialogs_are_one_implicit_step() {
        let d = Dialog::form("Edit", "Save", vec![Field::text("a", "A"), Field::text("b", "B")], |_| Err("unused".into()));
        let st = steps_of(&d);
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].fields, vec!["a", "b"]);
    }

    #[test]
    fn show_if_combines_with_when() {
        let only_x: fn(&Vals) -> bool = |v| v.get("k").map(String::as_str) == Some("x");
        let all = vec![Field::text("k", "K"), Field::text("f", "F").show_if(only_x)];
        assert!(visible(&all[1], &vals_of(&[("k", "x")]), &all));
        assert!(!visible(&all[1], &vals_of(&[("k", "y")]), &all));
    }
}

