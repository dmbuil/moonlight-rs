//! Light / dark theme. `System` follows `prefers-color-scheme`; an explicit
//! choice is remembered in a cookie (like the language) and rendered as
//! `data-theme` on `<html>` during SSR, so there is no flash on load.

use leptos::prelude::*;

use crate::i18n::{cookie_value, K};

/// Name of the cookie that remembers the chosen theme.
pub const THEME_COOKIE: &str = "ml_theme";

/// Colour theme: follow the system, or force light or dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl Theme {
    /// Every theme, in the menu's order.
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    /// The `ml_theme` cookie's value.
    pub fn cookie_value(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    /// A theme from its cookie or `[ui] default_theme` value.
    pub fn parse(s: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.cookie_value() == s.trim())
    }

    /// Value of `<html data-theme>`; absent for `System`, so the media query decides.
    pub fn attr(self) -> Option<&'static str> {
        match self {
            Theme::System => None,
            t => Some(t.cookie_value()),
        }
    }

    /// The menu label.
    pub fn label(self) -> K {
        match self {
            Theme::Light => K::ThemeLight,
            Theme::Dark => K::ThemeDark,
            Theme::System => K::ThemeSystem,
        }
    }
}

/// Reactive handle to the theme, provided as context by `App`.
#[derive(Clone, Copy)]
pub struct ThemeCtx(pub RwSignal<Theme>);

impl ThemeCtx {
    /// The current theme (reactive inside a closure).
    pub fn get(&self) -> Theme {
        self.0.get()
    }

    /// Applies the theme instantly and remembers it in a cookie.
    pub fn set(&self, theme: Theme) {
        self.0.set(theme);
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            if let Some(doc) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok())
            {
                let _ = doc.set_cookie(&format!(
                    "{THEME_COOKIE}={}; Path=/; Max-Age=31536000; SameSite=Lax",
                    theme.cookie_value()
                ));
                if let Some(html) = doc.document_element() {
                    let _ = match theme.attr() {
                        Some(v) => html.set_attribute("data-theme", v),
                        None => html.remove_attribute("data-theme"),
                    };
                }
            }
        }
    }
}

/// The theme handle provided by the app.
pub fn use_theme() -> ThemeCtx {
    expect_context::<ThemeCtx>()
}

/// Theme for the first render: the cookie, else `[ui] default_theme`. The
/// client reads the same cookie, else what the server put on `<html>`, so
/// hydration matches.
pub fn initial_theme() -> Theme {
    #[cfg(feature = "ssr")]
    {
        use_context::<http::request::Parts>()
            .and_then(|parts| {
                parts
                    .headers
                    .get_all(http::header::COOKIE)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .find_map(|c| cookie_value(c, THEME_COOKIE).and_then(|v| Theme::parse(&v)))
            })
            .unwrap_or(crate::server::config::state().config.default_theme)
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        use wasm_bindgen::JsCast;
        let doc = web_sys::window().and_then(|w| w.document()).and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok());
        let from_cookie = doc.as_ref().and_then(|d| d.cookie().ok()).and_then(|c| cookie_value(&c, THEME_COOKIE)).and_then(|v| Theme::parse(&v));
        // No cookie: the server rendered the configured default as `data-theme`.
        let rendered = || {
            doc.as_ref()
                .and_then(|d| d.document_element())
                .and_then(|e| e.get_attribute("data-theme"))
                .and_then(|v| Theme::parse(&v))
        };
        from_cookie.or_else(rendered).unwrap_or_default()
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        Theme::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_values_roundtrip_and_system_has_no_attribute() {
        for t in Theme::ALL {
            assert_eq!(Theme::parse(t.cookie_value()), Some(t));
        }
        assert_eq!(Theme::parse("sepia"), None);
        assert_eq!(Theme::System.attr(), None);
        assert_eq!(Theme::Dark.attr(), Some("dark"));
        assert_eq!(cookie_value("ml_lang=es; ml_theme=dark", THEME_COOKIE).as_deref(), Some("dark"));
    }
}
