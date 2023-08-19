use build_html::HtmlContainer;

use super::attribute::Attribute;
use super::{PageBuildError, PageParseError};

#[derive(Clone, Debug, PartialEq)]
pub enum Section {
    Text {
        tag: String,
        attributes: Vec<Attribute>,
        content: String,
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
                        "title" => "h1",
                        "subtitle" => "p",
                        tag => tag,
                    }
                    .to_owned(),
                    attributes: source.next_attrs()?,
                    content: source.next_text()?,
                })
            }
            _ => Err(PageParseError::UnknownSection(section.to_owned())),
        }
    }

    pub fn add_to_page(&self, page: &mut build_html::HtmlPage) -> Result<(), PageBuildError> {
        #[allow(clippy::match_single_binding)]
        match self {
            _ => self.add_to_html(page)?,
        }
        Ok(())
    }

    fn add_to_html(&self, html: &mut impl HtmlContainer) -> Result<(), PageBuildError> {
        match self {
            Section::Text {
                tag,
                attributes,
                content,
            } => html.add_html(format!(
                "<{tag}{}>{}</{tag}>",
                attributes.iter().fold("".to_owned(), |buffer, arg| format!("{} {}", buffer, arg.to_html())),
                text_to_html(content)
            )),
        }
        Ok(())
    }
}

pub fn text_to_html(text: &str) -> String {
    text.replace('\n', "<br>")
}
