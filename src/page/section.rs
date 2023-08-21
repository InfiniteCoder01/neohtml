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
    Wrapper {
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
    Notes {
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
            "aside" | "blockquote" => Ok(Self::Wrapper {
                tag: section.to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "note" => Ok(Self::Wrapper {
                tag: "div class = \"note\"".to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "article/" | "section/" | "div/" | "code/" | "pre/" | "script/" | "html/" | "css/" => {
                let tag = section.strip_suffix('/').unwrap();
                let attributes = source.next_attrs()?;
                Ok(match tag {
                    "code" | "pre" | "script" | "html" | "css" => Self::Code {
                        tag: match tag {
                            "css" => "style",
                            tag => tag,
                        }
                        .to_owned(),
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
            "pre" | "script" => {
                let attributes = source.next_attrs()?;
                Ok(Self::Code {
                    tag: section.to_owned(),
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
            "notes" => Ok(Self::Notes {
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
        macro_rules! attributes {
            ($attrs: expr) => {
                $attrs.iter().fold("".to_owned(), |buffer, arg| {
                    format!("{} {}", buffer, arg.to_html())
                })
            };
        }

        macro_rules! title {
            ($attrs: expr, $tag: expr) => {
                $attrs
                    .iter()
                    .find_map(|attr| match attr {
                        Attribute::Title(title) => {
                            Some(format!("<{}>{}</{}>", $tag, title.as_str(), $tag))
                        }
                        _ => None,
                    })
                    .unwrap_or_default()
            };
            ($attrs: expr) => {
                title!($attrs, "h4")
            };
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
            Section::Wrapper {
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
                "code" => format!(
                    "<pre>{}<code{}>{}</code></pre>",
                    title!(attributes),
                    attributes!(attributes),
                    escape_html(content),
                ),
                tag => format!("<{tag}{}>{}</{tag}>", attributes!(attributes), content),
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
            Section::Notes {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"note\"{}>{}<ul>{}</ul></div>",
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content
                        .iter()
                        .map(|item| format!("<li><p>{}</p></li>", item)),
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
                        .map(|item| format!("<li><p>{}</p></li>", item)),
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
                        item.strip_prefix("[]")
                            .or_else(|| item.strip_prefix("[x]"))
                            .unwrap()
                    )),
                    ""
                ),
            )),
            Section::Youtube { id } => Ok(format!(
                concat!(
                    r#"<iframe width="560" height="315" src="https://www.youtube-nocookie.com/embed/{}" "#,
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
    macro_rules! regex_replace {
        ($text: ident, $pattern: literal, $captures: ident => $replacement: expr) => {
            let $text = regex::Regex::new(&escape_html($pattern))
                .unwrap()
                .replace_all(&$text, |$captures: &regex::Captures| $replacement);
        };
    }

    let text = escape_html(text);
    regex_replace!(
        text,
        r"<<link\s*\|([^|]*)\w*\|([^|]*)\s*>>",
        caps => format!("<a href = \"{}\">{}</a>", &caps[2], &caps[1])
    );
    regex_replace!(
        text,
        r"\[([^\]]*)\]\(([^\]]*)\)",
        caps => format!("<a href = \"{}\">{}</a>", &caps[2], &caps[1])
    );
    text.replace('\n', "<br>")
}

pub fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
