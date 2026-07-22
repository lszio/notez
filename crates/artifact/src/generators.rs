use domain::Resource;
use serde_json::json;

pub fn generate_summary(resources: &[Resource]) -> String {
    let mut out = String::from("# Community Summary\n\n");
    for res in resources {
        out.push_str(&format!("- **{}** (`{}`)\n", res.title, res.r#ref));
    }
    out
}

pub fn generate_llms_txt(resources: &[Resource]) -> String {
    let mut out = String::from("# LLMs.txt Entry\n\n");
    out.push_str("> Notez Auto-generated LLMs Entry Index\n\n");
    for res in resources {
        out.push_str(&format!("- {} [{}]\n", res.title, res.r#ref));
    }
    out
}

pub fn generate_context_pack(resources: &[Resource]) -> String {
    let pack = json!({
        "version": "1.0",
        "total_resources": resources.len(),
        "resources": resources,
    });
    serde_json::to_string_pretty(&pack).unwrap_or_default()
}

pub fn generate_skill_ir_json(resources: &[Resource]) -> String {
    let skill_ir = json!({
        "skill_version": "1.0",
        "resources_count": resources.len(),
        "resources": resources,
    });
    serde_json::to_string_pretty(&skill_ir).unwrap_or_default()
}
