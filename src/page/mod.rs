pub mod attribute;
pub mod section;
use build_html::Html;
use build_html::HtmlContainer;
use build_html::HtmlPage;
use section::Section;
use thiserror::Error;

use self::attribute::Attribute;

pub fn has_section_prefix(line: &str) -> bool {
    line.starts_with("->")
        || line.starts_with("--")
        || line.starts_with("```")
        || line.starts_with('#')
}

pub fn strip_section_prefix(line: &str) -> Option<&str> {
    line.strip_prefix("->")
        .or_else(|| line.strip_prefix("--"))
        .or_else(|| {
            if line.starts_with("```") {
                Some(line)
            } else {
                None
            }
        })
        .map(|line| line.trim())
}

pub fn has_attr_prefix(line: &str) -> bool {
    line.starts_with("->") || line.starts_with("--")
}

pub fn strip_attr_prefix(line: &str) -> Option<&str> {
    line.strip_prefix("->")
        .or_else(|| line.strip_prefix("--"))
        .map(|line| line.trim())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    sections: Vec<Section>,
}

impl Page {
    pub fn from_source(source: &str) -> Result<Self, PageParseError> {
        Self::new(std::io::Cursor::new(source))
    }

    pub fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, PageParseError> {
        Self::new(std::io::BufReader::new(std::fs::File::open(path)?))
    }
}

impl Page {
    pub fn new<R: std::io::BufRead>(source: R) -> Result<Self, PageParseError> {
        Ok(Self {
            sections: Reader::new(source).next_sections(None)?,
        })
    }

    pub fn to_html(&self) -> Result<HtmlPage, PageBuildError> {
        let mut page = HtmlPage::new();
        page.add_head_link(
            "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.8.0/styles/monokai.min.css",
            "stylesheet",
        );
        page.add_script_link(
            "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.8.0/highlight.min.js",
        );
        page.add_head_link("global.css", "stylesheet");
        page.add_script_literal("hljs.highlightAll();");
        for section in &self.sections {
            page.add_html(section.to_html()?);
        }
        Ok(page)
    }

    pub fn to_html_string(&self) -> Result<String, PageBuildError> {
        Ok(self.to_html()?.to_html_string())
    }
}

// * ------------------------------------ Reader ------------------------------------ * //
pub struct Reader<R> {
    lines: std::io::Lines<R>,
    peek: Option<String>,
}

impl<R: std::io::BufRead> Reader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            lines: reader.lines(),
            peek: None,
        }
    }

    pub fn peek_line(&mut self) -> Result<Option<&String>, PageParseError> {
        if self.peek.is_none() {
            if let Some(line) = self.lines.next() {
                self.peek = Some(line?)
            }
        }
        Ok(self.peek.as_ref())
    }

    pub fn next_line(&mut self) -> Result<Option<String>, PageParseError> {
        self.peek_line()?;
        Ok(self.peek.take())
    }

    pub fn next_line_if(
        &mut self,
        pred: impl FnOnce(&str) -> bool,
    ) -> Result<Option<String>, PageParseError> {
        if let Some(line) = self.peek_line()? {
            if pred(line) {
                return Ok(self.peek.take());
            }
        }
        Ok(None)
    }

    pub fn next_line_if_map(
        &mut self,
        map: impl FnOnce(&str) -> Option<&str>,
    ) -> Result<Option<String>, PageParseError> {
        if let Some(line) = self.peek_line()? {
            if let Some(line) = map(line) {
                let line = line.to_owned();
                self.peek = None;
                return Ok(Some(line));
            }
        }
        Ok(None)
    }

    pub fn skip_blank(&mut self) -> Result<bool, PageParseError> {
        Ok(self.next_line_if(|line| line.trim().is_empty())?.is_some())
    }

    pub fn skip_blanks(&mut self) -> Result<(), PageParseError> {
        while self.skip_blank()? {}
        Ok(())
    }

    fn next_text(
        &mut self,
        mut filter_map: impl FnMut(&str) -> Option<&str>,
        raw: bool,
    ) -> Result<String, PageParseError> {
        self.skip_blanks()?;
        let mut text = String::new();
        loop {
            let line = if let Some(line) = self.next_line_if_map(&mut filter_map)? {
                line
            } else {
                break;
            };

            #[allow(clippy::collapsible_else_if)]
            if raw {
                text.push_str(&line);
                text.push('\n');
            } else {
                if line.trim().is_empty() {
                    text.push('\n');
                } else {
                    if !text.ends_with('\n') {
                        text.push(' ');
                    }
                    text.push_str(&line);
                }
            }
        }
        if raw {
            Ok(text.trim_end().to_owned())
        } else {
            Ok(text.trim().to_owned())
        }
    }

    fn next_text_until(
        &mut self,
        mut until: impl FnMut(&str) -> bool,
        raw: bool,
    ) -> Result<String, PageParseError> {
        self.next_text(|line| if until(line) { None } else { Some(line) }, raw)
    }

    fn next_text_until_tag(&mut self, tag: &str, raw: bool) -> Result<String, PageParseError> {
        let text = self.next_text_until(
            |line| {
                if tag == "```" && line == tag {
                    return true;
                }
                if let Some(section) = strip_section_prefix(line) {
                    if let Some(section_tag) = section.strip_prefix('/') {
                        if section_tag == tag {
                            return true;
                        }
                    }
                }
                false
            },
            raw,
        )?;
        self.next_line()?;
        Ok(text)
    }

    fn next_text_until_section(&mut self, raw: bool) -> Result<String, PageParseError> {
        self.next_text_until(has_section_prefix, raw)
    }

    // * ----------------------------------- Specials ----------------------------------- * //
    pub fn next_attr(&mut self) -> Result<Option<Attribute>, PageParseError> {
        if let Some(line) = self.next_line_if(has_attr_prefix)? {
            if let Some(attr) = strip_attr_prefix(&line) {
                if let Some(attr) = Attribute::parse(attr)? {
                    return Ok(Some(attr));
                } else {
                    self.peek = Some(line);
                }
            }
        }
        Ok(None)
    }

    pub fn next_attrs(&mut self) -> Result<Vec<Attribute>, PageParseError> {
        let mut attrs = Vec::new();
        while let Some(attr) = self.next_attr()? {
            attrs.push(attr);
        }
        Ok(attrs)
    }

    pub fn next_list(
        &mut self,
        filter: impl Fn(&str) -> bool,
    ) -> Result<Vec<String>, PageParseError> {
        self.skip_blanks()?;
        let mut list = Vec::new();
        while let Some(line) = self.peek_line()? {
            if !filter(line) {
                break;
            }
            let mut fist_line = true;
            let entry = self.next_text_until(
                |line| {
                    if fist_line {
                        fist_line = false;
                        return false;
                    }
                    has_section_prefix(line) || filter(line)
                },
                false,
            )?;
            list.push(entry);
            self.skip_blanks()?;
        }
        Ok(list)
    }

    pub fn next_list_prefixed(&mut self, prefix: &str) -> Result<Vec<String>, PageParseError> {
        Ok(self
            .next_list(|line| line.starts_with(prefix))?
            .iter()
            .map(|entry| entry.strip_prefix(prefix).unwrap().to_owned())
            .collect())
    }

    pub fn next_sections(&mut self, end_tag: Option<&str>) -> Result<Vec<Section>, PageParseError> {
        let mut sections = Vec::new();
        loop {
            self.skip_blanks()?;
            let line = if let Some(line) = self.peek_line()? {
                if let Some(end_tag) = end_tag {
                    if let Some(section) = strip_section_prefix(line) {
                        if let Some(tag) = section.strip_prefix('/') {
                            if tag == end_tag {
                                break;
                            }
                        }
                    }
                }
                line
            } else {
                break;
            };

            if line.starts_with('#') {
                let prefix = line.chars().take_while(|&c| c == '#').collect::<String>();
                sections.push(Section::Text {
                    tag: format!("h{}", prefix.len()),
                    attributes: Vec::new(),
                    content: self.next_text(
                        |line| {
                            if line.trim().is_empty() {
                                Some(line)
                            } else {
                                let line = line.strip_prefix(&prefix)?;
                                if line.starts_with('#') {
                                    None
                                } else {
                                    Some(line)
                                }
                            }
                        },
                        false,
                    )?,
                });
            } else if let Some(section) = strip_section_prefix(line) {
                let section = section.to_owned();
                self.next_line()?;
                sections.push(Section::parse(self, &section)?);
            } else {
                sections.push(Section::Text {
                    tag: "p".to_owned(),
                    attributes: Vec::new(),
                    content: self.next_text_until(has_section_prefix, false)?,
                });
            }
        }
        Ok(sections)
    }
}

// * ------------------------------------- Error ------------------------------------ * //
#[derive(Error, Debug)]
pub enum PageParseError {
    #[error("Page load error")]
    IOError(
        #[source]
        #[from]
        std::io::Error,
    ),
    #[error("Expected attribute, got '{0}'")]
    ExpectedAttribute(String),
    #[error("Expected section, got '{0}'")]
    ExpectedSection(String),
    #[error("Unknown section: '{0}'")]
    UnknownSection(String),
    #[error("Missing attribute argument in attribute '{0}'")]
    MissingAttributeArgument(String),
    #[error("Unexpected argument '{0}' in attribute '{1}', this attribute is ment to be used without arguments")]
    UnexpectedArgument(String, String),
    #[error("Title/Subtitle section is empty!")]
    EmptyTitle,
    #[error("Expected video ID")]
    ExpectedVideoID,
}

#[derive(Error, Debug)]
pub enum PageBuildError {}
