//! Pages, feeds and the files search engines and AI assistants read, made from a site and its posts.

use crate::db::{Post, Site};
use chrono::{DateTime, SecondsFormat};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const LANTERNLOG_URL: &str = "https://lanternlog.com";

/// A post's Markdown, rendered: its HTML, its outline (level, id, text) and its plain text.
pub struct Doc {
    pub html: String,
    pub toc: Vec<(u8, String, String)>,
    pub text: String,
}

type Heading<'a> = (HeadingLevel, Option<CowStr<'a>>, Vec<CowStr<'a>>, Vec<(CowStr<'a>, Option<CowStr<'a>>)>, Vec<Event<'a>>, String);

pub fn markdown(md: &str) -> Doc {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES;
    let (mut events, mut toc, mut text, mut ids) = (Vec::new(), Vec::new(), String::new(), HashSet::new());
    let mut heading: Option<Heading> = None;
    for ev in Parser::new_ext(md, opts) {
        match &ev {
            Event::Text(t) | Event::Code(t) => {
                text.push_str(t);
                if let Some(h) = heading.as_mut() {
                    h.5.push_str(t);
                }
            }
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::TableCell | TagEnd::CodeBlock) => {
                text.push(' ')
            }
            _ => {}
        }
        match ev {
            Event::Start(Tag::Heading { level, id, classes, attrs }) => {
                heading = Some((level, id, classes, attrs, Vec::new(), String::new()))
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, id, classes, attrs, inner, label)) = heading.take() {
                    let base = id.map(|i| i.to_string()).unwrap_or_else(|| slugify(&label));
                    let mut id = base.clone();
                    for n in 2.. {
                        if ids.insert(id.clone()) {
                            break;
                        }
                        id = format!("{base}-{n}");
                    }
                    if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) {
                        toc.push((level as u8, id.clone(), label.trim().to_string()));
                    }
                    events.push(Event::Start(Tag::Heading { level, id: Some(id.into()), classes, attrs }));
                    events.extend(inner);
                    events.push(Event::End(TagEnd::Heading(level)));
                }
            }
            ev => match heading.as_mut() {
                Some(h) => h.4.push(ev),
                None => events.push(ev),
            },
        }
    }
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events.into_iter());
    Doc { html: out, toc, text: text.split_whitespace().collect::<Vec<_>>().join(" ") }
}

/// Lowercase words joined by hyphens, for heading ids and tag addresses.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c)
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-')
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "section".into() } else { out }
}

pub fn iso(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, true)).unwrap_or_default()
}

fn day(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0).map(|d| d.format("%B %-d, %Y").to_string()).unwrap_or_default()
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut t: String = s.chars().take(n).collect();
    if let Some(i) = t.rfind(' ') {
        t.truncate(i);
    }
    t.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string() + "…"
}

/// The description a post gives, or the start of its text.
pub fn summary(p: &Post, doc: &Doc) -> String {
    if p.description.trim().is_empty() { clip(&doc.text, 155) } else { p.description.clone() }
}

/// Everything a page needs to know about the site it belongs to.
pub struct Ctx<'a> {
    pub site: &'a Site,
    pub home: String,
    pub posts: &'a [Post],
}

impl<'a> Ctx<'a> {
    pub fn new(site: &'a Site, scheme: &str, posts: &'a [Post]) -> Self {
        Ctx { site, home: format!("{scheme}://{}", site.host), posts }
    }
    pub fn url(&self, p: &Post) -> String {
        format!("{}/{}/", self.home, p.slug)
    }
    fn articles(&self) -> impl Iterator<Item = &'a Post> {
        self.posts.iter().filter(|p| p.kind != "page")
    }
    fn pages(&self) -> impl Iterator<Item = &'a Post> {
        let home = &self.site.home;
        self.posts.iter().filter(move |p| p.kind == "page" && p.slug != *home)
    }
    /// The page shown as the home page, when the site names one.
    fn home(&self) -> Option<&'a Post> {
        self.posts.iter().find(|p| !self.site.home.is_empty() && p.kind == "page" && p.slug == self.site.home)
    }
    fn project(&self) -> Option<(String, &str)> {
        let s = self.site;
        if s.project_url.is_empty() {
            return None;
        }
        let label = if s.project_name.is_empty() {
            s.project_url.split("://").nth(1).unwrap_or(&s.project_url).trim_end_matches('/').to_string()
        } else {
            s.project_name.clone()
        };
        Some((label, &s.project_url))
    }
    fn publisher(&self) -> Value {
        json!({"@type": "Organization", "name": self.site.title, "url": format!("{}/", self.home),
               "logo": {"@type": "ImageObject", "url": format!("{}/favicon.svg", self.home)}})
    }
    fn author(&self) -> Value {
        let s = self.site;
        match (s.author.is_empty(), s.author_url.is_empty()) {
            (true, _) => self.publisher(),
            (false, true) => json!({"@type": "Person", "name": s.author}),
            (false, false) => json!({"@type": "Person", "name": s.author, "url": s.author_url}),
        }
    }
}

struct Head<'a> {
    title: String,
    headline: &'a str,
    description: String,
    url: String,
    image: String,
    kind: &'static str,
    index: bool,
    published: Option<i64>,
    modified: Option<i64>,
    tags: &'a [String],
    markdown: Option<String>,
    ld: Value,
}

const CSS: &str = r#":root{--accent:ACCENT;--link:ACCENT;--bg:#fff;--fg:#1d1d1f;--muted:#5f6368;--line:#e6e6e6;--soft:#f6f6f5}
@media (prefers-color-scheme:dark){:root{--link:color-mix(in srgb,ACCENT 60%,#fff);--bg:#121316;--fg:#e8e8e9;--muted:#a3a7ad;--line:#2b2d31;--soft:#1c1d21}}
*{box-sizing:border-box}html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--fg);font:18px/1.7 system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif}
.wrap{max-width:46rem;margin:0 auto;padding:0 1.25rem}
a{color:var(--link);text-underline-offset:.15em}
header.site{border-bottom:1px solid var(--line)}
header.site .wrap{display:flex;gap:.5rem 1.25rem;align-items:center;justify-content:space-between;flex-wrap:wrap;padding-top:1rem;padding-bottom:1rem}
.brand{font-weight:700;color:var(--fg);text-decoration:none}
nav.top{display:flex;gap:1rem;flex-wrap:wrap;font-size:.95rem}
nav.top a{color:var(--muted);text-decoration:none}
nav.top a.project{color:var(--link);font-weight:600}
h1{font-size:2.3rem;line-height:1.2;margin:1.25rem 0 .75rem;letter-spacing:-.01em}
h2{font-size:1.5rem;line-height:1.3;margin:2.2rem 0 .6rem}
h3{font-size:1.2rem;line-height:1.35;margin:1.8rem 0 .5rem}
.lead{font-size:1.2rem;color:var(--muted);margin-top:0}
.meta,.crumbs{color:var(--muted);font-size:.9rem}
.crumbs{margin-top:1.5rem}.crumbs a{color:var(--muted)}
.toc{background:var(--soft);border-radius:10px;padding:.8rem 1.2rem;margin:1.5rem 0;font-size:.95rem}
.toc p{margin:0 0 .3rem;font-weight:600}.toc ol{margin:0;padding-left:1.2rem}.toc .l3{margin-left:1rem;list-style:circle}
img,video{max-width:100%;height:auto;border-radius:8px}
pre{background:var(--soft);padding:1rem;overflow-x:auto;border-radius:10px;font-size:.85rem;line-height:1.5}
code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.9em}
:not(pre)>code{background:var(--soft);padding:.1em .35em;border-radius:5px}
blockquote{margin:1.5rem 0;padding:.2rem 1.2rem;border-left:3px solid var(--line);color:var(--muted)}
table{border-collapse:collapse;display:block;overflow-x:auto;margin:1.5rem 0;font-size:.95rem}
th,td{border:1px solid var(--line);padding:.45rem .7rem;text-align:left}
hr{border:0;border-top:1px solid var(--line);margin:2.5rem 0}
.tags a{margin-right:.75rem;font-size:.9rem}
.cta{margin:3rem 0;padding:1.2rem 1.4rem;border:1px solid var(--line);border-left:4px solid var(--accent);border-radius:10px;background:var(--soft)}
.cta p{margin:0 0 .8rem}
.button{display:inline-block;background:var(--accent);color:#fff;padding:.55rem 1rem;border-radius:8px;text-decoration:none;font-weight:600}
ol.posts{list-style:none;padding:0;margin:1rem 0}
ol.posts li{padding:1.2rem 0;border-bottom:1px solid var(--line)}
ol.posts h2{margin:0 0 .3rem;font-size:1.35rem}ol.posts h2 a{color:var(--fg);text-decoration:none}
ol.posts p{margin:.3rem 0 0;color:var(--muted)}ol.posts time{color:var(--muted);font-size:.85rem}
footer{margin-top:4rem;border-top:1px solid var(--line);color:var(--muted);font-size:.85rem}
footer .wrap{padding-top:1.5rem;padding-bottom:2.5rem}footer a{color:var(--muted)}
@media (max-width:600px){body{font-size:17px}h1{font-size:1.85rem}}"#;

fn layout(cx: &Ctx, head: Head, body: Markup) -> String {
    let s = cx.site;
    let ld = serde_json::to_string(&head.ld).unwrap_or_default().replace("</", "<\\/");
    html! {
        (DOCTYPE)
        html lang=(s.lang) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (head.title) }
                meta name="description" content=(head.description);
                link rel="canonical" href=(head.url);
                meta name="robots" content=(if head.index { "index, follow, max-image-preview:large" } else { "noindex, follow" });
                meta name="theme-color" content=(s.accent);
                link rel="icon" href="/favicon.svg" type="image/svg+xml";
                link rel="alternate" type="application/atom+xml" title=(s.title) href="/feed.xml";
                @if let Some(md) = &head.markdown { link rel="alternate" type="text/markdown" href=(md); }
                meta property="og:type" content=(head.kind);
                meta property="og:site_name" content=(s.title);
                meta property="og:title" content=(head.headline);
                meta property="og:description" content=(head.description);
                meta property="og:url" content=(head.url);
                meta property="og:image" content=(head.image);
                meta property="og:image:width" content="1200";
                meta property="og:image:height" content="630";
                meta property="og:image:alt" content=(head.headline);
                @if let Some(t) = head.published { meta property="article:published_time" content=(iso(t)); }
                @if let Some(t) = head.modified { meta property="article:modified_time" content=(iso(t)); }
                @for t in head.tags { meta property="article:tag" content=(t); }
                meta name="twitter:card" content="summary_large_image";
                meta name="twitter:title" content=(head.headline);
                meta name="twitter:description" content=(head.description);
                meta name="twitter:image" content=(head.image);
                script type="application/ld+json" { (PreEscaped(ld)) }
                style { (PreEscaped(CSS.replace("ACCENT", &s.accent))) }
            }
            body {
                header.site { div.wrap {
                    a.brand href="/" { (s.title) }
                    nav.top {
                        @for p in cx.pages() { a href=(format!("/{}/", p.slug)) { (p.title) } }
                        @if let Some((label, url)) = cx.project() { a.project href=(url) { (label) " →" } }
                    }
                } }
                main.wrap { (body) }
                footer { div.wrap {
                    a href="/feed.xml" { "Feed" } " · " a href="/sitemap.xml" { "Sitemap" } " · " a href="/llms.txt" { "llms.txt" }
                    " · Published with " a href=(LANTERNLOG_URL) { "Lanternlog" }
                } }
            }
        }
    }
    .into_string()
}

fn list(posts: &[&Post]) -> Markup {
    html! {
        ol.posts { @for p in posts {
            li { article {
                h2 { a href=(format!("/{}/", p.slug)) { (p.title) } }
                @if let Some(t) = p.published_at { time datetime=(iso(t)) { (day(t)) } }
                p { (summary(p, &markdown(&p.body))) }
            } }
        } }
        @if posts.is_empty() { p { "Nothing has been published here yet." } }
    }
}

pub fn index(cx: &Ctx) -> String {
    let s = cx.site;
    let posts: Vec<&Post> = cx.articles().collect();
    let home = cx.home();
    let doc = home.map(|p| markdown(&p.body));
    let description = match (home, &doc) {
        (Some(p), Some(d)) => summary(p, d),
        _ => s.description.clone(),
    };
    let ld = json!({
        "@context": "https://schema.org", "@type": if home.is_some() { "WebSite" } else { "Blog" }, "name": s.title,
        "url": format!("{}/", cx.home), "description": description, "inLanguage": s.lang, "publisher": cx.publisher(),
        "blogPost": posts.iter().take(30).map(|p| json!({"@type": "BlogPosting", "headline": p.title, "url": cx.url(p),
            "datePublished": p.published_at.map(iso)})).collect::<Vec<_>>(),
    });
    let head = Head {
        title: home.map_or_else(|| s.title.clone(), |p| p.title.clone()),
        headline: home.map_or(&s.title, |p| &p.title),
        description,
        url: format!("{}/", cx.home),
        image: format!("{}/og.png?v={}", cx.home, s.updated_at),
        kind: "website",
        index: true,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld,
    };
    let body = html! {
        @if let (Some(p), Some(d)) = (home, &doc) {
            h1 { (p.title) }
            (PreEscaped(&d.html))
            @if !posts.is_empty() { h2 { "From the blog" } (list(&posts)) }
        } @else {
            h1 { (s.title) }
            @if !s.description.is_empty() { p.lead { (s.description) } }
            (list(&posts))
        }
    };
    layout(cx, head, body)
}

pub fn article(cx: &Ctx, p: &Post) -> String {
    let s = cx.site;
    let doc = markdown(&p.body);
    let url = cx.url(p);
    let description = summary(p, &doc);
    let words = doc.text.split_whitespace().count();
    let page = p.kind == "page";
    let image = format!("{}/og/{}.png?v={}", cx.home, p.slug, p.updated_at);
    let mut thing = json!({
        "@type": if page { "WebPage" } else { "BlogPosting" }, "headline": p.title, "name": p.title, "description": description,
        "url": url, "mainEntityOfPage": url, "image": [image], "inLanguage": s.lang, "author": cx.author(), "publisher": cx.publisher(),
        "dateModified": iso(p.updated_at), "wordCount": words, "keywords": p.tags,
        "isPartOf": {"@type": if page { "WebSite" } else { "Blog" }, "name": s.title, "url": format!("{}/", cx.home)},
    });
    if let Some(t) = p.published_at {
        thing["datePublished"] = json!(iso(t));
    }
    let crumbs = json!({"@type": "BreadcrumbList", "itemListElement": [
        {"@type": "ListItem", "position": 1, "name": s.title, "item": format!("{}/", cx.home)},
        {"@type": "ListItem", "position": 2, "name": p.title, "item": url}]});
    let ld = json!({"@context": "https://schema.org", "@graph": [thing, crumbs]});
    let mut related: Vec<&Post> =
        cx.articles().filter(|r| r.slug != p.slug && r.tags.iter().any(|t| p.tags.contains(t))).take(3).collect();
    if related.is_empty() {
        related = cx.articles().filter(|r| r.slug != p.slug).take(3).collect();
    }
    let edited = p.published_at.is_some_and(|t| p.updated_at - t > 86_400);
    let head = Head {
        title: format!("{} · {}", p.title, s.title),
        headline: &p.title,
        description,
        url: url.clone(),
        image,
        kind: "article",
        index: true,
        published: p.published_at.filter(|_| !page),
        modified: Some(p.updated_at),
        tags: &p.tags,
        markdown: Some(format!("{}/{}.md", cx.home, p.slug)),
        ld,
    };
    let body = html! {
        nav.crumbs aria-label="Breadcrumb" { a href="/" { (s.title) } " › " (p.title) }
        article {
            h1 { (p.title) }
            @if !page {
                p.meta {
                    @if let Some(t) = p.published_at { time datetime=(iso(t)) { (day(t)) } }
                    @if edited { " · updated " time datetime=(iso(p.updated_at)) { (day(p.updated_at)) } }
                    " · " ((words / 220).max(1)) " min read"
                    @if !s.author.is_empty() {
                        " · "
                        @if s.author_url.is_empty() { (s.author) } @else { a href=(s.author_url) rel="author" { (s.author) } }
                    }
                }
            }
            @if !p.cover.is_empty() { img src=(p.cover) alt=(p.title); }
            @if doc.toc.len() >= 3 {
                nav.toc aria-label="Contents" {
                    p { "Contents" }
                    ol { @for (level, id, text) in &doc.toc { li class=(format!("l{level}")) { a href=(format!("#{id}")) { (text) } } } }
                }
            }
            (PreEscaped(&doc.html))
            @if !p.tags.is_empty() {
                p.tags { @for t in &p.tags { a href=(format!("/tags/{}/", slugify(t))) { "#" (t) } } }
            }
        }
        @if let Some((label, url)) = cx.project() {
            aside.cta {
                @if !s.cta.is_empty() { p { (s.cta) } }
                a.button href=(url) { (label) " →" }
            }
        }
        @if !page && !related.is_empty() {
            section.related { h2 { "Read next" } ul { @for r in &related { li { a href=(format!("/{}/", r.slug)) { (r.title) } } } } }
        }
    };
    layout(cx, head, body)
}

/// The posts carrying a tag, or None when no post does.
pub fn tag(cx: &Ctx, slug: &str) -> Option<String> {
    let s = cx.site;
    let posts: Vec<&Post> = cx.articles().filter(|p| p.tags.iter().any(|t| slugify(t) == slug)).collect();
    let name = posts.first()?.tags.iter().find(|t| slugify(t) == slug)?.clone();
    let url = format!("{}/tags/{slug}/", cx.home);
    let head = Head {
        title: format!("#{name} · {}", s.title),
        headline: &name,
        description: format!("Posts about {name} on {}.", s.title),
        url: url.clone(),
        image: format!("{}/og.png?v={}", cx.home, s.updated_at),
        kind: "website",
        index: false,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld: json!({"@context": "https://schema.org", "@type": "CollectionPage", "name": format!("#{name}"), "url": url}),
    };
    Some(layout(cx, head, html! { nav.crumbs { a href="/" { (s.title) } " › #" (name) } h1 { "#" (name) } (list(&posts)) }))
}

pub fn not_found(cx: &Ctx) -> String {
    let s = cx.site;
    let latest: Vec<&Post> = cx.articles().take(5).collect();
    let head = Head {
        title: format!("Not found · {}", s.title),
        headline: "Not found",
        description: s.description.clone(),
        url: format!("{}/", cx.home),
        image: format!("{}/og.png?v={}", cx.home, s.updated_at),
        kind: "website",
        index: false,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld: json!({}),
    };
    layout(cx, head, html! { h1 { "Nothing here" } p { "This page doesn't exist, but these do:" } (list(&latest)) })
}

pub fn feed(cx: &Ctx) -> String {
    let s = cx.site;
    let updated = cx.posts.iter().map(|p| p.updated_at).max().unwrap_or(s.updated_at).max(s.updated_at);
    let author = if s.author.is_empty() { &s.title } else { &s.author };
    let mut x = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:lang=\"{lang}\">\n\
         <title>{title}</title>\n<subtitle>{sub}</subtitle>\n<link href=\"{home}/\"/>\n<link rel=\"self\" href=\"{home}/feed.xml\"/>\n\
         <id>{home}/</id>\n<updated>{updated}</updated>\n<author><name>{author}</name></author>\n<generator uri=\"{LANTERNLOG_URL}\">Lanternlog</generator>\n",
        lang = esc(&s.lang),
        title = esc(&s.title),
        sub = esc(&s.description),
        home = cx.home,
        updated = iso(updated),
        author = esc(author),
    );
    for p in cx.articles().take(50) {
        let doc = markdown(&p.body);
        let url = cx.url(p);
        x += &format!(
            "<entry>\n<title>{}</title>\n<link href=\"{url}\"/>\n<id>{url}</id>\n<published>{}</published>\n<updated>{}</updated>\n\
             <summary>{}</summary>\n<content type=\"html\">{}</content>\n</entry>\n",
            esc(&p.title),
            iso(p.published_at.unwrap_or(p.updated_at)),
            iso(p.updated_at),
            esc(&summary(p, &doc)),
            esc(&doc.html),
        );
    }
    x + "</feed>\n"
}

pub fn sitemap(cx: &Ctx) -> String {
    let latest = cx.posts.iter().map(|p| p.updated_at).max().unwrap_or(cx.site.updated_at).max(cx.site.updated_at);
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    x += &format!("<url><loc>{}/</loc><lastmod>{}</lastmod></url>\n", cx.home, iso(latest));
    for p in cx.posts.iter().filter(|p| p.slug != cx.site.home) {
        x += &format!("<url><loc>{}</loc><lastmod>{}</lastmod></url>\n", cx.url(p), iso(p.updated_at));
    }
    x + "</urlset>\n"
}

pub fn robots(cx: &Ctx) -> String {
    format!("User-agent: *\nAllow: /\nDisallow: /api/\n\nSitemap: {}/sitemap.xml\n", cx.home)
}

/// llms.txt (https://llmstxt.org): what the site is, with links to Markdown copies of every page.
pub fn llms(cx: &Ctx) -> String {
    let s = cx.site;
    let mut x = format!("# {}\n\n", s.title);
    if !s.description.is_empty() {
        x += &format!("> {}\n\n", s.description);
    }
    if let Some((label, url)) = cx.project() {
        x += &format!("This blog is published by the makers of [{label}]({url}).\n\n");
    }
    for (heading, kind) in [("Posts", false), ("Pages", true)] {
        let items: Vec<&Post> = cx.posts.iter().filter(|p| (p.kind == "page") == kind).collect();
        if items.is_empty() {
            continue;
        }
        x += &format!("## {heading}\n\n");
        for p in items {
            x += &format!("- [{}]({}/{}.md): {}\n", p.title, cx.home, p.slug, summary(p, &markdown(&p.body)));
        }
        x += "\n";
    }
    x
}

/// Every post's Markdown in one file, for assistants that read a whole site at once.
pub fn llms_full(cx: &Ctx) -> String {
    let mut x = llms(cx);
    for p in cx.posts {
        x += &format!("---\n\n{}", copy(cx, p));
    }
    x
}

/// A post as Markdown, served next to its page at /slug.md.
pub fn copy(cx: &Ctx, p: &Post) -> String {
    let mut x = format!("# {}\n\n", p.title);
    if !p.description.is_empty() {
        x += &format!("> {}\n\n", p.description);
    }
    match p.published_at {
        Some(t) if p.kind != "page" => x += &format!("Published {} on {}: {}\n\n", day(t), cx.site.title, cx.url(p)),
        _ => x += &format!("From {}: {}\n\n", cx.site.title, cx.url(p)),
    }
    x + p.body.trim_end() + "\n\n"
}

pub fn favicon(s: &Site) -> String {
    let letter: String = s.title.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect width=\"64\" height=\"64\" rx=\"14\" fill=\"{}\"/>\
         <text x=\"32\" y=\"44\" font-family=\"system-ui,-apple-system,Segoe UI,Roboto,Arial,sans-serif\" font-size=\"36\" \
         font-weight=\"700\" text-anchor=\"middle\" fill=\"#fff\">{}</text></svg>",
        s.accent,
        esc(&letter)
    )
}
