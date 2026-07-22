use domain::Resource;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillIr {
    pub name: String,
    pub description: String,
    pub resources: Vec<Resource>,
}

impl SkillIr {
    pub fn compile(name: &str, description: &str, resources: &[Resource]) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            resources: resources.to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackage {
    pub name: String,
    pub package_path: PathBuf,
}

pub struct SkillExporter;

impl SkillExporter {
    pub fn export(skill_ir: &SkillIr, export_path: &Path) -> Result<SkillPackage, io::Error> {
        if !export_path.exists() {
            fs::create_dir_all(export_path)?;
        }

        let references_dir = export_path.join("references");
        if !references_dir.exists() {
            fs::create_dir_all(&references_dir)?;
        }

        let mut skill_md = String::new();
        skill_md.push_str("---\n");
        skill_md.push_str(&format!("name: {}\n", skill_ir.name));
        skill_md.push_str(&format!("description: {}\n", skill_ir.description));
        skill_md.push_str("---\n\n");
        skill_md.push_str(&format!("# {}\n\n", skill_ir.name));
        skill_md.push_str(&format!("{}\n\n", skill_ir.description));
        skill_md.push_str("## Core Directives & References\n\n");

        for res in &skill_ir.resources {
            skill_md.push_str(&format!("- **{}** (`{}`)\n", res.title, res.r#ref));
        }

        fs::write(export_path.join("SKILL.md"), skill_md)?;

        let resources_json = serde_json::to_string_pretty(&skill_ir.resources)?;
        fs::write(export_path.join("resources.json"), resources_json)?;

        Ok(SkillPackage {
            name: skill_ir.name.clone(),
            package_path: export_path.to_path_buf(),
        })
    }
}
