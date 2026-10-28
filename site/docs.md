Everything Lanternlog does is set through environment variables and a small HTTP API. This page covers both.

## Install

With Docker:

```sh
git clone https://github.com/backmeupplz/lanternlog && cd lanternlog
docker build -t lanternlog .
docker run -d --name lanternlog -p 8080:8080 -v lanternlog:/data \
  -e LANTERNLOG_TOKEN="$(openssl rand -hex 32)" lanternlog
```

From source, with Rust installed:

```sh
cargo build --release
LANTERNLOG_TOKEN=change-me ./target/release/lanternlog
```

Point each blog's domain at the server and put a TLS proxy in front of it; any proxy that passes the `Host` header works. On [Tugport](https://tugport.com), deploy the repository as one service with a volume at `/data`, set `LANTERNLOG_TOKEN`, and add every blog's domain to the service. Tugport issues the certificates and deploys each push without downtime.

## Settings

| Variable | Default | What it does |
|---|---|---|
| `PORT` | `8080` | Port to listen on |
| `LANTERNLOG_DATA` | `./data` | Folder for `lanternlog.db` and uploaded images |
| `LANTERNLOG_TOKEN` | none | Bearer token for the admin API; the API is off without it |
| `LANTERNLOG_SCHEME` | `https` | Scheme used in addresses in pages and feeds |
| `LANTERNLOG_INDEXNOW` | `on` | `off` stops IndexNow pings |
| `LANTERNLOG_FONTS` | none | An extra folder of fonts for share images (Inter is in the Docker image) |

## Create a blog

Every API call needs `Authorization: Bearer $LANTERNLOG_TOKEN` and can go to any of the server's domains.

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com \
  -H "Authorization: Bearer $LANTERNLOG_TOKEN" -H "Content-Type: application/json" -d '{
    "title": "Example Notes",
    "description": "What we learn running Example in production.",
    "aliases": ["www.blog.example.com"],
    "author": "Jane Doe", "author_url": "https://jane.example",
    "project_name": "Example", "project_url": "https://example.com",
    "cta": "Example deploys an app with one command.",
    "accent": "#e4572e", "lang": "en"
  }'
```

| Field | What it does |
|---|---|
| `title`, `description` | The blog's name and one-sentence summary, used in titles, feeds, `llms.txt` and the home page |
| `aliases` | Other domains that redirect here, such as `www.` |
| `author`, `author_url` | The person in every post's structured data; without one, the blog itself is the author |
| `project_name`, `project_url`, `cta` | The product the blog writes about: linked in the header and in a call to action after every post |
| `accent` | A color such as `#e4572e` for links, buttons and share images |
| `lang` | The language tag of the blog, such as `en` or `pt-BR` |
| `home` | The slug of a page to show at `/` instead of the plain list of posts, with the latest posts below it |
| `indexnow_key` | The IndexNow key the blog serves at `/<key>.txt` and pings with. Optional: a new blog gets a random one, and leaving it out keeps the current one. Set it to share a key with other tools that tell IndexNow about the same domain |

### Your own brand

A blog can look like your product rather than like Lanternlog. All of these are optional.

| Field | What it does |
|---|---|
| `logo` | An image shown next to the blog's name in the header and drawn into every share card. Upload it with the media call first, as a PNG, JPEG or GIF, so the cards can use it |
| `favicon` | The browser icon; without one, the logo is used, and without a logo, a generated icon |
| `og_image` | The share image for the home page and other pages without their own; without one, Lanternlog draws a card |
| `links` | Up to 8 header links, such as `[{"label": "Pricing", "url": "https://example.com/pricing"}]` |
| `head` | HTML added to every page's head, such as a search console verification tag or an analytics snippet |
| `footer` | A line of text for the footer, such as a copyright notice |
| `font` | `serif` (the default) or `sans` for the text of posts |
| `credit` | `false` removes "Published with Lanternlog" from the footer |

A post can also set `og_image` to use a share image of its own instead of the generated card.

## Publish posts and pages

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com/posts/one-server \
  -H "Authorization: Bearer $LANTERNLOG_TOKEN" -H "Content-Type: application/json" -d '{
    "title": "Moving to one server",
    "description": "Why we put every service on one machine, and what it cost.",
    "body": "We moved **every service** to one box.\n\n## Why one box\n\n...",
    "tags": ["Self-hosting"],
    "status": "published"
  }'
```

The slug comes from the address. A post stays a draft until its `status` is `published`, and its first publication date is kept when you edit it later. Set `"kind": "page"` for pages such as About: they appear in the header rather than in the post list and the feed. `cover` takes an image address shown at the top of the post, and `og_image` replaces the post's generated share card.

Posts are Markdown with tables, footnotes, task lists, smart quotes and heading ids (`## Setup {#setup}`). Raw HTML passes through untouched, so only give the token to writers you trust.

## API reference

| Call | What it does |
|---|---|
| `GET /api/sites` | All blogs |
| `GET`, `PUT`, `DELETE /api/sites/{host}` | One blog; `PUT` creates or replaces its settings |
| `GET /api/sites/{host}/posts` | Its posts and pages, drafts included |
| `GET`, `PUT`, `DELETE /api/sites/{host}/posts/{slug}` | One post; `PUT` creates or replaces it |
| `POST /api/sites/{host}/media` | Uploads an image (the raw bytes with their `Content-Type`) and answers with its address |
| `GET /api/health` | `{"ok": true, "version"}` |

## What each blog serves

| Address | What it is |
|---|---|
| `/` | The home page: the latest posts, or the page named in `home` |
| `/{slug}/` | A post or page; `/{slug}` redirects here |
| `/{slug}.md` | The same post as Markdown, for AI assistants |
| `/tags/{tag}/` | Posts with a tag (kept out of search results) |
| `/feed.xml` | An Atom feed with the full text of the latest 50 posts |
| `/sitemap.xml`, `/robots.txt` | For search engines |
| `/llms.txt`, `/llms-full.txt` | A map of the blog for AI assistants, and every post in one file |
| `/og/{slug}.png`, `/og.png` | Share images for each post and for the blog |
| `/favicon.svg` | A favicon made from the blog's initial and accent color |
