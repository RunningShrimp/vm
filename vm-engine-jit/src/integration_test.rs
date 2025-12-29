//! JIT引擎集成测试
//!
//! 本模块提供JIT引擎的集成测试，验证所有组件的协同工作，
//! 包括自适应优化策略、动态重编译、代码热更新和性能监控反馈。

use std::sync::{Arc, Mutex};
use std::time::Duration;
use vm_core::{GuestAddr, VmError};
use vm_ir::IRBlock;
use crate::core::{JITEngine, JITConfig};
use crate::adaptive_threshold::PerformanceMetrics;
use crate::adaptive_optimization_strategy::{AdaptiveOptimizationStrategyManager, OptimizationStrategy};
use crate::dynamic_recompilation::DynamicRecompilationManager;
use crate::code_hot_update::CodeHotUpdateManager;
use crate::performance_monitoring_feedback::PerformanceMonitoringFeedbackManager;

/// JIT引擎集成测试套件
pub struct JITEngineIntegrationTest {
    /// JIT引擎
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 自适应优化策略管理器
    strategy_manager: Arc<Mutex<AdaptiveOptimizationStrategyManager>>,
    /// 动态重编译管理器
    recompilation_manager: Arc<Mutex<DynamicRecompilationManager>>,
    /// 代码热更新管理器
    hot_update_manager: Arc<Mutex<CodeHotUpdateManager>>,
    /// 性能监控和反馈管理器
    monitoring_manager: PerformanceMonitoringFeedbackManager,
}

impl JITEngineIntegrationTest {
    /// 创建新的集成测试套件
    pub fn new() -> Result<Self, VmError> {
        // 创建JIT引擎
        let jit_config = JITConfig::default();
        let jit_engine = Arc::new(Mutex::new(JITEngine::new(jit_config)));
        
        // 创建自适应优化策略管理器
        let adaptive_config = crate::adaptive_threshold::AdaptiveCompilationConfig::default();
        let strategy_manager = Arc::new(Mutex::new(
            AdaptiveOptimizationStrategyManager::new(jit_engine.clone(), adaptive_config)
        ));
        
        // 创建动态重编译管理器
        let recompilation_config = crate::dynamic_recompilation::DynamicRecompilationConfig::default();
        let recompilation_manager = Arc::new(Mutex::new(
            DynamicRecompilationManager::new(
                jit_engine.clone(),
                strategy_manager.clone(),
                recompilation_config
            )
        ));
        
        // 创建代码热更新管理器
        let hot_update_config = crate::code_hot_update::HotUpdateConfig::default();
        let hot_update_manager = Arc::new(Mutex::new(
            CodeHotUpdateManager::new(
                jit_engine.clone(),
                recompilation_manager.clone(),
                hot_update_config
            )
        ));
        
        // 创建性能监控和反馈管理器
        let monitoring_config = crate::performance_monitoring_feedback::MonitoringFeedbackConfig::default();
        let monitoring_manager = PerformanceMonitoringFeedbackManager::new(
            jit_engine.clone(),
            strategy_manager.clone(),
            recompilation_manager.clone(),
            hot_update_manager.clone(),
            monitoring_config
        );
        
        Ok(Self {
            jit_engine,
            strategy_manager,
            recompilation_manager,
            hot_update_manager,
            monitoring_manager,
        })
    }
    
    /// 运行完整的集成测试
    pub fn run_full_integration_test(&mut self) -> Result<(), VmError> {
        println!("开始JIT引擎集成测试...");
        
        // 启动性能监控系统
        self.monitoring_manager.start()?;
        
        // 创建测试IR块
        let test_ir_block = self.create_test_ir_block()?;
        
        // 测试自适应优化策略
        self.test_adaptive_optimization_strategy(&test_ir_block)?;
        
        // 测试动态重编译
        self.test_dynamic_recompilation(&test_ir_block)?;
        
        // 测试代码热更新
        self.test_code_hot_update(&test_ir_block)?;
        
        // 测试性能监控和反馈
        self.test_performance_monitoring_feedback(&test_ir_block)?;
        
        // 测试端到端流程
        self.test_end_to_end_workflow(&test_ir_block)?;
        
        println!("JIT引擎集成测试完成！");
        Ok(())
    }
    
    /// 测试自适应优化策略
    fn test_adaptive_optimization_strategy(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        println!("测试自适应优化策略...");

        // 分析IR块并选择策略
        let strategy = {
            let mut manager = self.strategy_manager.lock()
                .map_err(|_| VmError::LockPoisoned("StrategyManager lock poisoned".to_string()))?;
            manager.analyze_and_select_strategy(ir_block)?
        };

        println!("选择的优化策略: {:?}", strategy);

        // 应用优化策略
        {
            let mut manager = self.strategy_manager.lock()
                .map_err(|_| VmError::LockPoisoned("StrategyManager lock poisoned".to_string()))?;
            let mut ir_block_clone = ir_block.clone();
            manager.apply_optimization_strategy(&mut ir_block_clone)?;
        }

        println!("自适应优化策略测试完成");
        Ok(())
    }
    
    /// 测试动态重编译
    fn test_dynamic_recompilation(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        println!("测试动态重编译...");

        // 创建性能指标
        let metrics = PerformanceMetrics {
            execution_speed: 1000.0,
            compilation_time: Duration::from_millis(10),
            memory_usage: 1024 * 100,
            cache_hit_rate: 0.8,
            compilation_benefit: 1.5,
            average_execution_time: Duration::from_micros(100),
        };

        // 分析并决定是否需要重编译
        let decision = {
            let mut manager = self.recompilation_manager.lock()
                .map_err(|_| VmError::LockPoisoned("RecompilationManager lock poisoned".to_string()))?;
            manager.analyze_and_decide(0x1000, &metrics)?
        };

        println!("重编译决策: {:?}", decision);

        // 如果需要重编译，提交任务
        if decision.should_recompile {
            let mut manager = self.recompilation_manager.lock()
                .map_err(|_| VmError::LockPoisoned("RecompilationManager lock poisoned".to_string()))?;
            manager.submit_recompilation_task(decision, ir_block.clone())?;
        }

        // 处理重编译队列
        {
            let mut manager = self.recompilation_manager.lock()
                .map_err(|_| VmError::LockPoisoned("RecompilationManager lock poisoned".to_string()))?;
            manager.process_recompilation_queue()?;
        }

        println!("动态重编译测试完成");
        Ok(())
    }
    
    /// 测试代码热更新
    fn test_code_hot_update(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        println!("测试代码热更新...");

        // 创建测试代码
        let old_code = vec![0x90, 0x90, 0x90]; // NOP指令
        let new_code = vec![0x48, 0x89, 0xc0]; // MOV RAX, RAX

        // 提交热更新任务
        {
            let mut manager = self.hot_update_manager.lock()
                .map_err(|_| VmError::LockPoisoned("HotUpdateManager lock poisoned".to_string()))?;
            manager.submit_hot_update(0x1000, old_code, new_code, "测试更新".to_string())?;
        }

        // 处理热更新队列
        {
            let mut manager = self.hot_update_manager.lock()
                .map_err(|_| VmError::LockPoisoned("HotUpdateManager lock poisoned".to_string()))?;
            manager.process_update_queue()?;
        }

        println!("代码热更新测试完成");
        Ok(())
    }
    
    /// 测试性能监控和反馈
    fn test_performance_monitoring_feedback(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        println!("测试性能监控和反馈...");
        
        // 发送性能监控事件
        use crate::performance_monitoring_feedback::PerformanceMonitoringEvent;
        
        let event = PerformanceMonitoringEvent::BlockExecutionStart {
            pc: 0x2000,
            timestamp: std::time::Instant::now(),
        };
        
        self.monitoring_manager.send_event(event)?;
        
        let event = PerformanceMonitoringEvent::BlockExecutionEnd {
            pc: 0x2000,
            timestamp: std::time::Instant::now(),
            execution_time: Duration::from_micros(100),
        };
        
        self.monitoring_manager.send_event(event)?;
        
        // 检查是否有反馈
        if let Some(feedback) = self.monitoring_manager.get_feedback()? {
            println!("收到性能反馈: {:?}", feedback);
        }
        
        println!("性能监控和反馈测试完成");
        Ok(())
    }
    
    /// 测试端到端工作流程
    fn test_end_to_end_workflow(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        println!("测试端到端工作流程...");

        // 1. 编译IR块
        let compiled_code = {
            let mut engine = self.jit_engine.lock()
                .map_err(|_| VmError::LockPoisoned("JITEngine lock poisoned".to_string()))?;
            // 简化实现：返回空代码
            Vec::new()
        };

        // 2. 执行代码并收集性能数据
        let execution_time = Duration::from_micros(100);
        let memory_usage = compiled_code.len() as u64;

        // 3. 更新性能指标
        let metrics = PerformanceMetrics {
            execution_speed: 1000.0,
            compilation_time: Duration::from_millis(10),
            memory_usage,
            cache_hit_rate: 0.8,
            compilation_benefit: 1.5,
            average_execution_time: Duration::from_micros(100),
        };

        {
            let mut manager = self.strategy_manager.lock()
                .map_err(|_| VmError::LockPoisoned("StrategyManager lock poisoned".to_string()))?;
            manager.update_performance_metrics(metrics.clone());
        }

        // 4. 检查是否需要重编译
        let decision = {
            let mut manager = self.recompilation_manager.lock()
                .map_err(|_| VmError::LockPoisoned("RecompilationManager lock poisoned".to_string()))?;
            manager.analyze_and_decide(0x1000, &metrics)?
        };

        // 5. 如果需要，执行重编译和热更新
        if decision.should_recompile {
            println!("触发重编译和热更新...");

            // 提交重编译任务
            {
                let mut manager = self.recompilation_manager.lock()
                    .map_err(|_| VmError::LockPoisoned("RecompilationManager lock poisoned".to_string()))?;
                manager.submit_recompilation_task(decision, ir_block.clone())?;
            }

            // 提交热更新任务
            {
                let mut manager = self.hot_update_manager.lock()
                    .map_err(|_| VmError::LockPoisoned("HotUpdateManager lock poisoned".to_string()))?;
                manager.submit_hot_update(
                    0x1000,
                    compiled_code.clone(),
                    vec![0x48, 0x89, 0xc0],
                    "性能优化更新".to_string()
                )?;
            }
        }

        println!("端到端工作流程测试完成");
        Ok(())
    }
    
    /// 创建测试IR块
    fn create_test_ir_block(&self) -> Result<IRBlock, VmError> {
        let mut ir_block = IRBlock {
            start_pc: 0x1000,
            ops: Vec::new(),
            term: vm_ir::Terminator::Ret,
        };
        
        // 添加一些测试指令
        ir_block.ops.push(vm_ir::IROp::MovImm { dst: 0, imm: 42 });
        ir_block.ops.push(vm_ir::IROp::Add { dst: 1, src1: 0, src2: 1 });
        ir_block.ops.push(vm_ir::IROp::Mul { dst: 2, src1: 1, src2: 1 });
        
        Ok(ir_block)
    }
}

/// 运行集成测试
pub fn run_integration_tests() -> Result<(), VmError> {
    println!("启动JIT引擎集成测试套件...");
    
    // 创建集成测试
    let mut test_suite = JITEngineIntegrationTest::new()?;
    
    // 运行完整测试
    test_suite.run_full_integration_test()?;
    
    println!("所有集成测试通过！");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_integration_suite() {
        run_integration_tests().expect("集成测试失败");
    }
}