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
    Note {
        attributes: Vec<Attribute>,
        content: String,
    },
    Notes {
        attributes: Vec<Attribute>,
        content: Vec<String>,
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
            "startdiv" => {
                let tag = section.strip_prefix("start").unwrap();
                Ok(Self::Container {
                    tag: tag.to_owned(),
                    attributes: source.next_attrs()?,
                    content: source.next_sections(Some(tag))?,
                })
            }
            "note" => Ok(Self::Note {
                attributes: source.next_attrs()?,
                content: source.next_text()?,
            }),
            "notes" => Ok(Self::Notes {
                attributes: source.next_attrs()?,
                content: source.next_notes()?,
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
            ($attrs: expr, $default: literal) => {
                $attrs
                    .iter()
                    .find_map(|attr| match attr {
                        Attribute::Title(title) => Some(title.as_str()),
                        _ => None,
                    })
                    .unwrap_or($default)
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
            Section::Note {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"note\"{}><h4>{}</h4><p>{}</p></div>",
                attributes!(attributes),
                title!(attributes, "NOTE"),
                content,
            )),
            Section::Notes {
                attributes,
                content,
            } => Ok(format!(
                "<div class = \"note\"{}><h4>{}</h4><ul>{}</ul></div>",
                attributes!(attributes),
                title!(attributes, "NOTES"),
                join_iter(
                    content
                        .iter()
                        .map(|item| format!("<li><p>{}</p></li>", item)),
                    ""
                ),
            )),
        }
    }
}

pub fn text_to_html(text: &str) -> String {
    text.replace('\n', "<br>")
}

pub fn join_iter(iter: impl Iterator<Item = String>, intersperse: &str) -> String {
    Itertools::intersperse(iter, intersperse.to_owned()).collect::<String>()
}
