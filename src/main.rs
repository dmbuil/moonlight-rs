// The binary is its own crate, so `lib.rs`'s limit doesn't apply here. Without
// it the release build fails with "queries overflow the depth limit!" while
// computing the layout of the `DataTable<ServerUsage>` view future
// (`pages/overview.rs`); the debug build doesn't reach that depth.
#![recursion_limit = "256"]

/// Stack of each tokio worker thread. Tokio's 2 MiB is not enough for a
/// debug build: the first request after a start builds the scoped connection
/// (rescope, discovery, capability detection) and renders the page in one poll,
/// and unoptimised futures that size overflowed it (`tokio-rt-worker has
/// overflowed its stack`, roadmap 62). Release builds use far less.
#[cfg(feature = "ssr")]
const WORKER_STACK: usize = 16 * 1024 * 1024;

#[cfg(feature = "ssr")]
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(WORKER_STACK)
        .build()
        .expect("tokio runtime")
        .block_on(serve());
}

#[cfg(feature = "ssr")]
async fn serve() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use moonlight::app::{App, shell};
    use tracing_subscriber::util::SubscriberInitExt;

    use clap::Parser;
    use moonlight::server::settings;

    let cli = settings::Cli::parse();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let loaded = settings::sources(&cli, &settings::default_candidates(home.as_deref()))
        .and_then(|files| Ok((settings::load(&files, &|k| std::env::var(k).ok())?, files)));
    let (settings, files) = loaded.unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });
    // Held until `serve` returns: dropping it flushes the log file's last lines (roadmap 81).
    let (subscriber, _log_guard) = moonlight::server::logging::subscriber(&settings.log).unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });
    subscriber.init();
    match files.as_slice() {
        [] => tracing::info!("no moonlight.conf found: using built-in defaults and MOONLIGHT_* variables"),
        files => tracing::info!(
            "configuration files, in order: {}",
            files
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
    if let Some(n) = settings.log.legacy_keep() {
        tracing::warn!(
            "[log] keep = {n} is the old file count: read as max_files = {n}; keep is now yes/no (roadmap 84)"
        );
    }

    if let Err(e) = moonlight::server::config::init(&settings) {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    }

    let conf = get_configuration(None).expect("leptos configuration");
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let app = Router::new()
        // WebSSO (roadmap 80): plain redirects outside Leptos, so they answer 303.
        .route(
            moonlight::server::websso::START,
            axum::routing::get(moonlight::server::websso::start),
        )
        .route(
            moonlight::server::websso::CALLBACK,
            axum::routing::post(moonlight::server::websso::callback),
        )
        // Image file uploads (roadmap 45): a streamed body, so not a server function.
        .route(
            moonlight::model::IMAGE_UPLOAD_ROUTE,
            axum::routing::put(moonlight::server::upload::proxy),
        )
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(axum::middleware::from_fn(moonlight::server::same_origin_only))
        .layer(axum::middleware::map_response(moonlight::server::security_headers))
        .layer(moonlight::server::compression())
        .with_state(leptos_options);

    // HTTPS when `[server] tls_cert_file` and `tls_key_file` are set (roadmap 107).
    let tls = settings
        .server
        .tls_files()
        .and_then(|files| {
            files
                .map(|(cert, key)| moonlight::server::listen::Tls::load(&cert, &key))
                .transpose()
        })
        .unwrap_or_else(|e| {
            eprintln!("configuration error: {e}");
            std::process::exit(1);
        });

    let cfg = &moonlight::server::config::state().config;
    let scheme = if tls.is_some() { "https" } else { "http" };
    tracing::info!("Moonlight on {scheme}://{addr}  (keystone: {})", cfg.keystone_url);
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    match tls {
        Some(tls) => moonlight::server::listen::serve(listener, app, std::sync::Arc::new(tls))
            .await
            .unwrap_or_else(|e| {
                eprintln!("configuration error: {e}");
                std::process::exit(1);
            }),
        None => axum::serve(listener, app.into_make_service()).await.expect("server"),
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // client-side entry point is `moonlight::hydrate`
}
