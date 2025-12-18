//! TODO/FIXME标记扫描和解决工具
//!
//! 这个工具扫描项目中的TODO和FIXME标记，并使用vm-todo-tracker模块来管理它们。

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use regex::Regex;

// 导入vm-todo-tracker模块
use vm_todo_tracker::{TodoItem, TodoPriority, TodoCategory, TodoStatus, TodoTracker};

/// TODO项信息
#[derive(Debug, Clone)]
struct TodoInfo {
    file_path: String,
    line_number: usize,
    content: String,
    marker_type: String, // TODO, FIXME, XXX, HACK, BUG
}

/// TODO扫描器
struct TodoScanner {
    patterns: Vec<Regex>,
}

impl TodoScanner {
    fn new() -> Self {
        Self {
            patterns: vec![
                // TODO patterns
                Regex::new(r"(?i)//\s*TODO\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)").unwrap(),
                Regex::new(r"(?i)/\*\s*TODO\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)\*/").unwrap(),
                // FIXME patterns
                Regex::new(r"(?i)//\s*FIXME\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)").unwrap(),
                Regex::new(r"(?i)/\*\s*FIXME\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)\*/").unwrap(),
                // XXX patterns
                Regex::new(r"(?i)//\s*XXX\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)").unwrap(),
                Regex::new(r"(?i)/\*\s*XXX\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)\*/").unwrap(),
                // HACK patterns
                Regex::new(r"(?i)//\s*HACK\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)").unwrap(),
                Regex::new(r"(?i)/\*\s*HACK\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)\*/").unwrap(),
                // BUG patterns
                Regex::new(r"(?i)//\s*BUG\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)").unwrap(),
                Regex::new(r"(?i)/\*\s*BUG\s*[:\(]?\s*(?P<content>[^)]*)\)?\s*(?P<details>.*)\*/").unwrap(),
            ],
        }
    }

    /// 扫描文件中的TODO项
    fn scan_file(&self, file_path: &Path) -> Result<Vec<TodoInfo>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        let mut todos = Vec::new();
        
        for (line_num, line) in content.lines().enumerate() {
            for pattern in &self.patterns {
                if let Some(captures) = pattern.captures(line) {
                    if let (Some(content), Some(details)) = (captures.name("content"), captures.name("details")) {
                        let marker_type = if pattern.as_str().contains("TODO") {
                            "TODO"
                        } else if pattern.as_str().contains("FIXME") {
                            "FIXME"
                        } else if pattern.as_str().contains("XXX") {
                            "XXX"
                        } else if pattern.as_str().contains("HACK") {
                            "HACK"
                        } else if pattern.as_str().contains("BUG") {
                            "BUG"
                        } else {
                            "UNKNOWN"
                        };
                        
                        let full_content = if details.as_str().is_empty() {
                            content.as_str().to_string()
                        } else {
                            format!("{}: {}", content.as_str(), details.as_str())
                        };
                        
                        todos.push(TodoInfo {
                            file_path: file_path.to_string_lossy().to_string(),
                            line_number: line_num + 1,
                            content: full_content,
                            marker_type: marker_type.to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(todos)
    }

    /// 扫描目录中的TODO项
    fn scan_directory(&self, dir_path: &Path) -> Result<Vec<TodoInfo>, std::io::Error> {
        let mut all_todos = Vec::new();
        
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // 跳过target目录和.git目录
                if let Some(name) = path.file_name() {
                    if name == "target" || name == ".git" {
                        continue;
                    }
                }
                
                // 递归扫描子目录
                match self.scan_directory(&path) {
                    Ok(mut todos) => all_todos.append(&mut todos),
                    Err(_) => continue, // 忽略无法读取的目录
                }
            } else if path.is_file() {
                // 只扫描.rs和.md文件
                if let Some(extension) = path.extension() {
                    if extension == "rs" || extension == "md" {
                        match self.scan_file(&path) {
                            Ok(mut todos) => all_todos.append(&mut todos),
                            Err(_) => continue, // 忽略无法读取的文件
                        }
                    }
                }
            }
        }
        
        Ok(all_todos)
    }
}

/// TODO分类器
struct TodoClassifier;

impl TodoClassifier {
    /// 根据内容分类TODO项
    fn classify(&self, todo_info: &TodoInfo) -> (TodoPriority, TodoCategory) {
        let content = todo_info.content.to_lowercase();
        let marker_type = todo_info.marker_type.to_lowercase();
        
        // 根据标记类型确定基础优先级
        let base_priority = if marker_type.contains("fixme") {
            TodoPriority::High
        } else if marker_type.contains("bug") {
            TodoPriority::High
        } else if marker_type.contains("hack") {
            TodoPriority::Medium
        } else if marker_type.contains("xxx") {
            TodoPriority::Medium
        } else {
            TodoPriority::Low
        };
        
        // 根据内容调整优先级
        let priority = if content.contains("critical") || content.contains("urgent") {
            TodoPriority::High
        } else if content.contains("security") || content.contains("vulnerability") {
            TodoPriority::High
        } else if content.contains("performance") || content.contains("optimize") {
            TodoPriority::Medium
        } else if content.contains("refactor") || content.contains("cleanup") {
            TodoPriority::Low
        } else {
            base_priority
        };
        
        // 根据内容确定类别
        let category = if content.contains("test") || content.contains("testing") {
            TodoCategory::Test
        } else if content.contains("performance") || content.contains("optimize") {
            TodoCategory::Performance
        } else if content.contains("security") || content.contains("vulnerability") {
            TodoCategory::Security
        } else if content.contains("refactor") || content.contains("cleanup") {
            TodoCategory::Refactoring
        } else if content.contains("feature") || content.contains("implement") {
            TodoCategory::Feature
        } else if content.contains("bug") || content.contains("fix") {
            TodoCategory::Bug
        } else if content.contains("documentation") || content.contains("doc") {
            TodoCategory::Documentation
        } else if content.contains("api") || content.contains("interface") {
            TodoCategory::Api
        } else {
            TodoCategory::General
        };
        
        (priority, category)
    }
}

/// TODO解决器
struct TodoResolver {
    tracker: TodoTracker,
}

impl TodoResolver {
    fn new() -> Self {
        Self {
            tracker: TodoTracker::new(),
        }
    }

    /// 扫描并添加TODO项到跟踪器
    fn scan_and_add_todos(&mut self, project_root: &Path) -> Result<(), std::io::Error> {
        let scanner = TodoScanner::new();
        let classifier = TodoClassifier;
        
        println!("扫描TODO项...");
        let todo_infos = scanner.scan_directory(project_root)?;
        
        println!("找到 {} 个TODO项", todo_infos.len());
        
        for todo_info in todo_infos {
            let (priority, category) = classifier.classify(&todo_info);
            
            let todo_item = TodoItem {
                id: format!("{}:{}", todo_info.file_path, todo_info.line_number),
                title: format!("{}: {}", todo_info.marker_type, todo_info.content),
                description: format!(
                    "文件: {}\n行号: {}\n类型: {}\n内容: {}",
                    todo_info.file_path,
                    todo_info.line_number,
                    todo_info.marker_type,
                    todo_info.content
                ),
                priority,
                category,
                status: TodoStatus::Open,
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
                assignee: None,
                tags: vec![todo_info.marker_type],
            };
            
            if let Err(e) = self.tracker.add_todo(todo_item) {
                eprintln!("添加TODO项失败: {}", e);
            }
        }
        
        Ok(())
    }

    /// 生成TODO报告
    fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# TODO/FIXME标记解决报告\n\n");
        
        // 统计信息
        let stats = self.tracker.get_statistics();
        report.push_str("## 统计信息\n\n");
        report.push_str(&format!("- 总数: {}\n", stats.total));
        report.push_str(&format!("- 开放: {}\n", stats.open));
        report.push_str(&format!("- 进行中: {}\n", stats.in_progress));
        report.push_str(&format!("- 已完成: {}\n", stats.completed));
        report.push_str(&format!("- 高优先级: {}\n", stats.high_priority));
        report.push_str(&format!("- 中优先级: {}\n", stats.medium_priority));
        report.push_str(&format!("- 低优先级: {}\n", stats.low_priority));
        
        // 按优先级分组
        report.push_str("\n## 按优先级分组\n\n");
        
        let high_priority_todos = self.tracker.get_todos_by_priority(TodoPriority::High);
        if !high_priority_todos.is_empty() {
            report.push_str("### 高优先级\n\n");
            for todo in &high_priority_todos {
                report.push_str(&format!("- [{}] {}\n", todo.id, todo.title));
            }
            report.push_str("\n");
        }
        
        let medium_priority_todos = self.tracker.get_todos_by_priority(TodoPriority::Medium);
        if !medium_priority_todos.is_empty() {
            report.push_str("### 中优先级\n\n");
            for todo in &medium_priority_todos {
                report.push_str(&format!("- [{}] {}\n", todo.id, todo.title));
            }
            report.push_str("\n");
        }
        
        let low_priority_todos = self.tracker.get_todos_by_priority(TodoPriority::Low);
        if !low_priority_todos.is_empty() {
            report.push_str("### 低优先级\n\n");
            for todo in &low_priority_todos {
                report.push_str(&format!("- [{}] {}\n", todo.id, todo.title));
            }
            report.push_str("\n");
        }
        
        // 按类别分组
        report.push_str("## 按类别分组\n\n");
        
        let categories = vec![
            (TodoCategory::Bug, "Bug"),
            (TodoCategory::Feature, "功能"),
            (TodoCategory::Performance, "性能"),
            (TodoCategory::Security, "安全"),
            (TodoCategory::Test, "测试"),
            (TodoCategory::Documentation, "文档"),
            (TodoCategory::Refactoring, "重构"),
            (TodoCategory::Api, "API"),
            (TodoCategory::General, "通用"),
        ];
        
        for (category, name) in categories {
            let todos = self.tracker.get_todos_by_category(category);
            if !todos.is_empty() {
                report.push_str(&format!("### {}\n\n", name));
                for todo in &todos {
                    report.push_str(&format!("- [{}] {}\n", todo.id, todo.title));
                }
                report.push_str("\n");
            }
        }
        
        // 建议的解决方案
        report.push_str("## 建议的解决方案\n\n");
        
        // 高优先级TODO项的解决方案
        if !high_priority_todos.is_empty() {
            report.push_str("### 高优先级TODO项解决方案\n\n");
            for todo in &high_priority_todos {
                report.push_str(&format!("#### {}\n\n", todo.title));
                report.push_str(&format!("**位置**: {}\n\n", todo.description));
                
                // 根据TODO内容提供解决方案建议
                if todo.title.contains("implement") || todo.title.contains("实现") {
                    report.push_str("**建议**: 实现缺失的功能，确保代码完整性。\n\n");
                } else if todo.title.contains("fix") || todo.title.contains("修复") {
                    report.push_str("**建议**: 修复已知问题，确保代码正确性。\n\n");
                } else if todo.title.contains("security") || todo.title.contains("安全") {
                    report.push_str("**建议**: 优先解决安全问题，防止潜在漏洞。\n\n");
                } else {
                    report.push_str("**建议**: 尽快处理此高优先级项。\n\n");
                }
            }
        }
        
        report
    }

    /// 保存报告到文件
    fn save_report(&self, filename: &str) -> Result<(), std::io::Error> {
        let report = self.generate_report();
        fs::write(filename, report)
    }
}

fn main() -> Result<(), std::io::Error> {
    let project_root = Path::new(".");
    let mut resolver = TodoResolver::new();
    
    // 扫描并添加TODO项
    resolver.scan_and_add_todos(project_root)?;
    
    // 生成并保存报告
    resolver.save_report("TODO_RESOLUTION_REPORT.md")?;
    
    println!("TODO解决报告已保存到 TODO_RESOLUTION_REPORT.md");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_scanner() {
        let scanner = TodoScanner::new();
        
        // 创建测试文件
        let test_file = Path::new("test_todo.rs");
        fs::write(test_file, "// TODO: Implement this function\n// FIXME: Fix this bug\n").unwrap();
        
        // 扫描测试文件
        let todos = scanner.scan_file(test_file).unwrap();
        
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].marker_type, "TODO");
        assert_eq!(todos[1].marker_type, "FIXME");
        
        // 清理测试文件
        fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_todo_classifier() {
        let classifier = TodoClassifier;
        
        let todo_info = TodoInfo {
            file_path: "test.rs".to_string(),
            line_number: 10,
            content: "Implement critical security fix".to_string(),
            marker_type: "FIXME".to_string(),
        };
        
        let (priority, category) = classifier.classify(&todo_info);
        
        assert_eq!(priority, TodoPriority::High);
        assert_eq!(category, TodoCategory::Security);
    }
}