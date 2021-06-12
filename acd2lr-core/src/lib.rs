use std::collections::HashSet;

use serde::Serialize;

pub mod ns;
pub mod xmp;
pub mod xpacket;

fn xml_reader<R: std::io::Read>(reader: R) -> xml::EventReader<R> {
    xml::EventReader::new_with_config(
        reader,
        xml::ParserConfig::new()
            .trim_whitespace(true)
            .cdata_to_characters(true),
    )
}

/// A tag in a given hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Tag(Vec<String>);

/// A tag hierarchy
#[derive(Debug, Clone, Default, Serialize)]
pub struct TagHierarchy(HashSet<Tag>);

impl FromAcdSee for TagHierarchy {
    type Error = xml::reader::Error;

    fn from_acdsee(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(Self::default());
        }

        // Decode the XML for the tags
        let reader = xml_reader(value.as_bytes());

        // We need a stack for the current tags we're in
        let mut tag_stack: Vec<(String, bool)> = Vec::new();
        let mut set = HashSet::new();

        for event in reader.into_iter() {
            match event? {
                xml::reader::XmlEvent::StartElement {
                    name, attributes, ..
                } => {
                    // We're entering a new category
                    if name.local_name == "Category" {
                        // The category name is in the value, so use an empty string first, but
                        // record the Assigned value
                        tag_stack.push((
                            String::new(),
                            attributes
                                .iter()
                                .find(|attr| attr.name.local_name == "Assigned")
                                .map(|attr| attr.value == "1")
                                .unwrap_or(false),
                        ));
                    }
                }
                xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == "Category" {
                        if let Some((_, assigned)) = tag_stack.last() {
                            if *assigned {
                                set.insert(Tag(tag_stack.iter().map(|(s, _)| s.clone()).collect()));
                            }

                            tag_stack.pop();
                        }
                    }
                }
                xml::reader::XmlEvent::Characters(value) => {
                    if let Some(last) = tag_stack.last_mut() {
                        // Assign category value
                        last.0 = value;
                    }
                }
                _ => {}
            }
        }

        Ok(Self(set))
    }
}

impl TagHierarchy {
    pub fn new() -> Self {
        Self::default()
    }
}

pub trait FromAcdSee: Sized {
    type Error;

    fn from_acdsee(value: &str) -> Result<Self, Self::Error>;
}
