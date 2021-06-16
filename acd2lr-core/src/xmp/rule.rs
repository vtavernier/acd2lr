use thiserror::Error;
use xml::name::OwnedName;

pub struct RewriteRule {
    node_namespace: Option<&'static str>,
    node_name: &'static str,
    node_prefix: &'static str,
    allow_attribute: bool,
    required: bool,
    action: Box<dyn RewriteAction>,
}

#[derive(Debug, Error)]
pub enum RewriteRuleError {
    #[error("attributes are not supported by this rule")]
    Unsupported,
}

impl RewriteRule {
    pub fn new(
        node_namespace: Option<&'static str>,
        node_name: &'static str,
        node_prefix: &'static str,
        allow_attribute: bool,
        required: bool,
        action: impl RewriteAction + 'static,
    ) -> Self {
        Self {
            node_namespace,
            node_name,
            node_prefix,
            allow_attribute,
            required,
            action: Box::new(action),
        }
    }

    pub fn name(&self) -> OwnedName {
        if let Some(ns) = self.namespace() {
            xml::name::OwnedName::qualified::<_, _, _>(self.local_name(), ns, Some(self.prefix()))
        } else {
            xml::name::OwnedName::local(self.local_name())
        }
    }

    pub fn namespace(&self) -> Option<&'static str> {
        self.node_namespace
    }

    pub fn local_name(&self) -> &'static str {
        self.node_name
    }

    pub fn prefix(&self) -> &'static str {
        self.node_prefix
    }

    pub fn allow_attribute(&self) -> bool {
        self.allow_attribute
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn matches(&self, name: &xml::name::Name) -> bool {
        name.local_name == self.node_name && name.namespace.as_deref() == self.node_namespace
    }

    pub fn run(
        &self,
        input: &[&xml::reader::XmlEvent],
    ) -> Result<Vec<xml::reader::XmlEvent>, RewriteRuleError> {
        // Rewrite contents
        let mut output = Vec::with_capacity(input.len().max(6));
        self.action
            .rewrite(self, input, &mut output)
            .map(|_| output)
    }

    pub fn run_attribute(&self, input: &str) -> Result<String, RewriteRuleError> {
        // Rewrite contents
        self.action.rewrite_attribute(self, input)
    }
}

pub trait RewriteAction: Send {
    fn rewrite(
        &self,
        rule: &RewriteRule,
        input: &[&xml::reader::XmlEvent],
        output: &mut Vec<xml::reader::XmlEvent>,
    ) -> Result<(), RewriteRuleError>;

    fn rewrite_attribute(
        &self,
        _rule: &RewriteRule,
        _input: &str,
    ) -> Result<String, RewriteRuleError> {
        Err(RewriteRuleError::Unsupported)
    }
}

pub struct SetToCurrentDateTime;

impl SetToCurrentDateTime {
    fn now() -> String {
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string()
    }
}

impl RewriteAction for SetToCurrentDateTime {
    fn rewrite(
        &self,
        rule: &RewriteRule,
        input: &[&xml::reader::XmlEvent],
        output: &mut Vec<xml::reader::XmlEvent>,
    ) -> Result<(), RewriteRuleError> {
        let name = if let Some(xml::reader::XmlEvent::StartElement { name, .. }) = input.get(0) {
            name.to_owned()
        } else {
            rule.name()
        };

        output.push(xml::reader::XmlEvent::StartElement {
            name: name.clone(),
            attributes: vec![],
            namespace: xml::namespace::Namespace::empty(),
        });

        output.push(xml::reader::XmlEvent::Characters(Self::now()));

        output.push(xml::reader::XmlEvent::EndElement { name });

        Ok(())
    }

    fn rewrite_attribute(
        &self,
        _rule: &RewriteRule,
        _input: &str,
    ) -> Result<String, RewriteRuleError> {
        Ok(Self::now())
    }
}

pub struct SetRdfList {
    ty: &'static str,
    values: Vec<String>,
}

impl SetRdfList {
    pub fn new(ty: &'static str, values: Vec<String>) -> Self {
        Self { ty, values }
    }
}

fn rdf_node(name: &'static str) -> OwnedName {
    xml::name::OwnedName {
        local_name: name.to_owned(),
        namespace: crate::ns::RDF.to_owned().into(),
        prefix: "rdf".to_owned().into(),
    }
}

impl RewriteAction for SetRdfList {
    fn rewrite(
        &self,
        rule: &RewriteRule,
        input: &[&xml::reader::XmlEvent],
        output: &mut Vec<xml::reader::XmlEvent>,
    ) -> Result<(), RewriteRuleError> {
        let name = if let Some(xml::reader::XmlEvent::StartElement { name, .. }) = input.get(0) {
            name.to_owned()
        } else {
            rule.name()
        };

        output.push(xml::reader::XmlEvent::StartElement {
            name: name.clone(),
            attributes: vec![],
            namespace: xml::namespace::Namespace::empty(),
        });

        let rdf_seq = rdf_node(self.ty);

        output.push(xml::reader::XmlEvent::StartElement {
            name: rdf_seq.clone(),
            attributes: vec![],
            namespace: xml::namespace::Namespace::empty(),
        });

        let rdf_li = rdf_node("li");

        for item in &self.values {
            output.push(xml::reader::XmlEvent::StartElement {
                name: rdf_li.clone(),
                attributes: vec![],
                namespace: xml::namespace::Namespace::empty(),
            });

            output.push(xml::reader::XmlEvent::Characters(item.clone()));

            output.push(xml::reader::XmlEvent::EndElement {
                name: rdf_li.clone(),
            });
        }

        output.push(xml::reader::XmlEvent::EndElement { name: rdf_seq });

        output.push(xml::reader::XmlEvent::EndElement { name });

        Ok(())
    }
}

pub mod rules {
    use crate::TagHierarchy;

    use super::*;

    pub fn xmp_metadata_date() -> RewriteRule {
        RewriteRule::new(
            Some(crate::ns::XMP),
            "MetadataDate",
            "xmp",
            true,
            false,
            SetToCurrentDateTime,
        )
    }

    pub fn set_rdf_seq(
        namespace: &'static str,
        prefix: &'static str,
        name: &'static str,
        values: Vec<String>,
    ) -> RewriteRule {
        RewriteRule::new(
            Some(namespace),
            name,
            prefix,
            false,
            true,
            SetRdfList::new("Seq", values),
        )
    }

    pub fn set_rdf_alt(
        namespace: &'static str,
        prefix: &'static str,
        name: &'static str,
        values: Vec<String>,
    ) -> RewriteRule {
        RewriteRule::new(
            Some(namespace),
            name,
            prefix,
            false,
            true,
            SetRdfList::new("Alt", values),
        )
    }

    pub fn set_rdf_bag(
        namespace: &'static str,
        prefix: &'static str,
        name: &'static str,
        values: Vec<String>,
    ) -> RewriteRule {
        RewriteRule::new(
            Some(namespace),
            name,
            prefix,
            false,
            true,
            SetRdfList::new("Bag", values),
        )
    }

    pub fn set_dc_title(value: String) -> RewriteRule {
        set_rdf_alt(crate::ns::DC, "dc", "title", vec![value])
    }

    pub fn set_dc_subject(values: Vec<String>) -> RewriteRule {
        set_rdf_bag(crate::ns::DC, "dc", "subject", values)
    }

    pub fn set_dc_description(value: String) -> RewriteRule {
        set_rdf_alt(crate::ns::DC, "dc", "description", vec![value])
    }

    pub fn set_dc_creator(value: String) -> RewriteRule {
        set_rdf_seq(crate::ns::DC, "dc", "creator", vec![value])
    }

    pub fn set_lr_hierarchical_subject(tags: &TagHierarchy) -> RewriteRule {
        set_rdf_bag(
            crate::ns::LR,
            "lr",
            "hierarchicalSubject",
            tags.iter().map(|tag| tag[..].join("|")).collect(),
        )
    }
}
