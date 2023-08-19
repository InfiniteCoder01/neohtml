use super::PageParseError;

#[derive(Clone, Debug, PartialEq)]
pub enum Attribute {
    // AccessKey(String),
    // AutoCapitalize(String),
    // AutoFocus,
    // By(String),
    // Cite(String),
    // Class(Vec<String>),
    // ContentEditable(String),
    // Generic((String, String)),
    Hidden,
    Id(String),
    // Link(String),
    // Show(String),
    // ShowTitle(String),
    // Subtitle(String),
    // Title(String),
    // Type(String),
    // Url(String),
}

impl Attribute {
    pub fn parse(attr: &str) -> Result<Attribute, PageParseError> {
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
                Ok($attr(attr_value.ok_or(
                    PageParseError::MissingAttributeArgument(attr_name),
                )?))
            };
        }

        macro_rules! no_args {
            ($attr: path) => {{
                if let Some(value) = attr_value {
                    Err(PageParseError::UnexpectedArgument(value, attr_name))
                } else {
                    Ok($attr)
                }
            }};
        }

        match attr_name.as_str() {
            "id" => with_arg!(Attribute::Id),
            "hidden" => no_args!(Attribute::Hidden),
            _ => Err(PageParseError::UnknownAttribute(attr.to_owned())),
        }
        // let mut attr_name = String::new();
        // let mut attr_value = String::new();
        // scanf::sscanf!(attr, "{}: {}", attr_name, attr_value)
        //     .map_err(|_| PageParseError::ExpectedAttribute(attr.to_owned()))?;
    }

    pub fn to_html(&self) -> String {
        match self {
            Attribute::Hidden => "hidden".to_owned(),
            Attribute::Id(id) => format!("id=\"{id}\""),
        }
    }
}
