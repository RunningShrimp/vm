//! 修复vm-core中的条件编译问题
//!
//! 这个脚本用于修复vm-core中的条件编译问题，确保所有使用可选依赖的代码都被正确地条件化。

use std::fs;
use std::path::Path;

fn main() -> Result<(), std::io::Error> {
    println!("修复vm-core中的条件编译问题...");
    
    // 修复postgres_event_store.rs
    fix_postgres_event_store()?;
    
    // 修复enhanced_snapshot.rs
    fix_enhanced_snapshot()?;
    
    // 修复event_sourcing.rs
    fix_event_sourcing()?;
    
    // 修复enhanced_breakpoints.rs
    fix_enhanced_breakpoints()?;
    
    // 修复call_stack_tracker.rs
    fix_call_stack_tracker()?;
    
    // 修复symbol_table.rs
    fix_symbol_table()?;
    
    println!("条件编译问题修复完成！");
    
    Ok(())
}

fn fix_postgres_event_store() -> Result<(), std::io::Error> {
    println!("修复postgres_event_store.rs...");
    
    let file_path = "src/event_store/postgres_event_store.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 添加条件编译指令到所有使用sqlx、serde_json和chrono的代码
        if line.contains("sqlx::") || line.contains("serde_json::") || line.contains("chrono::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复postgres_event_store.rs");
    
    Ok(())
}

fn fix_enhanced_snapshot() -> Result<(), std::io::Error> {
    println!("修复enhanced_snapshot.rs...");
    
    let file_path = "src/snapshot/enhanced_snapshot.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 添加条件编译指令到所有使用serde_json的代码
        if line.contains("serde_json::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复enhanced_snapshot.rs");
    
    Ok(())
}

fn fix_event_sourcing() -> Result<(), std::io::Error> {
    println!("修复event_sourcing.rs...");
    
    let file_path = "src/event_sourcing.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复EventSourcingConfig和EnhancedEventSourcingService的使用
        if line.contains("EventSourcingConfig::") || line.contains("EnhancedEventSourcingService::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复event_sourcing.rs");
    
    Ok(())
}

fn fix_enhanced_breakpoints() -> Result<(), std::io::Error> {
    println!("修复enhanced_breakpoints.rs...");
    
    let file_path = "src/debugger/enhanced_breakpoints.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复BreakpointCondition的使用
        if line.contains("BreakpointCondition::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-debugging\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-debugging\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复enhanced_breakpoints.rs");
    
    Ok(())
}

fn fix_call_stack_tracker() -> Result<(), std::io::Error> {
    println!("修复call_stack_tracker.rs...");
    
    let file_path = "src/debugger/call_stack_tracker.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复CallStackConfig的使用
        if line.contains("CallStackConfig::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-debugging\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-debugging\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复call_stack_tracker.rs");
    
    Ok(())
}

fn fix_symbol_table() -> Result<(), std::io::Error> {
    println!("修复symbol_table.rs...");
    
    let file_path = "src/debugger/symbol_table.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复SymbolTableConfig的使用
        if line.contains("SymbolTableConfig::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-debugging\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-debugging\")]\n");
            }
        }
        new_content.push_str(line);
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复symbol_table.rs");
    
    Ok(())
}