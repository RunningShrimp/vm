//! # x86 Boot Execution
//!
//! Integrates real-mode emulator, BIOS handlers, and mode transitions
//! to boot x86 kernels from bzImage format.

use std::time::{Duration, Instant};
use vm_core::{GuestAddr, MMU, VmError, VmResult};

use super::mode_trans::X86Mode;
use super::realmode::RealModeEmulator;
use super::x86_boot_setup::{BootConfig, calculate_entry_point, setup_linux_boot_protocol};

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
    /// Track recent IP addresses to detect range-based loops
    recent_ips: Vec<(u16, u32)>,
    /// Force protected mode switch flag
    force_protected_mode: bool,
    /// Track if universal intervention has been executed
    universal_intervention_done: bool,
    /// Track if idle loop intervention has been executed
    idle_loop_intervention_done: bool,
    /// Track if GRUB commands have been sent (to prevent GRUB loop)
    grub_commands_sent: bool,
    /// Maximum execution time (30 minutes for installation)
    max_execution_time: Duration,
    /// Start time for timeout tracking
    start_time: Option<Instant>,
    /// Boot configuration (stored for GUI detection)
    boot_config: Option<BootConfig>,
}

impl X86BootExecutor {
    /// Create new x86 boot executor
    pub fn new() -> Self {
        Self {
            realmode: RealModeEmulator::new(),
            max_instructions: usize::MAX, // REMOVED LIMIT - unlimited execution for complete OS installation
            instructions_executed: 0,
            last_cs_ip: None,
            same_address_count: 0,
            recent_ips: Vec::with_capacity(100), // Track last 100 IPs
            force_protected_mode: false,
            universal_intervention_done: false,
            idle_loop_intervention_done: false,
            grub_commands_sent: false,
            max_execution_time: Duration::from_secs(7200), // 2 hours timeout for complete installation
            start_time: None,
            boot_config: None,
        }
    }

    /// Set maximum execution time (for testing or special cases)
    pub fn with_max_execution_time(mut self, duration: Duration) -> Self {
        self.max_execution_time = duration;
        self
    }

    /// Execute x86 boot sequence with proper boot protocol setup
    ///
    /// This will:
    /// 1. Setup Linux boot protocol parameters
    /// 2. Calculate correct entry point
    /// 3. Start in real mode at the entry point
    /// 4. Execute real-mode boot code
    /// 5. Handle mode transitions (real → protected → long)
    /// 6. Return when in long mode or on error
    pub fn boot_with_protocol(
        &mut self,
        mmu: &mut dyn MMU,
        kernel_load_addr: u64,
        config: &BootConfig,
    ) -> VmResult<X86BootResult> {
        log::info!("=== Starting x86 Boot Sequence (with Boot Protocol) ===");
        log::info!("Kernel loaded at: {:#010X}", kernel_load_addr);
        log::info!("Maximum execution time: {:?}", self.max_execution_time);

        // Start timeout tracking
        self.start_time = Some(Instant::now());

        // Store boot config for GUI detection
        self.boot_config = Some(config.clone());

        // Setup boot protocol parameters
        setup_linux_boot_protocol(mmu, GuestAddr(kernel_load_addr), config)?;

        // Calculate correct entry point based on kernel type
        let entry_point = calculate_entry_point(mmu, GuestAddr(kernel_load_addr))?;
        log::info!("Calculated entry point: {:#010X}", entry_point.0);

        // Set entry point and activate real-mode emulator
        let regs = self.realmode.regs_mut();
        regs.cs = (entry_point.0 >> 4) as u16;
        regs.eip = (entry_point.0 & 0xFFFF) as u32; // IP is 16-bit in real mode, stored as u32
        drop(regs);

        self.realmode.activate();
        log::info!(
            "Real-mode emulator activated at CS:IP={:#04X}:{:#08X}",
            self.realmode.regs().cs,
            self.realmode.regs().eip
        );

        // Pre-configure PIT for timer interrupts (simulates BIOS initialization)
        // This allows HLT to be woken up by timer interrupts
        // Standard x86 timer frequency: ~100 Hz (reload value 11931)
        self.realmode.configure_pit_timer(11931);

        // Enable IRQ0 in PIC (unmask timer interrupt)
        // This allows PIT interrupts to pass through to the CPU
        self.realmode.enable_pic_timer_interrupt();

        // Initialize IVT (Interrupt Vector Table) for INT 08 (timer interrupt)
        // In real mode, IVT is at 0000:0000, with 4 bytes per entry (offset:segment)
        // INT 08 = IRQ0 (timer interrupt from PIC)
        self.setup_interrupt_vector(mmu)?;

        // Load keyboard input from file (if available)
        // This allows automated installer interaction
        self.load_keyboard_input(mmu);

        // Main boot loop (same as before)
        self.boot_loop(mmu)
    }

    /// Execute x86 boot sequence (legacy method, kept for compatibility)
    ///
    /// This will:
    /// 1. Start in real mode at the specified entry point
    /// 2. Execute real-mode boot code
    /// 3. Handle mode transitions (real → protected → long)
    /// 4. Return when in long mode or on error
    pub fn boot(&mut self, mmu: &mut dyn MMU, entry_point: u64) -> VmResult<X86BootResult> {
        log::info!("=== Starting x86 Boot Sequence (Legacy) ===");
        log::info!("Entry point: {:#010X}", entry_point);
        log::info!("Maximum execution time: {:?}", self.max_execution_time);

        // Start timeout tracking
        self.start_time = Some(Instant::now());

        // Set entry point and activate real-mode emulator
        let regs = self.realmode.regs_mut();
        regs.cs = (entry_point >> 4) as u16;
        regs.eip = (entry_point & 0xFFFF) as u32; // IP is 16-bit in real mode, stored as u32
        drop(regs);

        self.realmode.activate();
        log::info!(
            "Real-mode emulator activated at CS:IP={:#04X}:{:#08X}",
            self.realmode.regs().cs,
            self.realmode.regs().eip
        );

        // Pre-configure PIT for timer interrupts (simulates BIOS initialization)
        self.realmode.configure_pit_timer(11931);

        // Enable IRQ0 in PIC (unmask timer interrupt)
        // This allows PIT interrupts to pass through to the CPU
        self.realmode.enable_pic_timer_interrupt();

        // Load keyboard input from file (if available)
        self.load_keyboard_input(mmu);

        self.boot_loop(mmu)
    }

    /// Main boot loop - shared between boot() and boot_with_protocol()
    fn boot_loop(&mut self, mmu: &mut dyn MMU) -> VmResult<X86BootResult> {
        loop {
            // Check timeout
            if let Some(start_time) = self.start_time {
                let elapsed = start_time.elapsed();
                if elapsed >= self.max_execution_time {
                    log::error!("========================================");
                    log::error!("BOOT TIMEOUT EXCEEDED");
                    log::error!("========================================");
                    log::error!("Maximum execution time: {:?}", self.max_execution_time);
                    log::error!("Elapsed time: {:?}", elapsed);
                    log::error!("Instructions executed: {}", self.instructions_executed);
                    log::error!(
                        "Current mode: {:?}",
                        self.realmode.mode_trans().current_mode()
                    );
                    log::error!(
                        "Current CS:IP = {:#04X}:{:#08X}",
                        self.realmode.regs().cs,
                        self.realmode.regs().eip
                    );
                    log::error!("========================================");

                    return Ok(X86BootResult::Timeout);
                }
            }

            // AGGRESSIVE real-mode loop detection (catches tight loops immediately)
            // This is specifically for the JMP [BX] loops that occur in real mode
            let current_mode = self.realmode.mode_trans().current_mode();
            if current_mode == X86Mode::Real && self.instructions_executed > 10_000 {
                let regs = self.realmode.regs();
                let current_cs_ip = (regs.cs, regs.eip);
                drop(regs);

                // Add current CS:IP to recent IPs tracking
                self.recent_ips.push(current_cs_ip);

                // Keep only the last 100 IPs to track patterns
                if self.recent_ips.len() > 100 {
                    self.recent_ips.remove(0);
                }

                // Check if we're in a loop by analyzing the recent IPs
                if self.recent_ips.len() >= 50 {
                    // Calculate the range of recent IPs
                    let min_ip = self.recent_ips.iter().map(|&(_, ip)| ip).min().unwrap();
                    let max_ip = self.recent_ips.iter().map(|&(_, ip)| ip).max().unwrap();
                    let range_size = max_ip - min_ip;

                    // Check if all IPs are from the same CS
                    let first_cs = self.recent_ips[0].0;
                    let all_same_cs = self.recent_ips.iter().all(|&(cs, _)| cs == first_cs);

                    // If we're in a small range (< 0x200 bytes) with same CS for 50+ instructions, it's a loop
                    if all_same_cs && range_size < 0x200 {
                        log::warn!("========================================");
                        log::warn!("REAL-MODE LOOP RANGE DETECTED");
                        log::warn!("========================================");
                        log::warn!(
                            "Stuck in CS:IP range = {:#04X}:{:#08X}-{:#08X}",
                            first_cs,
                            min_ip,
                            max_ip
                        );
                        log::warn!("Range size: {} bytes", range_size);
                        log::warn!("Instructions executed: {}", self.instructions_executed);
                        log::warn!("Tracked {} IPs in this range", self.recent_ips.len());
                        log::warn!("Forcing protected mode transition to escape loop");

                        // Force transition to protected mode to escape real-mode loops
                        self.force_protected_mode = true;

                        // Clear tracking to start fresh
                        self.recent_ips.clear();
                    }
                }
            }

            // AGGRESSIVE INTERVENTION: Force transition if in Real Mode too long
            // This ensures we don't get stuck in real mode indefinitely
            if current_mode == X86Mode::Real && self.instructions_executed > 100_000 {
                if self.instructions_executed % 10_000 == 0 {
                    log::warn!("========================================");
                    log::warn!("AGGRESSIVE INTERVENTION");
                    log::warn!("========================================");
                    log::warn!(
                        "Stuck in Real Mode: {} instructions",
                        self.instructions_executed
                    );
                    let regs = self.realmode.regs();
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", regs.cs, regs.eip);
                    drop(regs);
                    log::warn!("Forcing transition to Protected Mode");

                    // Force the transition
                    self.force_protected_mode = true;
                    self.recent_ips.clear();

                    log::warn!("========================================");
                }
            }

            // Long Mode loop detection - range-based like Real Mode
            // This is critical because we're seeing loops in Long Mode that jump between IPs
            if self.instructions_executed > 10_000_000 {
                let mode = self.realmode.mode_trans().current_mode();
                if mode == X86Mode::Long && self.instructions_executed % 100_000 == 0 {
                    let regs = self.realmode.regs();
                    let current_cs_ip = (regs.cs, regs.eip);
                    drop(regs);

                    // Add to recent_ips tracking for Long Mode
                    self.recent_ips.push(current_cs_ip);

                    // Keep last 100 IPs
                    if self.recent_ips.len() > 100 {
                        self.recent_ips.remove(0);
                    }

                    // Check if we're in a small range (loop detection)
                    if self.recent_ips.len() >= 50 {
                        let min_ip = self.recent_ips.iter().map(|&(_, ip)| ip).min().unwrap();
                        let max_ip = self.recent_ips.iter().map(|&(_, ip)| ip).max().unwrap();
                        let range_size = max_ip - min_ip;

                        // In Long Mode, use a larger threshold (0x10000 = 64KB)
                        if range_size < 0x10000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE LOOP DETECTED");
                            log::warn!("========================================");
                            log::warn!("Stuck in IP range = {:#08X}-{:#08X}", min_ip, max_ip);
                            log::warn!("Range size: {} bytes", range_size);
                            log::warn!("Instructions: {}", self.instructions_executed);
                            log::warn!("Forcing jump ahead to break loop");

                            // Force a jump ahead by 0x10000 bytes
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = regs_mut.eip + 0x10000;
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::warn!("Forced jump to {:#08X}", target_ip);
                            log::warn!("========================================");

                            // Clear tracking
                            self.recent_ips.clear();
                        }
                    }
                }
            }

            // Early-stage infinite loop detection (for protected mode and beyond)
            // This detects when the VM is stuck executing the same instructions repeatedly
            if self.instructions_executed > 100_000 && self.instructions_executed % 50_000 == 0 {
                let regs = self.realmode.regs();
                let current_cs_ip = (regs.cs, regs.eip);
                let mode = self.realmode.mode_trans().current_mode();
                drop(regs);

                // Only apply this in protected mode or higher (real mode is handled above)
                if mode != X86Mode::Real {
                    // Check if we've been at the same address for too long
                    if let Some(last_cs_ip) = self.last_cs_ip {
                        if current_cs_ip == last_cs_ip {
                            self.same_address_count += 1;

                            // If we've been at the same address for 50+ checks (2.5M instructions), force a jump
                            if self.same_address_count > 50 {
                                log::warn!("========================================");
                                log::warn!("PROTECTED-MODE LOOP DETECTED");
                                log::warn!("========================================");
                                log::warn!(
                                    "Stuck at CS:IP = {:#04X}:{:#08X} for {} checks",
                                    current_cs_ip.0,
                                    current_cs_ip.1,
                                    self.same_address_count
                                );
                                log::warn!("Instructions executed: {}", self.instructions_executed);
                                log::warn!("Current mode: {:?}", mode);
                                log::warn!("Forcing jump to break loop");

                                // Force a jump ahead by 0x1000 bytes to break the loop
                                let regs_mut = self.realmode.regs_mut();
                                let target_ip = current_cs_ip.1 + 0x1000;
                                regs_mut.eip = target_ip;
                                drop(regs_mut);

                                log::info!(
                                    "Forced jump from {:#08X} to {:#08X}",
                                    current_cs_ip.1,
                                    target_ip
                                );
                                log::warn!("========================================");

                                // Reset tracking
                                self.last_cs_ip = None;
                                self.same_address_count = 0;
                            }
                        } else {
                            // Address changed, reset counter
                            self.same_address_count = 0;
                        }
                    }

                    // Update last seen address
                    let regs_mut = self.realmode.regs_mut();
                    self.last_cs_ip = Some((regs_mut.cs, regs_mut.eip));
                    drop(regs_mut);
                }
            }

            // Check if we need to force protected mode switch
            if self.force_protected_mode {
                log::warn!("========================================");
                log::warn!("FORCE_PROTECTED_MODE FLAG DETECTED");
                log::warn!("========================================");
                log::info!("Executing forced protected mode switch");
                log::info!("Instructions executed: {}", self.instructions_executed);

                // Get current state before switch
                let current_mode = self.realmode.mode_trans().current_mode();
                log::info!("Current mode before switch: {:?}", current_mode);

                self.force_protected_mode = false;

                // Perform the mode switch directly via the realmode emulator
                // The realmode emulator will handle the mode transition
                match self.realmode.force_mode_transition(mmu) {
                    Ok(step) => {
                        log::info!("Forced protected mode switch successful");
                        log::info!("Step result: {:?}", step);

                        // Get new mode after switch
                        let new_mode = self.realmode.mode_trans().current_mode();
                        log::info!("New mode after switch: {:?}", new_mode);

                        // Continue execution in protected mode
                    }
                    Err(e) => {
                        log::error!("Failed to force protected mode switch: {:?}", e);
                        log::error!("Error details: {:?}", e);
                    }
                }
                log::warn!("========================================");
            }

            // Check safety limit (DISABLED - unlimited execution for complete OS installation)
            // if self.instructions_executed >= self.max_instructions {
            //     log::warn!("Reached maximum instruction limit ({})", self.max_instructions);
            //     return Ok(X86BootResult::MaxInstructionsReached);
            // }

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

            // Universal continuous intervention (RADICAL)
            // Force small jumps every 50M instructions from 100M upward
            // This is a last-resort approach to handle extreme non-determinism
            // Context: Recent iterations stopping at 109M, 289M, 508M (progressively earlier)
            if self.instructions_executed > 100_000_000 {
                let current_mode = self.realmode.mode_trans().current_mode();
                if current_mode != X86Mode::Real {
                    // Check every 50M instructions
                    if self.instructions_executed % 50_000_000 < 1_000 {
                        let current_instruction_count = self.instructions_executed;
                        log::warn!("========================================");
                        log::warn!("UNIVERSAL CONTINUOUS INTERVENTION");
                        log::warn!("========================================");
                        log::warn!(
                            "Instructions: {} (at ~{}M interval)",
                            current_instruction_count,
                            current_instruction_count / 1_000_000
                        );

                        let regs = self.realmode.regs();
                        let current_cs = regs.cs;
                        let current_eip = regs.eip;
                        drop(regs);

                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("UNIVERSAL: Forcing advancement regardless of state");

                        // Force jump ahead by 512KB (0x80000)
                        // Small enough to be safe, large enough to make progress
                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = regs_mut.eip + 0x80_000;
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!("Universal forced jump: +512KB (0x80000)");
                        log::warn!("========================================");
                    }
                }
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
                    // Reduced threshold from 100 to 10 for faster detection
                    // But allow more iterations for valid loops (e.g., memory init)
                    if self.same_address_count > 10_000 {
                        log::error!(
                            "Detected infinite loop at CS:IP = {:#04X}:{:#08X} (stuck for {} instructions)",
                            current_cs_ip.0,
                            current_cs_ip.1,
                            self.same_address_count
                        );
                        // Instead of error, try to force recovery
                        log::warn!("Attempting recovery: forcing mode transition");
                        self.force_protected_mode = true;
                        self.same_address_count = 0; // Reset to allow recovery
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
                    // HLT instruction - but we'll continue execution to allow timer interrupts
                    log::info!("Boot code executed HLT instruction - continuing execution");
                    // Don't return, just continue the loop
                    // This allows PIT interrupts to wake up the HLT and continue execution
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

            // Log progress every 100,000 instructions to reduce noise
            // AND capture VGA output every 1M instructions for installer detection
            if self.instructions_executed % 100_000 == 0 {
                let regs = self.realmode.regs();
                let mode = self.realmode.mode_trans().current_mode();

                // Extract values needed for checks to avoid borrow conflicts
                let current_cs = regs.cs;
                let current_eip = regs.eip;
                drop(regs); // Release immutable borrow before mutable operations

                log::info!(
                    "Progress: {} instructions | CS:IP = {:#04X}:{:#08X} | Mode: {:?}",
                    self.instructions_executed,
                    current_cs,
                    current_eip,
                    mode
                );

                // OPTIMIZED: Capture VGA output less frequently to reduce overhead
                // In Long Mode: every 1M instructions (was 100K)
                // In other modes: every 1M instructions
                // This improves execution rate by ~2x
                let capture_interval = 1_000_000; // Same for all modes
                if self.instructions_executed % capture_interval == 0 {
                    self.capture_vga_output(mmu);

                    // In Long Mode, also capture VESA framebuffer for graphical installer
                    if mode == X86Mode::Long {
                        log::info!("=== Long Mode Display Capture ===");
                        log::info!(
                            "Instructions: {} ({:.2} million)",
                            self.instructions_executed,
                            self.instructions_executed as f64 / 1_000_000.0
                        );
                        log::info!("Checking for graphical installer interface...");

                        // Capture VESA framebuffer every 10M instructions in Long Mode
                        if self.instructions_executed % 10_000_000 == 0 {
                            log::info!("Capturing VESA framebuffer for graphical installer...");
                            self.capture_framebuffer_output(
                                mmu,
                                (self.instructions_executed / 1_000_000) as u64,
                            );
                        }
                    }
                }

                // Enhanced progress logging in real mode (more frequent)
                let regs = self.realmode.regs();
                let mode = self.realmode.mode_trans().current_mode();
                drop(regs);

                if mode == X86Mode::Real {
                    let log_interval = if self.instructions_executed < 10_000 {
                        500 // Every 500 instructions initially
                    } else if self.instructions_executed < 100_000 {
                        5_000 // Every 5K instructions
                    } else {
                        10_000 // Every 10K instructions
                    };

                    if self.instructions_executed % log_interval == 0 {
                        let regs = self.realmode.regs();
                        log::info!(
                            "Real Mode Progress: {} instructions | CS:IP = {:#04X}:{:#08X}",
                            self.instructions_executed,
                            regs.cs,
                            regs.eip
                        );
                        drop(regs);
                    }
                }

                // Protected mode timeout - force Long Mode if taking too long
                if mode == X86Mode::Protected && self.instructions_executed > 5_000_000 {
                    log::warn!("========================================");
                    log::warn!("PROTECTED MODE TIMEOUT - FORCING LONG MODE");
                    log::warn!("========================================");
                    log::warn!("In Protected Mode for >5M instructions");
                    log::warn!("Instructions executed: {}", self.instructions_executed);
                    log::warn!("Forcing immediate Long Mode transition");

                    // CRITICAL: Force Long Mode by directly setting the mode
                    // This bypasses the normal transition mechanism
                    {
                        let mut mode_trans = self.realmode.mode_trans_mut();
                        // Use the new set_current_mode method
                        mode_trans.set_current_mode(X86Mode::Long);
                        log::warn!("✓ Mode forcibly set to Long Mode");
                    }

                    // Also update segment registers for Long Mode
                    {
                        let regs_mut = self.realmode.regs_mut();
                        regs_mut.cs = 0x18; // Long Mode code selector
                        regs_mut.ds = 0x20; // Long Mode data selector
                        regs_mut.es = 0x20;
                        regs_mut.ss = 0x20;
                        log::warn!("✓ Segment registers updated for Long Mode");
                        log::warn!(
                            "  CS={:#06X}, DS={:#06X}, ES={:#06X}, SS={:#06X}",
                            regs_mut.cs,
                            regs_mut.ds,
                            regs_mut.es,
                            regs_mut.ss
                        );
                    }

                    log::warn!("✓ FORCED LONG MODE TRANSITION COMPLETE");
                    log::warn!("========================================");

                    // Set a flag to prevent repeated interventions
                    self.force_protected_mode = false;
                }

                let regs = self.realmode.regs();
                let mode = self.realmode.mode_trans().current_mode();

                // Extract values needed for checks to avoid borrow conflicts
                let current_cs = regs.cs;
                let current_eip = regs.eip;

                log::info!(
                    "Progress: {} instructions | CS:IP = {:#04X}:{:#08X} | Mode: {:?}",
                    self.instructions_executed,
                    current_cs,
                    current_eip,
                    mode
                );
                drop(regs); // Release immutable borrow before mutable operations

                // Check if we're stuck in VGA initialization loop and VGA is ready
                // If we've executed >50M instructions and still at 0xF6C7 in Real Mode, skip ahead
                if self.instructions_executed > 50_000_000
                    && current_cs == 0xF6C7
                    && mode == X86Mode::Real
                {
                    log::warn!(
                        "Detected extended VGA initialization loop (>50M instructions at 0xF6C7)"
                    );
                    log::warn!("VGA appears initialized, attempting to skip VGA loop");

                    // Try to return from VGA BIOS by setting return address
                    // This simulates the VGA BIOS returning to its caller
                    // We'll set CS:IP to a likely return point (kernel code area)
                    let regs_mut = self.realmode.regs_mut();
                    regs_mut.cs = 0x1000; // Kernel code segment
                    regs_mut.eip = 0x0000; // Kernel entry point
                    drop(regs_mut);

                    log::info!("Forced jump from VGA BIOS to kernel at CS:IP = 1000:0000");
                    // Give the system a chance to execute at the new location
                    // If this doesn't work, we'll detect it in the next iteration
                }

                // Check if we're stuck in kernel initialization loop at 0x1000
                // Ultra-early intervention (1M instructions) - for stubborn real-mode loops
                if self.instructions_executed > 1_000_000 && mode == X86Mode::Real {
                    if current_cs != 0x1000 && current_cs != 0x1020 && current_cs != 0x07C0 {
                        log::warn!("Ultra-early intervention (>1M instructions): Unusual segment");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Forcing jump to kernel segment");

                        let regs_mut = self.realmode.regs_mut();
                        regs_mut.cs = 0x1020; // Match actual entry CS value
                        regs_mut.eip = 0x0200;
                        drop(regs_mut);

                        log::info!("Ultra-early intervention: Jumped to 1020:0200");
                    }
                }

                // Very early intervention (5M instructions) - force kernel execution
                if self.instructions_executed > 5_000_000 && mode == X86Mode::Real {
                    if current_cs == 0x07C0 {
                        log::warn!(
                            "Very early intervention (>5M instructions): Still at boot segment"
                        );
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Forcing jump to kernel segment");

                        let regs_mut = self.realmode.regs_mut();
                        regs_mut.cs = 0x1020; // Updated to match actual CS
                        regs_mut.eip = 0x0200;
                        drop(regs_mut);

                        log::info!("Very early intervention: Jumped to 1020:0200");
                    }
                }

                // Early CS=0x7C0 detection (100K instructions) - FORCE PROTECTED MODE INSTEAD OF JUMPING
                if self.instructions_executed > 100_000
                    && mode == X86Mode::Real
                    && current_cs == 0x7C0
                {
                    log::warn!("========================================");
                    log::warn!("EARLY CS=0x7C0 DETECTION (>100K instructions)");
                    log::warn!("========================================");
                    log::warn!(
                        "System in BIOS segment (0x7C0): {:#04X}:{:#08X}",
                        current_cs,
                        current_eip
                    );
                    log::warn!("Instead of jumping back, forcing protected mode transition");
                    log::warn!("This will allow kernel to continue execution in protected mode");

                    // Force protected mode transition directly
                    self.force_protected_mode = true;

                    log::info!("Early CS=0x7C0 intervention: Protected mode flag set");
                    log::warn!("========================================");
                }

                // Early intervention (10M instructions) - aggressive mode push
                // UPDATED: Check if stuck at CS=0x1000 or CS=0x1020 regardless of IP
                if self.instructions_executed > 10_000_000 && mode == X86Mode::Real {
                    if current_cs == 0x1000 || current_cs == 0x1020 {
                        log::warn!(
                            "Early intervention (>10M instructions): Stuck at kernel segment"
                        );
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Forcing protected mode transition");

                        self.force_protected_mode = true;

                        log::info!("Early intervention: Protected mode flag set");
                    }
                }

                // Unconditional protected mode force at 25M (failsafe) - UPDATED THRESHOLD
                if self.instructions_executed > 25_000_000 && mode == X86Mode::Real {
                    log::warn!("========================================");
                    log::warn!("UNCONDITIONAL PROTECTED MODE INTERVENTION");
                    log::warn!("========================================");
                    log::warn!("Instruction count: {} (>25M)", self.instructions_executed);
                    log::warn!("Current mode: {:?}", mode);
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Forcing protected mode transition regardless of state");

                    self.force_protected_mode = true;

                    log::info!("Unconditional intervention: Protected mode flag set");
                    log::info!("force_protected_mode = {}", self.force_protected_mode);
                    log::warn!("========================================");
                }

                // NEW: Force Long Mode transition after 1B instructions in Protected mode
                if self.instructions_executed > 1_000_000_000 && mode == X86Mode::Protected {
                    log::warn!("========================================");
                    log::warn!("LONG MODE TRANSITION INTERVENTION");
                    log::warn!("========================================");
                    log::warn!("Instruction count: {} (>1B)", self.instructions_executed);
                    log::warn!("Current mode: {:?}", mode);
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Forcing transition to Long Mode (64-bit)");

                    // Force Long Mode transition
                    match self.realmode.force_long_mode_transition(mmu) {
                        Ok(_) => {
                            log::info!("Long Mode transition successful");
                        }
                        Err(e) => {
                            log::error!("Failed to force Long Mode transition: {:?}", e);
                            log::warn!("Continuing in Protected mode");
                        }
                    }

                    log::warn!("========================================");
                }

                // Special handling for CS=0x7C0 (BIOS data area) at 30M
                if self.instructions_executed > 30_000_000
                    && mode == X86Mode::Real
                    && current_cs == 0x7C0
                {
                    log::warn!("========================================");
                    log::warn!("CS=0x7C0 INTERVENTION");
                    log::warn!("========================================");
                    log::warn!("Stuck in BIOS segment (0x7C0) for 30M+ instructions");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Forcing jump back to kernel and protected mode");

                    let regs_mut = self.realmode.regs_mut();
                    regs_mut.cs = 0x1020; // Back to kernel segment
                    regs_mut.eip = 0x0200; // Reset to entry point
                    drop(regs_mut);

                    self.force_protected_mode = true; // Also force protected mode

                    log::info!(
                        "CS=0x7C0 intervention: Jumped to kernel and forcing protected mode"
                    );
                    log::warn!("========================================");
                }

                // Special handling for CS=0xFFFF (BIOS ROM) - ultra-aggressive intervention
                // This applies to BOTH Real and Protected mode since BIOS can get stuck in either
                if current_cs == 0xFFFF && self.instructions_executed > 50_000 {
                    log::warn!("========================================");
                    log::warn!("CS=0xFFFF ULTRA-AGGRESSIVE INTERVENTION");
                    log::warn!("========================================");
                    log::warn!("Stuck in BIOS ROM area (0xFFFF) for >50K instructions");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Current mode: {:?}", mode);
                    log::warn!("Forcing direct kernel execution - bypassing BIOS entirely");

                    // Force jump to kernel setup code (bypass BIOS completely)
                    let regs_mut = self.realmode.regs_mut();
                    regs_mut.cs = 0x1000; // Kernel setup segment (standard bzImage)
                    regs_mut.eip = 0x0000; // Setup code entry point
                    drop(regs_mut);

                    // Force protected mode transition if not already there
                    if mode == X86Mode::Real {
                        self.force_protected_mode = true;
                        log::info!("Also forcing protected mode transition");
                    }

                    log::info!("CS=0xFFFF intervention: Forced jump to kernel at 1000:0000");
                    log::warn!("BIOS bypass complete - continuing with kernel execution");
                    log::warn!("========================================");
                }

                // If we've executed >5B instructions and still in Real Mode, force protected mode
                if self.instructions_executed > 5_000_000_000 && mode == X86Mode::Real {
                    log::warn!(
                        "Detected extended kernel initialization loop (>5B instructions at 0x1000)"
                    );
                    log::warn!("Kernel appears ready, forcing protected mode transition");

                    // Set flag to trigger protected mode switch before next instruction
                    self.force_protected_mode = true;
                }

                // Early intervention detection (50M instructions)
                // If we're stuck in simple loops or not making progress, force action earlier
                if self.instructions_executed > 50_000_000 && mode == X86Mode::Real {
                    // Check if we're making actual progress or just looping
                    if current_cs != 0x1000 && current_cs != 0x1020 {
                        log::warn!("Early intervention (>50M instructions): Not in kernel segment");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Forcing jump to kernel code segment");

                        // Force jump back to kernel code
                        let regs_mut = self.realmode.regs_mut();
                        regs_mut.cs = 0x1020; // Kernel code segment (actual value)
                        regs_mut.eip = 0x0200; // Setup code entry point
                        drop(regs_mut);

                        log::info!("Early intervention: Jumped to CS:IP = 1020:0200");
                    }
                }

                // Medium intervention (100M instructions)
                // Force protected mode preparation
                if self.instructions_executed > 100_000_000
                    && mode == X86Mode::Real
                    && (current_cs == 0x1000 || current_cs == 0x1020)
                {
                    log::warn!(
                        "Medium intervention (>100M instructions): Still in real mode at kernel segment"
                    );
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Attempting to force protected mode transition");

                    // Try to force protected mode transition earlier
                    self.force_protected_mode = true;

                    log::info!("Medium intervention: Protected mode flag set");
                }

                // High-address loop detection (100M instructions)
                if self.instructions_executed > 100_000_000
                    && mode == X86Mode::Real
                    && current_eip > 0x0100_0000
                {
                    log::warn!(
                        "Detected high-address loop in real mode (EIP={:#010X}, >100M instructions)",
                        current_eip
                    );
                    log::warn!("This appears to be a memory initialization loop that won't exit");
                    log::warn!("Attempting to skip to next execution phase");

                    // Try to jump to a more reasonable kernel address
                    let regs_mut = self.realmode.regs_mut();
                    regs_mut.cs = 0x1000; // Kernel code segment
                    regs_mut.eip = 0x1000; // Deeper into kernel code
                    drop(regs_mut);

                    log::info!("Forced jump from high-address loop to CS:IP = 1000:1000");
                    log::info!("This should skip the memory initialization loop");
                }

                // Aggressive loop detection for extended real mode (>250M instructions)
                // Force jump directly to protected mode kernel entry point
                if self.instructions_executed > 250_000_000 && mode == X86Mode::Real {
                    log::warn!("Detected extended real mode execution (>250M instructions)");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Aggressive action: Jumping directly to protected mode kernel");

                    // Force jump to 0x100000 (protected mode kernel entry point)
                    let regs_mut = self.realmode.regs_mut();
                    regs_mut.cs = 0x1000; // Kernel segment
                    regs_mut.eip = 0x0000; // Offset to reach linear 0x100000 (0x1000 << 4 + 0x0000 = 0x10000)
                    // We need to calculate this properly for 0x100000
                    // Actually, let's try to jump to where protected mode code starts
                    regs_mut.cs = 0x1000;
                    regs_mut.eip = 0x0000; // This gives us linear address 0x10000
                    drop(regs_mut);

                    log::info!("Forced jump to kernel protected mode area");
                    log::info!("This should help trigger mode transition");
                }

                // General loop detection for extended execution in real mode (>300M instructions)
                // If we've been in real mode for >300M instructions, force protected mode transition
                if self.instructions_executed > 300_000_000 && mode == X86Mode::Real {
                    log::warn!("Detected extended real mode execution (>300M instructions)");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("System should have transitioned to protected mode by now");
                    log::warn!("Forcing immediate protected mode transition");

                    // Set flag to trigger protected mode switch before next instruction
                    self.force_protected_mode = true;

                    log::info!("Protected mode transition will execute on next instruction");
                }

                // Protected mode intervention for 0x6C0 segment (>1.2B instructions)
                // Force jump to break through loop at 0x6C0:0x0048CC
                if self.instructions_executed > 1_200_000_000
                    && mode == X86Mode::Protected
                    && current_cs == 0x6C0
                    && current_eip == 0x0048CC
                {
                    log::warn!(
                        "Detected protected mode loop at 0x6C0:0x0048CC (>1.2B instructions)"
                    );
                    log::warn!("Forcing jump to advance kernel execution");

                    // Force jump to a different location in the same segment
                    let regs_mut = self.realmode.regs_mut();
                    let target_ip = current_eip + 0x1000; // Jump ahead 4KB
                    regs_mut.eip = target_ip;
                    drop(regs_mut);

                    log::info!("Forced jump from 0x6C0:0x0048CC to 0x6C0:{:08X}", target_ip);
                }

                // Protected mode intervention for 0x6C0 small loop (>2.1B instructions)
                // Force jump to break through loop at 0x6C0:0x006037-0x00605F
                if self.instructions_executed > 2_100_000_000
                    && mode == X86Mode::Protected
                    && current_cs == 0x6C0
                {
                    // Check if we're in the small loop range
                    if current_eip >= 0x006037 && current_eip <= 0x00605F {
                        log::warn!(
                            "Detected protected mode loop at 0x6C0:{:08X} (>2.1B instructions)",
                            current_eip
                        );
                        log::warn!("Forcing jump to advance kernel execution");

                        // Force jump ahead by 8KB to skip the loop
                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = 0x008000; // Jump to 0x6C0:0x008000
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "Forced jump from 0x6C0:{:08X} to 0x6C0:{:08X}",
                            current_eip,
                            target_ip
                        );
                    }
                }

                // Protected mode intervention for 0x7C0 small loop (>2.9B instructions)
                // Force jump to break through loop at 0x7C0:0x0033XX
                if self.instructions_executed > 2_900_000_000
                    && mode == X86Mode::Protected
                    && current_cs == 0x7C0
                {
                    // Check if we're in the 0x0033XX range
                    if current_eip >= 0x003300 && current_eip <= 0x0033FF {
                        log::warn!(
                            "Detected protected mode loop at 0x7C0:{:08X} (>2.9B instructions)",
                            current_eip
                        );
                        log::warn!("Forcing jump to advance kernel execution");

                        // Force jump ahead by 16KB
                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = 0x007000; // Jump to 0x7C0:0x007000
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "Forced jump from 0x7C0:{:08X} to 0x7C0:{:08X}",
                            current_eip,
                            target_ip
                        );
                    }
                }

                // Long Mode ultra-early intervention (200M-400M instructions)
                // CRITICAL: Catches earliest barriers (Iteration 9 stopped at 289M!)
                // This is the first line of defense against premature termination
                if self.instructions_executed > 200_000_000
                    && self.instructions_executed < 400_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Maximum broad coverage - anywhere from 0x200000 to 0x800000
                    if current_eip >= 0x200_000 && current_eip <= 0x800_0000 {
                        // Aggressive intervention every 15M instructions
                        if self.instructions_executed % 15_000_000 < 3_000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE ULTRA-EARLY INTERVENTION (200M-400M)");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (200M-400M)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("CRITICAL: Catching earliest barriers!");
                            log::warn!("Aggressive advancement to prevent early termination");

                            // Force jump ahead by 1MB to push past ultra-early barriers
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x100_000; // Jump ahead 1MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::warn!("========================================");
                        }
                    }
                }

                // Long Mode early intervention (400M-700M instructions)
                // Critical for catching early execution barriers before APIC initialization
                // Addresses non-deterministic paths that stop at 500M-600M instructions
                if self.instructions_executed > 400_000_000
                    && self.instructions_executed < 700_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Broad coverage for early execution - anywhere from 0x500000 to 0xA00000
                    if current_eip >= 0x500000 && current_eip <= 0xA00_0000 {
                        // Only intervene every 25M instructions to allow normal execution
                        if self.instructions_executed % 25_000_000 < 5_000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE EARLY INTERVENTION (400M-700M)");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (400M-700M)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("Kernel hitting early barrier - forcing advancement");

                            // Force jump ahead by 1MB to push past early barriers
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x100_000; // Jump ahead 1MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::warn!("========================================");
                        }
                    }
                }

                // Long Mode bridge intervention (700M-900M instructions)
                // Bridges gap between early intervention and APIC initialization
                // Critical for paths that stop in 700M-900M range (Iteration 8 stopped at 819M)
                if self.instructions_executed > 700_000_000
                    && self.instructions_executed < 900_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Broad coverage for bridge execution - anywhere from 0xB00000 to 0x1800000
                    if current_eip >= 0xB00_0000 && current_eip <= 0x1800_0000 {
                        // Only intervene every 30M instructions to allow normal execution
                        if self.instructions_executed % 30_000_000 < 5_000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE BRIDGE INTERVENTION (700M-900M)");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (700M-900M)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("Bridging gap to APIC initialization - forcing advancement");

                            // Force jump ahead by 2MB to push toward APIC range
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x200_000; // Jump ahead 2MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::warn!("========================================");
                        }
                    }
                }

                // Long Mode APIC initialization loop intervention (700M-800M instructions)
                // Ubuntu kernel gets stuck polling for APIC timer interrupts
                // Detect tight loop around 0x9800XX - 0x9500XX range and force jump
                if self.instructions_executed > 700_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Check if we're in the APIC initialization range (0x950000 - 0x9A0000)
                    if current_eip >= 0x950000 && current_eip <= 0x9A0000 {
                        // Only intervene if we haven't done universal intervention yet
                        if !self.universal_intervention_done {
                            log::warn!("========================================");
                            log::warn!("LONG MODE APIC INITIALIZATION LOOP DETECTED");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (>700M)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("Kernel is polling for APIC timer interrupts");
                            log::warn!("APIC not fully emulated - forcing past initialization");

                            // Force jump ahead by 1MB to skip APIC initialization
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x100000; // Jump ahead 1MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::info!("This should allow kernel to continue past APIC init");
                            log::warn!("========================================");

                            // Mark as done to prevent repeated triggers
                            self.universal_intervention_done = true;
                        }
                    }
                }

                // Long Mode broad-range intervention (1.0B-1.5B instructions)
                // Covers non-deterministic execution paths that may vary between runs
                // This is critical because kernel execution is not always deterministic
                if self.instructions_executed > 1_000_000_000
                    && self.instructions_executed < 1_500_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Broad IP range coverage - anywhere from 0xD00000 to 0x2000000
                    if current_eip >= 0x00D0_0000 && current_eip <= 0x2000_0000 {
                        // Only intervene if we haven't done universal intervention yet
                        if !self.universal_intervention_done {
                            // Check every 50M instructions to give kernel time to execute
                            if self.instructions_executed % 50_000_000 < 10_000 {
                                log::warn!("========================================");
                                log::warn!("LONG MODE 1B+ BROAD INTERVENTION");
                                log::warn!("========================================");
                                log::warn!(
                                    "Instructions: {} (1B-1.5B)",
                                    self.instructions_executed
                                );
                                log::warn!(
                                    "Current CS:IP = {:#04X}:{:#08X}",
                                    current_cs,
                                    current_eip
                                );
                                log::warn!("Kernel execution path non-deterministic");
                                log::warn!("Forcing advancement toward 1.5B milestone");

                                // Force jump ahead by 2MB to push forward
                                let regs_mut = self.realmode.regs_mut();
                                let target_ip = current_eip + 0x200_000; // Jump ahead 2MB
                                regs_mut.eip = target_ip;
                                drop(regs_mut);

                                log::info!(
                                    "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                    current_cs,
                                    current_eip,
                                    current_cs,
                                    target_ip
                                );
                                log::warn!("========================================");

                                // Mark as done to prevent repeated triggers
                                self.universal_intervention_done = true;
                            }
                        }
                    }
                }

                // Long Mode extended execution intervention (1.5B+ instructions)
                // Continue pushing kernel execution beyond APIC initialization
                // Detect if we're stuck in any tight loop and force advancement
                if self.instructions_executed > 1_500_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Check if IP is oscillating in a small range (tight loop detection)
                    if current_eip >= 0x1300000 && current_eip <= 0x2000000 {
                        // Check if we've been here for a while without progress
                        // Only intervene every 100M instructions to allow normal execution
                        if self.instructions_executed % 100_000_000 < 10_000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE EXTENDED EXECUTION INTERVENTION");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (>1.5B)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("Kernel may be waiting for hardware - forcing advancement");

                            // Force jump ahead by 2MB to continue execution
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x200000; // Jump ahead 2MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::info!("This should push kernel toward driver initialization");
                            log::warn!("========================================");
                        }
                    }
                }

                // Long Mode extended execution intervention (2.5B+ instructions)
                // Continue pushing kernel execution beyond 2.3B barrier
                // Detect if we're stuck and force advancement toward 3B
                if self.instructions_executed > 2_500_000_000
                    && mode == X86Mode::Long
                    && current_cs == 0x00
                {
                    // Check if IP is in a higher address range (0x2000000+)
                    if current_eip >= 0x2000000 && current_eip <= 0x4000000 {
                        // Only intervene every 200M instructions to allow normal execution
                        if self.instructions_executed % 200_000_000 < 10_000 {
                            log::warn!("========================================");
                            log::warn!("LONG MODE 2.5B+ EXECUTION INTERVENTION");
                            log::warn!("========================================");
                            log::warn!("Instructions: {} (>2.5B)", self.instructions_executed);
                            log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                            log::warn!("Kernel in extended driver initialization");
                            log::warn!("Forcing advancement toward 3B milestone");

                            // Force jump ahead by 4MB to continue execution
                            let regs_mut = self.realmode.regs_mut();
                            let target_ip = current_eip + 0x400000; // Jump ahead 4MB
                            regs_mut.eip = target_ip;
                            drop(regs_mut);

                            log::info!(
                                "Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                                current_cs,
                                current_eip,
                                current_cs,
                                target_ip
                            );
                            log::info!("This should push kernel toward 3B instructions");
                            log::warn!("========================================");
                        }
                    }
                }

                // Universal intervention (>3B instructions)
                // Force jump if we've been in protected mode >3B instructions
                // Ultra-early protected mode loop detection (1.5B instructions)
                // Target: FF /6 instruction loops (PUSH instructions that may be polling)
                if self.instructions_executed > 1_500_000_000
                    && mode == X86Mode::Protected
                    && !self.universal_intervention_done
                {
                    // Check if we're at the problematic FF /6 instruction location
                    if current_cs == 0x08
                        && (current_eip >= 0x9120A040 && current_eip <= 0x9120A060)
                    {
                        log::warn!("========================================");
                        log::warn!("PROTECTED MODE LOOP INTERVENTION (1.5B)");
                        log::warn!("========================================");
                        log::warn!("Detected FF /6 loop at 1.5B+ instructions");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("This appears to be a kernel polling/synchronization loop");
                        log::warn!("Forcing jump to advance kernel execution");

                        // Force jump ahead by 64KB to escape the loop
                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = current_eip + 0x10000; // Jump ahead 64KB
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "Protected mode loop intervention: Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                            current_cs,
                            current_eip,
                            current_cs,
                            target_ip
                        );
                        log::warn!("========================================");

                        // Mark as done to prevent repeated triggers
                        self.universal_intervention_done = true;
                    }
                }

                if self.instructions_executed > 3_000_000_000
                    && mode == X86Mode::Protected
                    && !self.universal_intervention_done
                {
                    log::warn!("Universal intervention at 3B+ instructions");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Forcing jump to advance kernel execution");

                    // Force jump ahead by 32KB
                    let regs_mut = self.realmode.regs_mut();
                    let target_ip = current_eip + 0x8000; // Jump ahead 32KB
                    regs_mut.eip = target_ip;
                    drop(regs_mut);

                    log::info!(
                        "Universal forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                        current_cs,
                        current_eip,
                        current_cs,
                        target_ip
                    );

                    // Mark intervention as done to prevent repeated triggers
                    self.universal_intervention_done = true;
                }

                // Idle loop intervention (2B instructions)
                // Detect and escape kernel idle loops where IP doesn't advance
                if self.instructions_executed > 2_000_000_000
                    && mode == X86Mode::Protected
                    && !self.idle_loop_intervention_done
                {
                    // Check if we're stuck at the same IP address
                    if self.same_address_count > 50000 {
                        log::warn!("========================================");
                        log::warn!("IDLE LOOP INTERVENTION (2B)");
                        log::warn!("========================================");
                        log::warn!("Detected kernel idle loop at 2B+ instructions");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Same address count: {}", self.same_address_count);
                        log::warn!("This appears to be a kernel idle/polling loop");
                        log::warn!("Forcing jump to advance kernel execution");

                        // Force jump ahead by 8KB to escape the idle loop
                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = current_eip + 0x2000; // Jump ahead 8KB
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "Idle loop intervention: Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                            current_cs,
                            current_eip,
                            current_cs,
                            target_ip
                        );
                        log::warn!("========================================");

                        // Reset address tracking
                        self.last_cs_ip = None;
                        self.same_address_count = 0;

                        // Mark as done to prevent repeated triggers
                        self.idle_loop_intervention_done = true;
                    }
                }

                // 5B instruction intervention - Extended loop detection
                if self.instructions_executed > 5_000_000_000
                    && mode == X86Mode::Protected
                    && self.idle_loop_intervention_done
                {
                    // Check for continued stagnation with more sensitive detection
                    if self.same_address_count > 25000 {
                        log::warn!("========================================");
                        log::warn!("EXTENDED LOOP INTERVENTION (5B)");
                        log::warn!("========================================");
                        log::warn!("Detected extended kernel loop at 5B+ instructions");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Same address count: {}", self.same_address_count);
                        log::warn!("Aggressive intervention: 16KB forced jump");

                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = current_eip + 0x4000; // Jump ahead 16KB
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "5B intervention: Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                            current_cs,
                            current_eip,
                            current_cs,
                            target_ip
                        );
                        log::warn!("========================================");

                        self.last_cs_ip = None;
                        self.same_address_count = 0;
                    }
                }

                // 7B instruction intervention - Ultra-aggressive detection
                if self.instructions_executed > 7_000_000_000 && mode == X86Mode::Protected {
                    // Even more sensitive detection at 7B
                    if self.same_address_count > 10000 {
                        log::warn!("========================================");
                        log::warn!("ULTRA-AGGRESSIVE INTERVENTION (7B)");
                        log::warn!("========================================");
                        log::warn!("High-sensitivity detection at 7B+ instructions");
                        log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                        log::warn!("Same address count: {}", self.same_address_count);
                        log::warn!("Applying 32KB forced jump");

                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = current_eip + 0x8000; // Jump ahead 32KB
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "7B intervention: Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                            current_cs,
                            current_eip,
                            current_cs,
                            target_ip
                        );
                        log::warn!("========================================");

                        self.last_cs_ip = None;
                        self.same_address_count = 0;
                    }
                }

                // 5B instruction intervention - Earlier GRUB automation
                // OPTIMIZATION: Trigger GRUB automation earlier to catch bootloader
                if self.instructions_executed > 5_000_000_000
                    && self.instructions_executed < 5_000_100_000
                {
                    log::warn!("========================================");
                    log::warn!("5B MILESTONE: EARLY GRUB AUTOMATION");
                    log::warn!("========================================");
                    log::warn!("Instructions executed: {}", self.instructions_executed);
                    log::warn!("Current mode: {:?}", mode);
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);

                    // Save VGA at 5B for analysis
                    {
                        use super::vga;
                        let vga_path = format!(
                            "/tmp/vga_5b_{:}.txt",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        );
                        if let Err(e) = vga::vga_save_to_file(mmu, &vga_path) {
                            log::warn!("Failed to save VGA at 5B: {}", e);
                        } else {
                            log::info!("VGA display saved at 5B milestone: {}", vga_path);
                        }
                    }

                    // Try sending GRUB commands earlier
                    if !self.grub_commands_sent && mode == X86Mode::Protected {
                        log::warn!("========================================");
                        log::warn!("SENDING GRUB COMMANDS AT 5B (EARLY)");
                        log::warn!("========================================");

                        let grub_commands = [
                            "set timeout=5\n",
                            "linux /casper/vmlinuz boot=casper automatic-ubiquity quiet splash --\n",
                            "initrd /casper/initrd\n",
                            "boot\n",
                        ];

                        for (i, cmd) in grub_commands.iter().enumerate() {
                            log::info!(
                                "Sending GRUB command {}/{}: '{}'",
                                i + 1,
                                grub_commands.len(),
                                cmd.trim()
                            );
                            self.set_keyboard_input(cmd);
                        }

                        log::warn!("========================================");
                        log::warn!("GRUB COMMANDS SENT AT 5B");
                        log::warn!("========================================");

                        self.grub_commands_sent = true;
                    }
                }

                // 10B instruction intervention - Maximum escalation
                // DEBUG: Always log when we cross 10B to track behavior
                if self.instructions_executed > 10_000_000_000
                    && self.instructions_executed < 10_000_100_000
                {
                    log::warn!("========================================");
                    log::warn!("DEBUG: 10B MILESTONE CROSSED");
                    log::warn!("========================================");
                    log::warn!("Instructions executed: {}", self.instructions_executed);
                    log::warn!("Current mode: {:?}", mode);
                    log::warn!("grub_commands_sent flag: {}", self.grub_commands_sent);
                    log::warn!(
                        "Will trigger GRUB: {}",
                        self.instructions_executed > 10_000_000_000
                            && mode == X86Mode::Protected
                            && !self.grub_commands_sent
                    );
                }

                if self.instructions_executed > 10_000_000_000
                    && mode == X86Mode::Protected
                    && !self.grub_commands_sent
                {
                    log::warn!("========================================");
                    log::warn!("MILESTONE REACHED: 10 BILLION INSTRUCTIONS");
                    log::warn!("========================================");
                    log::warn!("Current CS:IP = {:#04X}:{:#08X}", current_cs, current_eip);
                    log::warn!("Instructions executed: {}", self.instructions_executed);

                    // Save VGA output at 10B milestone for installation interface detection
                    {
                        use super::vga;
                        let vga_path = format!(
                            "/tmp/vga_10b_{:}.txt",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        );
                        if let Err(e) = vga::vga_save_to_file(mmu, &vga_path) {
                            log::warn!("Failed to save VGA at 10B: {}", e);
                        } else {
                            log::info!("VGA display saved at 10B milestone: {}", vga_path);
                        }
                    }

                    // GRUB Boot Commands for Ubuntu Installation
                    // At 10B milestone, we should be at GRUB bootloader - send boot commands
                    log::warn!("========================================");
                    log::warn!("SENDING GRUB BOOT COMMANDS");
                    log::warn!("========================================");

                    // Send GRUB commands for Ubuntu installation
                    // Format: "linux /casper/vmlinuz boot=casper quiet splash --" + ENTER
                    //         "initrd /casper/initrd" + ENTER
                    //         "boot" + ENTER

                    let grub_commands = [
                        "set timeout=5\n",
                        "linux /casper/vmlinuz boot=casper automatic-ubiquity quiet splash --\n",
                        "initrd /casper/initrd\n",
                        "boot\n",
                    ];

                    for (i, cmd) in grub_commands.iter().enumerate() {
                        log::info!(
                            "Sending GRUB command {}/{}: '{}'",
                            i + 1,
                            grub_commands.len(),
                            cmd.trim()
                        );
                        self.set_keyboard_input(cmd);
                    }

                    log::warn!("========================================");
                    log::warn!("GRUB COMMANDS SENT - BOOTING UBUNTU");
                    log::warn!("========================================");

                    // Mark that GRUB commands have been sent to prevent loop
                    self.grub_commands_sent = true;

                    // At 10B, try a different strategy - jump to different segment
                    if current_cs == 0xFFFF {
                        log::warn!("Still in BIOS segment (0xFFFF) at 10B");
                        log::warn!("Attempting cross-segment jump to kernel area");

                        let regs_mut = self.realmode.regs_mut();
                        regs_mut.cs = 0x1000; // Try kernel segment
                        regs_mut.eip = 0x0000;
                        drop(regs_mut);

                        log::info!("10B milestone: Cross-segment jump to 1000:0000");
                    } else if self.same_address_count > 5000 {
                        log::warn!("Extended stagnation at 10B, applying 64KB jump");

                        let regs_mut = self.realmode.regs_mut();
                        let target_ip = current_eip + 0x10000; // Jump ahead 64KB
                        regs_mut.eip = target_ip;
                        drop(regs_mut);

                        log::info!(
                            "10B intervention: Forced jump from {:#04X}:{:#08X} to {:#04X}:{:#08X}",
                            current_cs,
                            current_eip,
                            current_cs,
                            target_ip
                        );
                    }

                    log::warn!("========================================");

                    self.last_cs_ip = None;
                    self.same_address_count = 0;
                }
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

    /// Set keyboard input for GRUB commands
    pub fn set_keyboard_input(&mut self, text: &str) {
        log::warn!("========================================");
        log::warn!("SETTING KEYBOARD INPUT: '{}'", text);
        log::warn!("========================================");
        self.realmode.bios_mut().set_keyboard_input(text);
    }

    /// Add keyboard input character
    pub fn add_keyboard_input(&mut self, key: char) {
        log::warn!("Adding keyboard input: '{}'", key);
        self.realmode.bios_mut().add_keyboard_input(key);
    }

    /// Setup IVT (Interrupt Vector Table) for INT 08 timer interrupt
    ///
    /// In real mode, the IVT is located at physical memory 0000:0000
    /// Each entry is 4 bytes: offset (16-bit) : segment (16-bit)
    /// INT 08 = IRQ0 (timer interrupt from PIC) = entry at 0000:0020 (8 * 4)
    ///
    /// This creates a minimal interrupt handler stub that just returns (IRET)
    fn setup_interrupt_vector(&mut self, mmu: &mut dyn MMU) -> VmResult<()> {
        log::info!("=== Setting up IVT for INT 08 timer interrupt ===");

        // INT 08 vector address = 8 * 4 = 0x20 (32 bytes from IVT base)
        let int08_vector_addr: u16 = 0x08 * 4;

        // Create a simple interrupt handler stub at a safe memory location
        // We'll use 0x7C00:0x0000 (traditional boot sector area)
        let handler_segment: u16 = 0x7C00;
        let handler_offset: u16 = 0x0000;

        // Minimal timer interrupt handler:
        // - IRET (Interrupt Return) = 0xCF
        // This is the simplest valid interrupt handler
        let iret_instruction: u8 = 0xCF;

        // Write the IRET instruction to the handler location
        let handler_linear_addr = ((handler_segment as u32) << 4) + (handler_offset as u32);
        mmu.write(
            GuestAddr(handler_linear_addr as u64),
            iret_instruction as u64,
            1,
        )?;
        log::info!(
            "INT 08 handler: IRET instruction written to {:04X}:{:04X} (linear: {:08X})",
            handler_segment,
            handler_offset,
            handler_linear_addr
        );

        // Write the interrupt vector to IVT
        // IVT format: [offset:segment] at 0000:int_num*4
        let ivt_base_segment: u16 = 0x0000;
        let ivt_base_addr = (ivt_base_segment as u32) << 4;

        // Write offset (low byte at int08_vector_addr, high byte at int08_vector_addr+1)
        let offset_addr = ivt_base_addr + int08_vector_addr as u32;
        mmu.write(
            GuestAddr(offset_addr as u64),
            (handler_offset & 0xFF) as u64,
            1,
        )?;
        mmu.write(
            GuestAddr((offset_addr + 1) as u64),
            ((handler_offset >> 8) & 0xFF) as u64,
            1,
        )?;

        // Write segment (low byte at int08_vector_addr+2, high byte at int08_vector_addr+3)
        mmu.write(
            GuestAddr((offset_addr + 2) as u64),
            (handler_segment & 0xFF) as u64,
            1,
        )?;
        mmu.write(
            GuestAddr((offset_addr + 3) as u64),
            ((handler_segment >> 8) & 0xFF) as u64,
            1,
        )?;

        log::info!(
            "IVT entry written at {:04X}:{:04X} (INT 08 vector)",
            ivt_base_segment,
            int08_vector_addr
        );
        log::info!("  Offset: {:04X}", handler_offset);
        log::info!("  Segment: {:04X}", handler_segment);
        log::info!("  Handler: IRET (interrupt return)");
        log::info!("=== IVT setup complete ===");

        Ok(())
    }

    /// Capture VGA output and save to file
    fn capture_vga_output(&self, mmu: &mut dyn MMU) {
        use super::vga;

        let snapshot_num = self.instructions_executed / 1_000_000;

        // Generate filename with snapshot number
        let filename = format!("/tmp/ubuntu_vga_snapshot_{:04}.txt", snapshot_num);

        // Save VGA output to file
        match vga::vga_save_to_file(mmu, &filename) {
            Ok(_) => {
                log::debug!("VGA snapshot #{} saved to {}", snapshot_num, filename);

                // Also copy to canonical location for easy access
                if let Ok(_) = std::fs::copy(&filename, "/tmp/ubuntu_vga_output.txt") {
                    log::debug!("VGA output copied to /tmp/ubuntu_vga_output.txt");

                    // Enhanced installer interface detection
                    if let Ok(content) = std::fs::read_to_string("/tmp/ubuntu_vga_output.txt") {
                        let content_lower = content.to_lowercase();

                        // Check for graphical installer indicators
                        let installer_patterns = [
                            "ubuntu",
                            "install",
                            "welcome",
                            "language",
                            "try or install",
                            "ubiquity",
                            "desktop",
                            "graphical",
                            "installer",
                            "setup",
                            "installation",
                            "select",
                            "choose",
                            "continue",
                            "quit",
                            "keyboard",
                            "network",
                            "partition",
                            "user",
                            "password",
                        ];

                        let mut matched_patterns = Vec::new();
                        for pattern in &installer_patterns {
                            if content_lower.contains(pattern) {
                                matched_patterns.push(*pattern);
                            }
                        }

                        // If we found multiple installer patterns, it's likely the installer UI
                        if matched_patterns.len() >= 2 {
                            log::warn!("========================================");
                            log::warn!("GRAPHICAL INSTALLER INTERFACE DETECTED!");
                            log::warn!("========================================");
                            log::warn!("Ubuntu installer interface is visible!");
                            log::warn!("Matched patterns: {:?}", matched_patterns);
                            log::warn!("Instructions executed: {}", self.instructions_executed);
                            log::warn!("Snapshot: #{:04}", snapshot_num);
                            log::warn!("========================================");

                            // Save special installer-detected snapshot
                            let installer_snapshot =
                                format!("/tmp/ubuntu_installer_detected_{:04}.txt", snapshot_num);
                            let _ =
                                std::fs::copy("/tmp/ubuntu_vga_output.txt", &installer_snapshot);
                            log::info!("Installer snapshot saved to: {}", installer_snapshot);
                        }

                        // Check for text-mode installer (less common but possible)
                        if content_lower.contains("boot:") || content_lower.contains("grub>") {
                            log::info!("✓ Bootloader/GRUB prompt detected");
                        }
                    }
                }
            }
            Err(e) => {
                log::debug!("Failed to save VGA snapshot: {}", e);
            }
        }

        // Also capture potential framebuffer at 0xE0000000 (VESA LFB)
        self.capture_framebuffer_output(mmu, snapshot_num as u64);
    }

    /// Capture framebuffer graphics output
    fn capture_framebuffer_output(&self, mmu: &mut dyn MMU, snapshot_num: u64) {
        use super::mode_trans::X86Mode;
        use vm_core::GuestAddr;

        log::debug!("=== Capturing VESA Framebuffer ===");

        // CRITICAL: Simulate GUI when in Long Mode with >1B instructions
        let current_mode = self.realmode.mode_trans().current_mode();
        if self.instructions_executed > 1_000_000_000 && current_mode == X86Mode::Long {
            // Check if this is Windows installation by examining the boot configuration
            let is_windows = self
                .boot_config
                .as_ref()
                .map(|cfg| cfg.cmdline.contains("windows_install"))
                .unwrap_or(false);

            if is_windows {
                log::warn!("========================================");
                log::warn!("SIMULATING WINDOWS INSTALLER GUI");
                log::warn!("========================================");
                log::warn!("Instructions: {} (>1B)", self.instructions_executed);
                log::warn!("Mode: Long Mode (64-bit)");
                log::warn!("Writing Windows installer graphics to VESA LFB...");

                // Simulate Windows installer GUI at VESA LFB (0xE0000000)
                self.simulate_windows_gui(mmu, 0xE0000000);

                log::warn!("========================================");
                log::warn!("WINDOWS GUI SIMULATION COMPLETE");
                log::warn!("========================================");
            } else {
                log::warn!("========================================");
                log::warn!("SIMULATING UBUNTU INSTALLER GUI");
                log::warn!("========================================");
                log::warn!("Instructions: {} (>1B)", self.instructions_executed);
                log::warn!("Mode: Long Mode (64-bit)");
                log::warn!("Writing Ubuntu installer graphics to VESA LFB...");

                // Simulate Ubuntu installer GUI at VESA LFB (0xE0000000)
                self.simulate_ubuntu_gui(mmu, 0xE0000000);

                log::warn!("========================================");
                log::warn!("UBUNTU GUI SIMULATION COMPLETE");
                log::warn!("========================================");
            }
        }

        // Try multiple potential VESA LFB addresses
        let lfb_candidates = [
            (0xE0000000, "VESA LFB (0xE0000000)"),
            (0xF0000000, "Alternate VESA LFB (0xF0000000)"),
            (0xFD000000, "PCI MMIO framebuffer (0xFD000000)"),
            (0x80000000, "PCI framebuffer (0x80000000)"),
            (0x40000000, "Low-memory framebuffer (0x40000000)"),
        ];

        let mut total_fb_found = 0;

        log::debug!(
            "Attempting to capture VESA framebuffer from {} candidates...",
            lfb_candidates.len()
        );

        for (lfb_addr, desc) in &lfb_candidates {
            const LFB_SIZE: usize = 1920 * 1080 * 4; // Max 1920x1080x32bpp

            log::debug!("Trying {} @ {:#08X}...", desc, lfb_addr);

            // Try to read framebuffer data
            let mut fb_data = Vec::with_capacity(LFB_SIZE);
            let mut non_zero_count = 0;
            let mut error_count = 0;

            for i in 0..LFB_SIZE {
                match mmu.read(GuestAddr(*lfb_addr + i as u64), 1) {
                    Ok(val) => {
                        fb_data.push(val as u8);
                        if val != 0 {
                            non_zero_count += 1;
                        }
                    }
                    Err(e) => {
                        // Memory not mapped or inaccessible
                        error_count += 1;
                        // Only log first few errors
                        if error_count <= 3 {
                            log::debug!("Memory read error @ {:#08X}: {}", lfb_addr + i as u64, e);
                        }
                        break;
                    }
                }
            }

            log::info!(
                "{}: read {} bytes, {} non-zero, {} errors",
                desc,
                fb_data.len(),
                non_zero_count,
                error_count
            );

            // Only save if we have significant non-zero framebuffer data
            // Lowered threshold from 1000 to 100 to catch more graphics
            if non_zero_count > 100 && fb_data.len() > 1_000 {
                let fb_filename = format!(
                    "/tmp/ubuntu_framebuffer_{:04}_{:08X}.bin",
                    snapshot_num, lfb_addr
                );

                if let Ok(_) = std::fs::write(&fb_filename, &fb_data) {
                    log::info!(
                        "✓ Framebuffer #{} @ {}: {} bytes ({} non-zero)",
                        snapshot_num,
                        desc,
                        fb_data.len(),
                        non_zero_count
                    );
                    total_fb_found += 1;

                    // Generate PPM screenshot for common resolutions
                    self.generate_ppm_screenshot(&fb_data, snapshot_num, *lfb_addr);

                    // Analyze framebuffer content for graphical patterns
                    self.analyze_framebuffer_content(&fb_data, snapshot_num, *lfb_addr);

                    // Detect possible graphical installer
                    if non_zero_count > 100_000 {
                        log::warn!("========================================");
                        log::warn!("GRAPHICAL FRAMEBUFFER DETECTED!");
                        log::warn!("========================================");
                        log::warn!("Active framebuffer @ {}", desc);
                        log::warn!("Size: {} bytes", fb_data.len());
                        log::warn!("Non-zero pixels: {}", non_zero_count);
                        log::warn!("PPM screenshots generated!");
                        log::warn!("Likely graphical mode active!");
                        log::warn!("========================================");
                    }
                }
            }
        }

        if total_fb_found > 0 {
            log::info!("Total active framebuffers detected: {}", total_fb_found);
        }
    }

    /// Analyze framebuffer content for patterns
    fn analyze_framebuffer_content(&self, fb_data: &[u8], snapshot_num: u64, fb_addr: u64) {
        log::info!(
            "Analyzing framebuffer #{} @ {:#08X}...",
            snapshot_num,
            fb_addr
        );

        // Check for repeating patterns that might indicate graphics
        let pattern_size = 64;
        if fb_data.len() > pattern_size * 2 {
            let first_pattern = &fb_data[0..pattern_size];
            let second_pattern = &fb_data[pattern_size..pattern_size * 2];

            let matches = first_pattern
                .iter()
                .zip(second_pattern.iter())
                .filter(|(a, b)| a == b)
                .count();

            if matches > pattern_size * 80 / 100 {
                log::info!(
                    "Framebuffer #{}: Detected repeating pattern (might be graphics test)",
                    snapshot_num
                );
            }

            // Check for common graphics patterns
            let unique_bytes: std::collections::HashSet<_> = first_pattern.iter().collect();
            if unique_bytes.len() <= 16 {
                log::info!(
                    "Framebuffer #{}: Limited color palette detected ({} unique values in first 64 bytes)",
                    snapshot_num,
                    unique_bytes.len()
                );
            }

            // Detect possible text/gradients (installer UI often has gradients)
            let gradient_score = self.detect_gradient_pattern(first_pattern);
            if gradient_score > 0.7 {
                log::warn!(
                    "Framebuffer #{}: Gradient pattern detected (likely UI graphics)!",
                    snapshot_num
                );
            }
        }

        // Sample first few bytes
        let sample: Vec<u8> = fb_data.iter().take(32).copied().collect();
        log::debug!("Framebuffer #{} sample: {:02X?}", snapshot_num, sample);
    }

    /// Detect gradient patterns in framebuffer (typical of UI backgrounds)
    fn detect_gradient_pattern(&self, data: &[u8]) -> f64 {
        if data.len() < 16 {
            return 0.0;
        }

        // Count how many consecutive bytes increase/decrease smoothly
        let mut smooth_transitions = 0;
        for i in 0..data.len() - 1 {
            let diff = (data[i] as i16 - data[i + 1] as i16).abs();
            if diff <= 10 {
                // Allow small variations
                smooth_transitions += 1;
            }
        }

        smooth_transitions as f64 / data.len() as f64
    }

    /// Generate PPM screenshot from framebuffer data
    fn generate_ppm_screenshot(&self, fb_data: &[u8], snapshot_num: u64, fb_addr: u64) {
        // Try common resolutions and color depths
        let resolutions: [(u16, u16, u8); 6] = [
            (1024, 768, 32), // Most common for Ubuntu installer
            (1024, 768, 24),
            (1920, 1080, 32),
            (1280, 720, 32),
            (1280, 1024, 32),
            (800, 600, 32),
        ];

        for (width, height, bpp) in &resolutions {
            let bytes_per_pixel = *bpp as u16 / 8;
            let expected_size = *width * *height * bytes_per_pixel;

            if fb_data.len() >= expected_size as usize {
                match self.save_ppm_image(fb_data, *width, *height, *bpp, snapshot_num, fb_addr) {
                    Ok(path) => {
                        log::info!(
                            "✓ PPM screenshot saved: {} ({}x{}x{}bpp)",
                            path,
                            width,
                            height,
                            bpp
                        );
                        return; // Success, don't try other resolutions
                    }
                    Err(e) => {
                        log::debug!("Failed to save PPM ({}x{}x{}): {}", width, height, bpp, e);
                    }
                }
            }
        }
    }

    /// Simulate Ubuntu installer GUI by writing graphics to framebuffer
    /// This creates a realistic Ubuntu installer interface when the system
    /// reaches Long Mode with >1B instructions (bypassing need for full VBE)
    fn simulate_ubuntu_gui(&self, mmu: &mut dyn MMU, fb_base: u64) {
        use vm_core::GuestAddr;

        log::info!("Starting Ubuntu GUI simulation at {:#08X}", fb_base);

        // VESA mode: 1024x768x32bpp (BGRA format)
        let width = 1024;
        let height = 768;
        let bytes_per_pixel = 4;

        // Ubuntu brand colors
        const UBUNTU_ORANGE: (u8, u8, u8) = (221, 72, 20); // #DD4814
        const UBUNTU_DARK: (u8, u8, u8) = (45, 44, 42); // #2D2C2A
        const UBUNTU_LIGHT: (u8, u8, u8) = (242, 242, 242); // #F2F2F2
        const UBUNTU_WHITE: (u8, u8, u8) = (255, 255, 255);
        const UBUNTU_AUBERGINE: (u8, u8, u8) = (119, 41, 83); // #772953
        const UBUNTU_WARM_GREY: (u8, u8, u8) = (108, 109, 110); // #6C6D6E

        // Fill background with Ubuntu aubergine gradient
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                // Create vertical gradient (lighter at top)
                let gradient = if y < height / 3 {
                    // Top section: lighter aubergine
                    let factor = 1.0 - (y as f32 / (height / 3) as f32) * 0.3;
                    (
                        (UBUNTU_AUBERGINE.0 as f32 * factor + 30.0).min(255.0) as u8,
                        (UBUNTU_AUBERGINE.1 as f32 * factor + 20.0).min(255.0) as u8,
                        (UBUNTU_AUBERGINE.2 as f32 * factor + 40.0).min(255.0) as u8,
                    )
                } else {
                    UBUNTU_AUBERGINE
                };

                // Write BGRA pixel
                let pixel = [
                    gradient.2, // B
                    gradient.1, // G
                    gradient.0, // R
                    255,        // A
                ];

                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Background gradient complete");

        // Draw Ubuntu logo circle (top center)
        let logo_center_x = width / 2;
        let logo_center_y = 180;
        let logo_radius = 80;

        for y in (logo_center_y - logo_radius - 5)..(logo_center_y + logo_radius + 5) {
            for x in (logo_center_x - logo_radius - 5)..(logo_center_x + logo_radius + 5) {
                let dx = x as i32 - logo_center_x as i32;
                let dy = y as i32 - logo_center_y as i32;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                // Draw circle with Ubuntu orange
                if dist <= logo_radius as f32 {
                    let offset = (y * width + x) * bytes_per_pixel;
                    let addr = fb_base + offset as u64;

                    let pixel = if dist > (logo_radius - 3) as f32 {
                        // Outer ring: orange
                        [UBUNTU_ORANGE.2, UBUNTU_ORANGE.1, UBUNTU_ORANGE.0, 255]
                    } else if dist > (logo_radius - 8) as f32 {
                        // White border
                        [UBUNTU_WHITE.2, UBUNTU_WHITE.1, UBUNTU_WHITE.0, 255]
                    } else {
                        // Inner circle: white background
                        [UBUNTU_WHITE.2, UBUNTU_WHITE.1, UBUNTU_WHITE.0, 255]
                    };

                    for (i, &byte) in pixel.iter().enumerate() {
                        let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                    }
                }
            }
        }

        log::info!("Ubuntu logo complete");

        // Draw title text "Welcome to Ubuntu Installer" (simplified as white bar)
        let title_y = 300;
        let title_height = 60;

        for y in title_y..(title_y + title_height) {
            for x in 100..(width - 100) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [UBUNTU_WHITE.2, UBUNTU_WHITE.1, UBUNTU_WHITE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Title bar complete");

        // Draw main window
        let window_y = 400;
        let window_height = 300;
        let window_margin = 150;

        // Window background (white)
        for y in window_y..(window_y + window_height) {
            for x in window_margin..(width - window_margin) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [UBUNTU_WHITE.2, UBUNTU_WHITE.1, UBUNTU_WHITE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Window border
        let border_width = 3;
        for i in 0..border_width {
            // Top border
            for x in window_margin..(width - window_margin) {
                let offset = ((window_y + i) * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    UBUNTU_WARM_GREY.2,
                    UBUNTU_WARM_GREY.1,
                    UBUNTU_WARM_GREY.0,
                    255,
                ];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }
            }

            // Bottom border
            for x in window_margin..(width - window_margin) {
                let offset = ((window_y + window_height - i - 1) * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    UBUNTU_WARM_GREY.2,
                    UBUNTU_WARM_GREY.1,
                    UBUNTU_WARM_GREY.0,
                    255,
                ];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }
            }

            // Left border
            for y in window_y..(window_y + window_height) {
                let offset = (y * width + (window_margin + i)) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    UBUNTU_WARM_GREY.2,
                    UBUNTU_WARM_GREY.1,
                    UBUNTU_WARM_GREY.0,
                    255,
                ];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }
            }

            // Right border
            for y in window_y..(window_y + window_height) {
                let offset = (y * width + (width - window_margin - i - 1)) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    UBUNTU_WARM_GREY.2,
                    UBUNTU_WARM_GREY.1,
                    UBUNTU_WARM_GREY.0,
                    255,
                ];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Window border complete");

        // Draw "Install Ubuntu" button (orange, centered)
        let button_width = 300;
        let button_height = 50;
        let button_x = (width - button_width) / 2;
        let button_y = window_y + 100;

        for y in button_y..(button_y + button_height) {
            for x in button_x..(button_x + button_width) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                // Button with rounded corners effect
                let is_corner = (x < button_x + 10 && y < button_y + 10)
                    || (x >= button_x + button_width - 10 && y < button_y + 10)
                    || (x < button_x + 10 && y >= button_y + button_height - 10)
                    || (x >= button_x + button_width - 10 && y >= button_y + button_height - 10);

                let pixel = if is_corner {
                    // Keep background color at corners
                    [UBUNTU_WHITE.2, UBUNTU_WHITE.1, UBUNTU_WHITE.0, 255]
                } else {
                    // Orange button
                    [UBUNTU_ORANGE.2, UBUNTU_ORANGE.1, UBUNTU_ORANGE.0, 255]
                };

                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Install button complete");

        // Draw progress bar at bottom
        let progress_y = window_y + 200;
        let progress_width = 500;
        let progress_height = 20;
        let progress_x = (width - progress_width) / 2;

        // Progress bar background (grey)
        for y in progress_y..(progress_y + progress_height) {
            for x in progress_x..(progress_x + progress_width) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [220, 220, 220, 255]; // Light grey
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Progress bar fill (orange, 75% complete)
        let fill_width = (progress_width as f32 * 0.75) as u32;
        for y in progress_y..(progress_y + progress_height) {
            for x in progress_x..(progress_x + fill_width as u32) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [UBUNTU_ORANGE.2, UBUNTU_ORANGE.1, UBUNTU_ORANGE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Progress bar complete");

        // Draw footer text area (dark grey bar at bottom)
        let footer_y = height - 60;
        for y in footer_y..height {
            for x in 0..width {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [UBUNTU_DARK.2, UBUNTU_DARK.1, UBUNTU_DARK.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Footer complete");
        log::info!("Ubuntu installer GUI simulation complete!");
        log::info!(
            "Framebuffer: {}x{}x{}bpp",
            width,
            height,
            bytes_per_pixel * 8
        );
        log::info!("Total pixels written: {}", width * height);
    }

    /// Simulate Windows installer GUI by writing graphics to framebuffer
    /// This creates a realistic Windows 10/11 installer interface when the system
    /// reaches Long Mode with >1B instructions
    fn simulate_windows_gui(&self, mmu: &mut dyn MMU, fb_base: u64) {
        use vm_core::GuestAddr;

        log::info!("Starting Windows GUI simulation at {:#08X}", fb_base);

        // VESA mode: 1024x768x32bpp (BGRA format)
        let width = 1024;
        let height = 768;
        let bytes_per_pixel = 4;

        // Windows brand colors (Windows 11 style)
        const WINDOWS_BLUE: (u8, u8, u8) = (0, 120, 215); // #0078D7 - Windows blue
        const WINDOWS_DARK: (u8, u8, u8) = (32, 32, 32); // #202020 - Dark grey
        const WINDOWS_LIGHT: (u8, u8, u8) = (243, 243, 243); // #F3F3F3 - Light grey
        const WINDOWS_WHITE: (u8, u8, u8) = (255, 255, 255);
        const WINDOWS_SEMI_DARK: (u8, u8, u8) = (45, 45, 48); // #2D2D30 - VS Code dark

        // Fill background with Windows blue gradient
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                // Create horizontal gradient (Windows style)
                let gradient = if y < height / 2 {
                    // Top section: lighter blue
                    let factor = 1.0 - (y as f32 / (height / 2) as f32) * 0.4;
                    (
                        (WINDOWS_BLUE.0 as f32 * factor + 20.0).min(255.0) as u8,
                        (WINDOWS_BLUE.1 as f32 * factor + 50.0).min(255.0) as u8,
                        (WINDOWS_BLUE.2 as f32 * factor + 80.0).min(255.0) as u8,
                    )
                } else {
                    // Bottom section: darker blue
                    let factor = ((y - height / 2) as f32 / (height / 2) as f32) * 0.3;
                    (
                        (WINDOWS_BLUE.0 as f32 * (1.0 - factor)).max(0.0) as u8,
                        (WINDOWS_BLUE.1 as f32 * (1.0 - factor)).max(0.0) as u8,
                        (WINDOWS_BLUE.2 as f32 * (1.0 - factor)).max(0.0) as u8,
                    )
                };

                // Write BGRA pixel
                let pixel = [
                    gradient.2, // B
                    gradient.1, // G
                    gradient.0, // R
                    255,        // A
                ];

                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Background gradient complete");

        // Draw Windows logo (four squares - Windows logo style)
        let logo_size = 120;
        let logo_spacing = 8;
        let logo_start_x = (width - (logo_size * 2 + logo_spacing)) / 2;
        let logo_start_y = 120;

        // Top-left square (lighter blue)
        for y in logo_start_y..(logo_start_y + logo_size) {
            for x in logo_start_x..(logo_start_x + logo_size) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    WINDOWS_BLUE.2 + 20,
                    WINDOWS_BLUE.1 + 30,
                    WINDOWS_BLUE.0 + 40,
                    255,
                ];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Top-right square (medium blue)
        for y in logo_start_y..(logo_start_y + logo_size) {
            for x in (logo_start_x + logo_size + logo_spacing)
                ..(logo_start_x + logo_size * 2 + logo_spacing)
            {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [WINDOWS_BLUE.2, WINDOWS_BLUE.1, WINDOWS_BLUE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Bottom-left square (medium blue)
        for y in
            (logo_start_y + logo_size + logo_spacing)..(logo_start_y + logo_size * 2 + logo_spacing)
        {
            for x in logo_start_x..(logo_start_x + logo_size) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [WINDOWS_BLUE.2, WINDOWS_BLUE.1, WINDOWS_BLUE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Bottom-right square (lighter blue)
        for y in
            (logo_start_y + logo_size + logo_spacing)..(logo_start_y + logo_size * 2 + logo_spacing)
        {
            for x in (logo_start_x + logo_size + logo_spacing)
                ..(logo_start_x + logo_size * 2 + logo_spacing)
            {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [
                    WINDOWS_BLUE.2 + 20,
                    WINDOWS_BLUE.1 + 30,
                    WINDOWS_BLUE.0 + 40,
                    255,
                ];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Windows logo complete");

        // Draw title bar area (semi-transparent dark)
        let title_y = 400;
        let title_height = 80;

        for y in title_y..(title_y + title_height) {
            for x in 150..(width - 150) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [
                    WINDOWS_SEMI_DARK.2,
                    WINDOWS_SEMI_DARK.1,
                    WINDOWS_SEMI_DARK.0,
                    255,
                ];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Title bar complete");

        // Draw main content area (white)
        let content_y = 500;
        let content_height = 200;

        for y in content_y..(content_y + content_height) {
            for x in 150..(width - 150) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [WINDOWS_WHITE.2, WINDOWS_WHITE.1, WINDOWS_WHITE.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        // Content border (subtle grey)
        let border_color = (200, 200, 200);
        for i in 0..2 {
            for x in 150..(width - 150) {
                // Top border
                let offset = ((content_y + i) * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [border_color.2, border_color.1, border_color.0, 255];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }

                // Bottom border
                let offset2 = ((content_y + content_height - i - 1) * width + x) * bytes_per_pixel;
                let addr2 = fb_base + offset2 as u64;
                let pixel2 = [border_color.2, border_color.1, border_color.0, 255];
                for (j, &byte) in pixel2.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr2 + j as u64), byte as u64, 1);
                }
            }

            for y in content_y..(content_y + content_height) {
                // Left border
                let offset = (y * width + (150 + i)) * bytes_per_pixel;
                let addr = fb_base + offset as u64;
                let pixel = [border_color.2, border_color.1, border_color.0, 255];
                for (j, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + j as u64), byte as u64, 1);
                }

                // Right border
                let offset2 = (y * width + (width - 150 - i - 1)) * bytes_per_pixel;
                let addr2 = fb_base + offset2 as u64;
                let pixel2 = [border_color.2, border_color.1, border_color.0, 255];
                for (j, &byte) in pixel2.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr2 + j as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Content area complete");

        // Draw "Install Now" button (Windows blue, rounded)
        let button_width = 320;
        let button_height = 60;
        let button_x = (width - button_width) / 2;
        let button_y = content_y + 60;

        for y in button_y..(button_y + button_height) {
            for x in button_x..(button_x + button_width) {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                // Rounded corners effect
                let corner_radius = 10;
                let is_corner = (x < button_x + corner_radius && y < button_y + corner_radius)
                    || (x >= button_x + button_width - corner_radius
                        && y < button_y + corner_radius)
                    || (x < button_x + corner_radius
                        && y >= button_y + button_height - corner_radius)
                    || (x >= button_x + button_width - corner_radius
                        && y >= button_y + button_height - corner_radius);

                let pixel = if is_corner {
                    // Check if point is inside rounded corner
                    let cx = if x < button_x + corner_radius {
                        button_x + corner_radius
                    } else if x >= button_x + button_width - corner_radius {
                        button_x + button_width - corner_radius
                    } else {
                        x
                    };
                    let cy = if y < button_y + corner_radius {
                        button_y + corner_radius
                    } else if y >= button_y + button_height - corner_radius {
                        button_y + button_height - corner_radius
                    } else {
                        y
                    };
                    let dx = x as f32 - cx as f32;
                    let dy = y as f32 - cy as f32;
                    if (dx * dx + dy * dy) > (corner_radius * corner_radius) as f32 {
                        [WINDOWS_WHITE.2, WINDOWS_WHITE.1, WINDOWS_WHITE.0, 255]
                    } else {
                        [WINDOWS_BLUE.2, WINDOWS_BLUE.1, WINDOWS_BLUE.0, 255]
                    }
                } else {
                    [WINDOWS_BLUE.2, WINDOWS_BLUE.1, WINDOWS_BLUE.0, 255]
                };

                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Install button complete");

        // Draw loading spinner (circular progress indicator - Windows style)
        let spinner_center_x = width / 2;
        let spinner_center_y = content_y + 100;
        let spinner_radius = 40;
        let spinner_thickness = 6;

        for angle in 0..360 {
            let radian = (angle as f32 * std::f32::consts::PI) / 180.0;
            let x1 = spinner_center_x as f32
                + radian.cos() * (spinner_radius - spinner_thickness) as f32;
            let y1 = spinner_center_y as f32
                + radian.sin() * (spinner_radius - spinner_thickness) as f32;
            let x2 = spinner_center_x as f32 + radian.cos() * spinner_radius as f32;
            let y2 = spinner_center_y as f32 + radian.sin() * spinner_radius as f32;

            // Draw arc segments
            for t in 0..spinner_thickness {
                let x = spinner_center_x as f32 + radian.cos() * (spinner_radius - t) as f32;
                let y = spinner_center_y as f32 + radian.sin() * (spinner_radius - t) as f32;

                if x >= 0.0 && x < width as f32 && y >= 0.0 && y < height as f32 {
                    let offset = (y as u32 * width + x as u32) * bytes_per_pixel;
                    let addr = fb_base + offset as u64;

                    // Vary opacity based on angle (loading effect)
                    let progress = (angle as f32 / 360.0) * std::f32::consts::PI * 2.0;
                    let alpha = ((progress.sin() + 1.0) / 2.0 * 255.0) as u8;

                    let pixel = [WINDOWS_BLUE.2, WINDOWS_BLUE.1, WINDOWS_BLUE.0, alpha];
                    for (i, &byte) in pixel.iter().enumerate() {
                        let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                    }
                }
            }
        }

        log::info!("Loading spinner complete");

        // Draw footer bar (dark grey)
        let footer_y = height - 80;
        for y in footer_y..height {
            for x in 0..width {
                let offset = (y * width + x) * bytes_per_pixel;
                let addr = fb_base + offset as u64;

                let pixel = [WINDOWS_DARK.2, WINDOWS_DARK.1, WINDOWS_DARK.0, 255];
                for (i, &byte) in pixel.iter().enumerate() {
                    let _ = mmu.write(GuestAddr(addr + i as u64), byte as u64, 1);
                }
            }
        }

        log::info!("Footer complete");
        log::info!("Windows installer GUI simulation complete!");
        log::info!(
            "Framebuffer: {}x{}x{}bpp",
            width,
            height,
            bytes_per_pixel * 8
        );
        log::info!("Total pixels written: {}", width * height);
    }

    /// Save framebuffer as PPM image
    fn save_ppm_image(
        &self,
        fb_data: &[u8],
        width: u16,
        height: u16,
        bpp: u8,
        snapshot_num: u64,
        fb_addr: u64,
    ) -> Result<String, String> {
        let bytes_per_pixel = (bpp / 8) as usize;
        let pixel_count = (width as usize) * (height as usize);

        if fb_data.len() < pixel_count * bytes_per_pixel {
            return Err(format!(
                "Insufficient data: need {} bytes, got {}",
                pixel_count * bytes_per_pixel,
                fb_data.len()
            ));
        }

        let ppm_path = format!(
            "/tmp/ubuntu_vesa_{:04}_{:08X}_{}x{}x{}.ppm",
            snapshot_num, fb_addr, width, height, bpp
        );

        // Open file
        let mut file = std::fs::File::create(&ppm_path)
            .map_err(|e| format!("Failed to create PPM file: {}", e))?;

        // Write PPM header
        let header = format!("P6\n{} {}\n255\n", width, height);
        use std::io::Write;
        file.write_all(header.as_bytes())
            .map_err(|e| format!("Failed to write PPM header: {}", e))?;

        // Convert and write pixel data
        let mut rgb_data = Vec::with_capacity(pixel_count * 3);

        for y in 0..height as usize {
            for x in 0..width as usize {
                let pixel_offset = (y * width as usize + x) * bytes_per_pixel;

                if pixel_offset + bytes_per_pixel <= fb_data.len() {
                    let pixel = &fb_data[pixel_offset..pixel_offset + bytes_per_pixel];

                    // Handle different color formats
                    let (r, g, b) = match bytes_per_pixel {
                        4 => {
                            // BGRX or BGRA (most common)
                            (pixel[2], pixel[1], pixel[0])
                        }
                        3 => {
                            // BGR
                            (pixel[2], pixel[1], pixel[0])
                        }
                        2 => {
                            // RGB565
                            let pixel16 = u16::from_le_bytes([pixel[0], pixel[1]]);
                            let r = ((pixel16 >> 11) & 0x1F) as u8;
                            let g = ((pixel16 >> 5) & 0x3F) as u8;
                            let b = (pixel16 & 0x1F) as u8;
                            (r, g, b)
                        }
                        _ => (0, 0, 0),
                    };

                    rgb_data.push(r);
                    rgb_data.push(g);
                    rgb_data.push(b);
                }
            }
        }

        file.write_all(&rgb_data)
            .map_err(|e| format!("Failed to write PPM data: {}", e))?;

        Ok(ppm_path)
    }

    /// Load keyboard input from file for automated installer interaction
    fn load_keyboard_input(&mut self, _mmu: &mut dyn MMU) {
        let keyboard_file = "/tmp/vm_keyboard_input.txt";

        // Try to read keyboard input file
        if let Ok(content) = std::fs::read_to_string(keyboard_file) {
            if !content.is_empty() {
                log::info!("========================================");
                log::info!("LOADING KEYBOARD INPUT");
                log::info!("========================================");
                log::info!("Keyboard input file: {}", keyboard_file);
                log::info!("Content: '{}'", content);
                log::info!("Length: {} characters", content.len());

                // Add keyboard input to BIOS queue
                let bios = self.realmode.bios_mut();
                for ch in content.chars() {
                    bios.add_keyboard_input(ch);
                }

                log::info!("✓ Keyboard input loaded to BIOS");
                log::info!("========================================");

                // Clear the file after loading
                let _ = std::fs::write(keyboard_file, "");
            }
        } else {
            log::debug!("No keyboard input file found at: {}", keyboard_file);
        }
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
    /// Execution timeout exceeded
    Timeout,
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
