use super::attribute::Attribute;
use super::{PageBuildError, PageParseError};
use itertools::Itertools;
use std::collections::HashMap;
use std::path::Path;

// * ---------------------------------- Attributes ---------------------------------- * //
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

// * ----------------------------------- Sections ----------------------------------- * //
/// A section
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum Section {
    /// p, h1..h6, title, subtitle, nav, footnote
    Text {
        tag: String,
        class: Option<Vec<String>>,
        // htmlattrs: Vec<(String, String)>,
        attributes: Vec<Attribute>,
        content: String,
    },
    /// aside, blockquote, note, warning
    TextWrapper {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
    },
    /// article, section, div, code, pre, script, html, css
    Container {
        tag: String,
        attributes: Vec<Attribute>,
        content: Vec<Section>,
    },
    /// code, pre, script, html, css
    Code {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
    },
    /// hr
    Tag {
        tag: String,
        attributes: Vec<Attribute>,
    },

    /// bookmark
    Bookmark {
        attributes: Vec<Attribute>,
        content: String,
    },
    /// notes
    Notes {
        class: String,
        attributes: Vec<Attribute>,
        content: Vec<String>,
    },
    /// list, olist
    List {
        tag: String,
        attributes: Vec<Attribute>,
        content: Vec<String>,
    },
    /// checklist, todo
    Checklist {
        attributes: Vec<Attribute>,
        prelude: String,
        content: Vec<String>,
        todo: bool,
    },
    /// image
    Image {
        src: String,
        attributes: Vec<Attribute>,
    },

    /// youtube
    Youtube { id: String },
    /// vimeo
    Vimeo { id: String },

    /// hidden
    Hidden { content: String },
    /// metadata
    Metadata { data: HashMap<String, String> },
    /// cathegories
    Categories { categories: Vec<String> },
}

// * ------------------------------------- Parse ------------------------------------ * //
impl Section {
    pub(super) fn parse<R: std::io::BufRead>(
        source: &mut super::Reader<R>,
        section: &str,
    ) -> Result<Self, PageParseError> {
        fn map_code_tag(tag: &str) -> &str {
            match tag {
                "css" => "style",
                tag => tag,
            }
        }

        if let Some(language) = section.strip_prefix("```") {
            let mut attributes = source.next_attrs()?;
            if !language.is_empty() {
                attributes.push(Attribute::Class(format!("language-{language}")));
            }
            return Ok(Self::Code {
                tag: "code".to_owned(),
                attributes,
                content: source.next_text_until_tag("```", true)?,
            });
        }

        match section {
            "title" | "subtitle" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "nav"
            | "footnote" => Ok(Self::Text {
                tag: match section {
                    "title" => "h1",
                    "subtitle" => "p",
                    "footnote" => "p",
                    tag => tag,
                }
                .to_owned(),
                class: match section {
                    "title" | "subtitle" => Some(vec![section.to_owned()]),
                    _ => None,
                },

                attributes: source.next_attrs()?,
                content: match section {
                    "title" | "subtitle" => {
                        source.skip_blanks()?;
                        source.next_line()?.ok_or(PageParseError::EmptyTitle)?
                    }
                    _ => source.next_text_until_section(false)?,
                },
            }),
            "aside" => Ok(Self::TextWrapper {
                tag: section.to_owned(),
                attributes: source.next_attrs()?,
                content: source.next_text_until_section(false)?,
            }),
            "blockquote" => {
                let attributes = source.next_attrs()?;
                let mut content = source.next_text_until_section(false)?;
                if let Some(by) = attr!(attributes, By) {
                    content.push_str(&format!("\n-- {by}"));
                    if let Some(source) = attr!(attributes, Source) {
                        match attr!(attributes, Url) {
                            Some(url) => content.push_str(&format!(" (>{source}>{url}>)")),
                            None => content.push_str(&format!(" ({source})")),
                        }
                    }
                }

                Ok(Self::TextWrapper {
                    tag: section.to_owned(),
                    attributes,
                    content,
                })
            }
            "ref" => {
                // Title, subtitle, URL
                let mut attributes = source.next_attrs()?;
                let mut content = source.next_text_until_section(false)?;

                if let Some(title) = attr!(attributes, Title) {
                    let title = match attr!(attributes, Url) {
                        Some(url) => format!(">{title}>{url}>"),
                        None => title.to_owned(),
                    };
                    match attr!(attributes, Subtitle) {
                        Some(subtitle) => {
                            content.insert_str(0, &format!("{title} {subtitle}\n"));
                            attributes.remove(
                                attributes
                                    .iter()
                                    .position(|attr| matches!(attr, Attribute::Subtitle(_)))
                                    .unwrap(),
                            );
                        }
                        None => content.insert_str(0, &format!("{title}\n")),
                    }
                    attributes.remove(
                        attributes
                            .iter()
                            .position(|attr| matches!(attr, Attribute::Title(_)))
                            .unwrap(),
                    );
                }

                Ok(Self::TextWrapper {
                    tag: section.to_owned(),
                    attributes,
                    content,
                })
            }
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
                prelude: source.next_text_until(
                    |line| line.starts_with("[]") || line.starts_with("[x]"),
                    false,
                )?,
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
            "vimeo" => Ok(Self::Vimeo {
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
}

// * ------------------------------------- Build ------------------------------------ * //
impl Section {
    pub(super) fn to_html(&self, project_root: &Path) -> Result<String, PageBuildError> {
        // * Attrs
        macro_rules! attributes {
            ($attrs: expr) => {{
                let mut attrs = String::new();
                for attr in $attrs {
                    if let Some(html) = attr.to_html() {
                        attrs.push(' ');
                        attrs.push_str(&html);
                    }
                }
                attrs
            }};
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
                class,
                attributes,
                content,
            } => Ok(format!(
                "<{tag}{}{}>{}{}</{tag}>",
                match class {
                    Some(classes) => format!(
                        " class=\"{}\"",
                        classes
                            .iter()
                            .fold(String::new(), |buffer, class| buffer + class)
                    ),
                    None => String::new(),
                },
                attributes!(attributes),
                title!(attributes),
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
                attr!(attributes, Title)
                    .map(|title| {
                        format!(
                            "<h4>{}</h4>",
                            match attr!(attributes, Url) {
                                Some(url) =>
                                    text_to_html(project_root, &format!(">{title}>{url}>")),
                                None => text_to_html(project_root, title),
                            },
                        )
                    })
                    .unwrap_or_default(),
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
                prelude,
                content,
                todo,
            } => Ok(format!(
                "<div{}>{}<p>{}</p>{}</div>",
                attributes!(attributes),
                title!(attributes),
                text_to_html(project_root, prelude),
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
            Self::Vimeo { id } => Ok(format!(
                concat!(
                    r#"<div style="padding:56.25% 0 0 0;position:relative;">"#,
                    r#"<iframe src="https://player.vimeo.com/video/{}?title=0&byline=0&portrait=0" "#,
                    r#"style="position:absolute;top:0;left:0;width:100%;height:100%;" "#,
                    r#"frameborder="0" "#,
                    r#"allow="autoplay; fullscreen; picture-in-picture" "#,
                    r#"allowfullscreen></iframe></div>"#,
                ),
                id
            )),

            Self::Hidden { content } => Ok(format!("<!-- {} -->", escape_html(content))),
            Self::Metadata { data: _ } => Ok(String::new()),
            Self::Categories { categories: _ } => Ok(String::new()),
        }
    }
}

// * -------------------------------- Text formatting ------------------------------- * //
fn escape_html(code: &str) -> String {
    code.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn text_to_html(project_root: &Path, text: &str) -> String {
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

    fn make_link(project_root: &Path, text: &str, link: &str) -> String {
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

    // Escapes
    let text = text.replace("\\\\", "&bsol;");
    let text = text.replace("\\&lt;", "&#x003c;");
    let text = text.replace("\\&gt;", "&#x003e;");
    let text = text.replace("\\*", "&#x002a;");
    let text = text.replace("\\_", "&#x005f;");
    let text = text.replace("\\~", "&#x007e;");
    let text = text.replace("\\`", "&#x0060;");

    // Tag
    let text = regex_replace(&text, r"<<(\w+)\s*\|(.*?)>>", |captures| {
        match &captures[1] {
            "img" => format!("<img src=\"{}\" />", format_link(project_root, &captures[2])),
            tag => format!("<{tag} {} />", format_attrs!(captures[2])),
        }
    });

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

    let text = regex_replace(&text, r"<(.*?)>", |captures| {
        make_link(project_root, &captures[1], &captures[1])
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
            "<code {}>{}</code>",
            (format_attrs!(captures[2])),
            (&captures[1])
        )
    });
    text.replace('\n', "<br>")
}

fn format_link(project_root: &Path, link: &str) -> String {
    if let Some(local_url) = link.strip_prefix('/') {
        return project_root
            .join(Path::new(local_url))
            .to_string_lossy()
            .into_owned();
    }

    link.to_owned()
}

fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
