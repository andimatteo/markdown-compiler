use std::fs;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use serde::{Deserialize};
use std::mem::{discriminant, take};

use chrono::NaiveDate;
use regex::Regex;

enum InlineKind {
    Text,
    Emphasis,
    Strong,
    Code,
    Link,
}

enum Inline {
    Text(String),
    Emphasis(String),
    Strong(String),
    Code(String),
    Link { text: String, url: String }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Heading{
    H1,
    H2,
    H3,
    H4,
    H5,
    H6
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

enum Block {
    Heading{ level: Heading, content: Vec<Inline> },
    Paragraph(Vec<Inline>),
    CodeBlock(String),
    UnorderedList{ lines: u32, content: Vec<Inline> },
    OrderedList{ lines: u32, content: Vec<Inline> },
    BlockQuote(Vec<Inline>),
    NoBlock,
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

fn parse_inline(s: &str, kind: &BlockKind) -> Vec<Inline> {
    println!("line count: {}",s.lines().count());
    Vec::new()
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
    * */
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

        while let Some(line) = lines.next() {
            let line = line?;
            let line_trimmed = line.trim();
            let new_block =
            match line_trimmed {
                /*
                * must check first if we are opening or closing
                * a CodeBlock
                * */
                l if l.starts_with("```") || l.starts_with("~~~") => {
                    is_code = !is_code;
                    if is_code { BlockKind::CodeBlock } else { BlockKind::NoBlock }
                },
                /*
                * if we are in a codeblock then we are in a codeblock, period.
                * */
                _ if is_code => BlockKind::CodeBlock,
                l if l.starts_with("###### ") => BlockKind::Heading(Heading::H6),
                l if l.starts_with("##### ") => BlockKind::Heading(Heading::H5),
                l if l.starts_with("#### ") => BlockKind::Heading(Heading::H4),
                l if l.starts_with("### ") => BlockKind::Heading(Heading::H3),
                l if l.starts_with("## ") => BlockKind::Heading(Heading::H2),
                l if l.starts_with("# ") => BlockKind::Heading(Heading::H1),
                l if l.starts_with("> ") || l == ">" => BlockKind::BlockQuote,
                l if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") => BlockKind::UnorderedList,
                l if {
                    let d = l.chars().take_while(|c| c.is_ascii_digit()).count();
                    d > 0 && matches!(l.chars().nth(d), Some('.') | Some(')'))
                } => BlockKind::Orderedlist,
                l if l.is_empty() => BlockKind::NoBlock,
                _ => BlockKind::Paragraph,
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
                let block = 
                match current_block {
                    BlockKind::CodeBlock => Block::CodeBlock(
                        current_block_content
                    ),
                    BlockKind::BlockQuote => Block::BlockQuote(
                        parse_inline(&current_block_content, &current_block)
                    ),
                    BlockKind::Heading(h) => Block::Heading{
                        level: h,
                        content: parse_inline(&current_block_content, &current_block)
                    },
                    BlockKind::UnorderedList => Block::UnorderedList { 
                        lines: current_block_content.lines().count() as u32,
                        content: parse_inline(&current_block_content, &current_block)
                    },
                    BlockKind::Orderedlist => Block::OrderedList { 
                        lines: current_block_content.lines().count() as u32,
                        content: parse_inline(&current_block_content, &current_block)
                    },
                    BlockKind::Paragraph => Block::Paragraph(
                        parse_inline(&current_block_content, &current_block)
                    ),
                    BlockKind::NoBlock => Block::NoBlock,
                };

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
            current_block_content.push_str(line_trimmed);
            current_block_content.push('\n');
        }

        /* there could still be a non empty block open */
    }

    Ok(())
}

fn new() -> bool {
    todo!()
}
