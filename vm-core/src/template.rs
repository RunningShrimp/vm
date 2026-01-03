use std::collections::HashMap;
use std::string::String;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 虚拟机模板
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VmTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub base_snapshot_id: String,
}

/// 模板管理器
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TemplateManager {
    pub templates: HashMap<String, VmTemplate>,
}

impl TemplateManager {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    pub fn create_template(
        &mut self,
        name: String,
        description: String,
        base_snapshot_id: String,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let template = VmTemplate {
            id: id.clone(),
            name,
            description,
            base_snapshot_id,
        };
        self.templates.insert(id.clone(), template);
        id
    }

    pub fn get_template(&self, id: &str) -> Option<&VmTemplate> {
        self.templates.get(id)
    }

    pub fn list_templates(&self) -> Vec<&VmTemplate> {
        self.templates.values().collect()
    }
}
