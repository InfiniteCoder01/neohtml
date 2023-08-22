use super::attribute::Attribute;
use super::{PageBuildError, PageParseError};
use itertools::Itertools;

#[derive(Clone, Debug, PartialEq)]
pub enum Section {
    Text {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
    },
    TextWrapper {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
    },
    Container {
        tag: String,
        attributes: Vec<Attribute>,
        content: Vec<Section>,
    },
    Code {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
    },
    Tag {
        tag: String,
        attributes: Vec<Attribute>,
    },

    Bookmark {
        attributes: Vec<Attribute>,
        content: String,
    },
    Hidden {
        content: String,
    },
    Notes {
        class: String,
        attributes: Vec<Attribute>,
        content: Vec<String>,
    },
    List {
        tag: String,
        attributes: Vec<Attribute>,
        content: Vec<String>,
    },
    Checklist {
        attributes: Vec<Attribute>,
        content: Vec<String>,
        todo: bool,
    },
    Youtube {
        id: String,
    },
}

impl Section {
    pub fn parse<R: std::io::BufRead>(
        source: &mut super::Reader<R>,
        section: &str,
    ) -> Result<Self, PageParseError> {
        fn map_code_tag(tag: &str) -> &str {
            match tag {
                "css" => "style",
                tag => tag,
            }
        }

        match section {
            "title" | "subtitle" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "nav" => {
                Ok(Self::Text {
                    tag: match section {
                        "title" => "h1 class=\"title\"",
                        "subtitle" => "p class=\"subtitle\"",
                        tag => tag,
                    }
                    .to_owned(),
                    attributes: source.next_attrs()?,
                    content: match section {
                        "title" | "subtitle" => {
                            source.skip_blanks()?;
                            source.next_line()?.ok_or(PageParseError::EmptyTitle)?
                        }
                        _ => source.next_text_until_section(false)?,
                    },
                })
            }
            "aside" | "blockquote" => Ok(Self::TextWrapper {
                tag: section.to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "note" | "warning" => Ok(Self::TextWrapper {
                tag: format!("div class = \"{section}\""),
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "article/" | "section/" | "div/" | "code/" | "pre/" | "script/" | "html/" | "css/" => {
                let tag = section.strip_suffix('/').unwrap();
                let attributes = source.next_attrs()?;
                Ok(match tag {
                    "code" | "pre" | "script" | "html" | "css" => Self::Code {
                        tag: map_code_tag(tag).to_owned(),
                        attributes,
                        content: source.next_text_until_tag(tag, true)?,
                    },
                    tag => Self::Container {
                        tag: tag.to_owned(),
                        attributes,
                        content: source.next_sections(Some(tag))?,
                    },
                })
            }
            "code" | "pre" | "script" | "html" | "css" => {
                let attributes = source.next_attrs()?;
                Ok(Self::Code {
                    tag: map_code_tag(section).to_owned(),
                    attributes,
                    content: source.next_text_until_section(true)?,
                })
            }
            "```" => {
                let attributes = source.next_attrs()?;
                Ok(Self::Code {
                    tag: "code".to_owned(),
                    attributes,
                    content: source.next_text_until_tag("```", true)?,
                })
            }
            "hr" | "image" => {
                let attributes = source.next_attrs()?;
                Ok(Self::Tag {
                    tag: section.to_owned(),
                    attributes,
                })
            }

            "bookmark" => Ok(Self::Bookmark {
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "hidden" => Ok(Self::Hidden {
                content: source.next_text_until_section(true)?,
            }),
            "notes" | "warnings" => Ok(Self::Notes {
                class: section[0..section.len() - 1].to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_list_prefixed("- ")?,
            }),
            "list" | "olist" => Ok(Self::List {
                tag: match section {
                    "olist" => "ol",
                    _ => "ul",
                }
                .to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_list_prefixed("- ")?,
            }),
            "checklist" | "todo" => Ok(Self::Checklist {
                attributes: source.next_attrs()?,
                content: source
                    .next_list(|line| line.starts_with("[]") || line.starts_with("[x]"))?,
                todo: section == "todo",
            }),
            "youtube" => Ok(Self::Youtube {
                id: source
                    .next_line_if_map(super::strip_attr_prefix)?
                    .ok_or(PageParseError::ExpectedVideoID)?,
            }),
            _ => Err(PageParseError::UnknownSection(section.to_owned())),
        }
    }

    pub fn to_html(&self) -> Result<String, PageBuildError> {
        // * Attrs
        macro_rules! attributes {
            ($attrs: expr) => {
                $attrs.iter().fold("".to_owned(), |buffer, arg| {
                    format!("{} {}", buffer, arg.to_html())
                })
            };
        }

        macro_rules! attr {
            ($attrs: expr, $attr: ident) => {
                $attrs.iter().find_map(|attr| match attr {
                    Attribute::$attr(value) => Some(value),
                    _ => None,
                })
            };
        }

        macro_rules! has_attr {
            ($attrs: expr, $attr: ident) => {
                $attrs.iter().any(|attr| matches!(attr, Attribute::$attr))
            };
        }

        // * Specific attrs
        macro_rules! title {
            ($attrs: expr, $tag: expr) => {
                attr!($attrs, Title)
                    .map(|title| format!("<{}>{}</{}>", $tag, title.as_str(), $tag))
                    .unwrap_or_default()
            };
            ($attrs: expr) => {
                title!($attrs, "h4")
            };
        }

        // * Utils
        fn format_code(content: &str, title: String, attributes: String) -> String {
            format!(
                "<pre>{}<code{}>{}</code></pre>",
                title,
                attributes,
                escape_html(content),
            )
        }

        match self {
            Section::Text {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}>{}</{tag}>",
                attributes!(attributes),
                text_to_html(content)
            )),
            Section::TextWrapper {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}>{}<p>{}</p></{tag}>",
                attributes!(attributes),
                title!(attributes),
                text_to_html(content)
            )),
            Section::Container {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}>{}{}</{tag}>",
                attributes!(attributes),
                title!(attributes),
                {
                    let mut html = String::new();
                    for section in content {
                        html.push_str(&section.to_html()?);
                    }
                    html
                },
            )),
            Section::Code {
                tag,
                attributes,
                content,
            } => Ok(match tag.as_str() {
                "code" => format_code(content, title!(attributes), attributes!(attributes)),
                tag => {
                    format!("<{tag}{}>{}</{tag}>", attributes!(attributes), content)
                        + &if has_attr!(attributes, Show) {
                            format_code(content, title!(attributes), String::new())
                        } else {
                            String::new()
                        }
                }
            }),
            Section::Tag { tag, attributes } => Ok(format!("<{tag}{} />", attributes!(attributes))),

            Section::Bookmark {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"bookmark\"{}>{}{}</div>",
                attributes!(attributes),
                title!(attributes, "h3 class = \"bookmarkTitle\""),
                text_to_html(content),
            )),
            Section::Hidden { content } => Ok(format!("<!-- {} -->", escape_html(content))),
            Section::Notes {
                class,
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"{}\"{}>{}<ul>{}</ul></div>",
                class,
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content
                        .iter()
                        .map(|item| format!("<li><p>{}</p></li>", text_to_html(item))),
                    ""
                ),
            )),
            Section::List {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<div{}>{}<{tag}>{}</{tag}></div>",
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content
                        .iter()
                        .map(|item| format!("<li><p>{}</p></li>", text_to_html(item))),
                    ""
                ),
            )),
            Section::Checklist {
                attributes,
                content,
                todo,
            } => Ok(format!(
                "<div{}>{}{}</div>",
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content.iter().map(|item| format!(
                        "<label><input type=\"checkbox\" {}{}/> {}</label><br>",
                        if *todo { "disabled " } else { "" },
                        if item.starts_with("[x]") {
                            "checked "
                        } else {
                            ""
                        },
                        text_to_html(
                            item.strip_prefix("[]")
                                .or_else(|| item.strip_prefix("[x]"))
                                .unwrap()
                        )
                    )),
                    ""
                ),
            )),
            Section::Youtube { id } => Ok(format!(
                concat!(
                    r#"<iframe width="623" height="350" src="https://www.youtube-nocookie.com/embed/{}" "#,
                    r#"title="YouTube video player" allow="accelerometer; autoplay; clipboard-write; "#,
                    r#"encrypted-media; gyroscope; picture-in-picture; web-share" allowfullscreen=""></iframe>"#,
                ),
                id
            )),
        }
    }
}

pub fn escape_html(code: &str) -> String {
    code.replace('&', "&amp")
        .replace('<', "&lt")
        .replace('>', "&gt")
}

pub fn text_to_html(text: &str) -> String {
    fn regex_replace<'a>(
        text: &'a str,
        pattern: &str,
        replacer: impl Fn(&regex::Captures) -> String,
    ) -> std::borrow::Cow<'a, str> {
        regex::Regex::new(&escape_html(pattern))
            .unwrap()
            .replace_all(text, replacer)
    }

    macro_rules! format_attrs {
        ($attrs: expr) => {
            regex_replace(&$attrs, r"(\w+)\s*:\s*(\w+)", |captures| {
                format!("{} = \"{}\"", &captures[1], &captures[2])
            })
        };
    }

    macro_rules! wrap_tag {
        ($tag: expr, $attrs: expr, $content: expr) => {
            format!("<{} {}>{}</{}>", $tag, $attrs, $content, $tag)
        };
    }

    let text = escape_html(text);

    // Tag
    let text =
        regex_replace(
            &text,
            r"<<(\w+)\s*\|([^|]*)\w*\|([^|]*)\s*>>",
            |captures| match &captures[1] {
                "link" => wrap_tag!("a", format!("href = \"{}\"", &captures[3]), &captures[2]),
                tag => wrap_tag!(tag, format_attrs!(captures[3]), &captures[2]),
            },
        );

    // MD Link
    let text = regex_replace(&text, r"\[([^\]]*)\]\(([^\]]*)\)", |captures| {
        wrap_tag!("a", format!("href = \"{}\"", &captures[2]), &captures[1])
    });

    // Shortcuts
    let text = regex_replace(&text, r"\*([^\*]*)\*([^\*]*)\*", |captures| {
        wrap_tag!("strong", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"_([^_]*)_([^_]*)_", |captures| {
        wrap_tag!("em", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"\~([^\~]*)\~([^\~]*)\~", |captures| {
        wrap_tag!("s", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"`([^`]*)`([^`]*)`", |captures| {
        format!(
            "<pre><code {}>{}</code></pre>",
            (format_attrs!(captures[2])),
            (&captures[1])
        )
    });
    text.replace('\n', "<br>")
}

pub fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
