---
title: .md to .html compiler
date: 2026-07-23
description: >
    I'm new to Rust and thought that a very pratical way to get in
    touch with it was writing something.

    I've thought of writing my own .md to .html compiler for
    my micro-blog for a while. So here we are, both my micro-blog and my
    project have come to life.
tags: [rust, compiler, project]
---

Hi, this is Andrea.

I'm new to Rust and thought that a very pratical way to get in
touch with it was writing something. I've thought of writing my
own .md to .html compiler for for a while. So here we are, both
my micro-blog and my project have come to life. Before even thinking of
writing it, I was using [Hugo](https://github.com/gohugoio/hugo). 
It was really nice, but you know, sometimes it's just too much, I just
wanted a very simple thing, not taking a lifetime choosing a theme and
other stuff, 
~or maybe as a SWE I thought I could just build it myself~, and after 
12-13 hours of my ~not really~ spare time I came eventually with something.

It's very basic, written with no GenAI tools, and as much [docs](https://doc.rust-lang.org/book/) as possible.
I've implemented very basic `.md` syntax for now, ~eventually~ will expand later.
Some time ago I read [the dragon book](https://www.amazon.com/dp/0321486811?lv=shuf&channelId=500&plpRedirect=mhFallback),
so writing a compiler was not a completely new thing,
but it was fun to see that some of my own choices were pretty
similar to actual `.md` to `.html` compilers, even Hugo itself.

The compilation follows the following steps:

- scan post/ dir
- for each file scan for blocks elements
- for each block scan for inline elements
- we then end up with an intermediate representation (building ASTs is always pretty cool):

```
[
    Heading {
        level: H1,
        content: [
            Text(
                "this\n",
            ),
        ],
    },
    Heading {
        level: H2,
        content: [
            Text(
                "is\n",
            ),
        ],
    },
    Paragraph(
        [
            Text(
                "a very ",
            ),
            Italic(
                "simple",
            ),
            Text(
                " ",
            ),
            Bold(
                "post",
            ),
            Text(
                "\n",
            ),
        ],
    ),
]
```

- output as `.html` with templates (very basic)

Output to file was particularly simple since redefining `Display`
on `Block` => `Inline` made it no different than a simple display.

Here's a demo of the generated `index` and a couple posts.

![demo](posts.gif)

Don't think this tool will ever be useful for anyone besides me, but it
was fun to make and surely will expand.

