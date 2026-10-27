use std::{env, path::PathBuf};

const HELP: &str = "tugpress: one small server for many blogs on many domains

Settings come from the environment:
  PORT              port to listen on (8080)
  TUGPRESS_DATA      folder for the database and uploaded images (./data)
  TUGPRESS_TOKEN     bearer token for the admin API; without it the API is off
  TUGPRESS_SCHEME    scheme for the addresses in pages and feeds (https)
  TUGPRESS_INDEXNOW  set to off to stop telling IndexNow engines about new posts
  TUGPRESS_FONTS     extra folder of fonts for share images

Docs: https://tugpress.com";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::args().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return Ok(());
    }
    let var = |k: &str, default: &str| env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string());
    let data = PathBuf::from(var("TUGPRESS_DATA", "data"));
    let token = var("TUGPRESS_TOKEN", "");
    if token.is_empty() {
        eprintln!("TUGPRESS_TOKEN is not set, so the admin API is off; blogs are still served");
    }
    let app = tugpress::App::open(&data, &token, &var("TUGPRESS_SCHEME", "https"), var("TUGPRESS_INDEXNOW", "on") != "off")?;
    let port: u16 = var("PORT", "8080").parse()?;
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    println!("tugpress {} on :{port}, data in {}", env!("CARGO_PKG_VERSION"), data.display());
    axum::serve(listener, tugpress::router(app)).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

/// Finishes the requests in flight on Ctrl-C or SIGTERM, so a rolling deploy drops nothing.
async fn shutdown() {
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term => {}
    }
}
