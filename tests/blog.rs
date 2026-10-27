#![allow(clippy::unwrap_used, clippy::expect_used)]
//! One blog from creation to every file it serves, through the same router the binary runs.

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-token-0123456789";
const HOST: &str = "blog.example.com";

struct Res {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Res {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn send(app: &Router, method: &str, host: &str, path: &str, auth: bool, body: Option<Value>, extra: &[(&str, &str)]) -> Res {
    let mut req = Request::builder().method(method).uri(path).header("host", host);
    if auth {
        req = req.header("authorization", format!("Bearer {TOKEN}"));
    }
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req.header("content-type", "application/json").body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    };
    let res = app.clone().oneshot(req.unwrap()).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    Res { status, headers, body: res.into_body().collect().await.unwrap().to_bytes().to_vec() }
}

async fn get(app: &Router, path: &str) -> Res {
    send(app, "GET", HOST, path, false, None, &[]).await
}

#[tokio::test]
async fn a_blog_from_creation_to_every_file_it_serves() {
    let dir = std::env::temp_dir().join(format!("tugpress-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app = tugpress::router(tugpress::App::open(&dir, TOKEN, "https", false).unwrap());

    // The API wants the token; bad input is refused.
    assert_eq!(send(&app, "GET", HOST, "/api/sites", false, None, &[]).await.status, StatusCode::UNAUTHORIZED);
    let site = json!({"title": "Example Notes", "description": "What we learn running Example in production.",
        "aliases": ["www.blog.example.com"], "project_name": "Example", "project_url": "https://example.com",
        "cta": "Example deploys an app with one command.", "author": "Nikita", "author_url": "https://borodutch.com", "accent": "#e4572e"});
    let r = send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(site.clone()), &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let key = r.json()["indexnow_key"].as_str().unwrap().to_string();
    let mut evil = site.clone();
    evil["project_url"] = json!("javascript:alert(1)");
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(evil), &[]).await.status, StatusCode::BAD_REQUEST);
    evil = site.clone();
    evil["accent"] = json!("red;}body{display:none");
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(evil), &[]).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/not_a_host", true, Some(site.clone()), &[]).await.status, StatusCode::BAD_REQUEST);

    // A published post, a draft and a page.
    let body = "We moved **every service** to one box.\n\n## Why one box\n\nCost.\n\n## How we moved\n\nSlowly.\n\n### The database\n\n```sh\npg_dump app > app.sql\n```\n\n## Why one box\n\nTwice.\n";
    let post = json!({"title": "Moving to one server", "body": body, "tags": ["Self-hosting", "Deploys", "self-hosting"], "status": "published"});
    let r = send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/one-server", true, Some(post), &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["url"], "https://blog.example.com/one-server/");
    assert_eq!(r.json()["post"]["tags"], json!(["Self-hosting", "Deploys"]));
    let draft = json!({"title": "Secret plans", "body": "Not yet."});
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/secret", true, Some(draft), &[]).await.status, StatusCode::OK);
    let page = json!({"title": "About", "body": "Who we are.", "kind": "page", "status": "published"});
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/about", true, Some(page), &[]).await.status, StatusCode::OK);
    let reserved = json!({"title": "x", "body": "x"});
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/api", true, Some(reserved), &[]).await.status, StatusCode::BAD_REQUEST);

    // The home page.
    let r = get(&app, "/").await;
    let html = r.text();
    assert_eq!(r.status, StatusCode::OK);
    for needle in ["<title>Example Notes</title>", r#"rel="canonical" href="https://blog.example.com/""#, "og:image", r#""@type":"Blog""#,
        "Moving to one server", r#"href="/about/""#, r#"href="https://example.com""#, "twitter:card"] {
        assert!(html.contains(needle), "home page lacks {needle}");
    }
    assert!(!html.contains("Secret plans"));

    // The post: structured data, outline with unique ids, call to action, tags, Markdown alternate.
    let r = get(&app, "/one-server/").await;
    let html = r.text();
    assert_eq!(r.status, StatusCode::OK);
    for needle in [r#""@type":"BlogPosting""#, r#""@type":"BreadcrumbList""#, r#"id="why-one-box""#, r#"id="why-one-box-2""#,
        r##"href="#the-database""##, "Contents", "Example deploys an app with one command.", "article:published_time",
        r#"href="/tags/self-hosting/""#, "https://blog.example.com/one-server.md", "og/one-server.png?v=", "1 min read"] {
        assert!(html.contains(needle), "post page lacks {needle}");
    }

    // Caching: a repeat with the ETag gets 304, and gzip is offered.
    let etag = r.headers["etag"].to_str().unwrap().to_string();
    assert_eq!(send(&app, "GET", HOST, "/one-server/", false, None, &[("if-none-match", &etag)]).await.status, StatusCode::NOT_MODIFIED);
    let gz = send(&app, "GET", HOST, "/one-server/", false, None, &[("accept-encoding", "gzip, br")]).await;
    assert_eq!(gz.headers["content-encoding"], "gzip");
    assert!(gz.body.len() < html.len());

    // Everything else a crawler or an assistant reads.
    let r = get(&app, "/one-server").await;
    assert_eq!((r.status, r.headers["location"].to_str().unwrap()), (StatusCode::MOVED_PERMANENTLY, "/one-server/"));
    assert!(get(&app, "/one-server.md").await.text().starts_with("# Moving to one server\n"));
    assert_eq!(get(&app, "/secret/").await.status, StatusCode::NOT_FOUND);
    let sitemap = get(&app, "/sitemap.xml").await.text();
    assert!(sitemap.contains("<loc>https://blog.example.com/one-server/</loc>") && !sitemap.contains("secret"));
    let feed = get(&app, "/feed.xml").await.text();
    assert!(feed.contains("<entry>") && feed.contains("Moving to one server") && !feed.contains("About</title>"));
    let llms = get(&app, "/llms.txt").await.text();
    assert!(llms.contains("[Moving to one server](https://blog.example.com/one-server.md)") && llms.contains("## Pages"));
    assert!(get(&app, "/llms-full.txt").await.text().contains("pg_dump app > app.sql"));
    assert!(get(&app, "/robots.txt").await.text().contains("Sitemap: https://blog.example.com/sitemap.xml"));
    let tag = get(&app, "/tags/self-hosting/").await;
    assert!(tag.status == StatusCode::OK && tag.text().contains("noindex"));
    assert_eq!(get(&app, &format!("/{key}.txt")).await.text(), key);
    assert!(get(&app, "/favicon.svg").await.text().starts_with("<svg"));
    let card = get(&app, "/og/one-server.png").await;
    assert_eq!(card.headers["content-type"], "image/png");
    assert!(card.body.starts_with(b"\x89PNG") && card.body.len() > 5_000, "card is {} bytes", card.body.len());

    // Other hosts: aliases redirect, strangers get nothing.
    let r = send(&app, "GET", "www.blog.example.com", "/one-server/", false, None, &[]).await;
    assert_eq!(r.headers["location"], "https://blog.example.com/one-server/");
    assert_eq!(send(&app, "GET", "nobody.example.org", "/", false, None, &[]).await.status, StatusCode::NOT_FOUND);

    // Writes show up at once.
    let edit = json!({"title": "Moving everything to one server", "body": body, "status": "published"});
    send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/one-server", true, Some(edit), &[]).await;
    assert!(get(&app, "/").await.text().contains("Moving everything to one server"));

    // Images go in by content and come back immutable; odd names never reach the disk.
    let png = card.body.clone();
    let req = Request::builder().method("POST").uri("/api/sites/blog.example.com/media").header("host", "admin")
        .header("authorization", format!("Bearer {TOKEN}")).header("content-type", "image/png").body(Body::from(png.clone())).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let url: Value = serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let path = url["url"].as_str().unwrap().trim_start_matches("https://blog.example.com").to_string();
    let image = get(&app, &path).await;
    assert_eq!(image.body, png);
    assert!(image.headers["cache-control"].to_str().unwrap().contains("immutable"));
    assert_eq!(get(&app, "/media/../../tugpress.db").await.status, StatusCode::NOT_FOUND);

    // Deleting a post takes it off the site.
    send(&app, "DELETE", "admin", "/api/sites/blog.example.com/posts/one-server", true, None, &[]).await;
    assert_eq!(get(&app, "/one-server/").await.status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}
