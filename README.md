# Tugpress

Tugpress is one small server that hosts many blogs, each on its own domain, and makes every page easy for search engines and AI assistants to find, read and cite. It is a single Rust binary with an SQLite file, and you publish to it through a small HTTP API, which suits scripts and AI agents as well as people.

## What every blog gets

- **Pages that render in the HTML.** No JavaScript is needed to read a post, so crawlers that don't run scripts, including most AI crawlers, see everything.
- **Structured data on every page:** `BlogPosting` or `WebPage` with author, dates, word count and image, plus `BreadcrumbList` on posts and `Blog` on the home page.
- **A share image for every post,** a 1200×630 card drawn from the post's title, with Open Graph and Twitter tags pointing at it.
- **Canonical addresses.** Extra domains and `www.` redirect to the main one, and paths without a trailing slash redirect to the one with it.
- **Files for crawlers and assistants:** `sitemap.xml`, an Atom feed at `feed.xml` with full posts, `robots.txt`, [`llms.txt`](https://llmstxt.org) and `llms-full.txt`, and a Markdown copy of every page at `/slug.md`.
- **IndexNow.** Each blog has its own key, and publishing a post tells Bing, Yandex and the other IndexNow engines about it within seconds. Google reads the sitemap.
- **An outline for long posts.** Headings get stable ids, and posts with three or more get a table of contents.
- **A way back to your product.** Each blog names the project it writes about, and every post ends with a call to action that links to it.
- **Speed.** Pages are rendered once and served from memory, gzipped and with ETags, until the next write.

## Run it

With Docker:

```sh
docker build -t tugpress .
docker run -d --name tugpress -p 8080:8080 -v tugpress:/data -e TUGPRESS_TOKEN="$(openssl rand -hex 32)" tugpress
```

From source, with Rust installed:

```sh
cargo build --release
TUGPRESS_TOKEN=change-me ./target/release/tugpress
```

Point each blog's domain at the server, and put a TLS proxy in front of it (any proxy that passes the `Host` header works). On [Tugport](https://tugport.com), deploy the repository as one service and add every blog's domain to it; Tugport issues the certificates.

| Variable | Default | What it does |
|---|---|---|
| `PORT` | `8080` | Port to listen on |
| `TUGPRESS_DATA` | `./data` | Folder for `tugpress.db` and uploaded images |
| `TUGPRESS_TOKEN` | none | Bearer token for the admin API; the API is off without it |
| `TUGPRESS_SCHEME` | `https` | Scheme used in addresses in pages and feeds |
| `TUGPRESS_INDEXNOW` | `on` | `off` stops IndexNow pings |
| `TUGPRESS_FONTS` | none | Extra folder of fonts for share images (Inter is in the Docker image) |

## Publish

Every call needs `Authorization: Bearer $TUGPRESS_TOKEN`, and can go to any of the server's domains.

Create or change a blog:

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com \
  -H "Authorization: Bearer $TUGPRESS_TOKEN" -H "Content-Type: application/json" -d '{
    "title": "Example Notes",
    "description": "What we learn running Example in production.",
    "aliases": ["www.blog.example.com"],
    "author": "Jane Doe", "author_url": "https://jane.example",
    "project_name": "Example", "project_url": "https://example.com",
    "cta": "Example deploys an app with one command.",
    "accent": "#e4572e", "lang": "en"
  }'
```

Publish a post (the slug comes from the address; `status` is `draft` until you say `published`):

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com/posts/one-server \
  -H "Authorization: Bearer $TUGPRESS_TOKEN" -H "Content-Type: application/json" -d '{
    "title": "Moving to one server",
    "description": "Why we put every service on one machine, and what it cost.",
    "body": "We moved **every service** to one box.\n\n## Why one box\n\n...",
    "tags": ["Self-hosting"],
    "status": "published"
  }'
```

The answer holds the post's address. Set `"kind": "page"` for pages such as About: they appear in the header instead of the post list and the feed.

| Call | What it does |
|---|---|
| `GET /api/sites` | All blogs |
| `GET`, `PUT`, `DELETE /api/sites/{host}` | One blog; `PUT` creates or replaces its settings |
| `GET /api/sites/{host}/posts` | Its posts and pages, drafts included |
| `GET`, `PUT`, `DELETE /api/sites/{host}/posts/{slug}` | One post; `PUT` creates or replaces it |
| `POST /api/sites/{host}/media` | Uploads an image (the raw bytes, with its `Content-Type`) and answers with its address |
| `GET /api/health` | `{"ok": true, "version"}` |

Posts are Markdown with tables, footnotes, task lists and heading ids (`## Setup {#setup}`). Raw HTML passes through, so only give the token to writers you trust.

## What it leaves out

Tugpress has no web editor, comments, newsletters or themes beyond an accent color. Posts come in through the API, and anything that needs a browser is meant to live on your product's own site.

## License

MIT
