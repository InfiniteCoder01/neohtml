use std::collections::HashMap;

use super::attribute::Attribute;
use super::{relative_path_to, PageBuildError, PageParseError};
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
    Image {
        src: String,
        attributes: Vec<Attribute>,
    },

    Youtube {
        id: String,
    },

    Hidden {
        content: String,
    },
    Metadata {
        data: HashMap<String, String>,
    },
    Categories {
        categories: Vec<String>,
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
            "hr" => {
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
            "image" => {
                let src = source
                    .next_line_if_map(super::strip_attr_prefix)?
                    .ok_or(PageParseError::ExpectedImageSource)?;
                Ok(Self::Image {
                    src,
                    attributes: source.next_attrs()?,
                })
            }
            "youtube" => Ok(Self::Youtube {
                id: source
                    .next_line_if_map(super::strip_attr_prefix)?
                    .ok_or(PageParseError::ExpectedVideoID)?,
            }),

            "hidden" => Ok(Self::Hidden {
                content: source.next_text_until_section(true)?,
            }),
            "metadata" => Ok(Self::Metadata {
                data: {
                    let mut meta = HashMap::new();
                    for metaline in source.next_text_prefixed("--", true)?.split('\n') {
                        let mut name = String::new();
                        let mut value = String::new();
                        scanf::sscanf!(metaline, "{}:{}", name, value).map_err(|_| {
                            PageParseError::WrongMetadataFormat(metaline.to_owned())
                        })?;
                        meta.insert(name.trim().to_owned(), value.trim().to_owned());
                    }
                    meta
                },
            }),
            "categories" => Ok(Self::Categories {
                categories: source
                    .next_text_prefixed("--", true)?
                    .split('\n')
                    .map(str::trim)
                    .map(str::to_owned)
                    .collect(),
            }),
            _ => Err(PageParseError::UnknownSection(section.to_owned())),
        }
    }

    pub fn to_html(&self, project_root: &str) -> Result<String, PageBuildError> {
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
                    .map(|title| {
                        format!(
                            "<{}>{}</{}>",
                            $tag,
                            text_to_html(project_root, &title),
                            $tag
                        )
                    })
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
            Self::Text {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}>{}</{tag}>",
                attributes!(attributes),
                text_to_html(project_root, content)
            )),
            Self::TextWrapper {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}>{}<p>{}</p></{tag}>",
                attributes!(attributes),
                title!(attributes),
                text_to_html(project_root, content)
            )),
            Self::Container {
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
                        html.push_str(&section.to_html(project_root)?);
                    }
                    html
                },
            )),
            Self::Code {
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
            Self::Tag { tag, attributes } => Ok(format!("<{tag}{} />", attributes!(attributes))),

            Self::Bookmark {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"bookmark\"{}>{}{}</div>",
                attributes!(attributes),
                title!(attributes, "h3 class = \"bookmarkTitle\""),
                text_to_html(project_root, content),
            )),
            Self::Notes {
                class,
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"{}\"{}>{}<ul>{}</ul></div>",
                class,
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content.iter().map(|item| format!(
                        "<li><p>{}</p></li>",
                        text_to_html(project_root, item)
                    )),
                    ""
                ),
            )),
            Self::List {
                tag,
                attributes,
                content,
            } => Ok(format!(
                "<div{}>{}<{tag}>{}</{tag}></div>",
                attributes!(attributes),
                title!(attributes),
                join_iter(
                    content.iter().map(|item| format!(
                        "<li><p>{}</p></li>",
                        text_to_html(project_root, item)
                    )),
                    ""
                ),
            )),
            Self::Checklist {
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
                            project_root,
                            item.strip_prefix("[]")
                                .or_else(|| item.strip_prefix("[x]"))
                                .unwrap()
                        )
                    )),
                    ""
                ),
            )),
            Self::Image { src, attributes } => Ok(format!(
                "{}<image src = \"{}\"{} />",
                title!(attributes, "h2 class = \"imageTitle\""),
                format_link(project_root, src),
                attributes!(attributes)
            )),
            Self::Youtube { id } => Ok(format!(
                concat!(
                    r#"<iframe width="623" height="350" src="https://www.youtube-nocookie.com/embed/{}" "#,
                    r#"title="YouTube video player" allow="accelerometer; autoplay; clipboard-write; "#,
                    r#"encrypted-media; gyroscope; picture-in-picture; web-share" allowfullscreen=""></iframe>"#,
                ),
                id
            )),

            Self::Hidden { content } => Ok(format!("<!-- {} -->", escape_html(content))),
            Self::Metadata { data: _ } => Ok(String::new()),
            Self::Categories { categories: _ } => Ok(String::new()),
        }
    }
}

pub fn escape_html(code: &str) -> String {
    code.replace('&', "&amp")
        .replace('<', "&lt")
        .replace('>', "&gt")
}

pub fn text_to_html(project_root: &str, text: &str) -> String {
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
            regex_replace(&$attrs, r"(\w+)\s*:\s*(\w+)\|?", |captures| {
                format!("{} = \"{}\"", &captures[1], &captures[2])
            })
        };
    }

    macro_rules! wrap_tag {
        ($tag: expr, $attrs: expr, $content: expr) => {
            format!("<{} {}>{}</{}>", $tag, $attrs, $content, $tag)
        };
    }

    fn make_link(project_root: &str, text: &str, link: &str) -> String {
        let (link, attrs) = link.split_once('|').unwrap_or((link, ""));
        wrap_tag!(
            "a",
            format!(
                "href = \"{}\"{}",
                format_link(project_root, link),
                format_attrs!(attrs)
            ),
            text
        )
    }

    let text = escape_html(text);

    // Tag
    let text = regex_replace(
        &text,
        r"<<(\w+)\s*\|(.*?)\|(.*?)>>",
        |captures| match &captures[1] {
            "link" => make_link(project_root, &captures[2], &captures[3]),
            tag => wrap_tag!(tag, format_attrs!(captures[3]), &captures[2]),
        },
    );

    // Shortcuts
    let text = regex_replace(&text, r">(.*?)>(.*?)>", |captures| {
        make_link(project_root, &captures[1], &captures[2])
    });

    let text = regex_replace(&text, r"\*(.*?)\*(.*?)\*", |captures| {
        wrap_tag!("strong", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"_(.*?)_(.*?)_", |captures| {
        wrap_tag!("em", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"\~(.*?)\~(.*?)\~", |captures| {
        wrap_tag!("s", format_attrs!(captures[2]), &captures[1])
    });
    let text = regex_replace(&text, r"`(.*?)`(.*?)`", |captures| {
        format!(
            "<pre><code {}>{}</code></pre>",
            (format_attrs!(captures[2])),
            (&captures[1])
        )
    });
    text.replace('\n', "<br>")
}

pub fn format_link(project_root: &str, link: &str) -> String {
    if let Some(local_url) = link.strip_prefix('/') {
        return relative_path_to(project_root, local_url);
    }

    link.to_owned()
}

pub fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
