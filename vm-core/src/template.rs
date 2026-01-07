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

/// ============================================================================
/// 测试模块
/// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_manager_creation() {
        let manager = TemplateManager::new();
        assert_eq!(manager.templates.len(), 0);
    }

    #[test]
    fn test_template_manager_default() {
        let manager = TemplateManager::default();
        assert_eq!(manager.templates.len(), 0);
    }

    #[test]
    fn test_create_template() {
        let mut manager = TemplateManager::new();

        let id = manager.create_template(
            "Test Template".to_string(),
            "A test template".to_string(),
            "snapshot-123".to_string(),
        );

        // ID应该被生成
        assert!(!id.is_empty());

        // 模板应该被存储
        assert_eq!(manager.templates.len(), 1);

        // 验证模板内容
        let template = manager.get_template(&id).unwrap();
        assert_eq!(template.name, "Test Template");
        assert_eq!(template.description, "A test template");
        assert_eq!(template.base_snapshot_id, "snapshot-123");
        assert_eq!(template.id, id);
    }

    #[test]
    fn test_get_template_exists() {
        let mut manager = TemplateManager::new();

        let id = manager.create_template(
            "Test".to_string(),
            "Description".to_string(),
            "snapshot-1".to_string(),
        );

        let template = manager.get_template(&id);
        assert!(template.is_some());
        assert_eq!(template.unwrap().name, "Test");
    }

    #[test]
    fn test_get_template_not_exists() {
        let manager = TemplateManager::new();

        let template = manager.get_template("non-existent-id");
        assert!(template.is_none());
    }

    #[test]
    fn test_list_templates_empty() {
        let manager = TemplateManager::new();
        let templates = manager.list_templates();

        assert_eq!(templates.len(), 0);
    }

    #[test]
    fn test_list_templates_multiple() {
        let mut manager = TemplateManager::new();

        // 创建3个模板
        let _id1 = manager.create_template(
            "Template 1".to_string(),
            "Description 1".to_string(),
            "snapshot-1".to_string(),
        );

        let _id2 = manager.create_template(
            "Template 2".to_string(),
            "Description 2".to_string(),
            "snapshot-2".to_string(),
        );

        let _id3 = manager.create_template(
            "Template 3".to_string(),
            "Description 3".to_string(),
            "snapshot-3".to_string(),
        );

        let templates = manager.list_templates();
        assert_eq!(templates.len(), 3);
    }

    #[test]
    fn test_multiple_templates_unique_ids() {
        let mut manager = TemplateManager::new();

        let id1 = manager.create_template(
            "Template 1".to_string(),
            "Description 1".to_string(),
            "snapshot-1".to_string(),
        );

        let id2 = manager.create_template(
            "Template 2".to_string(),
            "Description 2".to_string(),
            "snapshot-2".to_string(),
        );

        // ID应该是唯一的
        assert_ne!(id1, id2);

        // 两个模板都应该存在
        assert!(manager.get_template(&id1).is_some());
        assert!(manager.get_template(&id2).is_some());
    }

    #[test]
    fn test_template_clone() {
        let template = VmTemplate {
            id: "test-id".to_string(),
            name: "Test Template".to_string(),
            description: "Test Description".to_string(),
            base_snapshot_id: "snapshot-123".to_string(),
        };

        let cloned = template.clone();
        assert_eq!(cloned.id, template.id);
        assert_eq!(cloned.name, template.name);
        assert_eq!(cloned.description, template.description);
        assert_eq!(cloned.base_snapshot_id, template.base_snapshot_id);
    }

    #[test]
    fn test_template_manager_clone() {
        let mut manager = TemplateManager::new();

        let _id = manager.create_template(
            "Test".to_string(),
            "Description".to_string(),
            "snapshot-1".to_string(),
        );

        let cloned = manager.clone();
        assert_eq!(cloned.templates.len(), manager.templates.len());
    }

    #[test]
    fn test_template_serialization() {
        let template = VmTemplate {
            id: "test-id".to_string(),
            name: "Test Template".to_string(),
            description: "Test Description".to_string(),
            base_snapshot_id: "snapshot-123".to_string(),
        };

        // 测试序列化/反序列化
        let serialized = serde_json::to_string(&template).unwrap();
        let deserialized: VmTemplate = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, template.id);
        assert_eq!(deserialized.name, template.name);
    }

    #[test]
    fn test_template_manager_serialization() {
        let mut manager = TemplateManager::new();

        let _id = manager.create_template(
            "Test".to_string(),
            "Description".to_string(),
            "snapshot-1".to_string(),
        );

        // 测试序列化/反序列化
        let serialized = serde_json::to_string(&manager).unwrap();
        let deserialized: TemplateManager = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.templates.len(), 1);
    }
}
