use serde::Serialize;
use thiserror::Error;

use crate::{
    xmp::{rules, RewriteRule},
    TagHierarchy,
};

#[derive(Default, Debug, Clone, Serialize)]
pub struct AcdSeeData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datetime: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<TagHierarchy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collections: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

impl AcdSeeData {
    pub fn is_empty(&self) -> bool {
        self.caption.is_none()
            && self.datetime.is_none()
            && self.author.is_none()
            && self.rating.is_none()
            && self.notes.is_none()
            && self.tagged.is_none()
            && self.categories.is_none()
            && self.collections.is_none()
    }

    pub fn to_ruleset(&self) -> Vec<RewriteRule> {
        let mut result = Vec::with_capacity(8);

        if let Some(caption) = &self.caption {
            result.push(rules::set_dc_title(caption.clone()));
        }

        if let Some(author) = &self.author {
            result.push(rules::set_dc_creator(author.clone()));
        }

        if let Some(notes) = &self.notes {
            result.push(rules::set_dc_description(notes.clone()));
        }

        if let Some(categories) = &self.categories {
            result.push(rules::set_lr_hierarchical_subject(categories));
        }

        if !self.keywords.is_empty() {
            result.push(rules::set_dc_subject(self.keywords.clone()));
        }

        result
    }
}

#[derive(Debug, Error)]
pub enum AcdSeeError {
    #[error(transparent)]
    Xml(#[from] xml::reader::Error),
    #[error(transparent)]
    Date(#[from] chrono::ParseError),
}
