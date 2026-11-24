// Rust Öğrenme Uygulaması — giriş noktası.


mod check;
mod content;
mod game;
mod i18n;
mod runner;
mod store;
mod web;

use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(log_level())
        .init();

    let dir = std::env::var("CONTENT_DIR").unwrap_or_else(|_| "content".to_string());
    let catalog = content::load(std::path::Path::new(&dir))?;

    // check::run kendi tokio runtime'ını kurar; bu yüzden runtime'dan önce çağrılır.
    if std::env::args().any(|a| a == "--check-content") {
        std::process::exit(check::run(&catalog));
    }

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let store = store::Store::open(std::path::Path::new(&data_dir).join("progress.json"))?;
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let state = web::AppState {
        catalog: Arc::new(catalog),
        store: Arc::new(store),
    };

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let app = web::router(state);
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
        tracing::info!("dinleniyor: http://0.0.0.0:{port}");
        axum::serve(listener, app).await?;
        Ok(())
    })
}

/// RUST_LOG içinde error/warn/info/debug/trace geçen basit seviye seçimi.
fn log_level() -> tracing::Level {
    let v = std::env::var("RUST_LOG").unwrap_or_default().to_lowercase();
    for (k, l) in [
        ("trace", tracing::Level::TRACE),
        ("debug", tracing::Level::DEBUG),
        ("warn", tracing::Level::WARN),
        ("error", tracing::Level::ERROR),
    ] {
        if v.contains(k) {
            return l;
        }
    }
    tracing::Level::INFO
}
