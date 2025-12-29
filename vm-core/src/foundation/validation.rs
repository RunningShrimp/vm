//! Common validation framework for VM project
//!
//! This module provides reusable validation components to reduce code duplication
//! and improve consistency across the VM project.

use crate::foundation::error::ConfigError;
use crate::foundation::error::{Architecture, GuestAddr, RegId};
use crate::foundation::error::{ErrorContext, VmError, VmResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Validation result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<ValidationWarning>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    pub fn to_result(self) -> VmResult<()> {
        if self.is_valid {
            Ok(())
        } else {
            Err(VmError::Configuration {
                source: ConfigError::ValidationFailed(
                    self.errors
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                message: "Validation failed".to_string(),
            })
        }
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
    pub severity: ErrorSeverity,
}

impl ValidationError {
    pub fn new(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
        severity: ErrorSeverity,
    ) -> Self {
        let field_str = field.into();
        Self {
            field: field_str,
            message: message.into(),
            code: code.into(),
            severity,
        }
    }

    pub fn required(field: impl Into<String> + Clone) -> Self {
        let field_str = field.into();
        Self::new(
            field_str.clone(),
            format!("{} is required", field_str),
            "REQUIRED",
            ErrorSeverity::Error,
        )
    }

    pub fn invalid_format(field: impl Into<String> + Clone, format: impl Into<String>) -> Self {
        let field_str = field.into();
        Self::new(
            field_str.clone(),
            format!("{} has invalid format: {}", field_str, format.into()),
            "INVALID_FORMAT",
            ErrorSeverity::Error,
        )
    }

    pub fn out_of_range(
        field: impl Into<String> + Clone,
        value: impl Into<String>,
        min: impl Into<String>,
        max: impl Into<String>,
    ) -> Self {
        let field_str = field.into();
        Self::new(
            field_str.clone(),
            format!(
                "{} value {} is out of range [{}, {}]",
                field_str,
                value.into(),
                min.into(),
                max.into()
            ),
            "OUT_OF_RANGE",
            ErrorSeverity::Error,
        )
    }

    pub fn format(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Validation warning details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub code: String,
}

impl ValidationWarning {
    pub fn new(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        let field_str = field.into();
        Self {
            field: field_str,
            message: message.into(),
            code: code.into(),
        }
    }

    pub fn deprecated(field: impl Into<String> + Clone, replacement: impl Into<String>) -> Self {
        let field_str = field.into();
        Self::new(
            field_str.clone(),
            format!(
                "{} is deprecated, use {} instead",
                field_str,
                replacement.into()
            ),
            "DEPRECATED",
        )
    }

    pub fn unusual_value(
        field: impl Into<String> + Clone,
        value: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        let field_str = field.into();
        Self::new(
            field_str.clone(),
            format!(
                "{} value {} is unusual, consider {}",
                field_str,
                value.into(),
                suggestion.into()
            ),
            "UNUSUAL_VALUE",
        )
    }
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
}

/// Trait for validation rules
pub trait ValidationRule<T> {
    fn validate(&self, value: &T) -> ValidationResult;
    fn name(&self) -> &str;
}

/// Trait for validators
pub trait Validator<T> {
    fn validate(&self, value: &T) -> VmResult<()>;
    fn validate_with_result(&self, value: &T) -> ValidationResult;
}

/// Common validation rules
pub mod rules {
    use super::*;

    /// Range validation rule
    #[derive(Debug, Clone)]
    pub struct RangeRule<T> {
        name: String,
        min: T,
        max: T,
        inclusive: bool,
    }

    impl<T> RangeRule<T>
    where
        T: PartialOrd + Clone,
    {
        pub fn new(name: impl Into<String>, min: T, max: T) -> Self {
            Self {
                name: name.into(),
                min,
                max,
                inclusive: true,
            }
        }

        pub fn exclusive(mut self) -> Self {
            self.inclusive = false;
            self
        }
    }

    impl<T> ValidationRule<T> for RangeRule<T>
    where
        T: PartialOrd + Clone + ToString,
    {
        fn validate(&self, value: &T) -> ValidationResult {
            let is_valid = if self.inclusive {
                value >= &self.min && value <= &self.max
            } else {
                value > &self.min && value < &self.max
            };

            if is_valid {
                ValidationResult::valid()
            } else {
                ValidationResult::invalid(vec![ValidationError::out_of_range(
                    &self.name,
                    value.to_string(),
                    self.min.to_string(),
                    self.max.to_string(),
                )])
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// Non-empty validation rule
    #[derive(Debug, Clone)]
    pub struct NonEmptyRule {
        name: String,
        allow_whitespace: bool,
    }

    impl NonEmptyRule {
        pub fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                allow_whitespace: false,
            }
        }

        pub fn allow_whitespace(mut self) -> Self {
            self.allow_whitespace = true;
            self
        }
    }

    impl ValidationRule<String> for NonEmptyRule {
        fn validate(&self, value: &String) -> ValidationResult {
            let is_empty = if self.allow_whitespace {
                value.trim().is_empty()
            } else {
                value.is_empty()
            };

            if is_empty {
                ValidationResult::invalid(vec![ValidationError::required(&self.name)])
            } else {
                ValidationResult::valid()
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// Regex validation rule
    #[derive(Debug, Clone)]
    pub struct RegexRule {
        name: String,
        pattern: String,
        message: String,
    }

    impl RegexRule {
        pub fn new(
            name: impl Into<String>,
            pattern: impl Into<String>,
            message: impl Into<String>,
        ) -> Self {
            Self {
                name: name.into(),
                pattern: pattern.into(),
                message: message.into(),
            }
        }
    }

    impl ValidationRule<String> for RegexRule {
        fn validate(&self, value: &String) -> ValidationResult {
            match Regex::new(&self.pattern) {
                Ok(re) => {
                    if re.is_match(value) {
                        ValidationResult::valid()
                    } else {
                        ValidationResult::invalid(vec![ValidationError::new(
                            &self.name,
                            &self.message,
                            "REGEX_MISMATCH",
                            ErrorSeverity::Error,
                        )])
                    }
                }
                Err(_) => ValidationResult::invalid(vec![ValidationError::new(
                    &self.name,
                    "Invalid regex pattern",
                    "INVALID_REGEX",
                    ErrorSeverity::Error,
                )]),
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }
}

/// Common validators
pub mod validators {
    use super::*;

    /// Architecture validator
    #[derive(Debug, Clone)]
    pub struct ArchitectureValidator {
        supported_architectures: Vec<Architecture>,
    }

    impl ArchitectureValidator {
        pub fn new(supported_architectures: Vec<Architecture>) -> Self {
            Self {
                supported_architectures,
            }
        }

        pub fn all_supported() -> Self {
            Self::new(vec![
                Architecture::X86_64,
                Architecture::ARM64,
                Architecture::RISCV64,
            ])
        }
    }

    impl Validator<Architecture> for ArchitectureValidator {
        fn validate(&self, value: &Architecture) -> VmResult<()> {
            self.validate_with_result(value).to_result()
        }

        fn validate_with_result(&self, value: &Architecture) -> ValidationResult {
            if self.supported_architectures.contains(value) {
                ValidationResult::valid()
            } else {
                ValidationResult::invalid(vec![ValidationError::new(
                    "architecture",
                    format!("Unsupported architecture: {:?}", value),
                    "UNSUPPORTED_ARCH",
                    ErrorSeverity::Error,
                )])
            }
        }
    }

    /// Register ID validator
    #[derive(Debug, Clone)]
    pub struct RegisterValidator {
        max_id: RegId,
    }

    impl RegisterValidator {
        pub fn new(max_id: RegId, _architecture: Architecture) -> Self {
            Self { max_id }
        }

        pub fn x86_64() -> Self {
            Self::new(15, Architecture::X86_64)
        }

        pub fn arm64() -> Self {
            Self::new(30, Architecture::ARM64)
        }

        pub fn riscv64() -> Self {
            Self::new(31, Architecture::RISCV64)
        }
    }

    impl Validator<RegId> for RegisterValidator {
        fn validate(&self, value: &RegId) -> VmResult<()> {
            self.validate_with_result(value).to_result()
        }

        fn validate_with_result(&self, value: &RegId) -> ValidationResult {
            if *value <= self.max_id {
                ValidationResult::valid()
            } else {
                ValidationResult::invalid(vec![ValidationError::out_of_range(
                    "register_id",
                    value.to_string(),
                    "0",
                    self.max_id.to_string(),
                )])
            }
        }
    }

    /// Memory address validator
    #[derive(Debug, Clone)]
    pub struct MemoryAddressValidator {
        min_address: GuestAddr,
        max_address: GuestAddr,
        alignment: Option<u64>,
    }

    impl MemoryAddressValidator {
        pub fn new(min_address: GuestAddr, max_address: GuestAddr) -> Self {
            Self {
                min_address,
                max_address,
                alignment: None,
            }
        }

        pub fn with_alignment(mut self, alignment: u64) -> Self {
            self.alignment = Some(alignment);
            self
        }
    }

    impl Validator<GuestAddr> for MemoryAddressValidator {
        fn validate(&self, value: &GuestAddr) -> VmResult<()> {
            self.validate_with_result(value).to_result()
        }

        fn validate_with_result(&self, value: &GuestAddr) -> ValidationResult {
            let mut result = ValidationResult::valid();

            // Check range
            if *value < self.min_address || *value > self.max_address {
                result.add_error(ValidationError::out_of_range(
                    "address",
                    format!("{:#x}", value),
                    format!("{:#x}", self.min_address),
                    format!("{:#x}", self.max_address),
                ));
            }

            // Check alignment
            if let Some(alignment) = self.alignment.filter(|a| !value.is_multiple_of(*a)) {
                result.add_error(ValidationError::new(
                    "address",
                    format!("Address {:#x} not aligned to {} bytes", value, alignment),
                    "ALIGNMENT_VIOLATION",
                    ErrorSeverity::Error,
                ));
            }

            result
        }
    }

    /// String length validator
    #[derive(Debug, Clone)]
    pub struct StringLengthValidator {
        name: String,
        min_length: Option<usize>,
        max_length: Option<usize>,
    }

    impl StringLengthValidator {
        pub fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                min_length: None,
                max_length: None,
            }
        }

        pub fn with_min_length(mut self, min_length: usize) -> Self {
            self.min_length = Some(min_length);
            self
        }

        pub fn with_max_length(mut self, max_length: usize) -> Self {
            self.max_length = Some(max_length);
            self
        }
    }

    impl Validator<String> for StringLengthValidator {
        fn validate(&self, value: &String) -> VmResult<()> {
            self.validate_with_result(value).to_result()
        }

        fn validate_with_result(&self, value: &String) -> ValidationResult {
            let mut result = ValidationResult::valid();

            // Check minimum length
            if let Some(min_length) = self.min_length.filter(|m| value.len() < *m) {
                result.add_error(ValidationError::new(
                    &self.name,
                    format!(
                        "{} is too short (minimum {} characters)",
                        &self.name, min_length
                    ),
                    "TOO_SHORT",
                    ErrorSeverity::Error,
                ));
            }

            // Check maximum length
            if let Some(max_length) = self.max_length.filter(|m| value.len() > *m) {
                result.add_error(ValidationError::new(
                    &self.name,
                    format!(
                        "{} is too long (maximum {} characters)",
                        &self.name, max_length
                    ),
                    "TOO_LONG",
                    ErrorSeverity::Error,
                ));
            }

            result
        }
    }
}

/// Composite validator that combines multiple validators
pub struct CompositeValidator {
    validators: Vec<Box<dyn Validator<String>>>,
}

impl CompositeValidator {
    pub fn new(_name: impl Into<String>) -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    pub fn add_validator(mut self, validator: Box<dyn Validator<String>>) -> Self {
        self.validators.push(validator);
        self
    }
}

impl Validator<String> for CompositeValidator {
    fn validate(&self, value: &String) -> VmResult<()> {
        self.validate_with_result(value).to_result()
    }

    fn validate_with_result(&self, value: &String) -> ValidationResult {
        let mut result = ValidationResult::valid();

        for validator in &self.validators {
            let validator_result = validator.validate_with_result(value);
            if !validator_result.is_valid {
                result.errors.extend(validator_result.errors);
                result.warnings.extend(validator_result.warnings);
            }
        }

        if !result.is_valid {
            result.is_valid = false;
        }

        result
    }
}

/// Utility functions for validation
pub mod utils {
    use super::*;

    /// Validate a value with multiple rules
    pub fn validate_with_rules<T>(
        value: &T,
        rules: &[Box<dyn ValidationRule<T>>],
    ) -> ValidationResult {
        let mut result = ValidationResult::valid();

        for rule in rules {
            let rule_result = rule.validate(value);
            if !rule_result.is_valid {
                result.errors.extend(rule_result.errors);
                result.warnings.extend(rule_result.warnings);
            }
        }

        if !result.is_valid {
            result.is_valid = false;
        }

        result
    }

    /// Create a validation error with context
    pub fn validation_error(
        field: impl Into<String>,
        message: impl Into<String>,
        context: &ErrorContext,
    ) -> VmError {
        let field_name = field.into();
        VmError::Configuration {
            source: ConfigError::ValidationFailed(format!("{}: {}", field_name, message.into())),
            message: format!("Validation failed in {}", context.operation),
        }
    }

    /// Check if a value is in a set of allowed values
    pub fn is_one_of<T: PartialEq + Clone + std::fmt::Debug>(
        value: &T,
        allowed: &[T],
        field_name: &str,
    ) -> ValidationResult {
        if allowed.contains(value) {
            ValidationResult::valid()
        } else {
            ValidationResult::invalid(vec![ValidationError::new(
                field_name,
                format!("Value {:?} is not in allowed set: {:?}", value, allowed),
                "INVALID_VALUE",
                ErrorSeverity::Error,
            )])
        }
    }

    /// Validate a collection of values
    pub fn validate_collection<T, F>(
        values: &[T],
        validator: F,
        field_name: &str,
    ) -> ValidationResult
    where
        F: Fn(&T) -> ValidationResult,
    {
        let mut result = ValidationResult::valid();

        for (index, value) in values.iter().enumerate() {
            let value_result = validator(value);
            if !value_result.is_valid {
                for error in value_result.errors {
                    result.add_error(ValidationError::new(
                        format!("{}[{}]", field_name, index),
                        error.message,
                        error.code,
                        error.severity,
                    ));
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::valid();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());

        result.add_error(ValidationError::required("test".to_string()));
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_range_rule() {
        let rule = rules::RangeRule::new("test", 1, 10);

        let valid_result = rule.validate(&5);
        assert!(valid_result.is_valid);

        let invalid_result = rule.validate(&0);
        assert!(!invalid_result.is_valid);
        assert_eq!(invalid_result.errors.len(), 1);
    }

    #[test]
    fn test_non_empty_rule() {
        let rule = rules::NonEmptyRule::new("test");

        let valid_result = rule.validate(&"not empty".to_string());
        assert!(valid_result.is_valid);

        let invalid_result = rule.validate(&"".to_string());
        assert!(!invalid_result.is_valid);
        assert_eq!(invalid_result.errors.len(), 1);
    }

    #[test]
    fn test_architecture_validator() {
        let validator = validators::ArchitectureValidator::all_supported();

        let valid_result = validator.validate_with_result(&Architecture::X86_64);
        assert!(valid_result.is_valid);

        // 创建一个自定义验证器来测试不支持的架构
        // 这里我们只支持X86_64和ARM64，但不支持RISCV64
        let custom_validator =
            validators::ArchitectureValidator::new(vec![Architecture::X86_64, Architecture::ARM64]);
        let invalid_result = custom_validator.validate_with_result(&Architecture::RISCV64);
        assert!(!invalid_result.is_valid);
        assert_eq!(invalid_result.errors.len(), 1);
    }

    #[test]
    fn test_composite_validator() {
        let validator = CompositeValidator::new("test")
            .add_validator(Box::new(
                validators::StringLengthValidator::new("field1").with_min_length(1),
            ))
            .add_validator(Box::new(
                validators::StringLengthValidator::new("field2").with_max_length(10),
            ));

        let valid_result = validator.validate_with_result(&"valid_string".to_string());
        assert!(valid_result.is_valid);
    }
}
