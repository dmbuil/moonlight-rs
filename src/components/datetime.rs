//! Themed date and time pickers for dialog fields (`Kind::Date`, `Kind::Time`),
//! used by *Create Application Credential* (roadmap 57). Each is a text input
//! (typing still works: `YYYY-MM-DD`, `HH:MM`) with a button that opens a popup
//! placed like `Combo`'s (`position: fixed`, flipping up near the bottom), not
//! the browser's own picker. Esc or a click outside closes it.

use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use leptos::prelude::*;

use crate::i18n::{K, month_name, use_i18n, weekday_initials};

/// The days to draw for `year`/`month`: whole Monday-first weeks, `None` for
/// the blanks before the 1st and after the last day.
pub fn month_grid(year: i32, month: u32) -> Vec<Option<NaiveDate>> {
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return vec![];
    };
    let lead = first.weekday().num_days_from_monday() as usize;
    let (ny, nm) = shift_month(year, month, 1);
    let days = NaiveDate::from_ymd_opt(ny, nm, 1)
        .map(|n| (n - first).num_days())
        .unwrap_or(30) as u32;
    let mut out: Vec<Option<NaiveDate>> = vec![None; lead];
    out.extend((1..=days).map(|d| NaiveDate::from_ymd_opt(year, month, d)));
    while !out.len().is_multiple_of(7) {
        out.push(None);
    }
    out
}

/// `(year, month)` moved by `by` months.
pub fn shift_month(year: i32, month: u32, by: i32) -> (i32, u32) {
    let n = year * 12 + month as i32 - 1 + by;
    (n.div_euclid(12), n.rem_euclid(12) as u32 + 1)
}

/// `HH:MM` with one part replaced; the other part kept from `current` (or 00).
pub fn set_time_part(current: &str, hour: Option<u32>, minute: Option<u32>) -> String {
    let now = NaiveTime::parse_from_str(current.trim(), "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(current.trim(), "%H:%M:%S"))
        .ok();
    let h = hour.or(now.map(|t| t.hour())).unwrap_or(0);
    let m = minute.or(now.map(|t| t.minute())).unwrap_or(0);
    format!("{h:02}:{m:02}")
}

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// `left/top` for a popup under (or above) `anchor`, `height` its usual size.
fn place(anchor: NodeRef<leptos::html::Div>, height: f64) -> String {
    #[cfg(feature = "hydrate")]
    {
        let Some(a) = anchor.get_untracked() else {
            return String::new();
        };
        let r = a.get_bounding_client_rect();
        let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let below = vh - r.bottom();
        if below < height && r.top() > below {
            format!("left:{}px;bottom:{}px", r.left(), vh - r.top() + 4.0)
        } else {
            format!("left:{}px;top:{}px", r.left(), r.bottom() + 4.0)
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (anchor, height);
        String::new()
    }
}

fn calendar_icon() -> impl IntoView {
    view! {
        <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="5" width="18" height="16" rx="2"></rect>
            <path d="M3 10h18M8 3v4M16 3v4"></path>
        </svg>
    }
}

fn clock_icon() -> impl IntoView {
    view! {
        <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="9"></circle>
            <path d="M12 7v5l3 2"></path>
        </svg>
    }
}

/// A `YYYY-MM-DD` field with a month calendar. Days before today are
/// disabled when `future_only` (an expiry can't be in the past).
#[component]
pub fn DatePicker(
    id: String,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    #[prop(optional)] future_only: bool,
    #[prop(optional, into)] described_by: Option<Signal<Option<String>>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    let pos = RwSignal::new(String::new());
    let anchor: NodeRef<leptos::html::Div> = NodeRef::new();
    let shown = RwSignal::new((2026, 1u32));
    let picked = move || NaiveDate::parse_from_str(value.get().trim(), "%Y-%m-%d").ok();
    let toggle = move |_| {
        if open.get_untracked() {
            open.set(false);
            return;
        }
        let base = NaiveDate::parse_from_str(value.get_untracked().trim(), "%Y-%m-%d").unwrap_or_else(|_| today());
        shown.set((base.year(), base.month()));
        pos.set(place(anchor, 340.0));
        open.set(true);
    };
    let choose = move |d: NaiveDate| {
        on_change.run(d.format("%Y-%m-%d").to_string());
        open.set(false);
    };
    let step = move |by: i32| shown.update(|(y, m)| (*y, *m) = shift_month(*y, *m, by));
    let close_on_esc = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            // Close the picker only, not the dialog around it.
            ev.prevent_default();
            ev.stop_propagation();
            open.set(false);
        }
    };
    view! {
        <div class="picker" node_ref=anchor on:keydown=close_on_esc>
            <input id=id type="text" class="mono" placeholder="YYYY-MM-DD" inputmode="numeric" prop:value=move || value.get()
                aria-describedby=move || described_by.and_then(|d| d.get())
                on:input=move |ev| on_change.run(event_target_value(&ev))/>
            <button type="button" class="btn btn-icon picker-btn" aria-haspopup="dialog" aria-expanded=move || open.get().to_string()
                title=move || i18n.t(K::PickDate) aria-label=move || i18n.t(K::PickDate) on:click=toggle>
                {calendar_icon()}
            </button>
            <Show when=move || open.get()>
                <div class="combo-veil" on:mousedown=move |_| open.set(false)></div>
                <div class="picker-pop" role="dialog" aria-label=move || i18n.t(K::PickDate) style=move || pos.get()>
                    <div class="cal-head">
                        <button type="button" class="btn btn-icon" aria-label=move || i18n.t(K::PreviousMonth) on:click=move |_| step(-1)>"‹"</button>
                        <strong aria-live="polite">{move || { let (y, m) = shown.get(); format!("{} {y}", month_name(i18n.lang(), m)) }}</strong>
                        <button type="button" class="btn btn-icon" aria-label=move || i18n.t(K::NextMonth) on:click=move |_| step(1)>"›"</button>
                    </div>
                    <div class="cal-grid" role="grid">
                        {move || weekday_initials(i18n.lang()).iter().map(|d| view! { <span class="cal-dow" aria-hidden="true">{*d}</span> }).collect_view()}
                        {move || {
                            let (y, m) = shown.get();
                            let (t, sel) = (today(), picked());
                            month_grid(y, m).into_iter().map(|day| match day {
                                None => view! { <span></span> }.into_any(),
                                Some(d) => {
                                    let past = future_only && d < t;
                                    view! {
                                        <button type="button" class="cal-day" class:today=d == t class:selected=Some(d) == sel
                                            disabled=past aria-pressed=(Some(d) == sel).to_string()
                                            aria-label=d.format("%Y-%m-%d").to_string() on:click=move |_| choose(d)>
                                            {d.day()}
                                        </button>
                                    }.into_any()
                                }
                            }).collect_view()
                        }}
                    </div>
                    <div class="cal-foot">
                        <button type="button" class="btn btn-sm" on:click=move |_| choose(today())>{move || i18n.t(K::Today)}</button>
                        <button type="button" class="btn btn-sm" on:click=move |_| { on_change.run(String::new()); open.set(false); }>{move || i18n.t(K::Clear)}</button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

/// An `HH:MM` field with hour and minute columns (minutes in steps of 5;
/// any minute can still be typed), and a *Now* button that fills the current
/// UTC time, so nobody has to convert from their own zone.
#[component]
pub fn TimePicker(
    id: String,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    #[prop(optional, into)] described_by: Option<Signal<Option<String>>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    let pos = RwSignal::new(String::new());
    let anchor: NodeRef<leptos::html::Div> = NodeRef::new();
    let parts = move || {
        NaiveTime::parse_from_str(value.get().trim(), "%H:%M")
            .ok()
            .map(|t| (t.hour(), t.minute()))
    };
    let toggle = move |_| {
        if !open.get_untracked() {
            pos.set(place(anchor, 260.0));
        }
        open.update(|o| *o = !*o);
    };
    let close_on_esc = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            ev.stop_propagation();
            open.set(false);
        }
    };
    view! {
        <div class="picker" node_ref=anchor on:keydown=close_on_esc>
            <input id=id type="text" class="mono" placeholder="HH:MM" inputmode="numeric" prop:value=move || value.get()
                aria-describedby=move || described_by.and_then(|d| d.get())
                on:input=move |ev| on_change.run(event_target_value(&ev))/>
            <button type="button" class="btn picker-now" title=move || i18n.t(K::NowUtcHelp)
                on:click=move |_| on_change.run(chrono::Utc::now().format("%H:%M").to_string())>
                {move || i18n.t(K::Now)}
            </button>
            <button type="button" class="btn btn-icon picker-btn" aria-haspopup="dialog" aria-expanded=move || open.get().to_string()
                title=move || i18n.t(K::PickTime) aria-label=move || i18n.t(K::PickTime) on:click=toggle>
                {clock_icon()}
            </button>
            <Show when=move || open.get()>
                <div class="combo-veil" on:mousedown=move |_| open.set(false)></div>
                <div class="picker-pop time-pop" role="dialog" aria-label=move || i18n.t(K::PickTime) style=move || pos.get()>
                    <div class="time-col" role="listbox" aria-label=move || i18n.t(K::Hour)>
                        {(0..24u32).map(|h| view! {
                            <button type="button" role="option" class="time-opt" aria-selected=move || (parts().map(|p| p.0) == Some(h)).to_string()
                                class:selected=move || parts().map(|p| p.0) == Some(h)
                                on:click=move |_| on_change.run(set_time_part(&value.get_untracked(), Some(h), None))>
                                {format!("{h:02}")}
                            </button>
                        }).collect_view()}
                    </div>
                    <div class="time-col" role="listbox" aria-label=move || i18n.t(K::Minute)>
                        {(0..60u32).step_by(5).map(|m| view! {
                            <button type="button" role="option" class="time-opt" aria-selected=move || (parts().map(|p| p.1) == Some(m)).to_string()
                                class:selected=move || parts().map(|p| p.1) == Some(m)
                                on:click=move |_| { on_change.run(set_time_part(&value.get_untracked(), None, Some(m))); open.set(false); }>
                                {format!("{m:02}")}
                            </button>
                        }).collect_view()}
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_grid_starts_on_monday_and_fills_whole_weeks() {
        // October 2026 starts on a Thursday: three blanks, 31 days, 1 blank.
        let g = month_grid(2026, 10);
        assert_eq!(g.len() % 7, 0);
        assert_eq!(g.iter().take_while(|d| d.is_none()).count(), 3);
        assert_eq!(g.iter().flatten().count(), 31);
        assert_eq!(month_grid(2028, 2).iter().flatten().count(), 29, "leap year");
        assert_eq!(month_grid(2026, 13), vec![], "no such month");
    }

    #[test]
    fn months_step_across_years() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 10, 0), (2026, 10));
    }

    #[test]
    fn picking_an_hour_or_minute_keeps_the_other() {
        assert_eq!(set_time_part("", Some(9), None), "09:00");
        assert_eq!(set_time_part("09:00", None, Some(45)), "09:45");
        assert_eq!(set_time_part("18:30:15", Some(7), None), "07:30");
        assert_eq!(set_time_part("garbage", None, Some(5)), "00:05");
    }
}
