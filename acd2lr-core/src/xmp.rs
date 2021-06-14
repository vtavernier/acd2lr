use thiserror::Error;

use crate::{
    acdsee::{AcdSeeData, AcdSeeError},
    TagHierarchy,
};

#[derive(Debug, Clone)]
pub struct XmpData {
    events: Vec<xml::reader::XmlEvent>,
}

#[derive(Debug, Error)]
pub enum XmpParseError {
    #[error(transparent)]
    Xml(#[from] xml::reader::Error),
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

    fn acdsee_tag_value(&self, local_name: &str) -> Option<String> {
        let result = self.acdsee_attr_value(local_name).or_else(|| {
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
                .and_then(|evt| match evt {
                    xml::reader::XmlEvent::EndElement { .. } => Some(String::new()),
                    xml::reader::XmlEvent::Characters(value) => Some(value.to_owned()),
                    _ => None,
                })
        });

        tracing::trace!(value = ?result, "acdsee tag {}", local_name);
        result
    }

    pub fn acdsee_data(&self) -> Result<AcdSeeData, AcdSeeError> {
        Ok(AcdSeeData {
            caption: self.acdsee_tag_value("caption"),
            categories: self
                .acdsee_tag_value("categories")
                .map(|value| TagHierarchy::from_acdsee_categories(&value))
                .transpose()?,
            datetime: self
                .acdsee_tag_value("datetime")
                .and_then(|val| if val.is_empty() { None } else { Some(val) })
                .map(|val| val.parse())
                .transpose()?,
            author: self.acdsee_tag_value("author"),
            rating: self
                .acdsee_tag_value("rating")
                .map(|value| value.parse().ok().unwrap_or(0)),
            notes: self.acdsee_tag_value("notes"),
            tagged: self
                .acdsee_tag_value("tagged")
                .map(|value| value.to_ascii_lowercase() == "true"),
            collections: self.acdsee_tag_value("collections"),
        })
    }

    pub fn write_events(&self) -> Vec<xml::reader::XmlEvent> {
        let mut evts = Vec::with_capacity(self.events.len());

        // Find all namespaces
        let mut all_namespaces = xml::namespace::Namespace::empty();
        for evt in &self.events {
            match evt {
                xml::reader::XmlEvent::StartElement {
                    name,
                    attributes: _,
                    namespace,
                } => {
                    if name.namespace.as_deref() == Some(crate::ns::RDF)
                        && name.local_name == "Description"
                    {
                        // A rdf::Description start
                        all_namespaces.extend(namespace.into_iter());
                    }
                }
                _ => {}
            }
        }

        enum State {
            Init,
            InDescription,
            SkipDescription,
        }

        let mut state = State::Init;
        let mut pending_end_element = None;
        for evt in &self.events {
            match state {
                State::Init => {
                    match evt {
                        xml::reader::XmlEvent::StartElement {
                            name, attributes, ..
                        } if name.namespace.as_deref() == Some(crate::ns::RDF)
                            && name.local_name == "Description" =>
                        {
                            // A description start node
                            evts.push(xml::reader::XmlEvent::StartElement {
                                name: name.clone(),
                                attributes: attributes.clone(),
                                namespace: all_namespaces.clone(),
                            });

                            state = State::InDescription;
                        }
                        xml::reader::XmlEvent::StartDocument { .. } => { // Just skip this
                        }
                        other => {
                            evts.push(other.clone());
                        }
                    }
                }
                State::InDescription => {
                    match evt {
                        xml::reader::XmlEvent::EndElement { name }
                            if name.namespace.as_deref() == Some(crate::ns::RDF)
                                && name.local_name == "Description" =>
                        {
                            // Finishing a description node
                            state = State::SkipDescription;
                            pending_end_element = Some(evt.clone());
                        }
                        other => {
                            evts.push(other.clone());
                        }
                    }
                }
                State::SkipDescription => {
                    match evt {
                        xml::reader::XmlEvent::StartElement { name, .. } => {
                            if name.namespace.as_deref() == Some(crate::ns::RDF)
                                && name.local_name == "Description"
                            {
                                // Start description, we're skipping this
                                state = State::InDescription;
                                pending_end_element.take();
                            }
                        }
                        other => {
                            if let Some(evt) = pending_end_element.take() {
                                evts.push(evt);
                            }

                            evts.push(other.clone());
                        }
                    }
                }
            }
        }

        evts
    }
}
