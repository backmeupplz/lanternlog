# Publishes lanternlog.com's own pages through the Lanternlog API. Run with the admin token in $KEY:
#   KEY=... python3 site/publish.py
import json, os, pathlib, urllib.request, urllib.error

HERE = pathlib.Path(__file__).parent
API = "https://lanternlog.com/api/sites/lanternlog.com"


def put(path, body):
    req = urllib.request.Request(API + path, data=json.dumps(body).encode(), method="PUT",
                                 headers={"Authorization": "Bearer " + os.environ["KEY"].strip(), "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.load(r)
    except urllib.error.HTTPError as e:
        return e.code, json.load(e)


site = {
    "title": "Lanternlog",
    "description": "One small, open-source server for many blogs, built so search engines and AI assistants can read every post.",
    "aliases": ["www.lanternlog.com"],
    "project_name": "Lanternlog on GitHub",
    "project_url": "https://github.com/backmeupplz/lanternlog",
    "cta": "Lanternlog is free and open source under the MIT license.",
    "accent": "#b45309",
    "lang": "en",
    "home": "home",
}
pages = {
    "home": {"title": "Lanternlog: blogs that search engines and AI assistants can read",
             "description": "An open-source server that hosts many blogs on their own domains and makes every post easy for search engines and AI assistants to find and cite.",
             "body": (HERE / "home.md").read_text(), "kind": "page", "status": "published"},
    "docs": {"title": "Docs", "description": "How to install Lanternlog, create blogs on it and publish posts through its API.",
             "body": (HERE / "docs.md").read_text(), "kind": "page", "status": "published"},
}
req = urllib.request.Request(API + "/media", data=(HERE / "logo.png").read_bytes(), method="POST",
                             headers={"Authorization": "Bearer " + os.environ["KEY"].strip(), "Content-Type": "image/png"})
try:
    with urllib.request.urlopen(req, timeout=60) as r:  # images are stored by content, so this is safe to repeat
        site["logo"] = "/media/" + json.load(r)["url"].rsplit("/media/", 1)[1]
except urllib.error.HTTPError as e:
    print("logo", e.code)
for slug, page in pages.items():  # pages first, so the home setting points at a page that exists
    st, r = put(f"/posts/{slug}", page)
    print("page", slug, st, r.get("url") or r)
st, r = put("", site)
print("site", st, r.get("host") or r)
