use core::fmt::{self, Display};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
use std::fs;
use std::fs::File;
use fs_extra::dir::{copy, CopyOptions};
use std::io::{self, BufRead, BufReader, BufWriter};
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use std::path::Path;
use serde::{Deserialize};
use std::mem::discriminant;
use chrono::NaiveDate;


/*
* structure for representing
* inline elements
* */
#[derive(Debug, PartialEq, Eq)]
enum Inline {
    Text(String),
    Italic(String),
    Bold(String),
    Code(String),
    Link{
        text: String,
        url: String,
    },
    Image{
        url: String,
        alt: String,
    },
    Strikethrough(String),
}

impl Display for Inline {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Inline::Text(s) => write!(f, "{}", escape_html(s)),
            Inline::Italic(s) => write!(f, "<em>{}</em>", escape_html(s)),
            Inline::Bold(s) => write!(f, "<strong>{}</strong>", escape_html(s)),
            Inline::Code(s) => write!(f, "<code>{}</code>", escape_html(s)),
            Inline::Link{text, url} => write!(f, "<a href=\"{}\">{}</a>", url, escape_html(text)),
            Inline::Image{url, alt} => write!(f, "<img src=\"static/{}\" alt=\"{}\" >", url, escape_html(alt)),
            Inline::Strikethrough(s) => write!(f, "<del>{}</del>", escape_html(s)),
        }
    }
}


/* enum for representing admissible heading types */
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Heading{ H1, H2, H3, H4, H5, H6 }

impl Heading {
    fn level(&self) -> u8 {
        match self {
            Heading::H1 => 1, Heading::H2 => 2, Heading::H3 => 3,
            Heading::H4 => 4, Heading::H5 => 5, Heading::H6 => 6,
        }
    }
}

/*
* this is a utility enum for defining the current
* context: when parsing, the context can be in one
* of the following admissible states
* */
enum BlockKind{
    Heading(Heading),
    Paragraph,
    CodeBlock,
    UnorderedList,
    Orderedlist,
    BlockQuote,
    NoBlock,
}


impl PartialEq for BlockKind {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

/*
* this is the actual enum for representing
* block elements in the intermediate
* representation format
* */
#[derive(Debug, PartialEq, Eq)]
enum Block {
    Heading {
        level: Heading,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock(String),
    UnorderedList {
        content: Vec<Inline>,
    },
    OrderedList {
        content: Vec<Inline>,
    },
    BlockQuote(Vec<Inline>),
    NoBlock,
}

impl Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Block::Heading{ level, content } => {
                let l = level.level();
                write!(
                    f,"<h{}>{}</h{}>",
                    l,
                    content
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join(""),
                    l
                )
            },
            Block::OrderedList{ content } => {
                write!(
                    f,
                    "<ol>{}</ol>",
                    content
                        .iter()
                        .map(|i| format!("<li>{}</li>", i))
                        .collect::<Vec<_>>()
                        .join("")
                )
            },
            Block::UnorderedList{ content } => {
                write!(
                    f,
                    "<ul>{}</ul>",
                    content
                        .iter()
                        .map(|i| format!("<li>{}</li>", i))
                        .collect::<Vec<_>>()
                        .join("")
                )
            },
            Block::BlockQuote(content) => {
                write!(
                    f,
                    "<blockquote>{}</blockquote>",
                    content
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join("")
                )
            },
            Block::Paragraph(content) => {
                write!(
                    f,
                    "<p>{}</p>",
                    content
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join("")
                )
            },
            Block::CodeBlock(content) => {
                write!(
                    f,
                    "<pre><code>{}</code></pre>",
                    escape_html(content.trim())
                )
            },
            Block::NoBlock => {
                write!(f,"#DEBUG")
            }
        }
    }
}

#[derive(PartialEq, Eq)]
struct Post {
    file_name : String,
    metadata : PostMetadata,
    body: Vec<Block>,
}

impl Ord for Post {
    fn cmp(&self, other: &Self) -> Ordering {
        other.metadata.date.cmp(&self.metadata.date)
    }
}

impl PartialOrd for Post {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Deserialize,PartialEq, Eq)]
struct PostMetadata {
    title: String,
    date: NaiveDate,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    draft: bool,
}

/*
* simple function to return the slug
* of a name
* */
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_dash = false;

    let push = |out: &mut String, txt: &str, pending: &mut bool| {
        if *pending && !out.is_empty() { out.push('-'); }
        *pending = false;
        out.push_str(txt);
    };

    for c in s.chars() {
        let repl: Option<&str> = match c {
            'à'|'á'|'â'|'ã'|'ä'|'å' => Some("a"),
            'è'|'é'|'ê'|'ë'         => Some("e"),
            'ì'|'í'|'î'|'ï'         => Some("i"),
            'ò'|'ó'|'ô'|'õ'|'ö'|'ø' => Some("o"),
            'ù'|'ú'|'û'|'ü'         => Some("u"),
            'ç' => Some("c"),
            'ñ' => Some("n"),
            'ß' => Some("ss"),
            'æ' => Some("ae"),
            'œ' => Some("oe"),
            _ => None,
        };

        match repl {
            Some(r) => push(&mut out, r, &mut pending_dash),
            None if c.is_ascii_alphanumeric() => {
                let lower = c.to_ascii_lowercase();
                push(&mut out, lower.encode_utf8(&mut [0u8; 4]), &mut pending_dash);
            }
            None => pending_dash = true,
        }
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/*
* TODO: done
* now that we are so proud of adding collision detection
* on the names we must also keep the name of the file inside
* the metadata, otherwise we cannot retrieve the correct
* name from just the title name...
* */
fn unique_name(base: &str, counts: &mut HashMap<String,usize>, used: &mut HashSet<String>) -> String {
    let n = counts.entry(base.to_string()).or_insert(0);
    loop {
        let name = if *n == 0 {
            format!("{base}.html")
        } else {
            format!("{base}-{n}.html")
        };

        if used.insert(name.clone()) {
            return name;
        }
        *n += 1;
    }
}


/*
* parsing functions
* - inline
* - block
* */
fn parse_inline(s: &str) -> Vec<Inline> {
    let mut result = Vec::new();

    let mut i = 0;
    let mut text_start = 0;
    let mut image;

    while i < s.len() {
        image = false;

        let delimiter = if s[i..].starts_with("**") {
            Some("**")
        } else if s[i..].starts_with('*') {
            Some("*")
        } else if s[i..].starts_with('~') {
            Some("~")
        } else if s[i..].starts_with('`') {
            Some("`")
        } else if s[i..].starts_with("![") {
            image = true;
            Some("![")
        } else if s[i..].starts_with('[') {
            Some("[")
        } else {
            None
        };

        if let Some(delimiter) = delimiter {
            let content_start = i + delimiter.len();
            
            /* 
            * if current char is a delimiter then find next occurrence
            * of delimiter. In the case of images of links the ending
            * delimiter is different from starting one.
            * */
            let end_delimiter = match delimiter {
                "![" | "[" => "]",
                d => d,
            };

            if let Some(relative_end) = s[content_start..].find(end_delimiter) {
                let mut content_end = content_start + relative_end;

                if text_start < i {
                    result.push(Inline::Text(
                        s[text_start..i].to_owned(),
                    ));
                }

                let content = s[content_start..content_end].to_owned();

                match delimiter {
                    "**" => result.push(Inline::Bold(content)),
                    "*" => result.push(Inline::Italic(content)),
                    "~" => result.push(Inline::Strikethrough(content)),
                    "`" => result.push(Inline::Code(content)),
                    "[" | "![" => {
                        /*
                        * well well well.
                        * Now we must check if there is delimited
                        * region made of [ ] afer, this will
                        * be the actual URL.
                        * if that isn't the case we will just push this
                        * into text.
                        * */

                        if !s[content_end + 1..].starts_with('(') {
                            result.push(Inline::Text(content));
                        } else {
                            let url_begin = content_end + 2;

                            if let Some(relative_url_end) = s[url_begin..].find(')') {
                                let url_end = url_begin + relative_url_end;
                                let url = s[url_begin..url_end].to_owned();

                                if image {
                                    result.push(Inline::Image { url, alt: content });
                                } else {
                                    result.push(Inline::Link {
                                        text: content,
                                        url,
                                    });
                                }

                                content_end = url_end;
                            }
                        }
                    }
                    _ => unreachable!(),
                }

                i = content_end + delimiter.len();
                text_start = i;
                continue;
            }
        }

        i += s[i..]
            .chars()
            .next()
            .unwrap()
            .len_utf8();
    }

    if text_start < s.len() {
        result.push(Inline::Text(
            s[text_start..].to_owned(),
        ));
    }

    result
}

fn parse_block(current_block: &BlockKind, current_block_content: String) -> Block {
    match current_block {
        BlockKind::CodeBlock => Block::CodeBlock(
            current_block_content
        ),
        BlockKind::BlockQuote => Block::BlockQuote(
            parse_inline(&current_block_content)
        ),
        BlockKind::Heading(h) => Block::Heading{
            level: *h,
            content: parse_inline(&current_block_content)
        },
        /*
        * TODO:
        * actually implement lists
        * */
        BlockKind::UnorderedList => Block::UnorderedList { 
            content: current_block_content
                .lines()
                .map(|line| Inline::Text(line.to_owned()))
                .collect(),
        },
        BlockKind::Orderedlist => Block::OrderedList { 
            content: current_block_content
                .lines()
                .map(|line| Inline::Text(line.to_owned()))
                .collect(),
        },
        BlockKind::Paragraph => Block::Paragraph(
            parse_inline(&current_block_content)
        ),
        BlockKind::NoBlock => Block::NoBlock,
    }
}

/*
* very simple rendering
* function, substitutes every {{name}}
* element in the code usign a map.
*
* TODO:
* implement also programmatic rendering
* (that's right, like angular frameworks).
* This will make it easier for
* index template definition.
*
* @for elem in list {
*   <li> {{elem.text}} </li>
* }
* */
fn render(template: &str, fields: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in fields {
        out = out.replace(&format!("{{{{{}}}}}", key), value);
    }
    out
}


/*
* compilation functions
* - single posts
* - index
* */
fn compile_post(post: &Post) -> io::Result<()> {
    let header = fs::read_to_string("templates/head.html")?;
    let body = fs::read_to_string("templates/body.html")?;
    let topbar = fs::read_to_string("templates/topbar.html")?;
    let out_dir = Path::new("dist");
    fs::create_dir_all(out_dir)?;
    
    /* clean filename from unwanted chars */
    let mut w = BufWriter::new(File::create(out_dir.join(&post.file_name))?);

    let content: String = post.body.iter().map(|b| b.to_string()).collect();

    let title = escape_html(&post.metadata.title);
    let date = post.metadata.date.to_string();

    let page = format!(
        "<!DOCTYPE html><html>{}{}</html>",
        render(&header, &[("title", &title)]),
        render(&body, &[
            ("title", &title),
            ("date", &date),
            ("topbar", &topbar),
            ("body", &content),
        ]),
    );

    w.write_all(page.as_bytes())?;
    w.flush()?;

    Ok(())
}

fn compile_index(posts: &[Post]) -> io::Result<()> {
    let header = fs::read_to_string("templates/head.html")?;
    let body = fs::read_to_string("templates/index.html")?;
    let topbar = fs::read_to_string("templates/topbar.html")?;

    /* should not be necessary, idempotent */
    let out_dir = Path::new("dist");
    fs::create_dir_all(out_dir)?;

    let mut w = BufWriter::new(
        File::create(out_dir.join("index.html"))?
    );

    let mut content = String::new();

    for post in posts {
        let href = &post.file_name;

        let description = if post.metadata.description.is_empty() {
            String::new()
        } else {
            format!(
                r#"<span class="post-desc">{}</span>"#,
                escape_html(&post.metadata.description)
            )
        };

        /*
        * tags are rendered as buttons, so they must stay *outside*
        * the post link: an <a> cannot contain interactive elements.
        * the row is still clickable end to end, the CSS lets the link
        * blanket the row and lifts this line back on top of it.
        *
        * the attribute is comma separated so that a tag is free to
        * contain a space without breaking the split on the JS side.
        * */
        let tags: String = post.metadata.tags
            .iter()
            .map(|t| format!(
                r#"<button class="tag" type="button" data-tag="{tag}">{tag}</button>"#,
                tag = escape_html(t),
            ))
            .collect::<Vec<_>>()
            .join(r#"<span class="sep">·</span>"#);

        let meta = if tags.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="post-meta">{}</div>"#, tags)
        };

        write!(
            &mut content,
            r#"<li class="post-item" data-name="{name}" data-date="{date}" data-tags="{tag_attr}" data-desc="{desc_attr}">
    <time class="post-date" datetime="{date}">{date}</time>
    <a class="post-link" href="{href}">
        <h2 class="post-title">{title}</h2>
        {description}
    </a>
    {meta}
</li>
"#,
            name = escape_html(&post.metadata.title),
            href = href,
            title = escape_html(&post.metadata.title),
            date = post.metadata.date,
            description = description,
            desc_attr = escape_html(&post.metadata.description),
            tag_attr = escape_html(&post.metadata.tags.join(",")),
            meta = meta,
        )
        .unwrap();
    }

    let page = format!(
        "<!DOCTYPE html><html>{}{}</html>",
        render(&header, &[("title", "index")]),
        render(&body, &[("topbar", &topbar), ("post-list", &content)]),
    );

    w.write_all(page.as_bytes())?;
    w.flush()?;

    Ok(())
}

/*
* Once we have completed all file compilation
* we can prooceed with moving all static files
* */
fn move_static() -> fs_extra::error::Result<()> {
    let opts = CopyOptions::new().overwrite(true).content_only(true);
    copy("static", "dist/static", &opts)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let files: Vec<_> = fs::read_dir("posts/")?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    let mut counts = HashMap::<String,usize>::new();
    let mut used = HashSet::<String>::new();

    /* 
    * the idea of this parser is the following:
    * after getting all the list of files we go file by file.
    *
    * For each file we get all the different blocks with their content.
    *
    * Technically we are not deriving any AST for the moment,
    * we have just one level of block and then a series of inline
    * modifiers for each block.
    *
    * We can completely parallelize this for
    * */
    let mut posts : Vec<Post> = Vec::new();
    for path in files {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut header = String::new();

        /* 
        * parse header
        * first lines MUST be header
        * */
        let first = lines.next().ok_or("Empty file")??;
        if first.trim() != "---" {
            Err("Post must start with `---`")?;
        }
        while let Some(line) = lines.next() {
            let line = line?;
            if line.trim() == "---" { break; }
            header.push_str(&line);
            header.push('\n');
        }
        /*
        * I think YAML syntax for file metadata
        * is just enough
        * */
        let metadata: PostMetadata = serde_yaml::from_str(&header)?;

        /* if post is draft then don't serve it. */
        if metadata.draft {
            continue;
        }

        /* 
        * block parsing.
        *
        * we have a previous context and then we extract new
        * context depending on the first chars of the line we
        * encounter.
        *
        * If we get into a new context then we push the old
        * context into the body of the post.
        *
        * For this reason the intermediate representation is a list of blocks
        * where each block has its own list of inlines, plain text
        * is treated as an inline with no modifiers.
        * */
        let mut body: Vec<Block> = Vec::new();
        let mut current_block = BlockKind::NoBlock;
        let mut current_block_content = String::new();
        let mut is_code = false;

        for line in lines {
            let line = line?;
            let line_trimmed = line.trim();
            let (new_block, new_line) =
            match line_trimmed {
                /*
                * must check first if we are opening or closing
                * a CodeBlock
                * */
                l if l.starts_with("```") || l.starts_with("~~~") => {
                    is_code = !is_code;
                    if is_code { (BlockKind::CodeBlock,"") } else { (BlockKind::NoBlock,"") }
                },
                /*
                * if we are in a codeblock then we are in a codeblock, period.
                * */
                l if is_code => (BlockKind::CodeBlock,l),
                l if l.starts_with("###### ") => {
                    (
                        BlockKind::Heading(Heading::H6),
                        &l[7..],
                    )
                },
                l if l.starts_with("##### ") => {
                    (
                        BlockKind::Heading(Heading::H5),
                        &l[6..],
                    )
                },
                l if l.starts_with("#### ") => {
                    (
                        BlockKind::Heading(Heading::H4),
                        &l[5..],
                    )
                },
                l if l.starts_with("### ") => {
                    (
                        BlockKind::Heading(Heading::H3),
                        &l[4..],
                    )
                },
                l if l.starts_with("## ") => {
                    (
                        BlockKind::Heading(Heading::H2),
                        &l[3..],
                    )
                },
                l if l.starts_with("# ") => {
                    (
                        BlockKind::Heading(Heading::H1),
                        &l[2..],
                    )
                },
                l if l.starts_with("> ") => {
                    (
                        BlockKind::BlockQuote,
                        &l[2..],
                    )
                },
                l if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") => {
                    (
                        BlockKind::UnorderedList,
                        &l[2..],
                    )
                },
                /*
                * TODO:
                * fix this stuff, it's just recomputing twice
                * the same thing...
                * */
                l if {
                    let d = l
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .count();

                    d > 0 && matches!(l.chars().nth(d), Some('.') | Some(')'))
                } => {
                    let d = l
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .count();

                    (
                        BlockKind::Orderedlist,
                        l[d + 1..].trim_start(),
                    )
                },
                l if l.is_empty() => {
                    (
                        BlockKind::NoBlock,
                        "",
                    )
                }
                l => {
                    (
                        BlockKind::Paragraph,l
                    )
                }
            };

            /* if keeping up with no block then continue */
            if new_block == current_block && current_block == BlockKind::NoBlock {
                continue;
            }

            /*
            * hehe gotcha, now we are in a new block.
            * need to change block and push on old block
            * */
            if new_block != current_block {
                let block = parse_block(&current_block, current_block_content);
                /*
                * bruh, current block is a no block,
                * no need to push
                * */
                if !matches!(block, Block::NoBlock) {
                    body.push(block);
                }

                current_block_content = String::new();
                current_block = new_block;
            }
            current_block_content.push_str(new_line);
            current_block_content.push('\n');
        }

        /*
        * still a closing block
        * */
        if current_block != BlockKind::NoBlock {
            let block = parse_block(&current_block,current_block_content);
            body.push(block);
        }

        /*
        * NOTE:
        * now we have completely parsed the .md file
        * */
        // println!("{:#?}",body);
        let post = Post {
            file_name: unique_name(&slugify(&metadata.title).to_string(),&mut counts, &mut used),
            metadata,
            body
        };

        /*
        * TODO:
        * could also async defer this operation
        * to another thread
        * */
        compile_post(&post)?;
        posts.push(post);
    }
    
    posts.sort_by_key(|p| p.metadata.date);
    compile_index(&posts)?;

    move_static()?;

    Ok(())
}
