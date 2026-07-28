# andreadimatteo

Hi, this is Andrea again.

I'm kind of learning Rust, I thought the best way to do so was getting into a
project. A markdown to static HTML compiler is a very simple project I've had in
mind for a while, so here it is — and it's the thing that actually builds
[andreadimatteo.com](https://andreadimatteo.com).

No dependencies beyond a handful of crates, no JS framework, no generator. You
write `.md` files in `posts/`, run the compiler, and get a `dist/` folder full of
plain HTML that GitHub Pages serves as is.

## Layout

```
posts/      the markdown sources, one file per post
static/     css, images, gifs — copied verbatim to dist/static/
templates/  head.html, topbar.html, body.html (post page), index.html (post list)
src/        the compiler
dist/       build output, gitignored, regenerated from scratch on every run
```

## Build

```sh
cargo run --release
```

That reads every `.md` in `posts/`, writes one HTML file per post plus
`dist/index.html` (the post list, sorted by date descending), then copies
`static/` into `dist/static/`.

To look at it locally, serve `dist/` over HTTP rather than opening the file
directly:

```sh
python3 -m http.server -d dist 8000   # then http://localhost:8000
```

## Makefile

`make` on its own prints the same summary:

| command | what it does |
| --- | --- |
| `make help` | list the available targets (default goal) |
| `make post` | prompt for title, date, description and tags, then create `posts/<slug>.md` with the front matter already filled in |
| `make gifs` | convert every `.mp4` under `static/` to a `.gif` next to it, with ffmpeg |

`make post` slugifies the title for the filename, defaults the date to today if
you just hit enter, and refuses to overwrite an existing file.

`make gifs` needs ffmpeg on `PATH` (`brew install ffmpeg`). It generates a
per-file palette at 25 fps, so the gifs come out reasonably small. The `.mp4`
originals are gitignored — only the generated `.gif` gets committed and shipped,
which is why the conversion happens locally and not in CI.

## Writing a post

Every post starts with a YAML front matter block delimited by `---`:

```markdown
---
title: Building a markdown compiler
date: 2026-07-27
description: A short line shown in the index
tags: [rust, web]
draft: false
---

Body starts here.
```

`title` and `date` are required, the rest are optional. `date` is `YYYY-MM-DD`
and drives the ordering of the index. Setting `draft: true` skips the post
entirely — it won't be compiled or listed.

Supported markdown: headings `#` through `######`, paragraphs, fenced code
blocks (``` or ~~~), unordered lists (`-`, `*`, `+`), ordered lists (`1.` or
`1)`), blockquotes (`>`), and the inline forms `**bold**`, `*italic*`,
`` `code` ``, `~strikethrough~` (single tilde), `[text](url)` and
`![alt](file.png)`.

Images resolve to `static/<file>`, so `![demo](demo.gif)` refers to
`static/demo.gif`. Drop the file in `static/` and reference it by bare filename.

## Deployment

`.github/workflows/deploy.yml` runs on every push to `master`. It builds the
site with `cargo run --release`, writes a `CNAME` into `dist/`, and publishes the
folder to GitHub Pages. Nothing generated is committed — `dist/` stays
gitignored and CI rebuilds it from the sources every time.

The site ends up at `https://andreadimatteo.com/index.html`. For that to
resolve, two things have to be set up once, outside this repo:

1. Repo settings — under `Settings → Pages`, set the source to `GitHub Actions`
   and the custom domain to `andreadimatteo.com`.
2. DNS, at whoever holds the domain — apex `A` records pointing to
   `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, `185.199.111.153`
   (and the matching `AAAA` records `2606:50c0:8000::153`, `2606:50c0:8001::153`,
   `2606:50c0:8002::153`, `2606:50c0:8003::153`), plus a `CNAME` for `www`
   pointing at `andimatteo.github.io`.

Once DNS propagates, tick `Enforce HTTPS` in the Pages settings. Note that an
apex domain can only be attached to one repository per account, so if
`andreadimatteo.com` is already claimed by another repo it has to be released
there first.

---

## credit

This README was written by Claude.
