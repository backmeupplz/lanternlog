//! The admin API: sites, posts and images, behind one bearer token.

use crate::{
    App,
    db::{self, Post, Site},
    lang,
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
        .route("/api/sites/{host}/posts/{slug}/translations", get(list_translations))
        .route(
            "/api/sites/{host}/posts/{slug}/translations/{lang}",
            get(get_translation).put(put_translation).delete(delete_translation),
        )
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

fn check_links(links: &mut [db::Link]) -> Result<()> {
    if links.len() > 8 {
        return Err(bad("a site has at most 8 header links"));
    }
    for l in links {
        l.label = l.label.trim().to_string();
        if l.label.is_empty() || l.label.chars().count() > 40 {
            return Err(bad("link labels are 1 to 40 characters"));
        }
        check_url("links", &l.url, true)?;
    }
    Ok(())
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
    if !s.home.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        return Err(bad("home is the slug of one of the site's pages"));
    }
    for (field, value) in [("logo", &s.logo), ("favicon", &s.favicon), ("og_image", &s.og_image)] {
        check_url(field, value, true)?;
    }
    if s.font.is_empty() {
        s.font = "serif".into();
    }
    if !["serif", "sans"].contains(&s.font.as_str()) {
        return Err(bad("font is serif or sans"));
    }
    check_links(&mut s.links)?;
    let mut locales = std::collections::BTreeMap::new();
    for (code, mut l) in std::mem::take(&mut s.locales) {
        let code = lang::code(&code).ok_or_else(|| bad(format!("locales: {code:?} is not a language code such as de or pt-br")))?;
        if locales.contains_key(&code) {
            return Err(bad(format!("locales name {code} twice")));
        }
        check_links(&mut l.links)?;
        check_url("project_url", &l.project_url, false)?;
        if l.footer.chars().count() > 500 || [&l.name, &l.title, &l.project_name].iter().any(|v| v.chars().count() > 200) {
            return Err(bad("in locales, footer is at most 500 characters and name, title and project_name at most 200"));
        }
        for (key, value) in &l.strings {
            if !lang::STRINGS.iter().any(|(k, _)| k == key) {
                let keys: Vec<&str> = lang::STRINGS.iter().map(|(k, _)| *k).collect();
                return Err(bad(format!("locales.{code}.strings: no string {key:?}; they are {}", keys.join(", "))));
            }
            if value.chars().count() > 200 {
                return Err(bad("locale strings are at most 200 characters"));
            }
        }
        locales.insert(code, l);
    }
    s.locales = locales;
    if s.head.len() > 20_000 || s.footer.chars().count() > 500 {
        return Err(bad("head is at most 20,000 bytes and footer at most 500 characters"));
    }
    if !s.indexnow_key.is_empty() && (!(8..=128).contains(&s.indexnow_key.len()) || !s.indexnow_key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) {
        return Err(bad("indexnow_key is 8 to 128 letters, digits or dashes"));
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
    if let Some(code) = lang::code(&s.lang)
        && db::has_lang(&c, &s.host, &code)?
    {
        return Err(bad(format!("the blog has {} translations, so its own language can't be {}; delete them first", s.lang, s.lang)));
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

/// Checks a post's fields and tidies its tags; shared by posts and their translations.
fn check_post(p: &mut Post) -> Result<()> {
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
    check_url("og_image", &p.og_image, true)?;
    let mut tags: Vec<String> = Vec::new();
    for t in p.tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty() && t.len() <= 50) {
        if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) && tags.len() < 20 {
            tags.push(t.to_string());
        }
    }
    p.tags = tags;
    p.updated_at = now();
    Ok(())
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
    // A post with a language other than the blog's is a translation of the post with its slug.
    let main = site_of(&*app.conn()?, &host)?.lang;
    if !p.lang.trim().is_empty() && !lang::same(p.lang.trim(), &main) {
        let of = match p.translation_of.trim() {
            "" => slug,
            of if of == slug => slug,
            of => {
                return Err(bad(format!(
                    "a translation shares its original's slug: PUT it to /api/sites/{host}/posts/{of} or /api/sites/{host}/posts/{of}/translations/{}",
                    p.lang.trim()
                )));
            }
        };
        let code = p.lang.clone();
        return translate(&app, &host, of, &code, p);
    }
    if !p.translation_of.trim().is_empty() {
        return Err(bad("translation_of needs a lang other than the blog's"));
    }
    check_post(&mut p)?;
    p.lang = String::new();
    p.slug = slug;
    let (site, translated) = {
        let c = app.conn()?;
        let site = site_of(&c, &host)?;
        if db::has_lang(&c, &host, &p.slug)? {
            return Err(bad(format!("/{}/ is the blog's {} edition, so no post can have that slug", p.slug, lang::name(&p.slug))));
        }
        let old = db::post(&c, &host, &p.slug)?;
        if p.status == "published" && p.published_at.is_none() {
            p.published_at = old.as_ref().and_then(|o| o.published_at).or(Some(p.updated_at));
        }
        db::put_post(&c, &host, &p)?;
        // Translations published while the original was a draft go live with it, so they're news then (and only then).
        let going_live = p.status == "published" && !old.is_some_and(|o| o.status == "published");
        let translated: Vec<String> = match going_live {
            true => db::translations_of(&c, &host, &p.slug)?.into_iter().filter(|t| t.status == "published").map(|t| t.lang).collect(),
            false => Vec::new(),
        };
        (site, translated)
    };
    app.forget();
    let url = format!("{}://{host}/{}/", app.scheme, p.slug);
    if p.status == "published" {
        let mut urls = vec![url.clone(), format!("{}://{host}/", app.scheme)];
        for code in translated {
            urls.push(format!("{}://{host}/{code}/{}/", app.scheme, p.slug));
            urls.push(format!("{}://{host}/{code}/", app.scheme));
        }
        app.ping(&site, urls);
    }
    Ok(Json(json!({"url": url, "post": p})))
}

/// Writes the translation of post `slug` into `code`; it lives at /<code>/<slug>/.
fn translate(app: &Arc<App>, host: &str, slug: String, code: &str, mut p: Post) -> Result<Json<Value>> {
    let code = lang::code(code).ok_or_else(|| bad(format!("{code:?} is not a language code such as de, pt-br or zh-hans")))?;
    check_post(&mut p)?;
    let (site, original) = {
        let c = app.conn()?;
        let site = site_of(&c, host)?;
        if lang::same(&code, &site.lang) {
            return Err(bad(format!("{code} is the blog's own language: PUT the post itself")));
        }
        let original = db::post(&c, host, &slug)?.ok_or_else(|| Error(StatusCode::NOT_FOUND, format!("no post {slug} to translate")))?;
        if db::post(&c, host, &code)?.is_some() {
            return Err(bad(format!("the post /{code}/ already uses that address, so the blog can't have a {code} edition")));
        }
        p.slug = slug;
        p.lang = code.clone();
        p.kind = original.kind.clone();
        if p.status == "published" && p.published_at.is_none() {
            // A translation is the same article, so it keeps the original's date unless it already has its own.
            p.published_at = db::translation(&c, host, &p.slug, &code)?
                .and_then(|o| o.published_at)
                .or(original.published_at)
                .or(Some(p.updated_at));
        }
        db::put_translation(&c, host, &p)?;
        (site, original)
    };
    app.forget();
    let url = format!("{}://{host}/{code}/{}/", app.scheme, p.slug);
    if p.status == "published" && original.status == "published" {
        app.ping(&site, vec![url.clone(), format!("{}://{host}/{code}/", app.scheme)]);
    }
    Ok(Json(json!({"url": url, "post": p})))
}

async fn list_translations(_: Admin, State(app): State<Arc<App>>, Path((host, slug)): Path<(String, String)>) -> Result<Json<Vec<Post>>> {
    let c = app.conn()?;
    db::post(&c, &host, &slug)?.ok_or_else(|| Error(StatusCode::NOT_FOUND, format!("no post {slug}")))?;
    Ok(Json(db::translations_of(&c, &host, &slug)?))
}

async fn get_translation(
    _: Admin,
    State(app): State<Arc<App>>,
    Path((host, slug, code)): Path<(String, String, String)>,
) -> Result<Json<Post>> {
    let code = lang::code(&code).unwrap_or(code);
    db::translation(&*app.conn()?, &host, &slug, &code)?.map(Json).ok_or_else(|| Error(StatusCode::NOT_FOUND, format!("no {code} translation of {slug}")))
}

async fn put_translation(
    _: Admin,
    State(app): State<Arc<App>>,
    Path((host, slug, code)): Path<(String, String, String)>,
    Json(p): Json<Post>,
) -> Result<Json<Value>> {
    translate(&app, &host, slug, &code, p)
}

async fn delete_translation(
    _: Admin,
    State(app): State<Arc<App>>,
    Path((host, slug, code)): Path<(String, String, String)>,
) -> Result<Json<Value>> {
    let code = lang::code(&code).unwrap_or(code);
    let deleted = db::delete_translation(&*app.conn()?, &host, &slug, &code)?;
    app.forget();
    Ok(Json(json!({"deleted": deleted})))
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
