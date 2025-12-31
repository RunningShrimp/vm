//! Device Simulation Property-Based Tests
//!
//! This module contains property-based tests for device simulation, particularly
//! focusing on block devices and state management. These tests verify that devices
//! maintain correct state and handle I/O operations correctly.

use proptest::prelude::*;
use proptest_derive::Arbitrary;
use std::sync::Arc;
use parking_lot::Mutex;
use vm_core::{VmError, MmioDevice};

// ============================================================================
// Mock Device Implementations for Testing
// ============================================================================

/// Simple block device mock for testing
#[derive(Debug)]
struct MockBlockDevice {
    data: Mutex<Vec<u8>>,
    block_size: usize,
    total_blocks: u64,
    read_only: bool,
}

impl MockBlockDevice {
    fn new(block_size: usize, total_blocks: u64) -> Self {
        Self {
            data: Mutex::new(vec![0u8; block_size * total_blocks as usize]),
            block_size,
            total_blocks,
            read_only: false,
        }
    }

    fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    fn read_block(&self, block_idx: u64, buf: &mut [u8]) -> Result<(), VmError> {
        if block_idx >= self.total_blocks {
            return Err(VmError::InvalidArgument);
        }

        let offset = block_idx as usize * self.block_size;
        let data = self.data.lock();

        if buf.len() != self.block_size {
            return Err(VmError::InvalidArgument);
        }

        buf.copy_from_slice(&data[offset..offset + self.block_size]);
        Ok(())
    }

    fn write_block(&self, block_idx: u64, buf: &[u8]) -> Result<(), VmError> {
        if self.read_only {
            return Err(VmError::PermissionDenied);
        }

        if block_idx >= self.total_blocks {
            return Err(VmError::InvalidArgument);
        }

        if buf.len() != self.block_size {
            return Err(VmError::InvalidArgument);
        }

        let offset = block_idx as usize * self.block_size;
        let mut data = self.data.lock();
        data[offset..offset + self.block_size].copy_from_slice(buf);
        Ok(())
    }

    fn get_block_count(&self) -> u64 {
        self.total_blocks
    }
}

/// Device state machine mock
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceState {
    Idle,
    Busy,
    Error,
    Ready,
}

#[derive(Debug)]
struct StatefulDevice {
    state: Mutex<DeviceState>,
    operation_count: Mutex<u64>,
    error_threshold: u64,
}

impl StatefulDevice {
    fn new(error_threshold: u64) -> Self {
        Self {
            state: Mutex::new(DeviceState::Idle),
            operation_count: Mutex::new(0),
            error_threshold,
        }
    }

    fn transition_to(&self, new_state: DeviceState) -> Result<(), VmError> {
        let mut state = self.state.lock();
        let mut count = self.operation_count.lock();

        // Check if we should simulate an error
        *count += 1;
        if *count >= self.error_threshold && self.error_threshold > 0 {
            *state = DeviceState::Error;
            return Err(VmError::DeviceError);
        }

        // Validate state transition
        let current = *state;
        if !is_valid_transition(current, new_state) {
            return Err(VmError::InvalidState);
        }

        *state = new_state;
        Ok(())
    }

    fn get_state(&self) -> DeviceState {
        *self.state.lock()
    }

    fn reset(&self) {
        let mut state = self.state.lock();
        *state = DeviceState::Idle;
        *self.operation_count.lock() = 0;
    }
}

/// Check if a state transition is valid
fn is_valid_transition(from: DeviceState, to: DeviceState) -> bool {
    match (from, to) {
        (DeviceState::Idle, DeviceState::Busy) => true,
        (DeviceState::Idle, DeviceState::Ready) => true,
        (DeviceState::Busy, DeviceState::Idle) => true,
        (DeviceState::Busy, DeviceState::Error) => true,
        (DeviceState::Busy, DeviceState::Ready) => true,
        (DeviceState::Error, DeviceState::Idle) => true,
        (DeviceState::Ready, DeviceState::Idle) => true,
        (DeviceState::Ready, DeviceState::Busy) => true,
        _ => false, // Invalid transitions
    }
}

// ============================================================================
// Property 1: Block Device Read-Write Consistency
// ============================================================================

proptest! {
    /// Property: Writing a block and then reading it should return the same data
    ///
    /// This is a fundamental consistency property for block devices. When we write
    /// data to a specific block and immediately read it back, we should get exactly
    /// what we wrote.
    #[test]
    fn prop_block_read_write_consistency(
        block_idx in 0u64..1000u64,
        data in prop::collection::vec(any::<u8>(), 512),
    ) {
        let device = MockBlockDevice::new(512, 1000);

        // Write block
        let write_result = device.write_block(block_idx, &data);
        prop_assert!(write_result.is_ok(), "Write should succeed");

        // Read back
        let mut read_buffer = vec![0u8; 512];
        let read_result = device.read_block(block_idx, &mut read_buffer);
        prop_assert!(read_result.is_ok(), "Read should succeed");

        // Verify consistency
        prop_assert_eq!(
            read_buffer,
            data,
            "Read data should match written data"
        );
    }
}

// ============================================================================
// Property 2: Block Device Write Independence
// ============================================================================

proptest! {
    /// Property: Writing different blocks should not affect each other
    ///
    /// This tests that writes to different blocks are independent and don't
    /// interfere with each other, which is crucial for data isolation.
    #[test]
    fn prop_block_write_independence(
        block1 in 0u64..1000u64,
        block2 in 0u64..1000u64,
        data1 in prop::collection::vec(any::<u8>(), 512),
        data2 in prop::collection::vec(any::<u8>(), 512),
    ) {
        prop_assume!(block1 != block2);

        let device = MockBlockDevice::new(512, 1000);

        // Write both blocks
        device.write_block(block1, &data1).unwrap();
        device.write_block(block2, &data2).unwrap();

        // Read back both blocks
        let mut buffer1 = vec![0u8; 512];
        let mut buffer2 = vec![0u8; 512];

        device.read_block(block1, &mut buffer1).unwrap();
        device.read_block(block2, &mut buffer2).unwrap();

        // Verify independence
        prop_assert_eq!(buffer1, data1, "Block 1 should contain its data");
        prop_assert_eq!(buffer2, data2, "Block 2 should contain its data");
    }
}

// ============================================================================
// Property 3: Read-Only Device Behavior
// ============================================================================

proptest! {
    /// Property: Read-only devices should reject writes
    ///
    /// This tests that read-only devices properly enforce write protection.
    #[test]
    fn prop_read_only_device(
        block_idx in 0u64..1000u64,
        data in prop::collection::vec(any::<u8>(), 512),
    ) {
        let device = MockBlockDevice::new(512, 1000).read_only();

        // Write should fail
        let write_result = device.write_block(block_idx, &data);
        prop_assert!(
            write_result.is_err(),
            "Write to read-only device should fail"
        );

        // Read should still succeed
        let mut buffer = vec![0u8; 512];
        let read_result = device.read_block(block_idx, &mut buffer);
        prop_assert!(read_result.is_ok(), "Read should succeed");

        // Data should be unchanged (all zeros)
        prop_assert!(
            buffer.iter().all(|&b| b == 0),
            "Read-only device should not be modified"
        );
    }
}

// ============================================================================
// Property 4: Block Boundary Checking
// ============================================================================

proptest! {
    /// Property: Operations should respect block boundaries
    ///
    /// This tests that block devices properly validate block indices and
    /// buffer sizes, rejecting invalid operations.
    #[test]
    fn prop_block_boundary_checking(
        block_idx in 0u64..2000u64,
        data in prop::collection::vec(any::<u8>(), 512),
    ) {
        let device = MockBlockDevice::new(512, 1000);

        // Test with potentially out-of-range block index
        let write_result = device.write_block(block_idx, &data);
        let read_result = {
            let mut buffer = vec![0u8; 512];
            device.read_block(block_idx, &mut buffer)
        };

        if block_idx < 1000 {
            // Within range: should succeed
            prop_assert!(write_result.is_ok(), "Valid write should succeed");
            prop_assert!(read_result.is_ok(), "Valid read should succeed");
        } else {
            // Out of range: should fail
            prop_assert!(write_result.is_err(), "Invalid write should fail");
            prop_assert!(read_result.is_err(), "Invalid read should fail");
        }
    }
}

// ============================================================================
// Property 5: Buffer Size Validation
// ============================================================================

proptest! {
    /// Property: Device should reject incorrectly sized buffers
    ///
    /// This tests that block devices validate buffer sizes match the block size.
    #[test]
    fn prop_buffer_size_validation(
        block_idx in 0u64..1000u64,
        size in 256usize..1024usize,
    ) {
        prop_assume!(size != 512); // Test non-matching sizes

        let device = MockBlockDevice::new(512, 1000);
        let data = vec![0u8; size];

        // Write with wrong size should fail
        let write_result = device.write_block(block_idx, &data);
        prop_assert!(write_result.is_err(), "Wrong-sized write should fail");

        // Read with wrong size should fail
        let mut buffer = vec![0u8; size];
        let read_result = device.read_block(block_idx, &mut buffer);
        prop_assert!(read_result.is_err(), "Wrong-sized read should fail");
    }
}

// ============================================================================
// Property 6: Multiple Sequential Writes
// ============================================================================

proptest! {
    /// Property: Multiple writes to the same block should preserve the last value
    ///
    /// This tests that overwriting a block correctly replaces the old data.
    #[test]
    fn prop_multiple_sequential_writes(
        block_idx in 0u64..1000u64,
        iterations in 2usize..10usize,
    ) {
        let device = MockBlockDevice::new(512, 1000);

        let mut expected_data = vec![0u8; 512];

        // Perform multiple writes
        for i in 0..iterations {
            expected_data = vec![((i % 256) as u8); 512];
            device.write_block(block_idx, &expected_data).unwrap();
        }

        // Final read should get the last written data
        let mut buffer = vec![0u8; 512];
        device.read_block(block_idx, &mut buffer).unwrap();

        prop_assert_eq!(
            buffer,
            expected_data,
            "Last write should be preserved"
        );
    }
}

// ============================================================================
// Property 7: Device State Machine Validity
// ============================================================================

proptest! {
    /// Property: Device state transitions should follow valid state machine rules
    ///
    /// This tests that a device's state machine only allows valid transitions.
    #[test]
    fn prop_state_machine_validity(
        initial_state in any::<DeviceState>(),
        next_state in any::<DeviceState>(),
    ) {
        let device = StatefulDevice::new(0); // No errors

        // Set initial state
        device.state.lock().replace(initial_state);

        // Try to transition
        let result = device.transition_to(next_state);

        // Check if transition is valid according to our rules
        let is_valid = is_valid_transition(initial_state, next_state);

        if is_valid {
            prop_assert!(result.is_ok(), "Valid transition should succeed");
            prop_assert_eq!(device.get_state(), next_state);
        } else {
            prop_assert!(result.is_err(), "Invalid transition should fail");
            prop_assert_eq!(device.get_state(), initial_state);
        }
    }
}

// ============================================================================
// Property 8: State Recovery After Error
// ============================================================================

proptest! {
    /// Property: Device should recover from error state
    ///
    /// This tests that devices can properly recover from error conditions
    /// and return to normal operation.
    #[test]
    fn prop_error_recovery(
        error_threshold in 1u64..10u64,
    ) {
        let device = StatefulDevice::new(error_threshold);

        // Perform operations until error
        for i in 0..=error_threshold {
            let result = device.transition_to(DeviceState::Busy);
            if i >= error_threshold {
                prop_assert!(result.is_err(), "Should error at threshold");
                prop_assert_eq!(device.get_state(), DeviceState::Error);
            }
        }

        // Reset and verify recovery
        device.reset();
        prop_assert_eq!(device.get_state(), DeviceState::Idle);

        // Should be able to transition again
        let result = device.transition_to(DeviceState::Busy);
        prop_assert!(result.is_ok(), "Should work after reset");
    }
}

// ============================================================================
// Property 9: Device Reset Consistency
// ============================================================================

proptest! {
    /// Property: Device reset should return device to known initial state
    ///
    /// This tests that resetting a device properly clears all state and
    /// returns it to a clean initial condition.
    #[test]
    fn prop_reset_consistency(
        error_threshold in 1u64..10u64,
    ) {
        let device = StatefulDevice::new(error_threshold);

        // Perform some operations
        device.transition_to(DeviceState::Busy).ok();
        device.transition_to(DeviceState::Ready).ok();

        // Reset
        device.reset();

        // Verify initial state
        prop_assert_eq!(device.get_state(), DeviceState::Idle);
        prop_assert_eq!(*device.operation_count.lock(), 0);

        // Should be able to perform normal operations
        device.transition_to(DeviceState::Busy).unwrap();
        prop_assert_eq!(device.get_state(), DeviceState::Busy);
    }
}

// ============================================================================
// Property 10: Random Access Pattern Consistency
// ============================================================================

proptest! {
    /// Property: Random access pattern should produce consistent results
    ///
    /// This tests that devices handle random access patterns correctly,
    /// maintaining data integrity across non-sequential operations.
    #[test]
    fn prop_random_access_consistency(
        accesses in prop::collection::vec((0u64..100u64, any::<u8>()), 10..50),
    ) {
        let device = MockBlockDevice::new(512, 100);

        // Write random pattern
        for (block_idx, value) in &accesses {
            let data = vec![*value; 512];
            device.write_block(*block_idx, &data).unwrap();
        }

        // Verify all writes
        for (block_idx, value) in &accesses {
            let mut buffer = vec![0u8; 512];
            device.read_block(*block_idx, &mut buffer).unwrap();

            prop_assert!(
                buffer.iter().all(|&b| b == *value),
                "Block {} should contain value 0x{:02x}",
                block_idx,
                value
            );
        }
    }
}

// ============================================================================
// Property 11: Device Capacity Invariance
// ============================================================================

proptest! {
    /// Property: Device capacity should remain constant
    ///
    /// This tests that a device's reported capacity doesn't change over time
    /// or after operations.
    #[test]
    fn prop_capacity_invariance(
        writes in prop::collection::vec((0u64..100u64(), any::<u8>()), 0..50),
    ) {
        let device = MockBlockDevice::new(512, 100);
        let initial_capacity = device.get_block_count();

        prop_assert_eq!(initial_capacity, 100);

        // Perform various operations
        for (block_idx, value) in &writes {
            let data = vec![*value; 512];
            let _ = device.write_block(*block_idx, &data);
        }

        // Capacity should not change
        let final_capacity = device.get_block_count();
        prop_assert_eq!(
            initial_capacity,
            final_capacity,
            "Device capacity should be invariant"
        );
    }
}

// ============================================================================
// Property 12: State Persistence
// ============================================================================

proptest! {
    /// Property: Device state should persist across operations
    ///
    /// This tests that device state is maintained correctly and doesn't
    /// change unexpectedly.
    #[test]
    fn prop_state_persistence(
        state in any::<DeviceState>(),
    ) {
        let device = StatefulDevice::new(0);

        // Transition to state
        if is_valid_transition(DeviceState::Idle, state) {
            device.transition_to(state).unwrap();

            // Perform various operations that shouldn't change state
            for _ in 0..5 {
                match state {
                    DeviceState::Idle => {
                        // Stay idle
                        device.operation_count.lock().reset();
                    }
                    DeviceState::Busy => {
                        // Stay busy
                        device.operation_count.lock().reset();
                    }
                    DeviceState::Ready => {
                        // Stay ready
                        device.operation_count.lock().reset();
                    }
                    DeviceState::Error => {
                        // Stay in error
                        device.operation_count.lock().reset();
                    }
                }

                prop_assert_eq!(
                    device.get_state(),
                    state,
                    "State should persist across operations"
                );
            }
        }
    }
}
