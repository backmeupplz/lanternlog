//! The admin API: sites, posts and images, behind one bearer token.

use crate::{
    App,
    db::{self, Post, Site},
    web::hash,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, FromRequestParts, Path, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub struct Error(pub StatusCode, pub String);

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

fn bad(msg: impl Into<String>) -> Error {
    Error(StatusCode::BAD_REQUEST, msg.into())
}

/// A request carrying the right `Authorization: Bearer` token.
pub struct Admin;

impl FromRequestParts<Arc<App>> for Admin {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self> {
        if app.token.is_empty() {
            return Err(Error(StatusCode::SERVICE_UNAVAILABLE, "the API is off until LANTERNLOG_TOKEN is set".into()));
        }
        let given = parts.headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
        match given {
            Some(t) if bool::from(t.as_bytes().ct_eq(app.token.as_bytes())) => Ok(Admin),
            _ => Err(Error(StatusCode::UNAUTHORIZED, "missing or wrong token".into())),
        }
    }
}

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({"ok": true, "version": env!("CARGO_PKG_VERSION")})) }))
        .route("/api/sites", get(list_sites))
        .route("/api/sites/{host}", get(get_site).put(put_site).delete(delete_site))
        .route("/api/sites/{host}/posts", get(list_posts))
        .route("/api/sites/{host}/posts/{slug}", get(get_post).put(put_post).delete(delete_post))
        .route("/api/sites/{host}/media", post(upload).layer(DefaultBodyLimit::max(16 << 20)))
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn check_host(h: &str) -> Result<()> {
    let label = |l: &str| {
        !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    if h.len() <= 253 && h.split('.').all(label) { Ok(()) } else { Err(bad(format!("{h:?} is not a domain name"))) }
}

fn check_url(field: &str, u: &str, relative: bool) -> Result<()> {
    let scheme = u.starts_with("https://") || u.starts_with("http://") || (relative && u.starts_with('/'));
    if u.is_empty() || (scheme && !u.contains(|c: char| c.is_whitespace() || "\"'<>".contains(c))) {
        Ok(())
    } else {
        Err(bad(format!("{field} must be an http(s) address")))
    }
}

fn site_of(c: &rusqlite::Connection, host: &str) -> Result<Site> {
    db::site(c, host)?.filter(|s| s.host == host).ok_or_else(|| Error(StatusCode::NOT_FOUND, format!("no site {host}")))
}

async fn list_sites(_: Admin, State(app): State<Arc<App>>) -> Result<Json<Vec<Site>>> {
    Ok(Json(db::sites(&*app.conn()?)?))
}

async fn get_site(_: Admin, State(app): State<Arc<App>>, Path(host): Path<String>) -> Result<Json<Site>> {
    Ok(Json(site_of(&*app.conn()?, &host)?))
}

async fn put_site(_: Admin, State(app): State<Arc<App>>, Path(host): Path<String>, Json(mut s): Json<Site>) -> Result<Json<Site>> {
    s.host = host.to_ascii_lowercase();
    check_host(&s.host)?;
    for a in &mut s.aliases {
        *a = a.to_ascii_lowercase();
        check_host(a)?;
    }
    s.aliases.retain(|a| *a != s.host);
    if s.title.trim().is_empty() {
        return Err(bad("title is required"));
    }
    if s.lang.is_empty() {
        s.lang = "en".into();
    }
    if !(2..=15).contains(&s.lang.len()) || !s.lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(bad("lang must be a language tag such as en or pt-BR"));
    }
    if s.accent.is_empty() {
        s.accent = "#2563eb".into();
    }
    if !(s.accent.len() == 4 || s.accent.len() == 7) || !s.accent.starts_with('#') || !s.accent[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad("accent must be a color such as #2563eb"));
    }
    check_url("project_url", &s.project_url, false)?;
    check_url("author_url", &s.author_url, false)?;
    s.updated_at = now();
    let c = app.conn()?;
    for name in std::iter::once(&s.host).chain(&s.aliases) {
        if let Some(other) = db::site(&c, name)?.filter(|o| o.host != s.host) {
            return Err(bad(format!("{name} already belongs to {}", other.host)));
        }
    }
    db::put_site(&c, &s)?;
    app.forget();
    Ok(Json(site_of(&c, &s.host)?))
}

async fn delete_site(_: Admin, State(app): State<Arc<App>>, Path(host): Path<String>) -> Result<Json<Value>> {
    let deleted = db::delete_site(&*app.conn()?, &host)?;
    app.forget();
    Ok(Json(json!({"deleted": deleted})))
}

async fn list_posts(_: Admin, State(app): State<Arc<App>>, Path(host): Path<String>) -> Result<Json<Vec<Post>>> {
    let c = app.conn()?;
    site_of(&c, &host)?;
    Ok(Json(db::posts(&c, &host, true)?))
}

async fn get_post(_: Admin, State(app): State<Arc<App>>, Path((host, slug)): Path<(String, String)>) -> Result<Json<Post>> {
    db::post(&*app.conn()?, &host, &slug)?.map(Json).ok_or_else(|| Error(StatusCode::NOT_FOUND, format!("no post {slug}")))
}

async fn put_post(
    _: Admin,
    State(app): State<Arc<App>>,
    Path((host, slug)): Path<(String, String)>,
    Json(mut p): Json<Post>,
) -> Result<Json<Value>> {
    let slug_ok = slug.len() <= 100
        && !slug.starts_with('-')
        && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !["", "api", "og", "media", "tags"].contains(&slug.as_str());
    if !slug_ok {
        return Err(bad("slugs are lowercase letters, digits and hyphens (and not api, og, media or tags)"));
    }
    if p.title.trim().is_empty() || p.body.trim().is_empty() {
        return Err(bad("title and body are required"));
    }
    if p.kind.is_empty() {
        p.kind = "post".into();
    }
    if p.status.is_empty() {
        p.status = "draft".into();
    }
    if !["post", "page"].contains(&p.kind.as_str()) || !["draft", "published"].contains(&p.status.as_str()) {
        return Err(bad("kind is post or page, and status is draft or published"));
    }
    check_url("cover", &p.cover, true)?;
    let mut tags: Vec<String> = Vec::new();
    for t in p.tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty() && t.len() <= 50) {
        if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) && tags.len() < 20 {
            tags.push(t.to_string());
        }
    }
    p.tags = tags;
    p.slug = slug;
    p.updated_at = now();
    let site = {
        let c = app.conn()?;
        let site = site_of(&c, &host)?;
        if p.status == "published" && p.published_at.is_none() {
            p.published_at = db::post(&c, &host, &p.slug)?.and_then(|o| o.published_at).or(Some(p.updated_at));
        }
        db::put_post(&c, &host, &p)?;
        site
    };
    app.forget();
    let url = format!("{}://{host}/{}/", app.scheme, p.slug);
    if p.status == "published" {
        app.ping(&site, vec![url.clone(), format!("{}://{host}/", app.scheme)]);
    }
    Ok(Json(json!({"url": url, "post": p})))
}

async fn delete_post(_: Admin, State(app): State<Arc<App>>, Path((host, slug)): Path<(String, String)>) -> Result<Json<Value>> {
    let deleted = db::delete_post(&*app.conn()?, &host, &slug)?;
    app.forget();
    Ok(Json(json!({"deleted": deleted})))
}

async fn upload(_: Admin, State(app): State<Arc<App>>, Path(host): Path<String>, headers: HeaderMap, body: Bytes) -> Result<Json<Value>> {
    let ext = match headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        Some("image/webp") => "webp",
        Some("image/gif") => "gif",
        Some("image/avif") => "avif",
        _ => return Err(bad("send a png, jpeg, webp, gif or avif image with its Content-Type")),
    };
    if body.is_empty() {
        return Err(bad("the image is empty"));
    }
    site_of(&*app.conn()?, &host)?;
    let dir = app.data.join("media").join(&host);
    let name = format!("{:016x}-{}.{ext}", hash(&body), body.len());
    std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&name), &body)).map_err(|e| Error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"url": format!("{}://{host}/media/{name}", app.scheme)})))
}
