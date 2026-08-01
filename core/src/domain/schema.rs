use crate::domain::resource::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trait {
    Taskable,
    Schedulable,
    ParaItem,
    Summarizable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    String,
    Ref,
    Duration,
    DateTime,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub property_type: PropertyType,
    pub required: bool,
    #[serde(default)]
    pub target_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub name: String,
    pub traits: Vec<Trait>,
    pub fields: BTreeMap<String, SchemaField>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ValidationError {
    #[error("missing required field: {field}")]
    MissingRequiredField { field: String },
    #[error("invalid property type for field '{field}': expected {expected}")]
    InvalidPropertyType { field: String, expected: String },
    #[error("unknown type definition: {name}")]
    UnknownType { name: String },
}

#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    types: BTreeMap<String, TypeDefinition>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, type_def: TypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }

    pub fn get(&self, name: &str) -> Option<&TypeDefinition> {
        self.types.get(name)
    }

    pub fn validate(&self, resource: &Resource) -> Result<(), ValidationError> {
        if let Some(type_name) = resource.properties.get("TYPE") {
            if let Some(type_def) = self.get(type_name) {
                for (field_name, field_spec) in &type_def.fields {
                    let prop_val = resource.properties.get(field_name);
                    if field_spec.required && prop_val.is_none() {
                        return Err(ValidationError::MissingRequiredField {
                            field: field_name.clone(),
                        });
                    }

                    if let Some(val) = prop_val {
                        match field_spec.property_type {
                            PropertyType::Ref => {
                                if !val.contains(':') && val.len() != 26 {
                                    return Err(ValidationError::InvalidPropertyType {
                                        field: field_name.clone(),
                                        expected: "ref".to_string(),
                                    });
                                }
                            }
                            PropertyType::Duration => {
                                if !val.ends_with('h') && !val.ends_with('m') && !val.ends_with('d')
                                {
                                    return Err(ValidationError::InvalidPropertyType {
                                        field: field_name.clone(),
                                        expected: "duration".to_string(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                return Err(ValidationError::UnknownType {
                    name: type_name.clone(),
                });
            }
        }

        Ok(())
    }
}
