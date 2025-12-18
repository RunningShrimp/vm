#[cfg(test)]
mod tests {
    use distributed_executor::*;

    #[tokio::test]
    async fn test_coordinator_initialization() {
        // Create a default distributed architecture config
        let config = DistributedArchitectureConfig::default();

        // Initialize the distributed environment
        match initialize_distributed_environment(config).await {
            Ok(coordinator) => {
                println!("Coordinator initialized successfully!");
                println!("Active VMs: {:?}", coordinator.get_active_vms().await);
                // For now, this will be empty since no VMs are registered
            }
            Err(e) => {
                panic!("Failed to initialize coordinator: {}", e);
            }
        }
    }
}
