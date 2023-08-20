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
    Note {
        attributes: Vec<Attribute>,
        content: String,
    },
    Notes {
        attributes: Vec<Attribute>,
        content: Vec<String>,
    },
    Checklist {
        attributes: Vec<Attribute>,
        content: Vec<String>,
        todo: bool,
    },
}

impl Section {
    pub fn parse<R: std::io::BufRead>(
        source: &mut super::Reader<R>,
        section: &str,
    ) -> Result<Self, PageParseError> {
        match section {
            "title" | "subtitle" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" => {
                Ok(Self::Text {
                    tag: match section {
                        "title" => "h1 class=\"title\"",
                        "subtitle" => "p class=\"subtitle\"",
                        tag => tag,
                    }
                    .to_owned(),
                    attributes: source.next_attrs()?,
                    content: source.next_text()?,
                })
            }
            "div/" | "script/" | "code/" => {
                let tag = section.strip_suffix('/').unwrap();
                let attributes = source.next_attrs()?;
                Ok(match tag {
                    "code" | "script" => Self::Code {
                        tag: tag.to_owned(),
                        attributes,
                        content: source.next_text_end_tag("".to_owned(), tag, true)?,
                    },
                    tag => Self::Container {
                        tag: tag.to_owned(),
                        attributes,
                        content: source.next_sections(Some(tag))?,
                    },
                })
            }
            "note" => Ok(Self::Note {
                attributes: source.next_attrs()?,
                content: source.next_text()?,
            }),
            "notes" => Ok(Self::Notes {
                attributes: source.next_attrs()?,
                content: source.next_list("- ")?,
            }),
            "checklist" | "todo" => Ok(Self::Checklist {
                attributes: source.next_attrs()?,
                content: source.next_list_raw(
                    |line| line.starts_with("[] ") || line.starts_with("[x] "),
                    |line| Some(line),
                )?,
                todo: section == "todo",
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
            ($attrs: expr) => {
                $attrs
                    .iter()
                    .find_map(|attr| match attr {
                        Attribute::Title(title) => Some(format!("<h4>{}</h4>", title.as_str())),
                        _ => None,
                    })
                    .unwrap_or_default()
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
            Section::Container {
                tag,
                attributes,
                content,
            } => Ok(format!("<{tag}{}>{}</{tag}>", attributes!(attributes), {
                let mut html = String::new();
                for section in content {
                    html.push_str(&section.to_html()?);
                }
                html
            },)),
            Section::Code {
                tag,
                attributes,
                content,
            } => Ok(match tag.as_str() {
                "code" => format!(
                    "<pre><code{}>{}</pre></code>",
                    attributes!(attributes),
                    escape_html(content),
                ),
                tag => format!(
                    "<{tag}{}>{}</{tag}>",
                    attributes!(attributes),
                    escape_html(content),
                ),
            }),
            Section::Note {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"note\"{}><h4>{}</h4><p>{}</p></div>",
                attributes!(attributes),
                title!(attributes),
                content,
            )),
            Section::Notes {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"note\"{}><h4>{}</h4><ul>{}</ul></div>",
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
                        item.strip_prefix("[] ")
                            .or_else(|| item.strip_prefix("[x] "))
                            .unwrap()
                    )),
                    ""
                ),
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
    escape_html(text).replace('\n', "<br>")
}

pub fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
