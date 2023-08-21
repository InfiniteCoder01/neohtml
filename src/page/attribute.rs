use super::PageParseError;

#[derive(Clone, Debug, PartialEq)]
pub enum Attribute {
    // AccessKey(String),
    // AutoCapitalize(String),
    // AutoFocus,
    Alt(String),
    // By(String),
    // Cite(String),
    Class(String),
    // ContentEditable(String),
    // Generic((String, String)),
    Hidden,
    Id(String),
    // Link(String),
    Show,
    // ShowTitle(String),
    // Subtitle(String),
    Src(String),
    Title(String),
    // Type(String),
    // Url(String),
}

impl Attribute {
    pub fn parse(attr: &str) -> Result<Option<Attribute>, PageParseError> {
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
            _ => Ok(None),
        }
        // let mut attr_name = String::new();
        // let mut attr_value = String::new();
        // scanf::sscanf!(attr, "{}: {}", attr_name, attr_value)
        //     .map_err(|_| PageParseError::ExpectedAttribute(attr.to_owned()))?;
    }

    pub fn to_html(&self) -> String {
        match self {
            Attribute::Alt(alt) => format!("alt=\"{alt}\""),
            Attribute::Class(class) => format!("class=\"{class}\""),
            Attribute::Hidden => "hidden".to_owned(),
            Attribute::Id(id) => format!("id=\"{id}\""),
            Attribute::Show => "show".to_owned(),
            Attribute::Src(src) => format!("src=\"{src}\""),
            Attribute::Title(title) => format!("title=\"{title}\""),
        }
    }
}
