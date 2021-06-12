use serde::Serialize;
use thiserror::Error;

use crate::{FromAcdSee, TagHierarchy};

#[derive(Debug, Clone)]
pub struct XmpData {
    events: Vec<xml::reader::XmlEvent>,
}

#[derive(Debug, Error)]
pub enum XmpParseError {
    #[error(transparent)]
    Xml(#[from] xml::reader::Error),
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct AcdSeeData {
    caption: String,
    datetime: Option<chrono::NaiveDateTime>,
    author: String,
    rating: i32,
    notes: String,
    tagged: bool,
    categories: TagHierarchy,
    collections: String,
}

#[derive(Debug, Error)]
pub enum AcdSeeError {
    #[error(transparent)]
    Xml(#[from] xml::reader::Error),
    #[error(transparent)]
    Date(#[from] chrono::ParseError),
}

impl XmpData {
    pub fn parse(source: &[u8]) -> Result<XmpData, XmpParseError> {
        Ok(Self {
            events: crate::xml_reader(source)
                .into_iter()
                .collect::<Result<_, _>>()?,
        })
    }

    fn acdsee_attr_value(&self, local_name: &str) -> Option<String> {
        self.events.iter().find_map(|evt| {
            if let xml::reader::XmlEvent::StartElement {
                name, attributes, ..
            } = evt
            {
                if name.namespace.as_deref() == Some(crate::ns::RDF)
                    && name.local_name == "Description"
                {
                    return attributes.iter().find_map(|attr| {
                        if attr.name.namespace.as_deref() == Some(crate::ns::ACDSEE)
                            && attr.name.local_name == local_name
                        {
                            return Some(attr.value.to_owned());
                        }

                        None
                    });
                }
            }

            None
        })
    }

    fn acdsee_tag_value(&self, local_name: &str) -> String {
        let result = self.acdsee_attr_value(local_name).unwrap_or_else(|| {
            self.events
                .iter()
                .skip_while(|evt| {
                    // Look for the right StartElement
                    if let xml::reader::XmlEvent::StartElement { name, .. } = evt {
                        !(name.namespace.as_deref() == Some(crate::ns::ACDSEE)
                            && name.local_name == local_name)
                    } else {
                        true
                    }
                })
                .skip(1)
                .next()
                .and_then(|evt| {
                    if let xml::reader::XmlEvent::Characters(value) = evt {
                        Some(value.to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(String::new)
        });

        tracing::trace!(value = %result, "acdsee tag {}", local_name);
        result
    }

    pub fn acdsee_data(&self) -> Result<AcdSeeData, AcdSeeError> {
        Ok(AcdSeeData {
            caption: self.acdsee_tag_value("caption"),
            categories: TagHierarchy::from_acdsee(&self.acdsee_tag_value("categories"))?,
            datetime: {
                let datetime = self.acdsee_tag_value("datetime");
                if datetime.is_empty() {
                    None
                } else {
                    Some(datetime.parse()?)
                }
            },
            author: self.acdsee_tag_value("author"),
            rating: self.acdsee_tag_value("rating").parse().ok().unwrap_or(0),
            notes: self.acdsee_tag_value("notes"),
            tagged: self.acdsee_tag_value("tagged").to_ascii_lowercase() == "true",
            collections: self.acdsee_tag_value("collections"),
        })
    }
}
