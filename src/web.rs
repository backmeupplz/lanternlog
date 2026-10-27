//! The public side: every request is answered by the site its Host header names, and every page is
//! rendered once, compressed once and served from memory until the next write.

use crate::{App, db, og, render};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Response, StatusCode, Uri, header},
};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
    sync::Arc,
};

pub fn hash(data: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

pub struct Cached {
    ctype: &'static str,
    body: Vec<u8>,
    gzip: Option<Vec<u8>>,
    etag: String,
    max_age: u32,
}

impl Cached {
    fn new(ctype: &'static str, body: impl Into<Vec<u8>>, max_age: u32) -> Self {
        let body = body.into();
        let gzip = (!ctype.starts_with("image/png") && body.len() > 1024)
            .then(|| {
                let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
                gz.write_all(&body).ok()?;
                gz.finish().ok()
            })
            .flatten();
        Cached { ctype, etag: format!("\"{:016x}\"", hash(&body)), body, gzip, max_age }
    }

    fn respond(&self, req: &HeaderMap) -> Response<Body> {
        let mut res = Response::builder()
            .header(header::CONTENT_TYPE, self.ctype)
            .header(header::ETAG, &self.etag)
            .header(header::CACHE_CONTROL, format!("public, max-age={}", self.max_age))
            .header(header::VARY, "Accept-Encoding");
        if req.get(header::IF_NONE_MATCH).is_some_and(|v| v.as_bytes() == self.etag.as_bytes()) {
            return res.status(StatusCode::NOT_MODIFIED).body(Body::empty()).unwrap_or_default();
        }
        let gzip_ok = req.get(header::ACCEPT_ENCODING).and_then(|v| v.to_str().ok()).is_some_and(|v| v.contains("gzip"));
        let body = match (&self.gzip, gzip_ok) {
            (Some(gz), true) => {
                res = res.header(header::CONTENT_ENCODING, "gzip");
                gz.clone()
            }
            _ => self.body.clone(),
        };
        res.body(Body::from(body)).unwrap_or_default()
    }
}

fn plain(status: StatusCode, text: &str) -> Response<Body> {
    let mut res = Response::new(Body::from(text.to_string()));
    *res.status_mut() = status;
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    res
}

fn redirect(to: &str) -> Response<Body> {
    let mut res = plain(StatusCode::MOVED_PERMANENTLY, "");
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn host(h: &HeaderMap) -> String {
    let raw = h.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let raw = raw.rsplit_once(':').filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit())).map_or(raw, |(h, _)| h);
    raw.trim_end_matches('.').to_ascii_lowercase()
}

/// Percent-decodes a path; anything malformed stays as it was.
fn decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(bytes.len()), 0);
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| path.to_string())
}

pub async fn public(State(app): State<Arc<App>>, headers: HeaderMap, uri: Uri) -> Response<Body> {
    let host = host(&headers);
    let path = decode(uri.path());
    let key = format!("{host}{path}");
    if let Some(hit) = app.cache.read().ok().and_then(|c| c.get(&key).cloned()) {
        return hit.respond(&headers);
    }
    match build(&app, &host, &path) {
        Ok(page) => {
            let page = Arc::new(page);
            if let Ok(mut cache) = app.cache.write() {
                cache.insert(key, page.clone());
            }
            page.respond(&headers)
        }
        Err(res) => *res,
    }
}

/// A page worth caching, or a response that isn't (redirects, misses and uploaded files).
fn build(app: &App, host: &str, path: &str) -> Result<Cached, Box<Response<Body>>> {
    let error = |e: rusqlite::Error| plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    let (site, posts) = {
        let c = app.conn().map_err(|_| plain(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;
        let site = db::site(&c, host).map_err(error)?.ok_or_else(|| plain(StatusCode::NOT_FOUND, "No blog lives at this address."))?;
        let posts = db::posts(&c, &site.host, false).map_err(error)?;
        (site, posts)
    };
    if site.host != host {
        return Err(Box::new(redirect(&format!("{}://{}{path}", app.scheme, site.host))));
    }
    let cx = render::Ctx::new(&site, &app.scheme, &posts);
    let html = |s: String| Cached::new("text/html; charset=utf-8", s, 60);
    let text = |s: String| Cached::new("text/plain; charset=utf-8", s, 300);
    let card = |title: &str, subtitle: &str, meta: &str| {
        let c = og::Card { site: &site.title, title, subtitle, host: &site.host, meta, accent: &site.accent };
        og::png(&og::svg(&c))
            .map(|png| Cached::new("image/png", png, 86_400))
            .map_err(|e| Box::new(plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())))
    };
    let missing = || {
        let mut res = Response::new(Body::from(render::not_found(&cx)));
        *res.status_mut() = StatusCode::NOT_FOUND;
        res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
        Box::new(res)
    };
    let find = |slug: &str| posts.iter().find(|p| p.slug == slug);
    match path {
        "/" => return Ok(html(render::index(&cx))),
        "/feed.xml" => return Ok(Cached::new("application/atom+xml; charset=utf-8", render::feed(&cx), 300)),
        "/sitemap.xml" => return Ok(Cached::new("application/xml; charset=utf-8", render::sitemap(&cx), 300)),
        "/robots.txt" => return Ok(text(render::robots(&cx))),
        "/llms.txt" => return Ok(text(render::llms(&cx))),
        "/llms-full.txt" => return Ok(text(render::llms_full(&cx))),
        "/favicon.svg" => return Ok(Cached::new("image/svg+xml", render::favicon(&site), 86_400)),
        "/favicon.ico" => return Err(Box::new(redirect("/favicon.svg"))),
        "/og.png" => return card(&site.title, &site.description, ""),
        _ => {}
    }
    let rest = &path[1..];
    if let Some(slug) = rest.strip_prefix("og/").and_then(|r| r.strip_suffix(".png")) {
        return find(slug).map_or_else(|| Err(missing()), |p| card(&p.title, "", &render::card_meta(p)));
    }
    if let Some(file) = rest.strip_prefix("media/") {
        return Err(Box::new(media(app, &site.host, file)));
    }
    if let Some(tag) = rest.strip_prefix("tags/") {
        return match tag.strip_suffix('/') {
            Some(slug) => render::tag(&cx, slug).map(html).ok_or_else(missing),
            None => Err(Box::new(redirect(&format!("/tags/{tag}/")))),
        };
    }
    if rest.strip_suffix(".txt") == Some(site.indexnow_key.as_str()) {
        return Ok(text(site.indexnow_key.clone()));
    }
    if let Some(p) = rest.strip_suffix(".md").and_then(find) {
        return Ok(Cached::new("text/markdown; charset=utf-8", render::copy(&cx, p), 300));
    }
    if !site.home.is_empty() && rest.trim_end_matches('/') == site.home {
        return Err(Box::new(redirect("/")));
    }
    match rest.strip_suffix('/').and_then(find) {
        Some(p) => Ok(html(render::article(&cx, p))),
        None if find(rest).is_some() => Err(Box::new(redirect(&format!("/{rest}/")))),
        None => Err(missing()),
    }
}

/// Uploaded images, named by their content, so they never change.
fn media(app: &App, host: &str, file: &str) -> Response<Body> {
    let ctype = match file.rsplit_once('.') {
        Some((name, ext)) if name.len() > 17 && name.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') => match ext {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "avif" => "image/avif",
            _ => "",
        },
        _ => "",
    };
    // The name is checked before touching the disk, so no path can leave the site's media folder.
    let bytes = match ctype.is_empty() {
        true => None,
        false => std::fs::read(app.data.join("media").join(host).join(file)).ok(),
    };
    let Some(bytes) = bytes else {
        return plain(StatusCode::NOT_FOUND, "No such file.");
    };
    Response::builder()
        .header(header::CONTENT_TYPE, ctype)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(bytes))
        .unwrap_or_default()
}
