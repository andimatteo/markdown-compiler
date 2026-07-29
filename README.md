Hi, this is Andrea.

I'm learning Rust and I thought the best way to do so was getting into a
project. A markdown to static HTML compiler is a very simple project I've had in
mind for a while, so here it is, it actually builds
[andreadimatteo.com](https://andreadimatteo.com).

No dependencies beyond a handful of crates, no JS framework, no generator. You
write `.md` files in `posts/`, run the compiler, and get a `dist/` folder full of
plain HTML that GitHub Pages serves as is.

Everything in the scope of this project, hence, the `.md` to `.html` compiler
was implented with no GenAI tools and as much [docs](https://doc.rust-lang.org/book/)
as possible, it's just a project to learn Rust on the way. 
Hoever, since I'm stating that I wrote this project myself I think this is a needed
disclosure: the *grotesquely large* `static/vim.js` file is a totally
AI-generated script for implementing vim moves on posts,
~didn't really review any single line of that code :)~. AI also
generated part of the html templates and a minor part of the stylesheet.
I thought they were just out of the scope of this project.

I will keep track of the current [CommonMark](https://spec.commonmark.org/)
coverage, main todos, how compilation is done,
and how you can create posts and use it yourself on
[this post](https://andreadimatteo.com/md-to-html-compiler.html).

## build

```sh
cargo run --release
```

That reads every `.md` in `posts/`, writes one HTML file in `posts/`,
generates an `index.html` and finally copies 
`static/` into `dist/static/`.

## Makefile

This is temporary and shouldn't apply to anyone beside me. It is
a bunchful commands I wrote for simplicity:

- `make post` => prompt for creating a post template
- `make gifs` => convert all `.mp4` files under `static/` to a `gif`. Needs
ffmpeg
if you pay attention `.mp4`s are currently gitignored ~just haven't implemented
videos yet, maybe will support in future~.
- `make link [pwd]` => creates a symlink to your post folder into `posts/`
