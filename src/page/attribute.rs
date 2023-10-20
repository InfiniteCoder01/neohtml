use super::PageParseError;

/// An attribute
#[derive(Clone, Debug, PartialEq)]
pub enum Attribute {
    // ! AccessKey(String),
    // ! AutoCapitalize(String),
    // ! AutoFocus,
    /// -- alt: Alt text
    Alt(String),
    // ! By(String),
    // ! Cite(String),
    /// -- class: alfa bravo
    Class(String),
    // ! ContentEditable(String),
    // ! Generic((String, String)),
    /// -- hidden
    Hidden,
    /// -- id: charlie
    Id(String),
    // ! Link(String),
    /// -- show
    Show,
    // ! ShowTitle(String),
    // ! Subtitle(String),
    /// -- src: source.js
    Src(String),
    /// -- tile: Neopolitan
    Title(String),
    /// -- subtitle: Some subtitle, works for ref
    Subtitle(String),
    // ! Type(String),
    /// -- by: Author
    By(String),
    /// -- source: A book of quotes
    Source(String),
    /// -- url: https://example.com/quote_source_url
    Url(String),
}

impl Attribute {
    pub(super) fn parse(attr: &str) -> Result<Option<Attribute>, PageParseError> {
        let mut attr_name = String::new();
        let mut attr_value = String::new();
        let attr_value = if scanf::sscanf!(attr, "{}: {}", attr_name, attr_value).is_ok() {
            Some(attr_value)
        } else {
            attr_name = attr.to_owned();
            None
        };

        macro_rules! with_arg {
            ($attr: path) => {
                Ok(Some($attr(attr_value.ok_or(
                    PageParseError::MissingAttributeArgument(attr_name),
                )?)))
            };
        }

        macro_rules! no_args {
            ($attr: path) => {{
                if let Some(value) = attr_value {
                    Err(PageParseError::UnexpectedArgument(value, attr_name))
                } else {
                    Ok(Some($attr))
                }
            }};
        }

        match attr_name.as_str() {
            "alt" => with_arg!(Attribute::Alt),
            "class" => with_arg!(Attribute::Class),
            "hidden" => no_args!(Attribute::Hidden),
            "id" => with_arg!(Attribute::Id),
            "show" => no_args!(Attribute::Show),
            "src" => with_arg!(Attribute::Src),
            "title" => with_arg!(Attribute::Title),
            "subtitle" => with_arg!(Attribute::Subtitle),
            "by" => with_arg!(Attribute::By),
            "source" => with_arg!(Attribute::Source),
            "url" => with_arg!(Attribute::Url),
            _ => Ok(None),
        }
    }

    pub(super) fn to_html(&self) -> Option<String> {
        match self {
            Attribute::Alt(alt) => Some(format!("alt=\"{alt}\"")),
            Attribute::Class(class) => Some(format!("class=\"{class}\"")),
            Attribute::Hidden => Some(String::from("hidden")),
            Attribute::Id(id) => Some(format!("id=\"{id}\"")),
            Attribute::Show => None,
            Attribute::Src(src) => Some(format!("src=\"{src}\"")),
            Attribute::Title(title) => Some(format!("title=\"{title}\"")),
            Attribute::Subtitle(_) => None,
            Attribute::By(_) => None,
            Attribute::Source(_) => None,
            Attribute::Url(_) => None,
        }
    }
}
