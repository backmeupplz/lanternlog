//! Lanternlog: one small server for many blogs on many domains, built for search engines and AI answers.

pub mod api;
pub mod db;
pub mod og;
pub mod render;
pub mod style;
pub mod web;

use axum::{Router, http::StatusCode};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::Duration,
};

pub struct App {
    db: Mutex<rusqlite::Connection>,
    pub data: PathBuf,
    pub token: String,
    pub scheme: String,
    indexnow: bool,
    http: reqwest::Client,
    cache: RwLock<HashMap<String, Arc<web::Cached>>>,
}

impl App {
    pub fn open(data: &Path, token: &str, scheme: &str, indexnow: bool) -> anyhow::Result<Arc<App>> {
        std::fs::create_dir_all(data.join("media"))?;
        Ok(Arc::new(App {
            db: Mutex::new(db::open(&data.join("lanternlog.db"))?),
            data: data.to_path_buf(),
            token: token.to_string(),
            scheme: scheme.to_string(),
            indexnow,
            http: reqwest::Client::builder().timeout(Duration::from_secs(15)).build()?,
            cache: RwLock::default(),
        }))
    }

    pub fn conn(&self) -> Result<MutexGuard<'_, rusqlite::Connection>, api::Error> {
        self.db.lock().map_err(|_| api::Error(StatusCode::INTERNAL_SERVER_ERROR, "database lock poisoned".into()))
    }

    /// Drops every rendered page; the next request renders it again.
    pub fn forget(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Tells Bing, Yandex and the other IndexNow engines about new or changed pages.
    pub fn ping(&self, site: &db::Site, urls: Vec<String>) {
        if !self.indexnow {
            return;
        }
        let body = serde_json::json!({"host": site.host, "key": site.indexnow_key,
            "keyLocation": format!("{}://{}/{}.txt", self.scheme, site.host, site.indexnow_key), "urlList": urls});
        let http = self.http.clone();
        tokio::spawn(async move {
            match http.post("https://api.indexnow.org/indexnow").json(&body).send().await {
                Ok(r) => eprintln!("indexnow {}: {}", body["host"], r.status()),
                Err(e) => eprintln!("indexnow {}: {e}", body["host"]),
            }
        });
    }
}

pub fn router(app: Arc<App>) -> Router {
    api::routes().fallback(web::public).with_state(app)
}
