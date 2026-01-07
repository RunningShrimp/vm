//! # x86 Boot Execution
//!
//! Integrates real-mode emulator, BIOS handlers, and mode transitions
//! to boot x86 kernels from bzImage format.

use vm_core::{GuestAddr, MMU, VmError, VmResult};

use super::realmode::RealModeEmulator;
use super::mode_trans::X86Mode;

/// x86 boot executor
pub struct X86BootExecutor {
    /// Real-mode emulator
    realmode: RealModeEmulator,
    /// Maximum instructions to execute (for safety)
    max_instructions: usize,
    /// Instructions executed so far
    instructions_executed: usize,
    /// Last CS:IP to detect infinite loops
    last_cs_ip: Option<(u16, u32)>,
    /// Count of consecutive same-address executions
    same_address_count: usize,
}

impl X86BootExecutor {
    /// Create new x86 boot executor
    pub fn new() -> Self {
        Self {
            realmode: RealModeEmulator::new(),
            max_instructions: 10_000_000, // Reduced to 10M to test if loop breaks quickly with new instructions
            instructions_executed: 0,
            last_cs_ip: None,
            same_address_count: 0,
        }
    }

    /// Execute x86 boot sequence
    ///
    /// This will:
    /// 1. Start in real mode at the specified entry point
    /// 2. Execute real-mode boot code
    /// 3. Handle mode transitions (real → protected → long)
    /// 4. Return when in long mode or on error
    pub fn boot(
        &mut self,
        mmu: &mut dyn MMU,
        entry_point: u64,
    ) -> VmResult<X86BootResult> {
        log::info!("=== Starting x86 Boot Sequence ===");
        log::info!("Entry point: {:#010X}", entry_point);

        // Set entry point and activate real-mode emulator
        let regs = self.realmode.regs_mut();
        regs.cs = (entry_point >> 4) as u16;
        regs.eip = (entry_point & 0xFFFF) as u32;  // IP is 16-bit in real mode, stored as u32
        drop(regs);

        self.realmode.activate();
        log::info!("Real-mode emulator activated at CS:IP={:#04X}:{:#08X}",
                  self.realmode.regs().cs,
                  self.realmode.regs().eip);

        // Main boot loop
        loop {
            // Check safety limit
            if self.instructions_executed >= self.max_instructions {
                log::warn!("Reached maximum instruction limit ({})", self.max_instructions);
                return Ok(X86BootResult::MaxInstructionsReached);
            }

            // Log first 100 instructions in detail for debugging
            if self.instructions_executed < 100 {
                let regs = self.realmode.regs();
                log::debug!(
                    "Instr #{:03}: CS:IP={:#04X}:{:#08X} AX={:#08X} BX={:#08X} CX={:#08X} DX={:#08X}",
                    self.instructions_executed,
                    regs.cs,
                    regs.eip,
                    regs.eax,
                    regs.ebx,
                    regs.ecx,
                    regs.edx
                );
            }

            // Execute one instruction
            let step = self.realmode.execute(mmu)?;
            self.instructions_executed += 1;

            // Detect infinite loops (same address)
            let current_cs_ip = {
                let regs = self.realmode.regs();
                (regs.cs, regs.eip)
            };

            if let Some(last) = self.last_cs_ip {
                if current_cs_ip == last {
                    self.same_address_count += 1;
                    if self.same_address_count > 100 {
                        log::error!(
                            "Detected infinite loop at CS:IP = {:#04X}:{:#08X} (stuck for {} instructions)",
                            current_cs_ip.0,
                            current_cs_ip.1,
                            self.same_address_count
                        );
                        return Ok(X86BootResult::Error);
                    }
                } else {
                    self.same_address_count = 0;
                }
            }
            self.last_cs_ip = Some(current_cs_ip);

            // Handle the result
            match step {
                super::realmode::RealModeStep::Continue => {
                    // Continue executing
                }
                super::realmode::RealModeStep::Halt => {
                    log::info!("Boot code executed HLT instruction");
                    return Ok(X86BootResult::Halted);
                }
                super::realmode::RealModeStep::SwitchMode => {
                    // Mode switch occurred
                    let current_mode = self.realmode.mode_trans().current_mode();
                    log::info!("Mode switched to: {:?}", current_mode);

                    // If we're in long mode, boot is complete
                    if current_mode == X86Mode::Long {
                        log::info!("=== Long Mode Active ===");
                        log::info!("Boot sequence complete, ready for 64-bit execution");

                        // Get the final execution address
                        let regs = self.realmode.regs();
                        let rip = regs.eip as u64; // In long mode, this is RIP

                        return Ok(X86BootResult::LongModeReady { entry_point: rip });
                    }
                }
                super::realmode::RealModeStep::NotActive => {
                    log::warn!("Real-mode emulator deactivated");
                    return Ok(X86BootResult::NotActive);
                }
                super::realmode::RealModeStep::Error(err) => {
                    log::error!("Boot execution error: {:?}", err);
                    return Ok(X86BootResult::Error);
                }
            }

            // Log progress every 1000 instructions with current CS:IP
            if self.instructions_executed % 1000 == 0 {
                let regs = self.realmode.regs();
                let mode = self.realmode.mode_trans().current_mode();
                log::info!(
                    "Progress: {} instructions | CS:IP = {:#04X}:{:#08X} | Mode: {:?}",
                    self.instructions_executed,
                    regs.cs,
                    regs.eip,
                    mode
                );
            }
        }
    }

    /// Get number of instructions executed
    pub fn instructions_executed(&self) -> usize {
        self.instructions_executed
    }

    /// Get current mode
    pub fn current_mode(&self) -> X86Mode {
        self.realmode.mode_trans().current_mode()
    }
}

impl Default for X86BootExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of x86 boot sequence
#[derive(Debug, Clone)]
pub enum X86BootResult {
    /// Long mode is ready, 64-bit kernel can start
    LongModeReady {
        /// Entry point for 64-bit code
        entry_point: u64,
    },
    /// Boot code executed HLT
    Halted,
    /// Real-mode emulator not active
    NotActive,
    /// Maximum instruction limit reached
    MaxInstructionsReached,
    /// Error occurred during boot
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_executor_create() {
        let executor = X86BootExecutor::new();
        assert_eq!(executor.current_mode(), X86Mode::Real);
        assert_eq!(executor.instructions_executed(), 0);
    }
}
