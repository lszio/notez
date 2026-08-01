use crate::artifact::recipe::{Recipe, RecipeEvaluator, RecipeKind};
use crate::domain::{Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;

#[test]
fn recipe_derivation_summary_llms_txt_and_context_pack() {
    let res1 = Resource {
        r#ref: ResourceRef::parse("heading:01J00000000000000000000010").unwrap(),
        kind: ResourceKind::Heading,
        title: "Architecture Design".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path1.org".to_string(),
        properties: BTreeMap::new(),
    };

    let res2 = Resource {
        r#ref: ResourceRef::parse("heading:01J00000000000000000000020").unwrap(),
        kind: ResourceKind::Heading,
        title: "Data Flow Specs".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path2.org".to_string(),
        properties: BTreeMap::new(),
    };

    let resources = vec![res1, res2];

    let recipe_summary = Recipe {
        name: "summary".to_string(),
        kind: RecipeKind::Summary,
        token_budget: 1000,
    };
    let artifact_summary = RecipeEvaluator::evaluate(&recipe_summary, &resources).unwrap();
    assert_eq!(artifact_summary.recipe_name, "summary");
    assert!(artifact_summary.content.contains("# Community Summary"));
    assert!(artifact_summary.content.contains("Architecture Design"));

    let recipe_llms = Recipe {
        name: "llms-txt".to_string(),
        kind: RecipeKind::LlmsTxt,
        token_budget: 1000,
    };
    let artifact_llms = RecipeEvaluator::evaluate(&recipe_llms, &resources).unwrap();
    assert!(artifact_llms.content.contains("# LLMs.txt Entry"));
    assert!(
        artifact_llms
            .content
            .contains("heading:01J00000000000000000000010")
    );

    let recipe_cp = Recipe {
        name: "context-pack".to_string(),
        kind: RecipeKind::ContextPack,
        token_budget: 1000,
    };
    let artifact_cp = RecipeEvaluator::evaluate(&recipe_cp, &resources).unwrap();
    assert!(artifact_cp.content.contains("\"resources\""));
}
