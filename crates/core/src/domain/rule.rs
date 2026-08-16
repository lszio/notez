use crate::domain::resource::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Classify,
    Validate,
    Derive,
    React,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub kind: RuleKind,
    pub name: String,
    pub condition_property: String,
    pub condition_value: String,
    pub target_property: String,
    pub target_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTrace {
    pub rule_id: String,
    pub kind: RuleKind,
    pub matched: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectResult {
    pub r_ref: String,
    pub classified_type: Option<String>,
    pub derived_properties: BTreeMap<String, String>,
    pub traces: Vec<RuleTrace>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn default_rules() -> Self {
        let mut engine = Self::new();
        engine.add_rule(Rule {
            id: "classify-project-type".to_string(),
            kind: RuleKind::Classify,
            name: "Classify TYPE=project".to_string(),
            condition_property: "TYPE".to_string(),
            condition_value: "project".to_string(),
            target_property: "TYPE".to_string(),
            target_value: "project".to_string(),
        });
        engine.add_rule(Rule {
            id: "derive-para-projects".to_string(),
            kind: RuleKind::Derive,
            name: "Derive para=projects for TYPE=project".to_string(),
            condition_property: "TYPE".to_string(),
            condition_value: "project".to_string(),
            target_property: "para".to_string(),
            target_value: "projects".to_string(),
        });
        engine
    }

    pub fn evaluate(&self, resource: &Resource) -> InspectResult {
        let mut traces = Vec::new();
        let mut classified_type = resource.properties.get("TYPE").cloned();
        let mut derived_properties = BTreeMap::new();

        for r in self.rules.iter().filter(|r| r.kind == RuleKind::Classify) {
            let matched = resource
                .properties
                .get(&r.condition_property)
                .is_some_and(|v| v.eq_ignore_ascii_case(&r.condition_value));
            if matched {
                classified_type = Some(r.target_value.clone());
            }
            traces.push(RuleTrace {
                rule_id: r.id.clone(),
                kind: RuleKind::Classify,
                matched,
                message: if matched {
                    format!("Classified type as {}", r.target_value)
                } else {
                    "Condition not met".to_string()
                },
            });
        }

        for r in self.rules.iter().filter(|r| r.kind == RuleKind::Validate) {
            let matched = resource
                .properties
                .get(&r.condition_property)
                .is_some_and(|v| v.eq_ignore_ascii_case(&r.condition_value));
            traces.push(RuleTrace {
                rule_id: r.id.clone(),
                kind: RuleKind::Validate,
                matched,
                message: if matched {
                    "Validation passed".to_string()
                } else {
                    "Validation skipped/failed".to_string()
                },
            });
        }

        for r in self.rules.iter().filter(|r| r.kind == RuleKind::Derive) {
            let current_type = classified_type
                .as_deref()
                .or_else(|| resource.properties.get("TYPE").map(|s| s.as_str()));
            let matched = current_type.is_some_and(|v| v.eq_ignore_ascii_case(&r.condition_value))
                || resource
                    .properties
                    .get(&r.condition_property)
                    .is_some_and(|v| v.eq_ignore_ascii_case(&r.condition_value));

            if matched {
                derived_properties.insert(r.target_property.clone(), r.target_value.clone());
            }
            traces.push(RuleTrace {
                rule_id: r.id.clone(),
                kind: RuleKind::Derive,
                matched,
                message: if matched {
                    format!("Derived {}={}", r.target_property, r.target_value)
                } else {
                    "Condition not met".to_string()
                },
            });
        }

        for r in self.rules.iter().filter(|r| r.kind == RuleKind::React) {
            let matched = resource
                .properties
                .get(&r.condition_property)
                .is_some_and(|v| v.eq_ignore_ascii_case(&r.condition_value));
            traces.push(RuleTrace {
                rule_id: r.id.clone(),
                kind: RuleKind::React,
                matched,
                message: if matched {
                    format!("React rule triggered: {}", r.name)
                } else {
                    "Condition not met".to_string()
                },
            });
        }

        InspectResult {
            r_ref: resource.r#ref.to_string(),
            classified_type,
            derived_properties,
            traces,
        }
    }
}
