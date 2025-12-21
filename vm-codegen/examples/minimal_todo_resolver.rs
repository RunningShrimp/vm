//! 简化的TODO/FIXME标记扫描和解决工具
//!
//! 这个工具扫描项目中的TODO和FIXME标记，并生成解决建议。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// TODO项信息
#[derive(Debug, Clone)]
struct TodoInfo {
    file_path: String,
    line_number: usize,
    content: String,
    marker_type: String, // TODO, FIXME, XXX, HACK, BUG
}

/// TODO扫描器
struct TodoScanner;

impl TodoScanner {
    /// 扫描文件中的TODO项
    fn scan_file(&self, file_path: &Path) -> Result<Vec<TodoInfo>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        let mut todos = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // 检查TODO标记
            if let Some(start) = line.find("// TODO") {
                let content = line[start + 7..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "TODO".to_string(),
                });
            }

            // 检查FIXME标记
            if let Some(start) = line.find("// FIXME") {
                let content = line[start + 8..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "FIXME".to_string(),
                });
            }

            // 检查XXX标记
            if let Some(start) = line.find("// XXX") {
                let content = line[start + 6..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "XXX".to_string(),
                });
            }

            // 检查HACK标记
            if let Some(start) = line.find("// HACK") {
                let content = line[start + 7..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "HACK".to_string(),
                });
            }

            // 检查BUG标记
            if let Some(start) = line.find("// BUG") {
                let content = line[start + 6..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "BUG".to_string(),
                });
            }

            // 检查块注释中的TODO标记
            if let Some(start) = line.find("/* TODO") {
                let content = line[start + 7..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "TODO".to_string(),
                });
            }

            // 检查块注释中的FIXME标记
            if let Some(start) = line.find("/* FIXME") {
                let content = line[start + 8..].trim();
                todos.push(TodoInfo {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1,
                    content: content.to_string(),
                    marker_type: "FIXME".to_string(),
                });
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
                if path
                    .file_name()
                    .is_some_and(|name| name == "target" || name == ".git")
                {
                    continue;
                }

                // 递归扫描子目录
                match self.scan_directory(&path) {
                    Ok(mut todos) => all_todos.append(&mut todos),
                    Err(_) => continue, // 忽略无法读取的目录
                }
            } else if path.is_file() {
                // 只扫描.rs和.md文件，排除示例文件和报告文件
                if path
                    .extension()
                    .is_some_and(|ext| ext == "rs" || ext == "md")
                {
                    // 排除示例文件和报告文件
                    let path_str = path.to_string_lossy();
                    if path_str.contains("examples/")
                        || path_str.contains("test_")
                        || path_str.contains("TODO_")
                        || path_str.contains("FRONTEND_CODEGEN.md")
                    {
                        continue;
                    }

                    match self.scan_file(&path) {
                        Ok(mut todos) => all_todos.append(&mut todos),
                        Err(_) => continue, // 忽略无法读取的文件
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
    fn classify(&self, todo_info: &TodoInfo) -> (String, String) {
        let content = todo_info.content.to_lowercase();
        let marker_type = todo_info.marker_type.to_lowercase();

        // 根据标记类型确定基础优先级
        let base_priority = if marker_type.contains("fixme") || marker_type.contains("bug") {
            "高".to_string()
        } else if marker_type.contains("hack") || marker_type.contains("xxx") {
            "中".to_string()
        } else {
            "低".to_string()
        };

        // 根据内容调整优先级
        let priority = if content.contains("critical")
            || content.contains("urgent")
            || content.contains("security")
            || content.contains("vulnerability")
        {
            "高".to_string()
        } else if content.contains("performance") || content.contains("optimize") {
            "中".to_string()
        } else if content.contains("refactor") || content.contains("cleanup") {
            "低".to_string()
        } else {
            base_priority
        };

        // 根据内容确定类别
        let category = if content.contains("test") || content.contains("testing") {
            "测试".to_string()
        } else if content.contains("performance") || content.contains("optimize") {
            "性能".to_string()
        } else if content.contains("security") || content.contains("vulnerability") {
            "安全".to_string()
        } else if content.contains("refactor") || content.contains("cleanup") {
            "重构".to_string()
        } else if content.contains("feature") || content.contains("implement") {
            "功能".to_string()
        } else if content.contains("bug") || content.contains("fix") {
            "Bug".to_string()
        } else if content.contains("documentation") || content.contains("doc") {
            "文档".to_string()
        } else if content.contains("api") || content.contains("interface") {
            "API".to_string()
        } else {
            "通用".to_string()
        };

        (priority, category)
    }

    /// 生成解决方案建议
    fn generate_solution(&self, todo_info: &TodoInfo, priority: &str, category: &str) -> String {
        let content = todo_info.content.to_lowercase();

        if priority == "高" {
            if category == "安全" {
                "立即解决此安全问题，防止潜在漏洞。考虑进行安全审计和渗透测试。".to_string()
            } else if category == "Bug" {
                "尽快修复此Bug，确保系统稳定性。添加单元测试验证修复。".to_string()
            } else if content.contains("implement") || content.contains("实现") {
                "优先实现此功能，确保系统完整性。考虑分阶段实现。".to_string()
            } else {
                "高优先级项，建议尽快处理。分配专门资源解决。".to_string()
            }
        } else if priority == "中" {
            if category == "性能" {
                "优化此性能问题，提升系统效率。使用性能分析工具定位瓶颈。".to_string()
            } else if category == "功能" {
                "计划实现此功能，纳入下一个开发周期。".to_string()
            } else if category == "重构" {
                "重构此代码，提高可维护性。确保重构后功能不变。".to_string()
            } else {
                "中优先级项，建议在当前开发周期内处理。".to_string()
            }
        } else if category == "文档" {
            "完善文档，提高代码可读性和可维护性。".to_string()
        } else if category == "测试" {
            "添加或完善测试，确保代码质量。考虑使用TDD方法。".to_string()
        } else if category == "重构" {
            "在有时间时重构此代码，提高代码质量。".to_string()
        } else {
            "低优先级项，可在有空闲时间时处理。".to_string()
        }
    }
}

/// TODO解决器
struct TodoResolver {
    todos: Vec<TodoInfo>,
}

impl TodoResolver {
    fn new() -> Self {
        Self { todos: Vec::new() }
    }

    /// 扫描TODO项
    fn scan_todos(&mut self, project_root: &Path) -> Result<(), std::io::Error> {
        let scanner = TodoScanner;

        println!("扫描TODO项...");
        self.todos = scanner.scan_directory(project_root)?;

        println!("找到 {} 个TODO项", self.todos.len());
        Ok(())
    }

    /// 生成TODO报告
    fn generate_report(&self) -> String {
        let mut report = String::new();
        let classifier = TodoClassifier;

        report.push_str("# TODO/FIXME标记解决报告\n\n");

        // 统计信息
        let mut stats = HashMap::new();
        let mut priority_stats = HashMap::new();
        let mut category_stats = HashMap::new();

        for todo in &self.todos {
            *stats.entry(todo.marker_type.clone()).or_insert(0) += 1;

            let (priority, category) = classifier.classify(todo);
            *priority_stats.entry(priority).or_insert(0) += 1;
            *category_stats.entry(category).or_insert(0) += 1;
        }

        report.push_str("## 统计信息\n\n");
        report.push_str("### 按标记类型统计\n\n");
        for (marker_type, count) in &stats {
            report.push_str(&format!("- {}: {}\n", marker_type, count));
        }

        report.push_str("\n### 按优先级统计\n\n");
        for (priority, count) in &priority_stats {
            report.push_str(&format!("- {}: {}\n", priority, count));
        }

        report.push_str("\n### 按类别统计\n\n");
        for (category, count) in &category_stats {
            report.push_str(&format!("- {}: {}\n", category, count));
        }

        // 高优先级TODO项
        report.push_str("\n## 高优先级TODO项\n\n");
        for todo in &self.todos {
            let (priority, _) = classifier.classify(todo);
            if priority == "高" {
                report.push_str(&format!(
                    "### {}: {}:{}\n\n",
                    todo.marker_type, todo.file_path, todo.line_number
                ));
                report.push_str(&format!("**内容**: {}\n\n", todo.content));

                let (_, category) = classifier.classify(todo);
                let solution = classifier.generate_solution(todo, &priority, &category);
                report.push_str(&format!("**建议解决方案**: {}\n\n", solution));
            }
        }

        // 中优先级TODO项
        report.push_str("## 中优先级TODO项\n\n");
        for todo in &self.todos {
            let (priority, _) = classifier.classify(todo);
            if priority == "中" {
                report.push_str(&format!(
                    "### {}: {}:{}\n\n",
                    todo.marker_type, todo.file_path, todo.line_number
                ));
                report.push_str(&format!("**内容**: {}\n\n", todo.content));

                let (_, category) = classifier.classify(todo);
                let solution = classifier.generate_solution(todo, &priority, &category);
                report.push_str(&format!("**建议解决方案**: {}\n\n", solution));
            }
        }

        // 低优先级TODO项
        report.push_str("## 低优先级TODO项\n\n");
        for todo in &self.todos {
            let (priority, _) = classifier.classify(todo);
            if priority == "低" {
                report.push_str(&format!(
                    "### {}: {}:{}\n\n",
                    todo.marker_type, todo.file_path, todo.line_number
                ));
                report.push_str(&format!("**内容**: {}\n\n", todo.content));

                let (_, category) = classifier.classify(todo);
                let solution = classifier.generate_solution(todo, &priority, &category);
                report.push_str(&format!("**建议解决方案**: {}\n\n", solution));
            }
        }

        // 实施计划
        report.push_str("## 实施计划\n\n");
        report.push_str("### 第一阶段：高优先级项（1-2周）\n\n");
        report.push_str("1. 解决所有安全相关的TODO项\n");
        report.push_str("2. 修复关键Bug\n");
        report.push_str("3. 实现核心功能\n\n");

        report.push_str("### 第二阶段：中优先级项（2-4周）\n\n");
        report.push_str("1. 优化性能问题\n");
        report.push_str("2. 实现次要功能\n");
        report.push_str("3. 重构关键代码\n\n");

        report.push_str("### 第三阶段：低优先级项（4-8周）\n\n");
        report.push_str("1. 完善文档\n");
        report.push_str("2. 添加测试\n");
        report.push_str("3. 代码清理和重构\n\n");

        report
    }

    /// 保存报告到文件
    fn save_report(&self, filename: &str) -> Result<(), std::io::Error> {
        let report = self.generate_report();
        fs::write(filename, report)
    }
}

fn main() -> Result<(), std::io::Error> {
    let project_root = Path::new("..");
    let mut resolver = TodoResolver::new();

    // 扫描TODO项
    resolver.scan_todos(project_root)?;

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
        let scanner = TodoScanner;

        // 创建测试文件
        let test_file = Path::new("test_todo.rs");
        fs::write(
            test_file,
            "// TODO: Implement this function\n// FIXME: Fix this bug\n",
        )
        .unwrap();

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

        assert_eq!(priority, "高");
        assert_eq!(category, "安全");
    }
}
