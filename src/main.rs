use core::fmt::{self, Display};
use std::fmt::Formatter;
use std::fs;
use std::fs::File;
use fs_extra::dir::{copy, CopyOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::thread::current;
use serde::{Deserialize};
use std::mem::{discriminant, take};

use chrono::NaiveDate;
use regex::{Regex, escape};

#[derive(Debug)]
enum Inline {
    Text(String),
    Italic(String),
    Bold(String),
    Code(String),
    Link {
        text: String,
        url: String,
    },
    Strikethrough(String),
}

impl Display for Inline {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Inline::Text(s) => write!(f, "{}", escape(s.trim())),
            Inline::Italic(s) => write!(f, "<em>{}</em>", escape(s.trim())),
            Inline::Bold(s) => write!(f, "<strong>{}</strong>", escape(s.trim())),
            Inline::Code(s) => write!(f, "<code>{}</code>", escape(s.trim())),
            Inline::Link{text, url} => write!(f, "<a href=\"{}\">{}</a>", url, escape(text)),
            Inline::Strikethrough(s) => write!(f, "<del>{}</del>", escape(s.trim())),
        }
    }
}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Heading{
    H1,
    H2,
    H3,
    H4,
    H5,
    H6
}

impl Heading {
    fn level(&self) -> u8 {
        match self {
            Heading::H1 => 1, Heading::H2 => 2, Heading::H3 => 3,
            Heading::H4 => 4, Heading::H5 => 5, Heading::H6 => 6,
        }
    }
}

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

#[derive(Debug)]
enum Block {
    Heading {
        level: Heading,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock(String),
    UnorderedList {
        lines: u32,
        content: Vec<Inline>,
    },
    OrderedList {
        lines: u32,
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
                        .collect::<Vec<_>>().join(" "),
                    l
                )
            },
            Block::OrderedList{ lines, content } => {
                write!(
                    f,
                    "<ol>{}</ol>",
                    content
                        .iter()
                        .map(|i| format!("<li>{}</li>", i))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            },
            Block::UnorderedList{ lines, content } => {
                write!(
                    f,
                    "<ul>{}</ul>",
                    content
                        .iter()
                        .map(|i| format!("<li>{}</li>", i))
                        .collect::<Vec<_>>()
                        .join(" ")
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
                        .join(" ")
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
                        .join(" ")
                )
            },
            Block::CodeBlock(content) => {
                write!(
                    f,
                    "<pre><code>{}</code></pre>",
                    content
                )
            },
            Block::NoBlock => {
                write!(f,"#DEBUG")
            }
        }
    }
}

#[derive(Deserialize)]
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

struct Post {
    metadata : PostMetadata,
    body: Vec<Block>,
}

fn skip_blank<I: Iterator<Item = std::io::Result<String>>>(lines: &mut std::iter::Peekable<I>) {
    while let Some(Ok(line)) = lines.peek() {
        if line.trim().trim_end().is_empty() {
            lines.next();
        } else {
            break;
        }
    }
}

fn parse_inline(s: &str) -> Vec<Inline> {
    let mut result = Vec::new();

    let mut i = 0;
    let mut text_start = 0;

    while i < s.len() {
        let delimiter = if s[i..].starts_with("**") {
            Some("**")
        } else if s[i..].starts_with('*') {
            Some("*")
        } else if s[i..].starts_with('~') {
            Some("~")
        } else if s[i..].starts_with('`') {
            Some("`")
        } else {
            None
        };

        if let Some(delimiter) = delimiter {
            let content_start = i + delimiter.len();

            if let Some(relative_end) = s[content_start..].find(delimiter) {
                let content_end = content_start + relative_end;

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
            lines: current_block_content.lines().count() as u32,
            content: current_block_content
                .lines()
                .map(|line| Inline::Text(line.to_owned()))
                .collect(),
        },
        BlockKind::Orderedlist => Block::OrderedList { 
            lines: current_block_content.lines().count() as u32,
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

fn render(template: &str, fields: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in fields {
        out = out.replace(&format!("{{{{{}}}}}", key), value);
    }
    out
}

/*
* very simple function that
* given the post title returns file name
* */
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
}

fn file_name(s: &str) -> String {
    format!("{}.html", normalize(s))
}

fn compile_post(post: &Post) -> io::Result<()> {
    let header = fs::read_to_string("templates/head.html")?;
    let body = fs::read_to_string("templates/body.html")?;
    let out_dir = Path::new("dist");
    let _ = fs::create_dir_all(out_dir);
    
    /* clean filename from unwanted chars */
    let out_file_name: String = file_name(&post.metadata.title);

    let mut w = BufWriter::new(File::create(out_dir.join(out_file_name))?);

    let content: String = post.body.iter().map(|b| b.to_string()).collect();

    let page = format!(
        "<!DOCTYPE html><html>{}{}</html>",
        render(&header, &[("title", &post.metadata.title)]),
        render(&body, &[("body", &content)]),
    );

    w.write_all(page.as_bytes())?;
    w.flush()?;

    Ok(())
}

fn compile_index() {
    
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
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

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
        let mut new_block = Block::NoBlock;
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
        let post = Post {
            metadata,
            body
        };

        /*
        * TODO:
        * could also async defer this operation
        * to another thread
        * */
        let _ = compile_post(&post);
        posts.push(post);
    }

    compile_index();

    let _ = move_static();

    Ok(())
}


