//! 依赖解析器
//!
//! 本模块实现了依赖解析的核心逻辑，包括依赖图构建、循环依赖检测和依赖解析策略。

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::di_service_descriptor::{
    DIError, ServiceDescriptor, ServiceProvider, ServiceLifetime,
};

/// 将锁错误转换为 DIError
fn lock_error(operation: &str) -> DIError {
    DIError::DependencyResolutionFailed(format!(
        "Failed to acquire lock for {}",
        operation
    ))
}

/// 依赖解析器
pub struct DependencyResolver {
    /// 依赖图缓存
    dependency_graph: Arc<std::sync::RwLock<HashMap<TypeId, DependencyNode>>>,
    
    /// 解析策略
    strategy: ResolutionStrategy,
}

/// 解析策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStrategy {
    /// 深度优先搜索
    DepthFirst,
    /// 广度优先搜索
    BreadthFirst,
    /// 拓扑排序
    TopologicalSort,
}

/// 依赖节点
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// 服务类型ID
    pub type_id: TypeId,
    /// 依赖的服务类型
    pub dependencies: Vec<TypeId>,
    /// 依赖此服务的类型
    pub dependents: Vec<TypeId>,
    /// 生命周期
    pub lifetime: ServiceLifetime,
    /// 是否已解析
    pub resolved: bool,
}

/// 依赖图
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// 所有节点
    nodes: HashMap<TypeId, DependencyNode>,
    /// 解析顺序
    resolution_order: Vec<TypeId>,
}

impl DependencyResolver {
    /// 创建新的依赖解析器
    pub fn new(strategy: ResolutionStrategy) -> Self {
        Self {
            dependency_graph: Arc::new(std::sync::RwLock::new(HashMap::new())),
            strategy,
        }
    }

    /// 辅助方法：获取读锁
    fn lock_read(&self) -> Result<std::sync::RwLockReadGuard<HashMap<TypeId, DependencyNode>>, DIError> {
        self.dependency_graph.read().map_err(|_| lock_error("read dependency graph"))
    }

    /// 辅助方法：获取写锁
    fn lock_write(&self) -> Result<std::sync::RwLockWriteGuard<HashMap<TypeId, DependencyNode>>, DIError> {
        self.dependency_graph.write().map_err(|_| lock_error("write dependency graph"))
    }

    /// 添加服务描述符到依赖图
    pub fn add_service_descriptor(&self, descriptor: &dyn ServiceDescriptor) -> Result<(), DIError> {
        let type_id = descriptor.service_type();
        let dependencies = descriptor.dependencies();
        let lifetime = descriptor.lifetime();

        let mut graph = self.lock_write()?;

        // 创建新节点
        let node = DependencyNode {
            type_id,
            dependencies: dependencies.clone(),
            dependents: Vec::new(),
            lifetime,
            resolved: false,
        };

        // 更新依赖关系
        for &dep_type_id in &dependencies {
            if let Some(dep_node) = graph.get_mut(&dep_type_id) {
                dep_node.dependents.push(type_id);
            }
        }

        graph.insert(type_id, node);
        Ok(())
    }
    
    /// 解析服务的依赖
    pub fn resolve_dependencies(
        &self,
        service_type: TypeId,
        provider: &dyn ServiceProvider,
    ) -> Result<Vec<Arc<dyn Any + Send + Sync>>, DIError> {
        // 构建依赖图
        let graph = self.build_dependency_graph(service_type)?;
        
        // 检测循环依赖
        self.detect_circular_dependencies(&graph, service_type)?;
        
        // 根据策略解析依赖
        match self.strategy {
            ResolutionStrategy::DepthFirst => {
                self.resolve_depth_first(&graph, service_type, provider)
            }
            ResolutionStrategy::BreadthFirst => {
                self.resolve_breadth_first(&graph, service_type, provider)
            }
            ResolutionStrategy::TopologicalSort => {
                self.resolve_topological_sort(&graph, service_type, provider)
            }
        }
    }
    
    /// 构建依赖图
    fn build_dependency_graph(&self, root_type: TypeId) -> Result<DependencyGraph, DIError> {
        let graph = self.lock_read()?;
        let mut nodes = HashMap::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(root_type);

        while let Some(current_type) = queue.pop_front() {
            if visited.contains(&current_type) {
                continue;
            }

            visited.insert(current_type);

            if let Some(node) = graph.get(&current_type) {
                let new_node = node.clone();

                // 递归添加依赖
                for &dep_type in &node.dependencies {
                    if !visited.contains(&dep_type) {
                        queue.push_back(dep_type);
                    }
                }

                nodes.insert(current_type, new_node);
            } else {
                return Err(DIError::ServiceNotRegistered(current_type));
            }
        }

        // 计算解析顺序
        let resolution_order = self.calculate_resolution_order(&nodes)?;

        Ok(DependencyGraph {
            nodes,
            resolution_order,
        })
    }
    
    /// 检测循环依赖
    fn detect_circular_dependencies(
        &self,
        graph: &DependencyGraph,
        root_type: TypeId,
    ) -> Result<(), DIError> {
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        
        if self.has_cycle(
            graph,
            root_type,
            &mut visiting,
            &mut visited,
            &mut path,
        ) {
            return Err(DIError::CircularDependency(path));
        }
        
        Ok(())
    }
    
    /// 检查是否存在循环依赖
    fn has_cycle(
        &self,
        graph: &DependencyGraph,
        current_type: TypeId,
        visiting: &mut HashSet<TypeId>,
        visited: &mut HashSet<TypeId>,
        path: &mut Vec<TypeId>,
    ) -> bool {
        if visited.contains(&current_type) {
            return false;
        }
        
        if visiting.contains(&current_type) {
            // 找到循环
            if let Some(cycle_start) = path.iter().position(|&t| t == current_type) {
                path.truncate(path.len() - 1);
                let cycle = path[cycle_start..].to_vec();
                path.extend_from_slice(&cycle);
                path.push(current_type);
            }
            return true;
        }
        
        visiting.insert(current_type);
        path.push(current_type);
        
        if let Some(node) = graph.nodes.get(&current_type) {
            for &dep_type in &node.dependencies {
                if self.has_cycle(graph, dep_type, visiting, visited, path) {
                    return true;
                }
            }
        }
        
        visiting.remove(&current_type);
        visited.insert(current_type);
        path.pop();
        
        false
    }
    
    /// 计算解析顺序（拓扑排序）
    fn calculate_resolution_order(&self, nodes: &HashMap<TypeId, DependencyNode>) -> Result<Vec<TypeId>, DIError> {
        let mut in_degree: HashMap<TypeId, usize> = HashMap::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::new();
        
        // 计算入度
        for node in nodes.values() {
            in_degree.insert(node.type_id, node.dependencies.len());
            if node.dependencies.is_empty() {
                queue.push_back(node.type_id);
            }
        }
        
        // 拓扑排序
        while let Some(current_type) = queue.pop_front() {
            order.push(current_type);
            
            if let Some(node) = nodes.get(&current_type) {
                for &dependent_type in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent_type) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent_type);
                        }
                    }
                }
            }
        }
        
        // 检查是否所有节点都已处理
        if order.len() != nodes.len() {
            return Err(DIError::CircularDependency(
                nodes.keys().cloned().collect(),
            ));
        }
        
        Ok(order)
    }
    
    /// 深度优先解析
    fn resolve_depth_first(
        &self,
        graph: &DependencyGraph,
        service_type: TypeId,
        provider: &dyn ServiceProvider,
    ) -> Result<Vec<Arc<dyn Any + Send + Sync>>, DIError> {
        let mut resolved = HashMap::new();
        let mut result = Vec::new();
        
        self.resolve_dfs(graph, service_type, provider, &mut resolved, &mut result)?;
        Ok(result)
    }
    
    /// 深度优先解析辅助函数
    fn resolve_dfs(
        &self,
        graph: &DependencyGraph,
        current_type: TypeId,
        provider: &dyn ServiceProvider,
        resolved: &mut HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
        result: &mut Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<(), DIError> {
        if resolved.contains_key(&current_type) {
            return Ok(());
        }
        
        if let Some(node) = graph.nodes.get(&current_type) {
            // 先解析依赖
            for &dep_type in &node.dependencies {
                self.resolve_dfs(graph, dep_type, provider, resolved, result)?;
            }
            
            // 解析当前服务
            let service = provider.get_service_by_id(current_type)?;
            if let Some(service) = service {
                resolved.insert(current_type, service.clone());
                result.push(service);
            } else {
                return Err(DIError::ServiceNotRegistered(current_type));
            }
        }
        
        Ok(())
    }
    
    /// 广度优先解析
    fn resolve_breadth_first(
        &self,
        graph: &DependencyGraph,
        service_type: TypeId,
        provider: &dyn ServiceProvider,
    ) -> Result<Vec<Arc<dyn Any + Send + Sync>>, DIError> {
        let mut resolved = HashSet::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        
        queue.push_back(service_type);
        
        while let Some(current_type) = queue.pop_front() {
            if resolved.contains(&current_type) {
                continue;
            }
            
            if let Some(node) = graph.nodes.get(&current_type) {
                // 检查所有依赖是否已解析
                let mut all_deps_resolved = true;
                for &dep_type in &node.dependencies {
                    if !resolved.contains(&dep_type) {
                        queue.push_back(dep_type);
                        all_deps_resolved = false;
                    }
                }
                
                if !all_deps_resolved {
                    queue.push_back(current_type);
                    continue;
                }
                
                // 解析当前服务
                let service = provider.get_service_by_id(current_type)?;
                if let Some(service) = service {

                    resolved.insert(current_type);
                    result.push(service);
                    
                    // 添加依赖此服务的类型到队列
                    for &dependent_type in &node.dependents {
                        if !resolved.contains(&dependent_type) {
                            queue.push_back(dependent_type);
                        }
                    }
                } else {
                    return Err(DIError::ServiceNotRegistered(current_type));
                }
            }
        }
        
        Ok(result)
    }
    
    /// 拓扑排序解析
    fn resolve_topological_sort(
        &self,
        graph: &DependencyGraph,
        _service_type: TypeId,
        provider: &dyn ServiceProvider,
    ) -> Result<Vec<Arc<dyn Any + Send + Sync>>, DIError> {
        let mut result = Vec::new();
        
        // 按照拓扑顺序解析
       for &type_id in &graph.resolution_order {
           let service = provider.get_service_by_id(type_id)?;

            if let Some(service) = service {
                result.push(service);
            } else {
                return Err(DIError::ServiceNotRegistered(type_id));
            }
        }
        
        Ok(result)
    }
    
    /// 获取服务的依赖链
    pub fn get_dependency_chain(&self, service_type: TypeId) -> Result<Vec<TypeId>, DIError> {
        let graph = self.lock_read()?;
        let mut chain = Vec::new();
        let mut visited = HashSet::new();

        self.build_dependency_chain(&graph, service_type, &mut chain, &mut visited)?;
        Ok(chain)
    }
    
    /// 构建依赖链
    fn build_dependency_chain(
        &self,
        graph: &HashMap<TypeId, DependencyNode>,
        current_type: TypeId,
        chain: &mut Vec<TypeId>,
        visited: &mut HashSet<TypeId>,
    ) -> Result<(), DIError> {
        if visited.contains(&current_type) {
            return Ok(());
        }
        
        visited.insert(current_type);
        
        if let Some(node) = graph.get(&current_type) {
            for &dep_type in &node.dependencies {
                self.build_dependency_chain(graph, dep_type, chain, visited)?;
            }
            chain.push(current_type);
        } else {
            return Err(DIError::ServiceNotRegistered(current_type));
        }
        
        Ok(())
    }
    
    /// 清除依赖图缓存
    pub fn clear_cache(&self) {
        match self.lock_write() {
            Ok(mut graph) => {
                graph.clear();
            }
            Err(_) => {
                // 静默失败
                eprintln!("Failed to clear cache: lock failed");
            }
        }
    }

    /// 获取依赖图统计信息
    pub fn stats(&self) -> ResolverStats {
        match self.lock_read() {
            Ok(graph) => {
                let mut total_dependencies = 0;
                let mut total_dependents = 0;

                for node in graph.values() {
                    total_dependencies += node.dependencies.len();
                    total_dependents += node.dependents.len();
                }

                ResolverStats {
                    total_nodes: graph.len(),
                    total_dependencies,
                    total_dependents,
                    average_dependencies: if graph.is_empty() {
                        0.0
                    } else {
                        total_dependencies as f64 / graph.len() as f64
                    },
                }
            }
            Err(_) => ResolverStats {
                total_nodes: 0,
                total_dependencies: 0,
                total_dependents: 0,
                average_dependencies: 0.0,
            },
        }
    }
}

/// 解析器统计信息
#[derive(Debug, Clone)]
pub struct ResolverStats {
    /// 总节点数
    pub total_nodes: usize,
    /// 总依赖数
    pub total_dependencies: usize,
    /// 总被依赖数
    pub total_dependents: usize,
    /// 平均依赖数
    pub average_dependencies: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::di_service_descriptor::GenericServiceDescriptor;
    
    struct TestServiceA;
    struct TestServiceB;
    struct TestServiceC;
    
    #[test]
    fn test_dependency_resolver_creation() {
        let resolver = DependencyResolver::new(ResolutionStrategy::DepthFirst);
        let stats = resolver.stats();
        assert_eq!(stats.total_nodes, 0);
    }
    
    #[test]
    fn test_add_service_descriptor() {
        let resolver = DependencyResolver::new(ResolutionStrategy::DepthFirst);
        let descriptor = GenericServiceDescriptor::<TestServiceA>::new(ServiceLifetime::Singleton);
        
        assert!(resolver.add_service_descriptor(&*descriptor).is_ok());
        let stats = resolver.stats();
        assert_eq!(stats.total_nodes, 1);
    }
    
    #[test]
    fn test_dependency_chain() {
        let resolver = DependencyResolver::new(ResolutionStrategy::DepthFirst);
        
        // 创建有依赖关系的服务描述符
        let descriptor_a = GenericServiceDescriptor::<TestServiceA>::new(ServiceLifetime::Singleton);
        let descriptor_b = GenericServiceDescriptor::<TestServiceB>::new(ServiceLifetime::Singleton)
            .with_dependencies(vec![TypeId::of::<TestServiceA>()]);
        let descriptor_c = GenericServiceDescriptor::<TestServiceC>::new(ServiceLifetime::Singleton)
            .with_dependencies(vec![TypeId::of::<TestServiceB>()]);
        
        let _ = resolver.add_service_descriptor(&*descriptor_a);
        let _ = resolver.add_service_descriptor(&*descriptor_b);
        let _ = resolver.add_service_descriptor(&*descriptor_c);

        let chain = resolver.get_dependency_chain(TypeId::of::<TestServiceC>())?;
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], TypeId::of::<TestServiceA>());
        assert_eq!(chain[1], TypeId::of::<TestServiceB>());
        assert_eq!(chain[2], TypeId::of::<TestServiceC>());
    }
    
    #[test]
    fn test_clear_cache() {
        let resolver = DependencyResolver::new(ResolutionStrategy::DepthFirst);
        let descriptor = GenericServiceDescriptor::<TestServiceA>::new(ServiceLifetime::Singleton);
        
        let _ = resolver.add_service_descriptor(&*descriptor);
        assert_eq!(resolver.stats().total_nodes, 1);
        
        resolver.clear_cache();
        assert_eq!(resolver.stats().total_nodes, 0);
    }
}