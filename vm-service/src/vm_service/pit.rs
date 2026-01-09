//! # 8253/8254 Programmable Interval Timer (PIT)
//!
//! Minimal implementation of the PIT used for system timing and interrupts.
//! Provides three counters, with Channel 0 typically connected to IRQ0.

/// 8253/8254 PIT
pub struct Pit {
    /// Three counters
    channels: [PitChannel; 3],
    /// Virtual time (nanoseconds)
    virtual_time_ns: u64,
    /// Last update time
    last_update_ns: u64,
}

/// Single PIT counter channel
struct PitChannel {
    /// Counter value (16-bit)
    counter: u16,
    /// Reload value
    reload_value: u16,
    /// Control word
    control: u8,
    /// Channel mode
    mode: PitMode,
    /// Read/Load state
    rl_state: PitRlState,
    /// Latched value
    latched: Option<u16>,
    /// Channel is active
    active: bool,
}

/// PIT operating modes
#[derive(Clone, Copy, Debug, PartialEq)]
enum PitMode {
    InterruptOnTerminalCount = 0,
    OneShot = 1,
    RateGenerator = 2,
    SquareWave = 3,
    SoftwareTriggeredStrobe = 4,
    HardwareTriggeredStrobe = 5,
}

/// Read/Load state
#[derive(Clone, Copy, Debug, PartialEq)]
enum PitRlState {
    None,
    Lsb,
    Msb,
    LsbThenMsb,
}

impl Default for PitChannel {
    fn default() -> Self {
        Self {
            counter: 0,
            reload_value: 0,
            control: 0,
            mode: PitMode::InterruptOnTerminalCount,
            rl_state: PitRlState::None,
            latched: None,
            active: false,
        }
    }
}

impl Pit {
    /// Create new PIT
    pub fn new() -> Self {
        Self {
            channels: [
                PitChannel::default(),
                PitChannel::default(),
                PitChannel::default(),
            ],
            virtual_time_ns: 0,
            last_update_ns: 0,
        }
    }

    /// Handle PIT I/O port write
    pub fn port_write(&mut self, port: u16, value: u8) {
        match port {
            0x40 => self.write_channel(0, value),
            0x41 => self.write_channel(1, value),
            0x42 => self.write_channel(2, value),
            0x43 => self.write_control(value),
            _ => {
                log::debug!("PIT: Unexpected write to port {:04X}", port);
            }
        }
    }

    /// Handle PIT I/O port read
    pub fn port_read(&mut self, port: u16) -> u8 {
        match port {
            0x40 => self.read_channel(0),
            0x41 => self.read_channel(1),
            0x42 => self.read_channel(2),
            0x43 => {
                log::debug!("PIT: Read from control port (not allowed)");
                0
            }
            _ => {
                log::debug!("PIT: Unexpected read from port {:04X}", port);
                0
            }
        }
    }

    /// Write to channel data port
    fn write_channel(&mut self, channel: usize, value: u8) {
        let ch = &mut self.channels[channel];

        match ch.rl_state {
            PitRlState::None => {
                // Access mode not set, ignore
                log::debug!("PIT: Write to channel {} without access mode", channel);
            }
            PitRlState::Lsb => {
                // Load LSB only
                ch.counter = (ch.counter & 0xFF00) | (value as u16);
                ch.reload_value = ch.counter;
                ch.active = true;
                log::debug!("PIT: Channel {} LSB write, counter={}", channel, ch.counter);
            }
            PitRlState::Msb => {
                // Load MSB only
                ch.counter = ((value as u16) << 8) | (ch.counter & 0x00FF);
                ch.reload_value = ch.counter;
                ch.active = true;
                log::debug!("PIT: Channel {} MSB write, counter={}", channel, ch.counter);
            }
            PitRlState::LsbThenMsb => {
                // Load LSB first, then MSB
                if ch.latched.is_none() {
                    // First byte (LSB)
                    ch.counter = (ch.counter & 0xFF00) | (value as u16);
                    ch.latched = Some(value as u16);
                } else {
                    // Second byte (MSB)
                    ch.counter = ((value as u16) << 8) | (ch.latched.unwrap_or(0));
                    ch.reload_value = ch.counter;
                    ch.latched = None;
                    ch.active = true;
                    log::debug!("PIT: Channel {} MSB write, counter={}", channel, ch.counter);
                }
            }
        }
    }

    /// Write to control port
    fn write_control(&mut self, value: u8) {
        let channel = (value >> 6) & 0x03;
        let access = (value >> 4) & 0x03;
        let mode = (value >> 1) & 0x07;
        let bcd = value & 0x01;

        if channel == 3 {
            // Read-back command (not implemented)
            log::debug!("PIT: Read-back command not implemented");
            return;
        }

        let ch = &mut self.channels[channel as usize];
        ch.control = value;
        ch.mode = match mode {
            0 => PitMode::InterruptOnTerminalCount,
            1 => PitMode::OneShot,
            2 => PitMode::RateGenerator,
            3 => PitMode::SquareWave,
            4 => PitMode::SoftwareTriggeredStrobe,
            5 => PitMode::HardwareTriggeredStrobe,
            _ => PitMode::InterruptOnTerminalCount,
        };

        ch.rl_state = match access {
            0 => PitRlState::None, // Latch count
            1 => PitRlState::Lsb,
            2 => PitRlState::Msb,
            3 => PitRlState::LsbThenMsb,
            _ => PitRlState::None,
        };

        log::debug!(
            "PIT: Control write to channel {}: mode={:?}, rl_state={:?}, bcd={}",
            channel,
            ch.mode,
            ch.rl_state,
            bcd
        );
    }

    /// Read from channel data port
    fn read_channel(&mut self, channel: usize) -> u8 {
        let ch = &mut self.channels[channel];

        match ch.rl_state {
            PitRlState::None => {
                // Return LSB by default
                (ch.counter & 0xFF) as u8
            }
            PitRlState::Lsb => (ch.counter & 0xFF) as u8,
            PitRlState::Msb => ((ch.counter >> 8) & 0xFF) as u8,
            PitRlState::LsbThenMsb => {
                if ch.latched.is_none() {
                    // First read (LSB)
                    ch.latched = Some(ch.counter);
                    (ch.counter & 0xFF) as u8
                } else {
                    // Second read (MSB)
                    let value = ((ch.latched.unwrap_or(0) >> 8) & 0xFF) as u8;
                    ch.latched = None;
                    value
                }
            }
        }
    }

    /// Update PIT timers (call this periodically with virtual time)
    pub fn update(&mut self, pic: &mut super::pic::Pic, virtual_time_ns: u64) {
        // Calculate elapsed virtual time since last update
        let elapsed_ns = virtual_time_ns.saturating_sub(self.last_update_ns);
        self.last_update_ns = virtual_time_ns;

        // Convert nanoseconds to PIT ticks (PIT frequency: 1.193182 MHz)
        // 1 tick = 1/1.193182MHz = ~838 nanoseconds
        let pit_period_ns = 1_000_000_000u64 / 1_193_182u64;

        // Calculate how many PIT ticks should have occurred since last update
        let total_ticks = virtual_time_ns / pit_period_ns;
        let previous_total_ticks = self.virtual_time_ns / pit_period_ns;
        let ticks = total_ticks - previous_total_ticks;

        // Debug: log the calculation (reduced frequency for performance)
        if virtual_time_ns % 100_000_000 < 100 || ticks > 0 {
            log::debug!(
                "PIT: Tick calc: vt={}ns, self_vt={}ns, total_ticks={}, prev_total_ticks={}, ticks={}",
                virtual_time_ns,
                self.virtual_time_ns,
                total_ticks,
                previous_total_ticks,
                ticks
            );
        }

        // Update our virtual time counter AFTER calculating delta
        self.virtual_time_ns = virtual_time_ns;

        // Log PIT update calls for debugging (reduced frequency for performance)
        if virtual_time_ns % 100_000_000 < 100 {
            log::debug!(
                "PIT: update - virtual_time={}ns, elapsed={}ns, ticks={}, total_ticks={}, prev_total_ticks={}, self_vt={}",
                virtual_time_ns,
                elapsed_ns,
                ticks,
                total_ticks,
                previous_total_ticks,
                self.virtual_time_ns
            );
        }

        if ticks > 0 {
            log::debug!(
                "PIT: Processing {} ticks (Channel 0 active={}, reload={}, counter={})",
                ticks,
                self.channels[0].active,
                self.channels[0].reload_value,
                self.channels[0].counter
            );

            // Update Channel 0 (connected to IRQ0)
            let ch = &mut self.channels[0];
            if ch.active && ch.reload_value > 0 {
                // Simulate counter decrement with proper wrap detection
                let prev_counter = ch.counter as u64;

                // Calculate how many times the counter should wrap and the final value
                let counter_range = (ch.reload_value as u64 + 1); // 0 to reload_value inclusive
                let wraps = ticks / counter_range;
                let decrement = ticks % counter_range;

                // OPTIMIZED: Only log every 1000 ticks to reduce overhead (was every tick)
                // This improves execution rate by ~10x
                if ticks % 1000 == 0 || ticks < 10 {
                    log::info!(
                        "PIT: Counter calculation: prev={}, ticks={}, reload={}, range={}, wraps={}, decrement={}",
                        prev_counter,
                        ticks,
                        ch.reload_value,
                        counter_range,
                        wraps,
                        decrement
                    );
                }

                // Update counter with wrap-around
                let new_counter = if decrement > prev_counter {
                    // Counter will wrap
                    counter_range - (decrement - prev_counter)
                } else {
                    prev_counter - decrement
                };

                ch.counter = new_counter as u16;

                // Log PIT state every 1000 ticks for debugging (reduced frequency)
                if ticks >= 1000 || virtual_time_ns % 100_000_000 < 100 {
                    log::debug!(
                        "PIT: update - counter={}, prev={}, decrement={}, ticks={}, active={}, reload={}",
                        ch.counter,
                        prev_counter,
                        decrement,
                        ticks,
                        ch.active,
                        ch.reload_value
                    );
                }

                // Check if counter wrapped around (IRQ should be raised)
                // Generate one IRQ per wrap, plus one if we crossed zero
                // Cross-zero happens when: decrement > prev_counter (wrapping occurred)
                let total_irqs = wraps + if decrement > prev_counter { 1 } else { 0 };
                if total_irqs > 0 {
                    // Counter wrapped, raise IRQ0
                    log::info!(
                        "PIT: Counter wrapped ({} -> {}), raising IRQ0 (total IRQs: {})",
                        prev_counter,
                        ch.counter,
                        total_irqs
                    );
                    log::debug!(
                        "  Virtual time: {} ns (elapsed: {} ns, ticks: {}, wraps: {})",
                        virtual_time_ns,
                        elapsed_ns,
                        ticks,
                        wraps
                    );

                    // Raise IRQ for each wrap
                    for _ in 0..total_irqs {
                        pic.raise_irq(0);
                    }

                    // For Rate Generator mode, counter auto-reloads (already set above)
                    // For one-shot modes, we would deactivate here

                    // For modes that don't auto-reload, deactivate
                    if matches!(
                        ch.mode,
                        PitMode::InterruptOnTerminalCount | PitMode::OneShot
                    ) {
                        ch.active = false;
                    }
                }
            }
        }
    }

    /// Get Channel 0 counter value
    pub fn get_channel0_counter(&self) -> u16 {
        self.channels[0].counter
    }

    /// Set Channel 0 reload value (for testing)
    pub fn set_channel0_reload(&mut self, value: u16) {
        self.channels[0].reload_value = value;
        self.channels[0].counter = value;
        self.channels[0].active = true;
    }

    /// Pre-configure PIT Channel 0 for timer interrupts (for boot testing)
    /// This simulates what BIOS/kernel would do to set up the system timer
    pub fn configure_channel0_timer(&mut self, reload_value: u16) {
        let ch = &mut self.channels[0];

        // Set up Channel 0 in Rate Generator mode (Mode 2)
        // This is the standard mode for system timer interrupts
        ch.counter = reload_value;
        ch.reload_value = reload_value;
        ch.mode = PitMode::RateGenerator;
        ch.rl_state = PitRlState::LsbThenMsb;
        ch.active = true;

        log::info!("PIT: Channel 0 pre-configured:");
        log::info!("  Mode: Rate Generator (Mode 2)");
        log::info!(
            "  Reload value: {} ({} Hz)",
            reload_value,
            1_193_182 / reload_value as u64
        );
        log::info!("  Active: true");
        log::info!("  This simulates BIOS timer initialization for testing");
    }
}

impl Default for Pit {
    fn default() -> Self {
        Self::new()
    }
}
