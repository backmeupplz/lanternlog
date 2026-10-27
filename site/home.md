Lanternlog is one small server that hosts many blogs, each on its own domain. Every post goes out as plain, fast HTML with the structured data, share images, feeds and Markdown copies that Google, Bing and AI assistants use to find a page, understand it and cite it. You publish through a short HTTP API, so a script or an AI agent can run a blog as easily as a person can.

<p><a class="button" href="https://github.com/backmeupplz/lanternlog">Get Lanternlog on GitHub</a></p>

## What every blog gets

- **Pages that render in the HTML.** No JavaScript is needed to read a post, so crawlers that don't run scripts, which includes most AI crawlers, see every word.
- **Structured data on every page.** Posts carry `BlogPosting` with the author, dates, word count and image, plus a `BreadcrumbList`; the home page carries `Blog` or `WebSite`.
- **A share image for every post.** Each post gets a 1200×630 card drawn from its title, and the Open Graph and Twitter tags point at it.
- **Files for crawlers and assistants.** Each blog serves `sitemap.xml`, an Atom feed with full posts, `robots.txt`, [`llms.txt`](https://llmstxt.org), `llms-full.txt` and a Markdown copy of every page at `/slug.md`.
- **IndexNow on publish.** Publishing a post tells Bing, Yandex and the other IndexNow engines about it right away; Google reads the sitemap.
- **Canonical addresses.** Extra domains and `www.` redirect to the main domain, and paths without a trailing slash redirect to the ones with it.
- **A way back to your product.** Each blog names the project it writes about, links it in the header, and ends every post with a call to action.
- **An outline for long posts.** Headings get stable ids, and posts with three or more headings get a table of contents.

## Small enough to forget about

Lanternlog is a single 8 MB Rust binary with an SQLite file. The instance serving this page uses about 8 MB of memory with its share images drawn, and it renders each page once, then serves it from memory with gzip and an ETag until something changes. One server can hold every blog you run: point their domains at it and create each one with a single API call.

## Run it

```sh
git clone https://github.com/backmeupplz/lanternlog && cd lanternlog
docker build -t lanternlog .
docker run -d --name lanternlog -p 8080:8080 -v lanternlog:/data \
  -e LANTERNLOG_TOKEN="$(openssl rand -hex 32)" lanternlog
```

Put a TLS proxy in front of it that passes the `Host` header. On [Tugport](https://tugport.com), deploy the repository as one service and add every blog's domain to it; Tugport issues the certificates.

## Publish a post

```sh
curl -X PUT https://blog.example.com/api/sites/blog.example.com/posts/hello \
  -H "Authorization: Bearer $LANTERNLOG_TOKEN" -H "Content-Type: application/json" \
  -d '{"title": "Hello", "body": "Written in **Markdown**.", "status": "published"}'
```

The [docs](/docs/) cover every setting and API call.

## When to use something else

Lanternlog has no web editor, comments, newsletters or paid memberships, and its only theme setting is an accent color. If you need those, Ghost and WordPress do them well. If your site is mostly custom pages rather than posts, a static site generator gives you more control. Lanternlog is for people and agents who publish a lot of posts across several products and want every one of them to be easy to find.

Lanternlog is free and open source under the MIT license. It's made by [Nikita Kolmogorov](https://borodutch.com), who also makes [Tugport](https://tugport.com).
