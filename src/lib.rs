//! Moonlight — leaving Horizon behind.
//!
//! An OpenStack dashboard: Leptos (SSR + hydration) on Axum, talking to OpenStack through
//! gtema's `openstack_sdk`. Reads by default; writes go through `OsAction` and the server's
//! `[writes] mode` (off, dry-run or on). AGENTS.md describes the layers.

#![recursion_limit = "256"]

pub mod api;
pub mod app;
pub mod components;
pub mod i18n;
pub mod model;
pub mod pages;
pub mod prefs;
#[cfg(feature = "ssr")]
pub mod server;
pub mod theme;

/// WASM entry point: the browser calls it once the bundle loads, and it hydrates the
/// server-rendered `<body>` with [`app::App`] (panics go to the console).
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}
