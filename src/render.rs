//! Pages, feeds and the files search engines and AI assistants read, made from a site and its posts.

use crate::db::{Post, Site};
use crate::{lang, style};
use chrono::{DateTime, SecondsFormat};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

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
                    let anchor = matches!(level, HeadingLevel::H2 | HeadingLevel::H3)
                        .then(|| format!("<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\" tabindex=\"-1\">#</a>"));
                    events.push(Event::Start(Tag::Heading { level, id: Some(id.into()), classes, attrs }));
                    events.extend(inner);
                    events.extend(anchor.map(|a| Event::InlineHtml(a.into())));
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
    pulldown_cmark::html::push_html(&mut out, tables(events).into_iter());
    let html = out.replace("<img src=", "<img loading=\"lazy\" decoding=\"async\" src=");
    Doc { html, toc, text: text.split_whitespace().collect::<Vec<_>>().join(" ") }
}

/// Tables that read on a phone too. Two columns are terms and what they mean, so they become a definition list;
/// wider tables keep their grid on wide screens, with every cell labelled by its column so that on narrow ones
/// each row turns into a card (style.rs).
fn tables(events: Vec<Event>) -> Vec<Event> {
    let mut out = Vec::with_capacity(events.len());
    let mut it = events.into_iter();
    while let Some(ev) = it.next() {
        if !matches!(ev, Event::Start(Tag::Table(_))) {
            out.push(ev);
            continue;
        }
        let (mut head, mut rows, mut row, mut cell, mut in_head) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), false);
        for ev in it.by_ref() {
            match ev {
                Event::End(TagEnd::Table) => break,
                Event::Start(Tag::TableHead) => in_head = true,
                Event::End(TagEnd::TableHead) => in_head = false,
                Event::Start(Tag::TableRow) => row = Vec::new(),
                Event::End(TagEnd::TableRow) if !in_head => rows.push(std::mem::take(&mut row)),
                Event::Start(Tag::TableCell) => cell = Vec::new(),
                Event::End(TagEnd::TableCell) => {
                    let mut html = String::new();
                    pulldown_cmark::html::push_html(&mut html, std::mem::take(&mut cell).into_iter());
                    if in_head { head.push(html) } else { row.push(html) }
                }
                ev => cell.push(ev),
            }
        }
        out.push(Event::Html(table_html(&head, &rows).into()));
    }
    out
}

fn table_html(head: &[String], rows: &[Vec<String>]) -> String {
    let cell = |r: &[String], i: usize| r.get(i).cloned().unwrap_or_default();
    if head.len() == 2 {
        let items: String = rows.iter().map(|r| format!("<div><dt>{}</dt><dd>{}</dd></div>\n", cell(r, 0), cell(r, 1))).collect();
        return format!("<dl class=\"defs\">\n{items}</dl>\n");
    }
    let label = |i: usize| {
        let (mut text, mut tag) = (String::new(), false);
        for c in head.get(i).map(String::as_str).unwrap_or("").chars() {
            match c {
                '<' => tag = true,
                '>' => tag = false,
                c if !tag => text.push(c),
                _ => {}
            }
        }
        text.trim().replace('"', "&quot;")
    };
    let th: String = head.iter().map(|h| format!("<th scope=\"col\">{h}</th>")).collect();
    let body: String = rows
        .iter()
        .map(|r| {
            let cells: String = (1..head.len()).map(|i| format!("<td data-label=\"{}\"><span>{}</span></td>", label(i), cell(r, i))).collect();
            format!("<tr><th scope=\"row\">{}</th>{cells}</tr>\n", cell(r, 0))
        })
        .collect();
    format!("<div class=\"table\"><table>\n<thead><tr>{th}</tr></thead>\n<tbody>\n{body}</tbody></table></div>\n")
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

/// "October 27, 2026 · 4 min read" for a post's share card, in the post's language; nothing for pages.
pub fn card_meta(cx: &Ctx, p: &Post) -> String {
    match p.published_at {
        Some(t) if p.kind != "page" => format!("{} · {}", cx.words.day(t), cx.words.min_read(minutes(&markdown(&p.body).text))),
        _ => String::new(),
    }
}

/// Reading time at 220 words a minute; scripts without spaces between words count about two characters a word.
fn minutes(text: &str) -> usize {
    let words = text.split_whitespace().count();
    let dense = text.chars().filter(|&c| crate::og::wide(c) || ('\u{0e00}'..='\u{0e7f}').contains(&c)).count();
    ((words + dense / 2) / 220).max(1)
}

/// The description a post gives, or the start of its text.
pub fn summary(p: &Post, doc: &Doc) -> String {
    if p.description.trim().is_empty() { clip(&doc.text, 155) } else { p.description.clone() }
}

/// The languages each published post can be read in: slug to (language, last change), the blog's own language first.
pub type Versions = BTreeMap<String, Vec<(String, i64)>>;

/// Everything a page needs to know about the site it belongs to, in the page's language.
pub struct Ctx<'a> {
    /// The site as the page's language shows it (`Site::localized`); `site.lang` is the page's language.
    pub site: &'a Site,
    pub home: String,
    /// The posts in the page's language.
    pub posts: &'a [Post],
    /// The blog's own language, whose pages live at the root.
    pub main: String,
    /// "" in the blog's own language, "/de" in its German edition.
    pub prefix: String,
    pub versions: &'a Versions,
    /// Every language the blog can be read in, its own first.
    pub langs: Vec<String>,
    pub words: lang::Words<'a>,
}

impl<'a> Ctx<'a> {
    pub fn new(site: &'a Site, scheme: &str, posts: &'a [Post], main: &str, versions: &'a Versions) -> Self {
        let prefix = if lang::same(&site.lang, main) { String::new() } else { format!("/{}", site.lang) };
        let mut langs = vec![main.to_string()];
        for (l, _) in versions.values().flatten() {
            if !langs.iter().any(|x| lang::same(x, l)) {
                langs.push(l.clone());
            }
        }
        langs[1..].sort();
        let words = lang::Words::new(&site.lang, site.locale(&site.lang).map(|l| &l.strings));
        Ctx { site, home: format!("{scheme}://{}", site.host), posts, main: main.to_string(), prefix, versions, langs, words }
    }
    /// The page's own address for a post, from the root ("/de/one-server/").
    pub fn path(&self, p: &Post) -> String {
        format!("{}/{}/", self.prefix, p.slug)
    }
    pub fn url(&self, p: &Post) -> String {
        format!("{}{}", self.home, self.path(p))
    }
    /// The home page of this edition ("https://host/" or "https://host/de/").
    pub fn root(&self) -> String {
        format!("{}{}/", self.home, self.prefix)
    }
    /// A post's address in another language, or that language's home page when `slug` is None.
    fn url_in(&self, lang: &str, slug: Option<&str>) -> String {
        let prefix = if lang::same(lang, &self.main) { String::new() } else { format!("/{lang}") };
        match slug {
            Some(slug) => format!("{}{prefix}/{slug}/", self.home),
            None => format!("{}{prefix}/", self.home),
        }
    }
    /// Every version of a post (or of the home page when `slug` is None) as (language, address), the blog's own first.
    pub fn alternates(&self, slug: Option<&str>) -> Vec<(String, String)> {
        let langs: Vec<&String> = match slug {
            Some(s) => self.versions.get(s).map(|v| v.iter().map(|(l, _)| l).collect()).unwrap_or_default(),
            None => self.langs.iter().collect(),
        };
        langs.into_iter().map(|l| (l.clone(), self.url_in(l, slug))).collect()
    }
    /// What a language calls itself, or what the blog chose to call it.
    pub fn lang_name(&self, code: &str) -> String {
        self.site.locale(code).map(|l| l.name.trim()).filter(|n| !n.is_empty()).map_or_else(|| lang::name(code), str::to_string)
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
    /// A brand image address as a full URL ("/media/x.png" becomes "https://host/media/x.png").
    fn abs(&self, url: &str) -> String {
        if url.starts_with('/') { format!("{}{url}", self.home) } else { url.to_string() }
    }
    /// The site's share image: its own if it set one, or the generated card.
    fn site_image(&self) -> (String, bool) {
        match self.site.og_image.as_str() {
            "" => (format!("{}{}/og.png?v={}", self.home, self.prefix, self.site.updated_at), true),
            own => (self.abs(own), false),
        }
    }
    fn publisher(&self) -> Value {
        let logo = if self.site.logo.is_empty() { format!("{}/favicon.svg", self.home) } else { self.abs(&self.site.logo) };
        json!({"@type": "Organization", "name": self.site.title, "url": self.root(),
               "logo": {"@type": "ImageObject", "url": logo}})
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
    /// Whether `image` is a generated 1200×630 card, whose size the tags can state.
    card: bool,
    kind: &'static str,
    index: bool,
    published: Option<i64>,
    modified: Option<i64>,
    tags: &'a [String],
    markdown: Option<String>,
    ld: Value,
    /// The page in every language it exists in, itself included: (language, address).
    alternates: Vec<(String, String)>,
}

/// Links to the page in each of its languages, each named in its own language.
fn switcher(cx: &Ctx, alternates: &[(String, String)]) -> Markup {
    html! {
        @if alternates.len() > 1 {
            nav.langs aria-label=(cx.words.get("languages")) {
                @for (code, url) in alternates {
                    @if lang::same(code, &cx.site.lang) { span lang=(code) aria-current="page" { (cx.lang_name(code)) } }
                    @else { a href=(url) lang=(code) hreflang=(code) { (cx.lang_name(code)) } }
                }
            }
        }
    }
}

fn layout(cx: &Ctx, head: Head, body: Markup) -> String {
    let s = cx.site;
    let w = &cx.words;
    let ld = serde_json::to_string(&head.ld).unwrap_or_default().replace("</", "<\\/");
    let many = head.alternates.len() > 1;
    let x_default = head.alternates.iter().find(|(l, _)| lang::same(l, &cx.main)).map(|(_, u)| u.clone());
    let own = lang::og(&s.lang);
    let mut others: Vec<String> = Vec::new();
    for (l, _) in &head.alternates {
        let og = lang::og(l);
        if og != own && !others.contains(&og) {
            others.push(og);
        }
    }
    html! {
        (DOCTYPE)
        html lang=(s.lang) dir=[lang::rtl(&s.lang).then_some("rtl")] {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (head.title) }
                meta name="description" content=(head.description);
                link rel="canonical" href=(head.url);
                @if many {
                    @for (code, url) in &head.alternates { link rel="alternate" hreflang=(code) href=(url); }
                    @if let Some(url) = &x_default { link rel="alternate" hreflang="x-default" href=(url); }
                }
                meta name="robots" content=(if head.index { "index, follow, max-image-preview:large" } else { "noindex, follow" });
                meta name="color-scheme" content="light dark";
                meta name="theme-color" media="(prefers-color-scheme: light)" content="#fbfaf7";
                meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0f1012";
                @if s.favicon.is_empty() && s.logo.is_empty() { link rel="icon" href="/favicon.svg" type="image/svg+xml"; }
                @else { link rel="icon" href=(if s.favicon.is_empty() { &s.logo } else { &s.favicon }); }
                link rel="alternate" type="application/atom+xml" title=(s.title) href=(format!("{}/feed.xml", cx.prefix));
                @if let Some(md) = &head.markdown { link rel="alternate" type="text/markdown" href=(md); }
                meta property="og:type" content=(head.kind);
                meta property="og:site_name" content=(s.title);
                meta property="og:locale" content=(lang::og(&s.lang));
                @if many { @for l in &others { meta property="og:locale:alternate" content=(l); } }
                meta property="og:title" content=(head.headline);
                meta property="og:description" content=(head.description);
                meta property="og:url" content=(head.url);
                meta property="og:image" content=(head.image);
                @if head.card { meta property="og:image:width" content="1200"; meta property="og:image:height" content="630"; }
                meta property="og:image:alt" content=(head.headline);
                @if let Some(t) = head.published { meta property="article:published_time" content=(iso(t)); }
                @if let Some(t) = head.modified { meta property="article:modified_time" content=(iso(t)); }
                @for t in head.tags { meta property="article:tag" content=(t); }
                meta name="twitter:card" content="summary_large_image";
                meta name="twitter:title" content=(head.headline);
                meta name="twitter:description" content=(head.description);
                meta name="twitter:image" content=(head.image);
                script type="application/ld+json" { (PreEscaped(ld)) }
                script { (PreEscaped(style::HEAD_JS)) }
                style { (PreEscaped(style::css(&s.accent, s.font == "sans"))) }
                (PreEscaped(&s.head))
            }
            body {
                a.skip href="#main" { (w.get("skip")) }
                header.top { div.wrap {
                    a.brand href=(format!("{}/", cx.prefix)) {
                        @if s.logo.is_empty() { span.mark aria-hidden="true" { (initial(&s.title)) } }
                        @else { img.logo src=(s.logo) alt="" width="28" height="28"; }
                        (s.title)
                    }
                    nav.nav aria-label=(w.get("site_nav")) {
                        @for p in cx.pages() { a href=(cx.path(p)) { (p.title) } }
                        @for l in &s.links { a href=(l.url) { (l.label) } }
                        @if let Some((label, url)) = cx.project() { a.project href=(url) { (label) " →" } }
                    }
                    button #theme .theme type="button" aria-label=(w.get("theme")) {
                        (PreEscaped(style::SUN)) (PreEscaped(style::MOON))
                    }
                } }
                main #main .wrap { (body) }
                footer.foot { div.wrap {
                    p { strong { (s.title) } @if !s.description.is_empty() { ". " (s.description) } }
                    nav aria-label=(w.get("feeds_nav")) {
                        a href=(format!("{}/feed.xml", cx.prefix)) { (w.get("feed")) }
                        a href="/sitemap.xml" { (w.get("sitemap")) }
                        a href=(format!("{}/llms.txt", cx.prefix)) { "llms.txt" }
                    }
                    @if !s.footer.is_empty() { p { (s.footer) } }
                    @if s.credit { p { (w.get("published_with")) " " a href=(LANTERNLOG_URL) { "Lanternlog" } } }
                } }
                script { (PreEscaped(style::TOGGLE_JS)) }
            }
        }
    }
    .into_string()
}

fn list(cx: &Ctx, posts: &[&Post], nested: bool) -> Markup {
    html! {
        ol.posts { @for p in posts {
            li { article {
                @if let Some(t) = p.published_at { time datetime=(iso(t)) { (cx.words.day(t)) } }
                @if nested { h3 { a href=(cx.path(p)) { (p.title) } } }
                @else { h2 { a href=(cx.path(p)) { (p.title) } } }
                p { (summary(p, &markdown(&p.body))) }
            } }
        } }
        @if posts.is_empty() { p { (cx.words.get("nothing_yet")) } }
    }
}

fn initial(title: &str) -> String {
    title.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default()
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
        "url": cx.root(), "description": description, "inLanguage": s.lang, "publisher": cx.publisher(),
        "blogPost": posts.iter().take(30).map(|p| json!({"@type": "BlogPosting", "headline": p.title, "url": cx.url(p),
            "datePublished": p.published_at.map(iso)})).collect::<Vec<_>>(),
    });
    let head = Head {
        title: home.map_or_else(|| s.title.clone(), |p| p.title.clone()),
        headline: home.map_or(&s.title, |p| &p.title),
        description,
        url: cx.root(),
        image: cx.site_image().0,
        card: cx.site_image().1,
        kind: "website",
        index: true,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld,
        alternates: cx.alternates(None),
    };
    let langs = switcher(cx, &head.alternates);
    let body = html! {
        @if let (Some(p), Some(d)) = (home, &doc) {
            h1 { (p.title) }
            (langs)
            div.prose { (PreEscaped(&d.html)) }
            @if !posts.is_empty() { h2.section { (cx.words.get("from_the_blog")) } (list(cx, &posts, true)) }
        } @else {
            h1 { (s.title) }
            @if !s.description.is_empty() { p.lead { (s.description) } }
            (langs)
            (list(cx, &posts, false))
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
    let (image, card) = match p.og_image.as_str() {
        "" => (format!("{}{}/og/{}.png?v={}", cx.home, cx.prefix, p.slug, p.updated_at), true),
        own => (cx.abs(own), false),
    };
    let mut thing = json!({
        "@type": if page { "WebPage" } else { "BlogPosting" }, "headline": p.title, "name": p.title, "description": description,
        "url": url, "mainEntityOfPage": url, "image": [image], "inLanguage": s.lang, "author": cx.author(), "publisher": cx.publisher(),
        "dateModified": iso(p.updated_at), "wordCount": words, "keywords": p.tags,
        "isPartOf": {"@type": if page { "WebSite" } else { "Blog" }, "name": s.title, "url": cx.root()},
    });
    if let Some(t) = p.published_at {
        thing["datePublished"] = json!(iso(t));
    }
    let crumbs = json!({"@type": "BreadcrumbList", "itemListElement": [
        {"@type": "ListItem", "position": 1, "name": s.title, "item": cx.root()},
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
        card,
        kind: "article",
        index: true,
        published: p.published_at.filter(|_| !page),
        modified: Some(p.updated_at),
        tags: &p.tags,
        markdown: Some(format!("{}{}/{}.md", cx.home, cx.prefix, p.slug)),
        ld,
        alternates: cx.alternates(Some(&p.slug)),
    };
    let w = &cx.words;
    let body = html! {
        @if !page { div.progress aria-hidden="true" {} }
        nav.crumbs aria-label=(w.get("breadcrumb")) { a href=(format!("{}/", cx.prefix)) { "← " (s.title) } }
        article {
            h1 { (p.title) }
            @if !p.description.trim().is_empty() { p.dek { (p.description) } }
            @if !page {
                p.meta {
                    @if let Some(t) = p.published_at { time datetime=(iso(t)) { (w.day(t)) } }
                    @if edited { span aria-hidden="true" { "·" } span { (w.get("updated")) " " time datetime=(iso(p.updated_at)) { (w.day(p.updated_at)) } } }
                    span aria-hidden="true" { "·" } span { (w.min_read(minutes(&doc.text))) }
                    @if !s.author.is_empty() {
                        span aria-hidden="true" { "·" }
                        @if s.author_url.is_empty() { span { (s.author) } } @else { a href=(s.author_url) rel="author" { (s.author) } }
                    }
                }
            }
            // Covers are share-sized (1200×630); the box is reserved from these, so the text under it never jumps (PageSpeed CLS).
            (switcher(cx, &head.alternates))
            @if !p.cover.is_empty() { img.cover src=(p.cover) alt=(p.title) width="1200" height="630" fetchpriority="high"; }
            @if doc.toc.len() >= 3 {
                details.toc open {
                    summary { (w.get("on_this_page")) }
                    ol { @for (level, id, text) in &doc.toc { li class=(format!("l{level}")) { a href=(format!("#{id}")) { (text) } } } }
                }
            }
            div.prose { (PreEscaped(&doc.html)) }
            @if !p.tags.is_empty() {
                ul.tags { @for t in &p.tags { li { a href=(format!("{}/tags/{}/", cx.prefix, slugify(t))) { "#" (t) } } } }
            }
        }
        @if let Some((label, url)) = cx.project() {
            aside.cta {
                @if !s.cta.is_empty() { p { (s.cta) } }
                a.button href=(url) { (label) " →" }
            }
        }
        @if !page && !related.is_empty() {
            section { h2.section { (w.get("read_next")) } (list(cx, &related, true)) }
        }
    };
    layout(cx, head, body)
}

/// The posts carrying a tag, or None when no post does.
pub fn tag(cx: &Ctx, slug: &str) -> Option<String> {
    let s = cx.site;
    let posts: Vec<&Post> = cx.articles().filter(|p| p.tags.iter().any(|t| slugify(t) == slug)).collect();
    let name = posts.first()?.tags.iter().find(|t| slugify(t) == slug)?.clone();
    let url = format!("{}tags/{slug}/", cx.root());
    let head = Head {
        title: format!("#{name} · {}", s.title),
        headline: &name,
        description: cx.words.get("tag_description").replace("{tag}", &name).replace("{site}", &s.title),
        url: url.clone(),
        image: cx.site_image().0,
        card: cx.site_image().1,
        kind: "website",
        index: false,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld: json!({"@context": "https://schema.org", "@type": "CollectionPage", "name": format!("#{name}"), "url": url, "inLanguage": s.lang}),
        alternates: Vec::new(),
    };
    let crumbs = html! { nav.crumbs aria-label=(cx.words.get("breadcrumb")) { a href=(format!("{}/", cx.prefix)) { "← " (s.title) } } };
    Some(layout(cx, head, html! { (crumbs) h1 { "#" (name) } (list(cx, &posts, false)) }))
}

pub fn not_found(cx: &Ctx) -> String {
    let s = cx.site;
    let latest: Vec<&Post> = cx.articles().take(5).collect();
    let head = Head {
        title: format!("{} · {}", cx.words.get("not_found"), s.title),
        headline: cx.words.get("not_found"),
        description: s.description.clone(),
        url: cx.root(),
        image: cx.site_image().0,
        card: cx.site_image().1,
        kind: "website",
        index: false,
        published: None,
        modified: None,
        tags: &[],
        markdown: None,
        ld: json!({}),
        alternates: Vec::new(),
    };
    layout(cx, head, html! { h1 { (cx.words.get("not_found")) } p.lead { (cx.words.get("not_found_lead")) } (list(cx, &latest, false)) })
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
        home = format!("{}{}", cx.home, cx.prefix),
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

/// Every address of the blog in every language; addresses that exist in several languages list them all.
pub fn sitemap(cx: &Ctx) -> String {
    let latest = cx.versions.values().flatten().map(|(_, t)| *t).max().unwrap_or(cx.site.updated_at).max(cx.site.updated_at);
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n",
    );
    let mut entry = |loc: &str, lastmod: i64, alternates: &[(String, String)]| {
        x += &format!("<url><loc>{loc}</loc><lastmod>{}</lastmod>", iso(lastmod));
        if alternates.len() > 1 {
            for (code, url) in alternates {
                x += &format!("<xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{url}\"/>", esc(code));
            }
            if let Some((_, url)) = alternates.first() {
                x += &format!("<xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{url}\"/>");
            }
        }
        x += "</url>\n";
    };
    let homes = cx.alternates(None);
    for (code, url) in &homes {
        let mine = cx.versions.values().flatten().filter(|(l, _)| l == code).map(|(_, t)| *t).max().unwrap_or(latest);
        entry(url, if lang::same(code, &cx.main) { latest } else { mine.max(cx.site.updated_at) }, &homes);
    }
    for (slug, versions) in cx.versions.iter().filter(|(slug, _)| **slug != cx.site.home) {
        let alternates = cx.alternates(Some(slug));
        for ((_, url), (_, updated)) in alternates.iter().zip(versions) {
            entry(url, *updated, &alternates);
        }
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
    let others: Vec<(String, String)> = cx.alternates(None).into_iter().filter(|(l, _)| !lang::same(l, &cx.site.lang)).collect();
    for (heading, kind) in [("Posts", false), ("Pages", true)] {
        let items: Vec<&Post> = cx.posts.iter().filter(|p| (p.kind == "page") == kind).collect();
        if items.is_empty() {
            continue;
        }
        x += &format!("## {heading}\n\n");
        for p in items {
            x += &format!("- [{}]({}{}/{}.md): {}\n", p.title, cx.home, cx.prefix, p.slug, summary(p, &markdown(&p.body)));
        }
        x += "\n";
    }
    if !others.is_empty() {
        x += "## Other languages\n\n";
        for (code, url) in others {
            x += &format!("- [{}]({url}llms.txt): this blog in {code}, at {url}\n", cx.lang_name(&code));
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
        Some(t) if p.kind != "page" => x += &format!("Published {} on {}: {}\n\n", cx.words.day(t), cx.site.title, cx.url(p)),
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
