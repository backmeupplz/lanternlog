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
| `LANTERNLOG_FONTS` | none | An extra folder of fonts for share images (Inter, and Noto for other scripts, are in the Docker image) |

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
| `lang` | The language tag of the blog, such as `en` or `pt-BR`; its posts live at the root, and their translations in folders such as `/de/` |
| `locales` | What changes in each language the blog is translated into: see [Translate posts](#translate-posts) |
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

## Translate posts

A post can be read in other languages too. Each translation has its own address in a folder named after its language, such as `/de/one-server/` or `/pt-br/one-server/`, and each language gets a home page of its own at `/de/` that lists only the posts translated into it. Nothing redirects by browser language, so search engines can index every version.

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com/posts/one-server/translations/de \
  -H "Authorization: Bearer $LANTERNLOG_TOKEN" -H "Content-Type: application/json" -d '{
    "title": "Umzug auf einen Server",
    "description": "Warum alles auf einer Maschine läuft, und was es gekostet hat.",
    "body": "Wir haben **jeden Dienst** auf eine Kiste gezogen.\n\n...",
    "tags": ["Selbst hosten"],
    "status": "published"
  }'
```

A translation takes the same fields as a post (`title`, `description`, `body`, `tags`, `cover`, `og_image`, `status`, `published_at`) and keeps the original's slug and kind. It shows once both it and the original are published, it keeps the original's publication date unless you give it one, and it goes when the original is deleted. The same call also works as a `PUT` to the post's own address with `"lang": "de"` in the body (and, optionally, `"translation_of": "one-server"`, which must be that same slug). Language codes are lowercase, as in `de`, `pt-br`, `zh-hans` or `zh-hant`, and can't be a slug one of the blog's posts already uses.

Every version of a page names its language in `<html lang>` (with `dir="rtl"` for Arabic, Persian, Hebrew and Urdu), has its own canonical address, links every other version with `hreflang` and an `x-default` that points at the original, and carries `og:locale`, `og:locale:alternate` and `inLanguage` to match. The sitemap lists each version with its alternates, each language has its own feed at `/{lang}/feed.xml` and `llms.txt` at `/{lang}/llms.txt` (the blog's own `llms.txt` links to them), publishing a translation tells IndexNow about it, share cards are drawn from the translated title in that language's script, and every post that exists in several languages links to the others by their own names.

The blog's chrome (its title, call to action, header links and the words around posts) comes from the blog's `locales`, one entry per language. Anything a language leaves out stays as the blog has it, in English:

```json
"locales": {
  "de": {
    "title": "Example Notizen",
    "description": "Was wir beim Betrieb von Example lernen.",
    "cta": "Example bringt eine App mit einem Befehl online.",
    "project_url": "https://example.com/de/",
    "links": [{"label": "Preise", "url": "https://example.com/de/pricing"}],
    "strings": {"read_next": "Weiterlesen", "on_this_page": "Auf dieser Seite", "min_read": "{n} Min. Lesezeit"}
  }
}
```

| Field | What it does |
|---|---|
| `name` | The language's name in the switcher, when the built-in one (`Deutsch`, `简体中文`...) doesn't suit |
| `title`, `description`, `project_name`, `project_url`, `cta`, `footer` | The blog's own fields, in this language |
| `links` | The header links for this language, in place of the blog's |
| `strings` | The words around posts: `skip`, `theme`, `site_nav`, `breadcrumb`, `on_this_page`, `read_next`, `from_the_blog`, `min_read` (`{n}` is the number of minutes), `updated`, `nothing_yet`, `not_found`, `not_found_lead`, `tag_description` (with `{tag}` and `{site}`), `languages`, `feeds_nav`, `feed`, `sitemap`, `published_with`, and `date`, a [strftime](https://docs.rs/chrono/latest/chrono/format/strftime/) format whose month names come out in the language (each language has a sensible one already) |

## API reference

| Call | What it does |
|---|---|
| `GET /api/sites` | All blogs |
| `GET`, `PUT`, `DELETE /api/sites/{host}` | One blog; `PUT` creates or replaces its settings |
| `GET /api/sites/{host}/posts` | Its posts and pages, drafts included |
| `GET`, `PUT`, `DELETE /api/sites/{host}/posts/{slug}` | One post; `PUT` creates or replaces it, and with a `lang` other than the blog's it writes a translation instead |
| `GET /api/sites/{host}/posts/{slug}/translations` | The post's translations, drafts included |
| `GET`, `PUT`, `DELETE /api/sites/{host}/posts/{slug}/translations/{lang}` | One translation; `PUT` creates or replaces it |
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
| `/{lang}/`, `/{lang}/{slug}/` | A language's home page and its translations, with `.md` copies, `feed.xml`, `llms.txt`, `llms-full.txt`, tag pages and share cards under the same folder |
