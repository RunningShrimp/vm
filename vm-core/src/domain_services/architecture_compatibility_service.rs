//! Architecture Compatibility Domain Service
//!
//! This service provides validation and compatibility checking for cross-architecture
//! translation and optimization.

use crate::{GuestArch, VmResult};

/// Architecture compatibility domain service
///
/// Validates compatibility between different architectures for cross-architecture
/// translation and optimization operations.
pub struct ArchitectureCompatibilityDomainService {
    /// Enable detailed logging
    enable_logging: bool,
}

impl ArchitectureCompatibilityDomainService {
    /// Create a new architecture compatibility service
    pub fn new() -> Self {
        Self {
            enable_logging: false,
        }
    }

    /// Create a new service with logging enabled
    pub fn with_logging(mut self) -> Self {
        self.enable_logging = true;
        self
    }

    /// Validate if translation from source to target architecture is supported
    pub fn validate_translation_support(
        &self,
        source: GuestArch,
        target: GuestArch,
    ) -> VmResult<bool> {
        // Check if the translation pair is supported
        let supported = match (source, target) {
            // Same architecture is always supported
            (s, t) if s == t => true,

            // Supported cross-architecture translations
            (GuestArch::X86_64, GuestArch::Arm64) => true,
            (GuestArch::Arm64, GuestArch::X86_64) => true,
            (GuestArch::X86_64, GuestArch::Riscv64) => true,
            (GuestArch::Riscv64, GuestArch::X86_64) => true,
            (GuestArch::Arm64, GuestArch::Riscv64) => true,
            (GuestArch::Riscv64, GuestArch::Arm64) => true,

            // Other combinations are not yet supported
            _ => false,
        };

        if self.enable_logging {
            eprintln!(
                "Translation validation: {:?} -> {:?} = {}",
                source, target, supported
            );
        }

        Ok(supported)
    }

    /// Check if specific features are compatible between architectures
    pub fn check_feature_compatibility(
        &self,
        source: GuestArch,
        target: GuestArch,
        feature: &str,
    ) -> VmResult<bool> {
        // This is a simplified implementation
        // In a real system, this would check specific feature compatibility
        let compatible = match (source, target, feature) {
            // SIMD features might not be directly compatible
            (_, _, "simd" | "avx" | "neon" | "sse") => source == target,

            // Basic features are generally compatible
            _ => true,
        };

        Ok(compatible)
    }

    /// Get compatibility report for translation pair
    pub fn get_compatibility_report(
        &self,
        source: GuestArch,
        target: GuestArch,
    ) -> VmResult<String> {
        let supported = self.validate_translation_support(source, target)?;

        if supported {
            Ok(format!(
                "Translation from {:?} to {:?} is supported",
                source, target
            ))
        } else {
            Ok(format!(
                "Translation from {:?} to {:?} is not yet supported",
                source, target
            ))
        }
    }
}

impl Default for ArchitectureCompatibilityDomainService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_architecture() {
        let service = ArchitectureCompatibilityDomainService::new();
        let result = service.validate_translation_support(GuestArch::X86_64, GuestArch::X86_64);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_cross_architecture() {
        let service = ArchitectureCompatibilityDomainService::new();
        let result = service.validate_translation_support(GuestArch::X86_64, GuestArch::Arm64);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_feature_compatibility() {
        let service = ArchitectureCompatibilityDomainService::new();
        let result =
            service.check_feature_compatibility(GuestArch::X86_64, GuestArch::Arm64, "basic");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
