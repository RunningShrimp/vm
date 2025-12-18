//! 全面修复vm-core中的编译错误
//!
//! 这个脚本用于修复vm-core中的所有编译错误，包括条件编译、类型定义和未使用导入等问题。

use std::fs;
use std::path::Path;

fn main() -> Result<(), std::io::Error> {
    println!("全面修复vm-core中的编译错误...");
    
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
    
    // 修复unified_debugger.rs
    fix_unified_debugger()?;
    
    // 修复integration.rs
    fix_integration()?;
    
    // 修复enhanced_gdb_server.rs
    fix_enhanced_gdb_server()?;
    
    // 修复multi_thread_debug.rs
    fix_multi_thread_debug()?;
    
    println!("编译错误修复完成！");
    
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
        if line.contains("sqlx::") || line.contains("serde_json::") || line.contains("chrono::") || line.contains("DateTime::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
        }
        
        // 修复PostgresEventStoreConfig和PostgresEventStore的使用
        if line.contains("PostgresEventStoreConfig::") {
            new_content.push_str(&line.replace("PostgresEventStoreConfig::", "PostgresEventStoreBuilder::"));
        } else if line.contains("PostgresEventStore::new") {
            new_content.push_str(&line.replace("PostgresEventStore::new", "PostgresEventStoreBuilder::default().build"));
        } else {
            new_content.push_str(line);
        }
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
        // 添加条件编译指令到所有使用serde_json和chrono的代码
        if line.contains("serde_json::") || line.contains("chrono::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
        }
        
        // 修复metadata_json变量问题
        if line.contains("std::fs::write(&metadata_path, metadata_json)") {
            new_content.push_str("        std::fs::write(&metadata_path, &metadata_json)");
        } else if line.contains("let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)") {
            new_content.push_str("        let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)");
        } else if line.contains("if calculated_checksum != metadata.checksum") {
            new_content.push_str("        if calculated_checksum != loaded_metadata.checksum");
        } else if line.contains("expected: metadata.checksum.clone()") {
            new_content.push_str("                expected: loaded_metadata.checksum.clone()");
        } else if line.trim() == "metadata," {
            new_content.push_str("            loaded_metadata,");
        } else if line.contains("let cutoff_date = Utc::now() - chrono::Duration::days(self.config.retention_days as i64)") {
            new_content.push_str("            #[cfg(feature = \"enhanced-event-sourcing\")]");
            new_content.push_str("            let cutoff_date = Utc::now() - chrono::Duration::days(self.config.retention_days as i64)");
        } else {
            new_content.push_str(line);
        }
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
        if line.contains("EventSourcingConfig::") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
            new_content.push_str(&line.replace("EventSourcingConfig::", "EventSourcingBuilder::"));
        } else if line.contains("EnhancedEventSourcingService::new") {
            if !line.trim().starts_with("#[cfg(feature = \"enhanced-event-sourcing\")]") {
                new_content.push_str("#[cfg(feature = \"enhanced-event-sourcing\")]\n");
            }
            new_content.push_str(&line.replace("EnhancedEventSourcingService::new", "EventSourcingBuilder::default().build"));
        } else {
            new_content.push_str(line);
        }
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
        
        // 修复Breakpoint的实现
        if line.trim() == "impl Breakpoint {" {
            new_content.push_str("impl crate::debugger::breakpoint::Breakpoint {");
        } else if line.trim() == "impl BreakpointGroup {" {
            new_content.push_str("impl crate::debugger::breakpoint::BreakpointType {");
        } else if line.trim() == "impl Default for BreakpointManager {" {
            new_content.push_str("impl Default for crate::debugger::breakpoint::BreakpointManager {");
        } else {
            new_content.push_str(line);
        }
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
            new_content.push_str(&line.replace("CallStackConfig::", "CallStackBuilder::"));
        }
        
        // 修复CallStackTracker的实现
        if line.trim() == "impl Default for CallStackTracker {" {
            new_content.push_str("impl Default for crate::debugger::call_stack_tracker::CallStackTracker {");
        } else {
            new_content.push_str(line);
        }
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
            new_content.push_str(&line.replace("SymbolTableConfig::", "SymbolTableBuilder::"));
        }
        
        // 修复SymbolTable的实现
        if line.trim() == "impl Default for SymbolTable {" {
            new_content.push_str("impl Default for crate::debugger::symbol_table::SymbolTable {");
        } else {
            new_content.push_str(line);
        }
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复symbol_table.rs");
    
    Ok(())
}

fn fix_unified_debugger() -> Result<(), std::io::Error> {
    println!("修复unified_debugger.rs...");
    
    let file_path = "src/debugger/unified_debugger.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复配置类型的使用
        if line.contains("crate::debugger::enhanced_breakpoints::BreakpointConfig") {
            new_content.push_str(&line.replace("crate::debugger::enhanced_breakpoints::BreakpointConfig", "crate::debugger::enhanced_breakpoints::BreakpointBuilder"));
        } else if line.contains("crate::debugger::call_stack_tracker::CallStackConfig") {
            new_content.push_str(&line.replace("crate::debugger::call_stack_tracker::CallStackConfig", "crate::debugger::call_stack_tracker::CallStackBuilder"));
        } else if line.contains("crate::debugger::symbol_table::SymbolTableConfig") {
            new_content.push_str(&line.replace("crate::debugger::symbol_table::SymbolTableConfig", "crate::debugger::symbol_table::SymbolTableBuilder"));
        } else if line.contains("crate::debugger::enhanced_breakpoints::BreakpointStatistics") {
            new_content.push_str(&line.replace("crate::debugger::enhanced_breakpoints::BreakpointStatistics", "crate::debugger::enhanced_breakpoints::BreakpointStats"));
        } else if line.contains("crate::debugger::call_stack_tracker::CallStackStatistics") {
            new_content.push_str(&line.replace("crate::debugger::call_stack_tracker::CallStackStatistics", "crate::debugger::call_stack_tracker::CallStackStats"));
        } else {
            new_content.push_str(line);
        }
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复unified_debugger.rs");
    
    Ok(())
}

fn fix_integration() -> Result<(), std::io::Error> {
    println!("修复integration.rs...");
    
    let file_path = "src/debugger/integration.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复VcpuStateContainer的使用
        if line.contains("VcpuStateContainer") {
            new_content.push_str(&line.replace("VcpuStateContainer", "VcpuState"));
        } else {
            new_content.push_str(line);
        }
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复integration.rs");
    
    Ok(())
}

fn fix_enhanced_gdb_server() -> Result<(), std::io::Error> {
    println!("修复enhanced_gdb_server.rs...");
    
    let file_path = "src/debugger/enhanced_gdb_server.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复VariableValue的使用
        if line.contains("super::call_stack_tracker::VariableValue") {
            new_content.push_str(&line.replace("super::call_stack_tracker::VariableValue", "crate::debugger::call_stack_tracker::VariableValue"));
        } else {
            new_content.push_str(line);
        }
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复enhanced_gdb_server.rs");
    
    Ok(())
}

fn fix_multi_thread_debug() -> Result<(), std::io::Error> {
    println!("修复multi_thread_debug.rs...");
    
    let file_path = "src/debugger/multi_thread_debug.rs";
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    
    for line in lines {
        // 修复self的使用
        if line.contains("use std::thread::{self, ThreadId}") {
            new_content.push_str("use std::thread::ThreadId;");
        } else {
            new_content.push_str(line);
        }
        new_content.push_str("\n");
    }
    
    fs::write(file_path, new_content)?;
    println!("已修复multi_thread_debug.rs");
    
    Ok(())
}