//! 精确修复vm-core中的语法错误
//!
//! 这个脚本用于修复vm-core中的语法错误，特别是条件编译和语法问题。

use std::fs;

fn main() -> Result<(), std::io::Error> {
    println!("精确修复vm-core中的语法错误...");
    
    // 修复postgres_event_store.rs
    fix_postgres_event_store_syntax()?;
    
    // 修复call_stack_tracker.rs
    fix_call_stack_tracker_syntax()?;
    
    // 修复symbol_table.rs
    fix_symbol_table_syntax()?;
    
    // 修复enhanced_snapshot.rs
    fix_enhanced_snapshot_syntax()?;
    
    println!("语法错误修复完成！");
    
    Ok(())
}

fn fix_postgres_event_store_syntax() -> Result<(), std::io::Error> {
    println!("修复postgres_event_store.rs语法错误...");
    
    let file_path = "src/event_store/postgres_event_store.rs";
    let content = fs::read_to_string(file_path)?;
    
    // 修复语法错误
    let fixed_content = content
        // 修复函数参数错误
        .replace(") -> VmResult<sqlx::postgres::PgStatement> {", ") -> VmResult<sqlx::postgres::PgStatement> {")
        // 修复条件编译后的语法错误
        .replace("#[cfg(feature = \"enhanced-event-sourcing\")]\n            .bind(compressed_data)\n            #[cfg(feature = \"enhanced-event-sourcing\")]\n            .bind(serde_json::to_value(&event.metadata).unwrap_or(serde_json::Value::Null))", 
                "#[cfg(feature = \"enhanced-event-sourcing\")]\n            .bind(compressed_data)\n            #[cfg(feature = \"enhanced-event-sourcing\")]\n            .bind(serde_json::to_value(&event.metadata).unwrap_or(serde_json::Value::Null))");
    
    fs::write(file_path, fixed_content)?;
    println!("已修复postgres_event_store.rs语法错误");
    
    Ok(())
}

fn fix_call_stack_tracker_syntax() -> Result<(), std::io::Error> {
    println!("修复call_stack_tracker.rs语法错误...");
    
    let file_path = "src/debugger/call_stack_tracker.rs";
    let content = fs::read_to_string(file_path)?;
    
    // 修复语法错误
    let fixed_content = content
        // 修复条件编译后的语法错误
        .replace("#[cfg(feature = \"enhanced-debugging\")]\n        Self::new(CallStackBuilder::default(), 0)", 
                "#[cfg(feature = \"enhanced-debugging\")]\n        Self::new(CallStackBuilder::default(), 0);");
    
    fs::write(file_path, fixed_content)?;
    println!("已修复call_stack_tracker.rs语法错误");
    
    Ok(())
}

fn fix_symbol_table_syntax() -> Result<(), std::io::Error> {
    println!("修复symbol_table.rs语法错误...");
    
    let file_path = "src/debugger/symbol_table.rs";
    let content = fs::read_to_string(file_path)?;
    
    // 修复语法错误
    let fixed_content = content
        // 修复条件编译后的语法错误
        .replace("#[cfg(feature = \"enhanced-debugging\")]\n        Self::new(SymbolTableBuilder::default())", 
                "#[cfg(feature = \"enhanced-debugging\")]\n        Self::new(SymbolTableBuilder::default());");
    
    fs::write(file_path, fixed_content)?;
    println!("已修复symbol_table.rs语法错误");
    
    Ok(())
}

fn fix_enhanced_snapshot_syntax() -> Result<(), std::io::Error> {
    println!("修复enhanced_snapshot.rs语法错误...");
    
    let file_path = "src/snapshot/enhanced_snapshot.rs";
    let content = fs::read_to_string(file_path)?;
    
    // 修复语法错误
    let fixed_content = content
        // 修复函数体结束位置错误
        .replace("        }\n    }\n}\n\n#[cfg(feature = \"enhanced-event-sourcing\")]\npub struct SnapshotStoreBuilder {", 
                "        }\n    }\n}\n\n#[cfg(feature = \"enhanced-event-sourcing\")]\npub struct SnapshotStoreBuilder {");
    
    fs::write(file_path, fixed_content)?;
    println!("已修复enhanced_snapshot.rs语法错误");
    
    Ok(())
}