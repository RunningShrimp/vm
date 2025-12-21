//! TODO tracking and resolution system for VM project
//!
//! This module provides a comprehensive system for tracking, prioritizing,
//! and resolving TODO/FIXME items across the VM project.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use vm_core::{AccessType, CoreError, VmError, VmResult};
use vm_error::{VmError as VmErrorAlias, VmResult as VmResultAlias};

/// TODO item with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub file_path: String,
    pub line_number: Option<u32>,
    pub priority: TodoPriority,
    pub category: TodoCategory,
    pub status: TodoStatus,
    pub assigned_to: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub due_date: Option<String>,
    pub tags: Vec<String>,
    pub estimated_hours: Option<f32>,
    pub actual_hours: Option<f32>,
    pub dependencies: Vec<String>,
    pub resolution: Option<String>,
}

impl TodoItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            file_path: file_path.into(),
            line_number: None,
            priority: TodoPriority::Medium,
            category: TodoCategory::General,
            status: TodoStatus::Open,
            assigned_to: None,
            created_at: now.clone(),
            updated_at: now,
            due_date: None,
            tags: Vec::new(),
            estimated_hours: None,
            actual_hours: None,
            dependencies: Vec::new(),
            resolution: None,
        }
    }

    pub fn with_priority(mut self, priority: TodoPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_category(mut self, category: TodoCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_line_number(mut self, line_number: u32) -> Self {
        self.line_number = Some(line_number);
        self
    }

    pub fn with_assigned_to(mut self, assigned_to: impl Into<String>) -> Self {
        self.assigned_to = Some(assigned_to.into());
        self
    }

    pub fn with_due_date(mut self, due_date: impl Into<String>) -> Self {
        self.due_date = Some(due_date.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_estimated_hours(mut self, hours: f32) -> Self {
        self.estimated_hours = Some(hours);
        self
    }

    pub fn with_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.dependencies.push(dependency.into());
        self
    }

    pub fn resolve(mut self, resolution: impl Into<String>) -> Self {
        self.status = TodoStatus::Resolved;
        self.resolution = Some(resolution.into());
        self.updated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self
    }

    pub fn close(mut self) -> Self {
        self.status = TodoStatus::Closed;
        self.updated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self
    }

    pub fn is_overdue(&self) -> bool {
        if let Some(due_date_str) = &self.due_date {
            if let Ok(due_date) =
                chrono::DateTime::parse_from_str(due_date_str, "%Y-%m-%d %H:%M:%S")
            {
                let due_date_utc = due_date.with_timezone(&chrono::Utc);
                chrono::Utc::now() > due_date_utc
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn age_days(&self) -> i64 {
        if let Ok(created) = chrono::DateTime::parse_from_str(&self.created_at, "%Y-%m-%d %H:%M:%S")
        {
            let created_utc = created.with_timezone(&chrono::Utc);
            (chrono::Utc::now() - created_utc).num_days()
        } else {
            0
        }
    }
}

/// TODO priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TodoPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl std::fmt::Display for TodoPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoPriority::Low => write!(f, "Low"),
            TodoPriority::Medium => write!(f, "Medium"),
            TodoPriority::High => write!(f, "High"),
            TodoPriority::Critical => write!(f, "Critical"),
        }
    }
}

impl TodoPriority {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" => TodoPriority::Low,
            "medium" => TodoPriority::Medium,
            "high" => TodoPriority::High,
            "critical" => TodoPriority::Critical,
            _ => TodoPriority::Medium,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            TodoPriority::Low => "low",
            TodoPriority::Medium => "medium",
            TodoPriority::High => "high",
            TodoPriority::Critical => "critical",
        }
    }
}

/// TODO categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TodoCategory {
    General,
    Bug,
    Feature,
    Documentation,
    Performance,
    Security,
    Refactoring,
    Testing,
    Build,
    Infrastructure,
}

impl TodoCategory {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "general" => TodoCategory::General,
            "bug" => TodoCategory::Bug,
            "feature" => TodoCategory::Feature,
            "documentation" => TodoCategory::Documentation,
            "performance" => TodoCategory::Performance,
            "security" => TodoCategory::Security,
            "refactoring" => TodoCategory::Refactoring,
            "testing" => TodoCategory::Testing,
            "build" => TodoCategory::Build,
            "infrastructure" => TodoCategory::Infrastructure,
            _ => TodoCategory::General,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            TodoCategory::General => "general",
            TodoCategory::Bug => "bug",
            TodoCategory::Feature => "feature",
            TodoCategory::Documentation => "documentation",
            TodoCategory::Performance => "performance",
            TodoCategory::Security => "security",
            TodoCategory::Refactoring => "refactoring",
            TodoCategory::Testing => "testing",
            TodoCategory::Build => "build",
            TodoCategory::Infrastructure => "infrastructure",
        }
    }
}

/// TODO status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Open,
    InProgress,
    Resolved,
    Closed,
    Rejected,
}

impl TodoStatus {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => TodoStatus::Open,
            "in_progress" => TodoStatus::InProgress,
            "resolved" => TodoStatus::Resolved,
            "closed" => TodoStatus::Closed,
            "rejected" => TodoStatus::Rejected,
            _ => TodoStatus::Open,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            TodoStatus::Open => "open",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Resolved => "resolved",
            TodoStatus::Closed => "closed",
            TodoStatus::Rejected => "rejected",
        }
    }
}

/// TODO tracker for managing TODO items
#[derive(Debug)]
pub struct TodoTracker {
    items: HashMap<String, TodoItem>,
    filters: TodoFilters,
}

impl Default for TodoTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoTracker {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            filters: TodoFilters::default(),
        }
    }

    /// Add a new TODO item
    pub fn add_item(&mut self, item: TodoItem) -> VmResult<()> {
        if self.items.contains_key(&item.id) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: format!("TODO item with ID {} already exists", item.id),
                current: "duplicate".to_string(),
                expected: "new".to_string(),
            }));
        }

        self.items.insert(item.id.clone(), item);
        Ok(())
    }

    /// Get a TODO item by ID
    pub fn get_item(&self, id: &str) -> Option<&TodoItem> {
        self.items.get(id)
    }

    /// Update a TODO item
    pub fn update_item(&mut self, id: &str, item: TodoItem) -> VmResult<()> {
        if !self.items.contains_key(id) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: format!("TODO item with ID {} not found", id),
                current: "not_found".to_string(),
                expected: "existing".to_string(),
            }));
        }

        self.items.insert(id.to_string(), item);
        Ok(())
    }

    /// Remove a TODO item
    pub fn remove_item(&mut self, id: &str) -> VmResult<()> {
        if !self.items.contains_key(id) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: format!("TODO item with ID {} not found", id),
                current: "not_found".to_string(),
                expected: "existing".to_string(),
            }));
        }

        self.items.remove(id);
        Ok(())
    }

    /// Get all TODO items
    pub fn get_all_items(&self) -> Vec<&TodoItem> {
        self.items.values().collect()
    }

    /// Get filtered TODO items
    pub fn get_filtered_items(&self) -> Vec<&TodoItem> {
        self.items
            .values()
            .filter(|item| self.filters.matches(item))
            .collect()
    }

    /// Get items by priority
    pub fn get_items_by_priority(&self, priority: TodoPriority) -> Vec<&TodoItem> {
        self.items
            .values()
            .filter(|item| item.priority == priority)
            .collect()
    }

    /// Get items by category
    pub fn get_items_by_category(&self, category: TodoCategory) -> Vec<&TodoItem> {
        self.items
            .values()
            .filter(|item| item.category == category)
            .collect()
    }

    /// Get items by status
    pub fn get_items_by_status(&self, status: TodoStatus) -> Vec<&TodoItem> {
        self.items
            .values()
            .filter(|item| item.status == status)
            .collect()
    }

    /// Get overdue items
    pub fn get_overdue_items(&self) -> Vec<&TodoItem> {
        self.items
            .values()
            .filter(|item| item.is_overdue())
            .collect()
    }

    /// Set filters
    pub fn set_filters(&mut self, filters: TodoFilters) {
        self.filters = filters;
    }

    /// Get statistics
    pub fn get_stats(&self) -> TodoStats {
        let mut stats = TodoStats::default();

        for item in self.items.values() {
            stats.total_items += 1;

            match item.status {
                TodoStatus::Open => stats.open_items += 1,
                TodoStatus::InProgress => stats.in_progress_items += 1,
                TodoStatus::Resolved => stats.resolved_items += 1,
                TodoStatus::Closed => stats.closed_items += 1,
                TodoStatus::Rejected => stats.rejected_items += 1,
            }

            match item.priority {
                TodoPriority::Low => stats.low_priority_items += 1,
                TodoPriority::Medium => stats.medium_priority_items += 1,
                TodoPriority::High => stats.high_priority_items += 1,
                TodoPriority::Critical => stats.critical_priority_items += 1,
            }

            if let Some(hours) = item.estimated_hours {
                stats.total_estimated_hours += hours;
            }

            if let Some(hours) = item.actual_hours {
                stats.total_actual_hours += hours;
            }
        }

        stats
    }

    /// Export todos to a format compatible with VM error system
    pub fn export_to_vm_error_format(&self) -> VmResultAlias<String> {
        let mut output = String::new();

        for item in self.items.values() {
            let line = format!(
                "{}:{}: {} [{}] {}\n",
                item.file_path,
                item.line_number.map_or(0, |n| n),
                item.title,
                item.priority,
                item.description
            );
            output.push_str(&line);
        }

        Ok(output)
    }

    /// Import todos from a VM error compatible format
    pub fn import_from_vm_error_format(&mut self, content: &str) -> VmResultAlias<usize> {
        let lines = content.lines();
        let mut count = 0;

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            // Parse format: file:line: title [priority] description
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() >= 3 {
                let file_path = parts[0].to_string();
                let line_number = parts[1].parse::<u32>().ok();
                let remainder = parts[2..].join(":");

                let title_end = remainder.find('[').unwrap_or(remainder.len());
                let title = remainder[..title_end].trim().to_string();

                let description = if remainder.contains(']') {
                    remainder[remainder.find(']').unwrap() + 1..]
                        .trim()
                        .to_string()
                } else {
                    remainder.clone()
                };

                let item = TodoItem::new(
                    format!("imported-{}", count),
                    &title,
                    &description,
                    &file_path,
                )
                .with_line_number(line_number.unwrap_or(0));

                if let Err(e) = self.add_item(item) {
                    return Err(VmErrorAlias::Io {
                        message: format!("Failed to add item: {}", e),
                    });
                }
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get unique tags across all TODO items
    pub fn get_unique_tags(&self) -> HashSet<String> {
        let mut tags = HashSet::new();

        for item in self.items.values() {
            for tag in &item.tags {
                tags.insert(tag.clone());
            }
        }

        tags
    }

    /// Get unique assignees across all TODO items
    pub fn get_unique_assignees(&self) -> HashSet<String> {
        let mut assignees = HashSet::new();

        for item in self.items.values() {
            if let Some(ref assignee) = item.assigned_to {
                assignees.insert(assignee.clone());
            }
        }

        assignees
    }
}

/// Filters for TODO items
#[derive(Debug, Clone, Default)]
pub struct TodoFilters {
    pub priorities: Option<Vec<TodoPriority>>,
    pub categories: Option<Vec<TodoCategory>>,
    pub statuses: Option<Vec<TodoStatus>>,
    pub assigned_to: Option<String>,
    pub tags: Option<Vec<String>>,
    pub overdue_only: bool,
}

impl TodoFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_priorities(mut self, priorities: Vec<TodoPriority>) -> Self {
        self.priorities = Some(priorities);
        self
    }

    pub fn with_categories(mut self, categories: Vec<TodoCategory>) -> Self {
        self.categories = Some(categories);
        self
    }

    pub fn with_statuses(mut self, statuses: Vec<TodoStatus>) -> Self {
        self.statuses = Some(statuses);
        self
    }

    pub fn with_assigned_to(mut self, assigned_to: impl Into<String>) -> Self {
        self.assigned_to = Some(assigned_to.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn overdue_only(mut self) -> Self {
        self.overdue_only = true;
        self
    }

    /// Check if an item matches the filters
    pub fn matches(&self, item: &TodoItem) -> bool {
        // Check priority filter
        if let Some(ref priorities) = self.priorities {
            if !priorities.contains(&item.priority) {
                return false;
            }
        }

        // Check category filter
        if let Some(ref categories) = self.categories {
            if !categories.contains(&item.category) {
                return false;
            }
        }

        // Check status filter
        if let Some(ref statuses) = self.statuses {
            if !statuses.contains(&item.status) {
                return false;
            }
        }

        // Check assigned to filter
        if let Some(ref assigned_to) = self.assigned_to {
            if let Some(ref item_assigned) = item.assigned_to {
                if item_assigned != assigned_to {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check tags filter
        if let Some(ref tags) = self.tags {
            if !tags.iter().any(|tag| item.tags.contains(tag)) {
                return false;
            }
        }

        // Check overdue filter
        if self.overdue_only && !item.is_overdue() {
            return false;
        }

        true
    }
}

/// TODO statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoStats {
    pub total_items: usize,
    pub open_items: usize,
    pub in_progress_items: usize,
    pub resolved_items: usize,
    pub closed_items: usize,
    pub rejected_items: usize,
    pub low_priority_items: usize,
    pub medium_priority_items: usize,
    pub high_priority_items: usize,
    pub critical_priority_items: usize,
    pub total_estimated_hours: f32,
    pub total_actual_hours: f32,
}

impl TodoStats {
    /// Get completion rate
    pub fn completion_rate(&self) -> f32 {
        if self.total_items == 0 {
            0.0
        } else {
            (self.resolved_items + self.closed_items) as f32 / self.total_items as f32
        }
    }

    /// Get average estimated hours
    pub fn avg_estimated_hours(&self) -> f32 {
        if self.total_items == 0 {
            0.0
        } else {
            self.total_estimated_hours / self.total_items as f32
        }
    }

    /// Get average actual hours
    pub fn avg_actual_hours(&self) -> f32 {
        let completed_items = self.resolved_items + self.closed_items;
        if completed_items == 0 {
            0.0
        } else {
            self.total_actual_hours / completed_items as f32
        }
    }

    /// Get estimation accuracy
    pub fn estimation_accuracy(&self) -> f32 {
        let completed_items = self.resolved_items + self.closed_items;
        if completed_items == 0 {
            0.0
        } else {
            let avg_estimated = self.total_estimated_hours / self.total_items as f32;
            let avg_actual = self.total_actual_hours / completed_items as f32;

            if avg_estimated == 0.0 {
                0.0
            } else {
                1.0 - (avg_estimated - avg_actual).abs() / avg_estimated
            }
        }
    }
}

/// Scanner for finding TODO items in source code
#[derive(Debug)]
pub struct TodoScanner {
    patterns: Vec<regex::Regex>,
}

impl Default for TodoScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoScanner {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // TODO patterns
                regex::Regex::new(r"(?i)//\s*TODO\s*[:\(]?\s*([^)]*)\)?\s*(.*)").unwrap(),
                // FIXME patterns
                regex::Regex::new(r"(?i)//\s*FIXME\s*[:\(]?\s*([^)]*)\)?\s*(.*)").unwrap(),
                // HACK patterns
                regex::Regex::new(r"(?i)//\s*HACK\s*[:\(]?\s*([^)]*)\)?\s*(.*)").unwrap(),
                // NOTE patterns
                regex::Regex::new(r"(?i)//\s*NOTE\s*[:\(]?\s*([^)]*)\)?\s*(.*)").unwrap(),
                // XXX patterns
                regex::Regex::new(r"(?i)//\s*XXX\s*[:\(]?\s*([^)]*)\)?\s*(.*)").unwrap(),
            ],
        }
    }

    /// Scan a file for TODO items
    pub fn scan_file(&self, file_path: &Path) -> VmResult<Vec<TodoItem>> {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            VmError::Io(format!(
                "Failed to read file: {:?}, error: {}",
                file_path, e
            ))
        })?;

        let mut items = Vec::new();
        let file_path_str = file_path.to_string_lossy().to_string();

        for (line_num, line) in content.lines().enumerate() {
            for pattern in &self.patterns {
                if let Some(captures) = pattern.captures(line) {
                    let (title, description) = if let (Some(title), Some(description)) =
                        (captures.get(1), captures.get(2))
                    {
                        (
                            title.as_str().trim().to_string(),
                            description.as_str().trim().to_string(),
                        )
                    } else if let Some(title) = captures.get(1) {
                        (title.as_str().trim().to_string(), "".to_string())
                    } else {
                        ("".to_string(), "".to_string())
                    };

                    if !title.is_empty() || !description.is_empty() {
                        let item = TodoItem::new(
                            format!("{}:{}:{}", file_path_str, line_num + 1, line_num + 1),
                            title,
                            description,
                            &file_path_str,
                        )
                        .with_line_number((line_num + 1) as u32);

                        items.push(item);
                    }
                }
            }
        }

        Ok(items)
    }

    /// Scan a directory recursively for TODO items
    pub fn scan_directory(&self, dir_path: &Path) -> VmResult<Vec<TodoItem>> {
        let mut all_items = Vec::new();

        for entry in walkdir::WalkDir::new(dir_path) {
            let entry = entry.map_err(|e| {
                VmError::Io(format!(
                    "Failed to walk directory: {:?}, error: {}",
                    dir_path, e
                ))
            })?;

            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                let items = self.scan_file(path)?;
                all_items.extend(items);
            }
        }

        Ok(all_items)
    }

    /// Scan and categorize TODO items
    pub fn scan_and_categorize(
        &self,
        dir_path: &Path,
    ) -> VmResult<HashMap<TodoCategory, Vec<TodoItem>>> {
        let items = self.scan_directory(dir_path)?;
        let mut categorized = HashMap::new();

        for item in items {
            let category = self.categorize_item(&item);
            categorized
                .entry(category)
                .or_insert_with(Vec::new)
                .push(item);
        }

        Ok(categorized)
    }

    /// Check if a TODO item relates to memory access issues
    pub fn is_memory_access_related(&self, item: &TodoItem) -> bool {
        let content = format!("{} {}", item.title, item.description).to_lowercase();
        content.contains("memory")
            || content.contains("access")
            || content.contains("mmu")
            || content.contains("tlb")
            || content.contains("cache")
            || content.contains("page")
    }

    /// Get the type of memory access issue from a TODO item
    pub fn get_memory_access_type(&self, item: &TodoItem) -> Option<AccessType> {
        let content = format!("{} {}", item.title, item.description).to_lowercase();

        if content.contains("read") && !content.contains("write") {
            Some(AccessType::Read)
        } else if content.contains("write") && !content.contains("read") {
            Some(AccessType::Write)
        } else if content.contains("execute") {
            Some(AccessType::Execute)
        } else if content.contains("read") && content.contains("write") {
            // Since ReadWrite is not available, we'll default to Read for mixed access
            Some(AccessType::Read)
        } else {
            None
        }
    }

    /// Categorize a TODO item based on its content
    fn categorize_item(&self, item: &TodoItem) -> TodoCategory {
        let content = format!("{} {}", item.title, item.description).to_lowercase();

        if content.contains("performance")
            || content.contains("optimize")
            || content.contains("speed")
        {
            TodoCategory::Performance
        } else if content.contains("bug") || content.contains("fix") || content.contains("error") {
            TodoCategory::Bug
        } else if content.contains("feature")
            || content.contains("implement")
            || content.contains("add")
        {
            TodoCategory::Feature
        } else if content.contains("doc")
            || content.contains("document")
            || content.contains("readme")
        {
            TodoCategory::Documentation
        } else if content.contains("security")
            || content.contains("vulnerability")
            || content.contains("protect")
        {
            TodoCategory::Security
        } else if content.contains("refactor")
            || content.contains("clean")
            || content.contains("reorganize")
        {
            TodoCategory::Refactoring
        } else if content.contains("test") || content.contains("spec") || content.contains("verify")
        {
            TodoCategory::Testing
        } else if content.contains("build")
            || content.contains("compile")
            || content.contains("make")
        {
            TodoCategory::Build
        } else if content.contains("infra") || content.contains("deploy") || content.contains("ci")
        {
            TodoCategory::Infrastructure
        } else {
            TodoCategory::General
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_item_creation() {
        let item = TodoItem::new("test-id", "Test TODO", "Test description", "/test/file.rs")
            .with_priority(TodoPriority::High)
            .with_category(TodoCategory::Bug)
            .with_tag("urgent");

        assert_eq!(item.id, "test-id");
        assert_eq!(item.title, "Test TODO");
        assert_eq!(item.description, "Test description");
        assert_eq!(item.file_path, "/test/file.rs");
        assert_eq!(item.priority, TodoPriority::High);
        assert_eq!(item.category, TodoCategory::Bug);
        assert!(item.tags.contains(&"urgent".to_string()));
    }

    #[test]
    fn test_todo_tracker() {
        let mut tracker = TodoTracker::new();

        let item = TodoItem::new("test-id", "Test TODO", "Test description", "/test/file.rs");

        tracker.add_item(item.clone()).unwrap();
        match tracker.get_item("test-id") {
            Some(stored_item) => {
                assert_eq!(stored_item.id, item.id);
                assert_eq!(stored_item.title, item.title);
                assert_eq!(stored_item.description, item.description);
                assert_eq!(stored_item.file_path, item.file_path);
            }
            None => panic!("Expected item to be stored but found None"),
        }

        let stats = tracker.get_stats();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.open_items, 1);
    }

    #[test]
    fn test_todo_filters() {
        let filters = TodoFilters::new()
            .with_priorities(vec![TodoPriority::High, TodoPriority::Critical])
            .with_categories(vec![TodoCategory::Bug, TodoCategory::Security])
            .with_assigned_to("developer")
            .with_tags(vec!["urgent".to_string()])
            .overdue_only();

        assert!(filters.priorities.is_some());
        assert!(filters.categories.is_some());
        assert_eq!(filters.assigned_to, Some("developer".to_string()));
        assert!(filters.tags.is_some());
        assert!(filters.overdue_only);
    }

    #[test]
    fn test_todo_stats() {
        let stats = TodoStats {
            total_items: 10,
            resolved_items: 5,
            closed_items: 3,
            total_estimated_hours: 50.0,
            total_actual_hours: 45.0,
            ..TodoStats::default()
        };

        assert_eq!(stats.completion_rate(), 0.8);
        assert_eq!(stats.avg_estimated_hours(), 5.0);
        assert_eq!(stats.avg_actual_hours(), 5.625);
        assert!((stats.estimation_accuracy() - 0.1).abs() < 0.01);
    }
}
