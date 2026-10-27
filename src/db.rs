//! Sites and posts in one SQLite file. Posts keep their Markdown; pages are rendered on request
//! and cached until the next write.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS sites (
  host TEXT PRIMARY KEY,
  aliases TEXT NOT NULL DEFAULT '[]',
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  lang TEXT NOT NULL DEFAULT 'en',
  author TEXT NOT NULL DEFAULT '',
  author_url TEXT NOT NULL DEFAULT '',
  project_name TEXT NOT NULL DEFAULT '',
  project_url TEXT NOT NULL DEFAULT '',
  cta TEXT NOT NULL DEFAULT '',
  accent TEXT NOT NULL DEFAULT '#2563eb',
  home TEXT NOT NULL DEFAULT '',
  indexnow_key TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS posts (
  host TEXT NOT NULL REFERENCES sites(host) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'post',
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',
  cover TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'draft',
  published_at INTEGER,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (host, slug)
);
";

/// A blog. `host` is its canonical domain; `aliases` redirect to it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Site {
    #[serde(skip_deserializing)]
    pub host: String,
    pub aliases: Vec<String>,
    pub title: String,
    pub description: String,
    pub lang: String,
    pub author: String,
    pub author_url: String,
    /// The product this blog sends readers to, shown in the header and after every post.
    pub project_name: String,
    pub project_url: String,
    pub cta: String,
    pub accent: String,
    /// The slug of a page to show as the home page, above the latest posts.
    pub home: String,
    /// The brand, for blogs that shouldn't look like Lanternlog: a logo in place of the initial, a favicon,
    /// a share image for pages without their own, header links, tags for the head, footer text, the font
    /// for posts ("serif" or "sans") and whether the footer credits Lanternlog.
    pub logo: String,
    pub favicon: String,
    pub og_image: String,
    pub links: Vec<Link>,
    pub head: String,
    pub footer: String,
    pub font: String,
    #[serde(default = "yes")]
    pub credit: bool,
    #[serde(skip_deserializing)]
    pub indexnow_key: String,
    #[serde(skip_deserializing)]
    pub updated_at: i64,
}

/// A post, or a page when `kind` is "page" (no date, not in the feed).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Post {
    #[serde(skip_deserializing)]
    pub slug: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub tags: Vec<String>,
    pub cover: String,
    /// A share image of its own instead of the generated card.
    pub og_image: String,
    pub status: String,
    pub published_at: Option<i64>,
    #[serde(skip_deserializing)]
    pub updated_at: i64,
}

/// Columns added after the first release, so older databases gain them on start.
const COLUMNS: &[(&str, &str, &str)] = &[
    ("sites", "logo", "TEXT NOT NULL DEFAULT ''"),
    ("sites", "favicon", "TEXT NOT NULL DEFAULT ''"),
    ("sites", "og_image", "TEXT NOT NULL DEFAULT ''"),
    ("sites", "font", "TEXT NOT NULL DEFAULT 'serif'"),
    ("sites", "links", "TEXT NOT NULL DEFAULT '[]'"),
    ("sites", "head", "TEXT NOT NULL DEFAULT ''"),
    ("sites", "footer", "TEXT NOT NULL DEFAULT ''"),
    ("sites", "credit", "INTEGER NOT NULL DEFAULT 1"),
    ("posts", "og_image", "TEXT NOT NULL DEFAULT ''"),
];

pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.execute_batch(SCHEMA)?;
    for (table, column, kind) in COLUMNS {
        if !c.prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"))?.exists([column])? {
            c.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind}"))?;
        }
    }
    Ok(c)
}

fn yes() -> bool {
    true
}

/// A link in a blog's header, such as the product's pricing page.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Link {
    pub label: String,
    pub url: String,
}

fn json<T: for<'de> Deserialize<'de> + Default>(s: String) -> T {
    serde_json::from_str(&s).unwrap_or_default()
}

fn site_row(r: &Row) -> rusqlite::Result<Site> {
    Ok(Site {
        host: r.get("host")?,
        aliases: json(r.get("aliases")?),
        title: r.get("title")?,
        description: r.get("description")?,
        lang: r.get("lang")?,
        author: r.get("author")?,
        author_url: r.get("author_url")?,
        project_name: r.get("project_name")?,
        project_url: r.get("project_url")?,
        cta: r.get("cta")?,
        accent: r.get("accent")?,
        home: r.get("home")?,
        logo: r.get("logo")?,
        favicon: r.get("favicon")?,
        og_image: r.get("og_image")?,
        links: json(r.get("links")?),
        head: r.get("head")?,
        footer: r.get("footer")?,
        font: r.get("font")?,
        credit: r.get("credit")?,
        indexnow_key: r.get("indexnow_key")?,
        updated_at: r.get("updated_at")?,
    })
}

fn post_row(r: &Row) -> rusqlite::Result<Post> {
    Ok(Post {
        slug: r.get("slug")?,
        kind: r.get("kind")?,
        title: r.get("title")?,
        description: r.get("description")?,
        body: r.get("body")?,
        tags: json(r.get("tags")?),
        cover: r.get("cover")?,
        og_image: r.get("og_image")?,
        status: r.get("status")?,
        published_at: r.get("published_at")?,
        updated_at: r.get("updated_at")?,
    })
}

/// The site served at `host`, whether that's its domain or one of its aliases.
pub fn site(c: &Connection, host: &str) -> rusqlite::Result<Option<Site>> {
    c.query_row(
        "SELECT * FROM sites WHERE host = ?1 OR EXISTS (SELECT 1 FROM json_each(sites.aliases) WHERE value = ?1)",
        [host],
        site_row,
    )
    .optional()
}

pub fn sites(c: &Connection) -> rusqlite::Result<Vec<Site>> {
    c.prepare("SELECT * FROM sites ORDER BY host")?.query_map([], site_row)?.collect()
}

pub fn put_site(c: &Connection, s: &Site) -> rusqlite::Result<()> {
    let key = format!("{:032x}", rand::random::<u128>());
    c.execute(
        "INSERT INTO sites (host, aliases, title, description, lang, author, author_url, project_name, project_url, cta, accent, home,
           logo, favicon, og_image, links, head, footer, font, credit, indexnow_key, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
         ON CONFLICT (host) DO UPDATE SET aliases = ?2, title = ?3, description = ?4, lang = ?5, author = ?6, author_url = ?7,
           project_name = ?8, project_url = ?9, cta = ?10, accent = ?11, home = ?12, logo = ?13, favicon = ?14, og_image = ?15,
           links = ?16, head = ?17, footer = ?18, font = ?19, credit = ?20, updated_at = ?22",
        params![
            s.host,
            serde_json::to_string(&s.aliases).unwrap_or_default(),
            s.title,
            s.description,
            s.lang,
            s.author,
            s.author_url,
            s.project_name,
            s.project_url,
            s.cta,
            s.accent,
            s.home,
            s.logo,
            s.favicon,
            s.og_image,
            serde_json::to_string(&s.links).unwrap_or_default(),
            s.head,
            s.footer,
            s.font,
            s.credit,
            key,
            s.updated_at
        ],
    )?;
    Ok(())
}

pub fn delete_site(c: &Connection, host: &str) -> rusqlite::Result<bool> {
    Ok(c.execute("DELETE FROM sites WHERE host = ?1", [host])? > 0)
}

/// Newest first. Drafts are left out unless `drafts` is set.
pub fn posts(c: &Connection, host: &str, drafts: bool) -> rusqlite::Result<Vec<Post>> {
    c.prepare(
        "SELECT * FROM posts WHERE host = ?1 AND (?2 OR status = 'published')
         ORDER BY COALESCE(published_at, updated_at) DESC, slug",
    )?
    .query_map(params![host, drafts], post_row)?
    .collect()
}

pub fn post(c: &Connection, host: &str, slug: &str) -> rusqlite::Result<Option<Post>> {
    c.query_row("SELECT * FROM posts WHERE host = ?1 AND slug = ?2", [host, slug], post_row).optional()
}

pub fn put_post(c: &Connection, host: &str, p: &Post) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO posts (host, slug, kind, title, description, body, tags, cover, status, published_at, updated_at, og_image)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT (host, slug) DO UPDATE SET kind = ?3, title = ?4, description = ?5, body = ?6, tags = ?7, cover = ?8,
           status = ?9, published_at = ?10, updated_at = ?11, og_image = ?12",
        params![
            host,
            p.slug,
            p.kind,
            p.title,
            p.description,
            p.body,
            serde_json::to_string(&p.tags).unwrap_or_default(),
            p.cover,
            p.status,
            p.published_at,
            p.updated_at,
            p.og_image
        ],
    )?;
    Ok(())
}

pub fn delete_post(c: &Connection, host: &str, slug: &str) -> rusqlite::Result<bool> {
    Ok(c.execute("DELETE FROM posts WHERE host = ?1 AND slug = ?2", [host, slug])? > 0)
}
