//! Moonlight — leaving Horizon behind.
//!
//! A read-only OpenStack dashboard PoC: Leptos (SSR + hydration) on Axum,
//! talking to OpenStack through gtema's `openstack_sdk`.

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

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}
