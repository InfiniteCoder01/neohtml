pub mod attribute;
pub mod section;
use build_html::Html;
use build_html::HtmlContainer;
use build_html::HtmlPage;
use section::Section;
use thiserror::Error;

use self::attribute::Attribute;

pub fn section_prefixed(line: &str) -> bool {
    line.starts_with("->") || line.starts_with("--")
}

pub fn strip_section_prefix(line: &str) -> Option<&str> {
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
        page.add_head_link("global.css", "stylesheet");
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

    pub fn skip_blank(&mut self) -> Result<bool, PageParseError> {
        Ok(self.next_line_if(|line| line.trim().is_empty())?.is_some())
    }

    pub fn skip_blanks(&mut self) -> Result<(), PageParseError> {
        while self.skip_blank()? {}
        Ok(())
    }

    fn next_text_cond(
        &mut self,
        prefix: String,
        cond: impl Fn(&str) -> bool,
    ) -> Result<String, PageParseError> {
        let mut text = prefix;
        loop {
            let line = if let Some(line) = self.next_line_if(&cond)? {
                line
            } else {
                break;
            };

            if line.trim().is_empty() {
                text.push('\n');
            } else {
                if !text.ends_with('\n') {
                    text.push(' ');
                }
                text.push_str(&line);
            }
        }
        Ok(text.trim().to_owned())
    }

    fn next_text(&mut self) -> Result<String, PageParseError> {
        self.skip_blanks()?;
        self.next_text_cond("".to_owned(), |line| !section_prefixed(line))
    }

    fn next_text_end(&mut self, end: &str) -> Result<String, PageParseError> {
        self.skip_blanks()?;
        let text = self.next_text_cond("".to_owned(), |line| {
            if let Some(section) = strip_section_prefix(line) {
                if let Some(tag) = section.strip_prefix('/') {
                    if tag == end {
                        return false;
                    }
                }
            }
            true
        })?;
        self.next_line()?;
        Ok(text)
    }

    pub fn next_attr(&mut self) -> Result<Option<Attribute>, PageParseError> {
        if let Some(line) = self.next_line_if(section_prefixed)? {
            if let Some(attr) = strip_section_prefix(&line) {
                return Ok(Some(Attribute::parse(attr)?));
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

    pub fn next_note(&mut self) -> Result<Option<String>, PageParseError> {
        self.skip_blanks()?;
        if let Some(line) = self.next_line_if(|line| line.starts_with("- "))? {
            if let Some(note_start) = line.strip_prefix("- ") {
                return Ok(Some(
                    self.next_text_cond(note_start.to_owned(), |line| {
                        !section_prefixed(line) && !line.starts_with("- ")
                    })?,
                ));
            }
        }
        Ok(None)
    }

    pub fn next_notes(&mut self) -> Result<Vec<String>, PageParseError> {
        let mut notes = Vec::new();
        while let Some(note) = self.next_note()? {
            notes.push(note);
        }
        Ok(notes)
    }

    pub fn next_sections(&mut self, end_tag: Option<&str>) -> Result<Vec<Section>, PageParseError> {
        let mut sections = Vec::new();
        loop {
            self.skip_blanks()?;
            let line = if let Some(line) = self.next_line()? {
                if let Some(end_tag) = end_tag {
                    if let Some(section) = strip_section_prefix(&line) {
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

            if let Some(section) = strip_section_prefix(&line) {
                sections.push(Section::parse(self, section)?);
            } else {
                return Err(PageParseError::ExpectedSection(line));
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
    #[error("Unknown attribute: '{0}'")]
    UnknownAttribute(String),
    #[error("Missing attribute argument in attribute '{0}'")]
    MissingAttributeArgument(String),
    #[error("Unexpected argument '{0}' in attribute '{1}', this attribute is ment to be used without arguments")]
    UnexpectedArgument(String, String),
}

#[derive(Error, Debug)]
pub enum PageBuildError {}
