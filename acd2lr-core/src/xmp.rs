use std::collections::HashMap;

use thiserror::Error;
use xml::name::OwnedName;

use crate::{
    acdsee::{AcdSeeData, AcdSeeError},
    TagHierarchy,
};

mod rule;
pub use rule::*;

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

    fn acdsee_bag_value(&self, local_name: &str) -> Vec<String> {
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
            .take_while(|evt| {
                // Look for the right EndElement
                if let xml::reader::XmlEvent::EndElement { name, .. } = evt {
                    !(name.namespace.as_deref() == Some(crate::ns::ACDSEE)
                        && name.local_name == local_name)
                } else {
                    true
                }
            })
            .filter_map(|item| {
                if let xml::reader::XmlEvent::Characters(chs) = item {
                    Some(chs.to_owned())
                } else {
                    None
                }
            })
            .collect()
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
            keywords: self.acdsee_bag_value("keywords"),
        })
    }

    pub fn write_events(
        &self,
        rules: Vec<RewriteRule>,
    ) -> Result<Vec<xml::reader::XmlEvent>, WriteError> {
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

        // Add all rule namespaces
        for rule in &rules {
            if let Some(namespace) = rule.namespace() {
                if !all_namespaces.contains(rule.prefix()) {
                    all_namespaces.put(rule.prefix(), namespace);
                }
            }
        }

        // Add all rules to a hash map to speed up lookups
        let mut rules: HashMap<_, _> = rules
            .into_iter()
            .map(|rule| ((rule.namespace(), rule.local_name()), rule))
            .collect();

        enum State {
            Init,
            InDescription,
            SkipDescription,
        }

        let mut state = State::Init;
        let mut pending_end_element = None;
        let mut evt_iter = self.events.iter();

        while let Some(evt) = evt_iter.next() {
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
                        xml::reader::XmlEvent::StartDocument { .. } => {
                            // Just skip this
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
                            pending_end_element = Some((*evt).clone());
                        }
                        xml::reader::XmlEvent::StartElement { name, .. } => {
                            let id = (name.namespace.as_deref(), name.local_name.as_str());
                            if let Some(rule) = rules.get(&id) {
                                if rule.matches(&name.borrow()) {
                                    // Buffer all events
                                    let mut rule_events = Vec::with_capacity(6);
                                    rule_events.push(evt);

                                    let mut level = 1;
                                    while level > 0 {
                                        if let Some(evt) = evt_iter.next() {
                                            match evt {
                                                xml::reader::XmlEvent::StartElement { .. } => {
                                                    level += 1;
                                                }
                                                xml::reader::XmlEvent::EndElement { .. } => {
                                                    level -= 1;
                                                }
                                                _ => {}
                                            }

                                            rule_events.push(evt);
                                        } else {
                                            break;
                                        }
                                    }

                                    evts.extend(
                                        rule.run(&rule_events[..])
                                            .map_err(|_| WriteError::RuleFailed(rule.name()))?
                                            .into_iter(),
                                    );
                                    rules.remove(&id);
                                    continue;
                                }
                            }

                            evts.push(evt.clone());
                        }
                        other => {
                            evts.push(other.clone());
                        }
                    }
                }
                State::SkipDescription => {
                    match evt {
                        xml::reader::XmlEvent::StartElement { name, .. }
                            if name.namespace.as_deref() == Some(crate::ns::RDF)
                                && name.local_name == "Description" =>
                        {
                            // Start description, we're skipping this
                            state = State::InDescription;
                            pending_end_element.take();
                        }
                        other => {
                            if let Some(evt) = pending_end_element.take() {
                                // Before we close the rdf:Description, we need to make sure we ran
                                // all rules
                                for (_, rule) in rules.drain() {
                                    evts.extend(
                                        rule.run(&[])
                                            .map_err(|_| WriteError::RuleFailed(rule.name()))?
                                            .into_iter(),
                                    );
                                }

                                evts.push(evt);
                            }

                            evts.push(other.clone());
                        }
                    }
                }
            }
        }

        Ok(evts)
    }
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("rule failed for node {:?}", 0)]
    RuleFailed(OwnedName),
}
