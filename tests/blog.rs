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
    let dir = std::env::temp_dir().join(format!("lanternlog-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app = lanternlog::router(lanternlog::App::open(&dir, TOKEN, "https", false).unwrap());

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
        r##"href="#the-database""##, "On this page", r#"id="theme""#, "Example deploys an app with one command.", "article:published_time",
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
    // A PUT without a key keeps it; one with a key of its own replaces it; a malformed key is refused.
    let r = send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(site.clone()), &[]).await;
    assert_eq!(r.json()["indexnow_key"], json!(key));
    let mut own = site.clone();
    own["indexnow_key"] = json!("143f4ce9a39c0691d2e85e8b486880a4");
    let r = send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(own.clone()), &[]).await;
    assert_eq!(r.json()["indexnow_key"], "143f4ce9a39c0691d2e85e8b486880a4");
    assert_eq!(get(&app, "/143f4ce9a39c0691d2e85e8b486880a4.txt").await.text(), "143f4ce9a39c0691d2e85e8b486880a4");
    assert_eq!(get(&app, &format!("/{key}.txt")).await.status, StatusCode::NOT_FOUND);
    own["indexnow_key"] = json!("../x.txt");
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(own), &[]).await.status, StatusCode::BAD_REQUEST);
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
    assert_eq!(get(&app, "/media/../../lanternlog.db").await.status, StatusCode::NOT_FOUND);

    // A page can stand in for the home page, with the posts below it.
    let mut with_home = site.clone();
    with_home["home"] = json!("about");
    assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(with_home), &[]).await.status, StatusCode::OK);
    let home = get(&app, "/").await.text();
    assert!(home.contains("Who we are.") && home.contains("From the blog") && home.contains(r#""@type":"WebSite""#));
    assert_eq!(get(&app, "/about/").await.headers["location"], "/");
    assert!(!get(&app, "/sitemap.xml").await.text().contains("/about/"));

    // White-label settings: the blog's own logo (drawn into share cards too), share image, links, head tags,
    // footer and font, without the Lanternlog credit.
    let mut brand = site.clone();
    brand["logo"] = json!(path.clone());
    brand["og_image"] = json!("https://cdn.example.com/share.png");
    brand["links"] = json!([{"label": "Pricing", "url": "https://example.com/pricing"}]);
    brand["head"] = json!(r#"<meta name="google-site-verification" content="abc123">"#);
    brand["footer"] = json!("© 2026 Example Inc.");
    brand["credit"] = json!(false);
    brand["font"] = json!("sans");
    let r = send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(brand.clone()), &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let home = get(&app, "/").await.text();
    for needle in [r#"class="logo""#, r#"property="og:image" content="https://cdn.example.com/share.png""#, ">Pricing</a>",
        "google-site-verification", "© 2026 Example Inc.", "--serif:var(--sans)", &format!(r#"rel="icon" href="{path}""#)] {
        assert!(home.contains(needle), "branded home lacks {needle}");
    }
    assert!(!home.contains("Published with") && !home.contains("og:image:width"));
    let branded = get(&app, "/og/one-server.png").await;
    assert!(branded.body.starts_with(b"\x89PNG") && branded.body != card.body);
    let own = json!({"title": "Moving to one server", "body": body, "status": "published", "og_image": "/media/own.png"});
    send(&app, "PUT", "admin", "/api/sites/blog.example.com/posts/one-server", true, Some(own), &[]).await;
    assert!(get(&app, "/one-server/").await.text().contains(r#"property="og:image" content="https://blog.example.com/media/own.png""#));
    for (field, value) in [("font", json!("comic")), ("links", json!([{"label": "x", "url": "javascript:alert(1)"}]))] {
        let mut bad = brand.clone();
        bad[field] = value;
        assert_eq!(send(&app, "PUT", "admin", "/api/sites/blog.example.com", true, Some(bad), &[]).await.status, StatusCode::BAD_REQUEST);
    }

    // Deleting a post takes it off the site.
    send(&app, "DELETE", "admin", "/api/sites/blog.example.com/posts/one-server", true, None, &[]).await;
    assert_eq!(get(&app, "/one-server/").await.status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tables_read_on_a_phone() {
    let two = lanternlog::render::markdown("| Variable | What it does |\n|---|---|\n| `PORT` | Port to [listen](https://x.org) on |\n");
    assert!(two.html.contains("<dl class=\"defs\">"), "{}", two.html);
    assert!(two.html.contains("<dt><code>PORT</code></dt><dd>Port to <a href=\"https://x.org\">listen</a> on</dd>"), "{}", two.html);
    let wide = lanternlog::render::markdown("| Plan | *Price* | Seats |\n|---|---|---|\n| Team | $5 | 10 |\n| Solo | Free |  |\n");
    assert!(wide.html.contains("<th scope=\"row\">Team</th><td data-label=\"Price\"><span>$5</span></td><td data-label=\"Seats\"><span>10</span></td>"), "{}", wide.html);
    assert!(wide.html.contains("<th scope=\"col\"><em>Price</em></th>"), "{}", wide.html);
    assert!(wide.text.contains("Team $5 10"), "plain text keeps the cells: {}", wide.text);
}

#[tokio::test]
async fn translations_get_their_own_indexable_addresses() {
    let dir = std::env::temp_dir().join(format!("lanternlog-i18n-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app = lanternlog::router(lanternlog::App::open(&dir, TOKEN, "https", false).unwrap());
    let put = |path: &str, body: Value| {
        let app = app.clone();
        let path = path.to_string();
        async move { send(&app, "PUT", "admin", &path, true, Some(body), &[]).await }
    };

    // The blog says its chrome in German; whatever it leaves out stays English.
    let site = json!({"title": "Example Notes", "description": "What we learn running Example.", "lang": "en",
        "project_name": "Example", "project_url": "https://example.com", "cta": "Example deploys an app with one command.",
        "links": [{"label": "Pricing", "url": "https://example.com/pricing"}],
        "locales": {"DE": {"title": "Example Notizen", "cta": "Example bringt eine App mit einem Befehl online.",
            "project_url": "https://example.com/de/", "links": [{"label": "Preise", "url": "https://example.com/de/pricing"}],
            "strings": {"on_this_page": "Auf dieser Seite", "min_read": "{n} Min. Lesezeit", "read_next": "Weiterlesen"}}}});
    let r = put("/api/sites/blog.example.com", site.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["locales"]["de"].is_object(), "codes are kept lowercase: {}", r.text());
    for (field, value) in [("locales", json!({"Deutsch!": {}})), ("locales", json!({"de": {"strings": {"read_more_pls": "x"}}})),
        ("locales", json!({"de": {"links": [{"label": "x", "url": "javascript:alert(1)"}]}}))] {
        let mut bad = site.clone();
        bad[field] = value;
        assert_eq!(put("/api/sites/blog.example.com", bad).await.status, StatusCode::BAD_REQUEST);
    }

    let body = "We moved **every service** to one box.\n\n## Why\n\nCost.\n\n## How\n\nSlowly.\n\n## After\n\nCalm.\n";
    let post = json!({"title": "Moving to one server", "body": body, "tags": ["Self-hosting"], "status": "published"});
    assert_eq!(put("/api/sites/blog.example.com/posts/one-server", post).await.status, StatusCode::OK);
    let other = json!({"title": "Only in English", "body": "Not translated.", "status": "published"});
    assert_eq!(put("/api/sites/blog.example.com/posts/english-only", other).await.status, StatusCode::OK);

    // Two ways in: the translations address, or the post's own address with a lang.
    let de = json!({"title": "Umzug auf einen Server", "description": "Warum alles auf einer Maschine läuft.",
        "body": "Wir haben **jeden Dienst** auf eine Kiste gezogen.\n\n## Warum\n\nKosten.\n\n## Wie\n\nLangsam.\n\n## Danach\n\nRuhe.\n",
        "tags": ["Selbst hosten"], "status": "published"});
    let r = put("/api/sites/blog.example.com/posts/one-server/translations/de", de).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["url"], "https://blog.example.com/de/one-server/");
    assert_eq!(r.json()["post"]["lang"], "de");
    let pt = json!({"lang": "pt-BR", "translation_of": "one-server", "title": "Mudando para um servidor", "body": "Tudo numa máquina.", "status": "published"});
    let r = put("/api/sites/blog.example.com/posts/one-server", pt).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["url"], "https://blog.example.com/pt-br/one-server/");
    let zh = json!({"title": "我们如何把所有服务搬到一台服务器上", "body": "一台机器。", "status": "published"});
    assert_eq!(put("/api/sites/blog.example.com/posts/one-server/translations/zh-hans", zh).await.status, StatusCode::OK);
    let fr = json!({"title": "Brouillon", "body": "Pas encore.", "status": "draft"});
    assert_eq!(put("/api/sites/blog.example.com/posts/one-server/translations/fr", fr.clone()).await.status, StatusCode::OK);
    // A translation needs a post to translate, a real language other than the blog's, and the original's slug.
    assert_eq!(put("/api/sites/blog.example.com/posts/nope/translations/de", fr.clone()).await.status, StatusCode::NOT_FOUND);
    for path in ["/api/sites/blog.example.com/posts/one-server/translations/og", "/api/sites/blog.example.com/posts/one-server/translations/en",
        "/api/sites/blog.example.com/posts/one-server/translations/de%2F.."] {
        assert_eq!(put(path, fr.clone()).await.status, StatusCode::BAD_REQUEST, "{path}");
    }
    let moved = json!({"lang": "de", "translation_of": "one-server", "title": "x", "body": "x"});
    assert_eq!(put("/api/sites/blog.example.com/posts/umzug", moved).await.status, StatusCode::BAD_REQUEST);
    // Language editions and post slugs can't share an address.
    assert_eq!(put("/api/sites/blog.example.com/posts/de", json!({"title": "x", "body": "x"})).await.status, StatusCode::BAD_REQUEST);
    let list = send(&app, "GET", "admin", "/api/sites/blog.example.com/posts/one-server/translations", true, None, &[]).await.json();
    let langs: Vec<&str> = list.as_array().unwrap().iter().map(|t| t["lang"].as_str().unwrap()).collect();
    assert_eq!(langs, ["de", "fr", "pt-br", "zh-hans"]);

    // The German post: its own language, canonical, every version paired, German chrome, a switcher.
    let r = get(&app, "/de/one-server/").await;
    let html = r.text();
    assert_eq!(r.status, StatusCode::OK, "{html}");
    for needle in [r#"<html lang="de">"#, r#"rel="canonical" href="https://blog.example.com/de/one-server/""#,
        r#"hreflang="en" href="https://blog.example.com/one-server/""#, r#"hreflang="de" href="https://blog.example.com/de/one-server/""#,
        r#"hreflang="pt-br" href="https://blog.example.com/pt-br/one-server/""#, r#"hreflang="zh-hans" href="https://blog.example.com/zh-hans/one-server/""#,
        r#"hreflang="x-default" href="https://blog.example.com/one-server/""#, r#"property="og:locale" content="de_DE""#,
        r#"property="og:locale:alternate" content="en_US""#, r#"property="og:locale:alternate" content="pt_BR""#, r#""inLanguage":"de""#,
        "Umzug auf einen Server · Example Notizen", "Example bringt eine App mit einem Befehl online.", r#"href="https://example.com/de/""#,
        ">Preise</a>", "Auf dieser Seite", "1 Min. Lesezeit", r#"href="/de/tags/selbst-hosten/""#, r#"href="/de/""#,
        "https://blog.example.com/de/og/one-server.png?v=", "https://blog.example.com/de/one-server.md",
        r#"<span lang="de" aria-current="page">Deutsch</span>"#, r#"<a href="https://blog.example.com/one-server/" lang="en" hreflang="en">English</a>"#,
        ">Português (Brasil)</a>", ">简体中文</a>", r#"href="/de/feed.xml""#] {
        assert!(html.contains(needle), "German post lacks {needle}");
    }
    assert!(!html.contains("hreflang=\"fr\"") && !html.contains("Pricing"), "drafts stay out, and so does the English chrome");

    // The English original pairs back with every published translation, and stays English.
    let html = get(&app, "/one-server/").await.text();
    for needle in [r#"<html lang="en">"#, r#"hreflang="de" href="https://blog.example.com/de/one-server/""#,
        r#"hreflang="x-default" href="https://blog.example.com/one-server/""#, r#"property="og:locale" content="en_US""#,
        r#"property="og:locale:alternate" content="de_DE""#, "On this page", r#""inLanguage":"en""#, ">Deutsch</a>"] {
        assert!(html.contains(needle), "English post lacks {needle}");
    }
    // A post without translations has no alternates or switcher.
    let html = get(&app, "/english-only/").await.text();
    assert!(!html.contains("hreflang") && !html.contains("og:locale:alternate") && !html.contains(r#"class="langs""#));

    // Each language has a home listing only what is translated into it.
    let r = get(&app, "/de/").await;
    let html = r.text();
    assert_eq!(r.status, StatusCode::OK);
    assert!(html.contains("Umzug auf einen Server") && !html.contains("Only in English") && html.contains(r#"href="/de/one-server/""#));
    assert!(html.contains(r#"hreflang="x-default" href="https://blog.example.com/""#) && html.contains(r#"rel="canonical" href="https://blog.example.com/de/""#));
    assert_eq!(get(&app, "/de").await.headers["location"], "/de/");
    assert_eq!(get(&app, "/de/one-server").await.headers["location"], "/de/one-server/");
    assert_eq!(get(&app, "/fr/one-server/").await.status, StatusCode::NOT_FOUND, "drafts aren't served");
    assert_eq!(get(&app, "/de/english-only/").await.status, StatusCode::NOT_FOUND);
    assert_eq!(get(&app, "/de/sitemap.xml").await.status, StatusCode::NOT_FOUND, "one sitemap, at the root");
    assert!(get(&app, "/zh-hans/one-server/").await.text().contains(r#"property="og:locale" content="zh_CN""#));

    // Crawler files: sitemap alternates, per-language feeds, llms.txt, Markdown copies, share cards.
    let sitemap = get(&app, "/sitemap.xml").await.text();
    for needle in [r#"xmlns:xhtml="http://www.w3.org/1999/xhtml""#, "<loc>https://blog.example.com/de/one-server/</loc>",
        "<loc>https://blog.example.com/pt-br/</loc>", r#"<xhtml:link rel="alternate" hreflang="de" href="https://blog.example.com/de/one-server/"/>"#,
        r#"<xhtml:link rel="alternate" hreflang="x-default" href="https://blog.example.com/one-server/"/>"#] {
        assert!(sitemap.contains(needle), "sitemap lacks {needle}");
    }
    assert!(!sitemap.contains("/fr/") && sitemap.matches("<loc>").count() == 9, "{sitemap}");
    let feed = get(&app, "/de/feed.xml").await.text();
    assert!(feed.contains(r#"xml:lang="de""#) && feed.contains("Umzug auf einen Server") && !feed.contains("Only in English"));
    assert!(feed.contains(r#"<link rel="self" href="https://blog.example.com/de/feed.xml"/>"#));
    let llms = get(&app, "/llms.txt").await.text();
    assert!(llms.contains("[Deutsch](https://blog.example.com/de/llms.txt)"), "{llms}");
    assert!(get(&app, "/de/llms.txt").await.text().contains("[Umzug auf einen Server](https://blog.example.com/de/one-server.md)"));
    assert!(get(&app, "/de/one-server.md").await.text().starts_with("# Umzug auf einen Server\n"));
    let en_card = get(&app, "/og/one-server.png").await;
    let de_card = get(&app, "/de/og/one-server.png").await;
    let zh_card = get(&app, "/zh-hans/og/one-server.png").await;
    assert!(de_card.body.starts_with(b"\x89PNG") && de_card.body != en_card.body && zh_card.body.starts_with(b"\x89PNG"));
    assert!(get(&app, "/de/tags/selbst-hosten/").await.text().contains("Umzug auf einen Server"));

    // The blog can't switch its own language to one it has translations in; a blog written in pt-BR finds its pt-br locale.
    let mut german = site.clone();
    german["lang"] = json!("de");
    assert_eq!(put("/api/sites/blog.example.com", german).await.status, StatusCode::BAD_REQUEST);
    let br = json!({"title": "Notas", "lang": "pt-BR", "project_url": "https://example.com.br", "locales": {"pt-br": {"cta": "Conheça o Example."}}});
    assert_eq!(put("/api/sites/blog.example.com.br", br).await.status, StatusCode::OK);
    put("/api/sites/blog.example.com.br/posts/oi", json!({"title": "Oi", "body": "Olá.", "status": "published"})).await;
    let html = send(&app, "GET", "blog.example.com.br", "/oi/", false, None, &[]).await.text();
    assert!(html.contains("Conheça o Example.") && html.contains(r#"<html lang="pt-BR">"#) && html.contains(r#"content="pt_BR""#), "{html}");

    // An RTL language reads right to left.
    put("/api/sites/blog.example.com/posts/one-server/translations/ar", json!({"title": "الانتقال إلى خادم واحد", "body": "كل شيء.", "status": "published"})).await;
    assert!(get(&app, "/ar/one-server/").await.text().contains(r#"<html lang="ar" dir="rtl">"#));

    // Unpublishing the original hides its translations too; deleting it deletes them.
    let draft = json!({"title": "Moving to one server", "body": body, "status": "draft"});
    put("/api/sites/blog.example.com/posts/one-server", draft).await;
    assert_eq!(get(&app, "/de/one-server/").await.status, StatusCode::NOT_FOUND);
    send(&app, "DELETE", "admin", "/api/sites/blog.example.com/posts/one-server", true, None, &[]).await;
    let r = send(&app, "GET", "admin", "/api/sites/blog.example.com/posts/one-server/translations/de", true, None, &[]).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}
