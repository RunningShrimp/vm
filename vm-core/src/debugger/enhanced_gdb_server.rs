//! Enhanced GDB remote debugging server
//!
//! This module provides an enhanced GDB remote debugging server that integrates
//! with the unified debugger interface for comprehensive debugging capabilities.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::SystemTime;

use crate::{GuestAddr, VmError, VmResult};
use super::unified_debugger::{
    UnifiedDebugger, UnifiedDebuggerConfig
};
use super::enhanced_breakpoints::{BreakpointType, BreakpointCondition};
use crate::debugger::call_stack_tracker::VariableValue;

/// Enhanced GDB server configuration
#[derive(Debug, Clone)]
pub struct EnhancedGdbServerConfig {
    /// Server port
    pub port: u16,
    /// Enable verbose logging
    pub verbose: bool,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Enable packet logging
    pub enable_packet_logging: bool,
    /// Enable multi-threading support
    pub enable_multi_threading: bool,
    /// Enable source-level debugging
    pub enable_source_level_debugging: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
}

impl Default for EnhancedGdbServerConfig {
    fn default() -> Self {
        Self {
            port: 1234,
            verbose: false,
            max_connections: 10,
            connection_timeout: 30,
            enable_packet_logging: false,
            enable_multi_threading: true,
            enable_source_level_debugging: true,
            enable_performance_monitoring: true,
        }
    }
}

/// GDB response
#[derive(Debug, Clone)]
pub enum GdbResponse {
    /// OK response
    Ok,
    /// Error response
    Error(String),
    /// Data response
    Data(String),
    /// Signal response
    Signal(GdbSignal),
    /// Stop response
    Stop(GdbStopReason),
}

/// GDB packet
#[derive(Debug, Clone)]
pub struct GdbPacket {
    /// Packet data
    pub data: Vec<u8>,
    /// Packet checksum
    pub checksum: u8,
}

/// GDB error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbErrorCode {
    /// No error
    None,
    /// Invalid command
    InvalidCommand,
    /// Invalid argument
    InvalidArgument,
    /// Memory access error
    MemoryError,
    /// Register access error
    RegisterError,
    /// Breakpoint error
    BreakpointError,
    /// Thread error
    ThreadError,
}

/// GDB signal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbSignal {
    /// SIGHUP
    SIGHUP = 1,
    /// SIGINT
    SIGINT = 2,
    /// SIGQUIT
    SIGQUIT = 3,
    /// SIGILL
    SIGILL = 4,
    /// SIGTRAP
    SIGTRAP = 5,
    /// SIGABRT
    SIGABRT = 6,
    /// SIGBUS
    SIGBUS = 7,
    /// SIGFPE
    SIGFPE = 8,
    /// SIGKILL
    SIGKILL = 9,
    /// SIGUSR1
    SIGUSR1 = 10,
    /// SIGSEGV
    SIGSEGV = 11,
    /// SIGUSR2
    SIGUSR2 = 12,
    /// SIGPIPE
    SIGPIPE = 13,
    /// SIGALRM
    SIGALRM = 14,
    /// SIGTERM
    SIGTERM = 15,
    /// SIGSTKFLT
    SIGSTKFLT = 16,
    /// SIGCHLD
    SIGCHLD = 17,
    /// SIGCONT
    SIGCONT = 18,
    /// SIGSTOP
    SIGSTOP = 19,
    /// SIGTSTP
    SIGTSTP = 20,
    /// SIGTTIN
    SIGTTIN = 21,
    /// SIGTTOU
    SIGTTOU = 22,
    /// SIGURG
    SIGURG = 23,
    /// SIGXCPU
    SIGXCPU = 24,
    /// SIGXFSZ
    SIGXFSZ = 25,
    /// SIGVTALRM
    SIGVTALRM = 26,
    /// SIGPROF
    SIGPROF = 27,
    /// SIGWINCH
    SIGWINCH = 28,
    /// SIGIO
    SIGIO = 29,
    /// SIGPWR
    SIGPWR = 30,
    /// SIGSYS
    SIGSYS = 31,
}

/// GDB stop reason
#[derive(Debug, Clone)]
pub enum GdbStopReason {
    /// Signal received
    Signal(GdbSignal),
    /// Breakpoint hit
    Breakpoint(u64),
    /// Watchpoint hit
    Watchpoint(u64),
    /// Step completed
    Step,
    /// Exception occurred
    Exception(String),
}

/// GDB register information
#[derive(Debug, Clone)]
pub struct GdbRegisterInfo {
    /// Register number
    pub number: u32,
    /// Register name
    pub name: String,
    /// Register size in bits
    pub size: u32,
    /// Register offset in register buffer
    pub offset: u32,
    /// Register type
    pub register_type: String,
}

/// GDB thread information
#[derive(Debug, Clone)]
pub struct GdbThreadInfo {
    /// Thread ID
    pub thread_id: u32,
    /// Thread name
    pub name: String,
    /// Thread state
    pub state: String,
    /// Thread priority
    pub priority: u32,
}

/// GDB memory map
#[derive(Debug, Clone)]
pub struct GdbMemoryMap {
    /// Memory regions
    pub regions: Vec<GdbMemoryRegion>,
}

/// GDB memory region
#[derive(Debug, Clone)]
pub struct GdbMemoryRegion {
    /// Region start address
    pub start: GuestAddr,
    /// Region end address
    pub end: GuestAddr,
    /// Region permissions
    pub permissions: String,
    /// Region type
    pub region_type: String,
}

/// GDB breakpoint information
#[derive(Debug, Clone)]
pub struct GdbBreakpointInfo {
    /// Breakpoint number
    pub number: u64,
    /// Breakpoint address
    pub address: GuestAddr,
    /// Breakpoint type
    pub breakpoint_type: String,
    /// Breakpoint enabled
    pub enabled: bool,
    /// Breakpoint hit count
    pub hit_count: u64,
}

/// GDB watchpoint information
#[derive(Debug, Clone)]
pub struct GdbWatchpointInfo {
    /// Watchpoint number
    pub number: u64,
    /// Watchpoint address
    pub address: GuestAddr,
    /// Watchpoint size
    pub size: u32,
    /// Watchpoint type
    pub watchpoint_type: String,
    /// Watchpoint enabled
    pub enabled: bool,
    /// Watchpoint hit count
    pub hit_count: u64,
}

/// GDB server statistics
#[derive(Debug, Clone, Default)]
pub struct GdbServerStatistics {
    /// Total connections
    pub total_connections: u64,
    /// Active connections
    pub active_connections: u32,
    /// Total commands processed
    pub total_commands: u64,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
}

/// GDB connection statistics
#[derive(Debug, Clone, Default)]
pub struct GdbConnectionStats {
    /// Connection ID
    pub connection_id: String,
    /// Remote address
    pub remote_address: String,
    /// Connection start time
    pub start_time: std::time::SystemTime,
    /// Commands processed
    pub commands_processed: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Last activity time
    pub last_activity: std::time::SystemTime,
}

/// Enhanced GDB server
/// 
/// This provides a comprehensive GDB remote debugging server that integrates
/// with the unified debugger interface to provide advanced debugging capabilities.
pub struct EnhancedGdbServer {
    /// Configuration
    config: EnhancedGdbServerConfig,
    /// Unified debugger
    debugger: Arc<UnifiedDebugger>,
    /// Server state
    running: Arc<RwLock<bool>>,
    /// Active connections
    connections: Arc<RwLock<HashMap<String, GdbConnection>>>,
    /// Server listener
    listener: Arc<RwLock<Option<TcpListener>>>,
    /// Connection ID counter
    next_connection_id: Arc<RwLock<u64>>,
}

/// GDB connection information
#[derive(Debug)]
struct GdbConnection {
    /// Connection ID
    id: String,
    /// TCP stream
    stream: TcpStream,
    /// Connection state
    state: GdbConnectionState,
    /// Current thread ID being debugged
    current_thread: Option<u32>,
    /// Last activity timestamp
    last_activity: SystemTime,
    /// Packet buffer
    packet_buffer: Vec<u8>,
    /// Acknowledgment number
    ack_number: u8,
}

/// GDB connection states
#[derive(Debug, Clone, Copy, PartialEq)]
enum GdbConnectionState {
    /// Connection is being established
    Connecting,
    /// Waiting for command
    Waiting,
    /// Processing command
    Processing,
    /// Connection is closing
    Closing,
    /// Connection is closed
    Closed,
}

/// GDB packet types
#[derive(Debug, Clone, Copy)]
enum GdbPacketType {
    /// Command packet
    Command,
    /// Acknowledgment packet
    Ack,
    /// Not acknowledgment packet
    Nack,
    /// Error packet
    Error,
    /// Status packet
    Status,
    /// Output packet
    Output,
}

/// GDB command types
#[derive(Debug, Clone)]
enum GdbCommand {
    /// Query command
    Query(String),
    /// Set command
    Set(String),
    /// Read registers command
    ReadRegisters,
    /// Write registers command
    WriteRegisters(Vec<u8>),
    /// Read memory command
    ReadMemory { address: GuestAddr, length: usize },
    /// Write memory command
    WriteMemory { address: GuestAddr, data: Vec<u8> },
    /// Continue command
    Continue { signal: Option<u8> },
    /// Step command
    Step { signal: Option<u8> },
    /// Breakpoint command
    Breakpoint { action: BreakpointAction, address: GuestAddr, kind: Option<String> },
    /// Kill command
    Kill,
    /// Detach command
    Detach,
    /// Unknown command
    Unknown(String),
}

/// Breakpoint actions
#[derive(Debug, Clone)]
enum BreakpointAction {
    /// Set breakpoint
    Set,
    /// Remove breakpoint
    Remove,
    /// Query breakpoints
    Query,
}

impl EnhancedGdbServer {
    /// Create a new enhanced GDB server
    pub fn new(
        config: EnhancedGdbServerConfig,
        debugger_config: UnifiedDebuggerConfig,
    ) -> VmResult<Self> {
        let debugger = Arc::new(UnifiedDebugger::new(debugger_config)?);

        Ok(Self {
            config,
            debugger,
            running: Arc::new(RwLock::new(false)),
            connections: Arc::new(RwLock::new(HashMap::new())),
            listener: Arc::new(RwLock::new(None)),
            next_connection_id: Arc::new(RwLock::new(1)),
        })
    }

    /// Helper: Lock running for reading
    fn lock_running(&self) -> VmResult<std::sync::RwLockReadGuard<'_, bool>> {
        self.running.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on running".to_string(),
            operation: "lock_running".to_string(),
        })
    }

    /// Helper: Lock running for writing
    fn lock_running_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, bool>> {
        self.running.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on running".to_string(),
            operation: "lock_running_mut".to_string(),
        })
    }

    /// Helper: Lock connections for reading
    fn lock_connections(&self) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<String, GdbConnection>>> {
        self.connections.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on connections".to_string(),
            operation: "lock_connections".to_string(),
        })
    }

    /// Helper: Lock connections for writing
    fn lock_connections_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<String, GdbConnection>>> {
        self.connections.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on connections".to_string(),
            operation: "lock_connections_mut".to_string(),
        })
    }

    /// Helper: Lock listener for reading
    fn lock_listener(&self) -> VmResult<std::sync::RwLockReadGuard<'_, Option<TcpListener>>> {
        self.listener.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on listener".to_string(),
            operation: "lock_listener".to_string(),
        })
    }

    /// Helper: Lock listener for writing
    fn lock_listener_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, Option<TcpListener>>> {
        self.listener.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on listener".to_string(),
            operation: "lock_listener_mut".to_string(),
        })
    }

    /// Helper: Lock next_connection_id for writing
    fn lock_next_connection_id_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, u64>> {
        self.next_connection_id.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on next_connection_id".to_string(),
            operation: "lock_next_connection_id_mut".to_string(),
        })
    }

    /// Start the GDB server
    pub fn start(&self) -> VmResult<()> {
        // Create TCP listener
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.config.port))
            .map_err(|e| VmError::Io(format!("Failed to bind to port {}: {}", self.config.port, e)))?;

        // Store listener
        {
            let mut listener_ref = self.lock_listener_mut()?;
            *listener_ref = Some(listener);
        }

        // Set running state
        {
            let mut running = self.lock_running_mut()?;
            *running = true;
        }

        // Start debugger
        self.debugger.start()?;

        // Start accepting connections
        let server = self.clone();
        thread::spawn(move || {
            server.accept_connections();
        });

        if self.config.verbose {
            println!("Enhanced GDB server started on port {}", self.config.port);
        }

        Ok(())
    }

    /// Stop the GDB server
    pub fn stop(&self) -> VmResult<()> {
        // Set running state to false
        {
            let mut running = self.lock_running_mut()?;
            *running = false;
        }

        // Close all connections
        {
            let mut connections = self.lock_connections_mut()?;
            for (_, connection) in connections.iter() {
                let _ = connection.stream.shutdown(std::net::Shutdown::Both);
            }
            connections.clear();
        }

        // Close listener
        {
            let mut listener = self.lock_listener_mut()?;
            if let Some(ref listener) = *listener {
                drop(listener);
            }
            *listener = None;
        }

        // Stop debugger
        self.debugger.stop("GDB server stopped".to_string())?;

        if self.config.verbose {
            println!("Enhanced GDB server stopped");
        }

        Ok(())
    }

    /// Accept incoming connections
    fn accept_connections(&self) {
        let running = match self.lock_running() {
            Ok(r) => *r,
            Err(_) => return,
        };

        while running {
            let listener_opt = match self.lock_listener() {
                Ok(l) => l.clone(),
                Err(_) => break,
            };

            if let Some(ref listener) = listener_opt {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        if self.config.verbose {
                            println!("New connection from {}", addr);
                        }

                        // Check connection limit
                        {
                            let connections = match self.lock_connections() {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            if connections.len() >= self.config.max_connections {
                                if self.config.verbose {
                                    println!("Connection limit reached, rejecting connection from {}", addr);
                                }
                                let _ = stream.shutdown(std::net::Shutdown::Both);
                                continue;
                            }
                        }

                        // Create connection
                        let connection_id = match self.generate_connection_id() {
                            Ok(id) => id,
                            Err(_) => continue,
                        };
                        let connection = GdbConnection {
                            id: connection_id.clone(),
                            stream,
                            state: GdbConnectionState::Connecting,
                            current_thread: None,
                            last_activity: SystemTime::now(),
                            packet_buffer: Vec::new(),
                            ack_number: 0,
                        };

                        // Add to connections
                        {
                            if let Ok(mut connections) = self.connections.write() {
                                connections.insert(connection_id.clone(), connection);
                            } else {
                                continue;
                            }
                        }

                        // Handle connection
                        let server = self.clone();
                        thread::spawn(move || {
                            server.handle_connection(connection_id);
                        });
                    }
                    Err(e) => {
                        if self.config.verbose {
                            println!("Error accepting connection: {}", e);
                        }
                    }
                }
            } else {
                // Listener was closed
                break;
            }
        }
    }

    /// Handle a GDB connection
    fn handle_connection(&self, connection_id: String) {
        // Get connection
        let connection = {
            let connections = match self.lock_connections() {
                Ok(c) => c,
                Err(_) => return,
            };
            connections.get(&connection_id).cloned()
        };

        if let Some(mut conn) = connection {
            // Send acknowledgment
            if let Err(e) = self.send_packet(&mut conn, b"+") {
                if self.config.verbose {
                    println!("Error sending ack to {}: {}", connection_id, e);
                }
                return;
            }

            // Set connection state
            conn.state = GdbConnectionState::Waiting;

            // Handle commands
            let running = match self.lock_running() {
                Ok(r) => *r,
                Err(_) => false,
            };

            while running && conn.state != GdbConnectionState::Closed {
                match self.receive_packet(&mut conn) {
                    Ok(Some(packet)) => {
                        if self.config.enable_packet_logging {
                            println!("Received packet from {}: {}", connection_id, packet);
                        }

                        // Process command
                        if let Err(e) = self.process_command(&mut conn, &packet) {
                            if self.config.verbose {
                                println!("Error processing command from {}: {}", connection_id, e);
                            }
                            // Send error response
                            let _ = self.send_packet(&mut conn, format!("E{}", e).as_bytes());
                        }
                    }
                    Ok(None) => {
                        // Connection closed
                        conn.state = GdbConnectionState::Closed;
                        break;
                    }
                    Err(e) => {
                        if self.config.verbose {
                            println!("Error receiving packet from {}: {}", connection_id, e);
                        }
                        conn.state = GdbConnectionState::Closed;
                        break;
                    }
                }
            }

            // Remove connection
            if let Ok(mut connections) = self.connections.write() {
                connections.remove(&connection_id);
            }

            if self.config.verbose {
                println!("Connection {} closed", connection_id);
            }
        }
    }

    /// Process a GDB command
    fn process_command(&self, conn: &mut GdbConnection, packet: &str) -> VmResult<()> {
        let command = self.parse_command(packet)?;

        match command {
            GdbCommand::Query(query) => {
                self.handle_query(conn, &query)?;
            }
            GdbCommand::Set(set_cmd) => {
                self.handle_set(conn, &set_cmd)?;
            }
            GdbCommand::ReadRegisters => {
                self.handle_read_registers(conn)?;
            }
            GdbCommand::WriteRegisters(data) => {
                self.handle_write_registers(conn, data)?;
            }
            GdbCommand::ReadMemory { address, length } => {
                self.handle_read_memory(conn, address, length)?;
            }
            GdbCommand::WriteMemory { address, data } => {
                self.handle_write_memory(conn, address, &data)?;
            }
            GdbCommand::Continue { signal } => {
                self.handle_continue(conn, signal)?;
            }
            GdbCommand::Step { signal } => {
                self.handle_step(conn, signal)?;
            }
            GdbCommand::Breakpoint { action, address, kind } => {
                self.handle_breakpoint(conn, action, address, kind)?;
            }
            GdbCommand::Kill => {
                self.handle_kill(conn)?;
            }
            GdbCommand::Detach => {
                self.handle_detach(conn)?;
            }
            GdbCommand::Unknown(cmd) => {
                if self.config.verbose {
                    println!("Unknown command: {}", cmd);
                }
                // Send empty response for unknown commands
                let _ = self.send_packet(conn, b"");
            }
        }

        Ok(())
    }

    /// Handle query commands
    fn handle_query(&self, conn: &mut GdbConnection, query: &str) -> VmResult<()> {
        match query {
            "Supported" => {
                // Send supported features
                let response = "PacketSize=1000;QStartNoAckMode+;qRelocInsn+;qXfer:features:read+;qXfer:exec-file:read+;qXfer:auxv:read+;qXfer:libraries-svr4-read+;qXfer:memory-map:read+;qXfer:siginfo-read+;qXfer:symbols-read+;qXfer:threads-inferior1+";
                let _ = self.send_packet(conn, response.as_bytes());
            }
            "Attached" => {
                // Check if any process is attached
                let response = "1"; // Yes, attached
                let _ = self.send_packet(conn, response.as_bytes());
            }
            "C" => {
                // Current thread
                if let Some(thread_id) = conn.current_thread {
                    let response = format!("QC{}", thread_id);
                    let _ = self.send_packet(conn, response.as_bytes());
                } else {
                    let response = "QC0"; // No current thread
                    let _ = self.send_packet(conn, response.as_bytes());
                }
            }
            "fThreadInfo" => {
                // Thread information
                let threads = self.debugger.multi_thread_debugger.get_all_threads()?;
                let mut response = String::new();
                
                for (i, thread) in threads.iter().enumerate() {
                    if i > 0 {
                        response.push(',');
                    }
                    response.push_str(&format!(
                        "{:x},{:x},{:x},{:x}",
                        thread.thread_id,
                        thread.native_thread_id.as_u64().get().unwrap_or(0),
                        thread.created_at.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
                        thread.stack_pointer,
                        thread.frame_pointer.unwrap_or(0),
                        thread.instruction_pointer
                    ));
                }
                
                let _ = self.send_packet(conn, response.as_bytes());
            }
            "Symbol::" => {
                // Symbol lookup (would need to parse symbol name)
                let response = ""; // Empty response for now
                let _ = self.send_packet(conn, response.as_bytes());
            }
            _ => {
                // Unknown query
                let response = "";
                let _ = self.send_packet(conn, response.as_bytes());
            }
        }

        Ok(())
    }

    /// Handle set commands
    fn handle_set(&self, conn: &mut GdbConnection, set_cmd: &str) -> VmResult<()> {
        match set_cmd {
            cmd if cmd.starts_with("Breakpoint:") => {
                // Parse breakpoint command
                let parts: Vec<&str> = cmd.split(':').collect();
                if parts.len() >= 3 {
                    let action = match parts[1] {
                        "soft" => BreakpointAction::Set,
                        "hard" => BreakpointAction::Set,
                        "delete" => BreakpointAction::Remove,
                        _ => BreakpointAction::Query,
                    };
                    
                    if let Ok(address) = parts[2].parse::<GuestAddr>() {
                        self.handle_breakpoint(conn, action, address, None)?;
                    }
                }
            }
            "ThreadEvents" => {
                // Enable thread events
                let response = "OK";
                let _ = self.send_packet(conn, response.as_bytes());
            }
            "StopReason" => {
                // Set stop reason
                let response = "OK";
                let _ = self.send_packet(conn, response.as_bytes());
            }
            _ => {
                // Unknown set command
                let response = "EUnknown set command";
                let _ = self.send_packet(conn, response.as_bytes());
            }
        }

        Ok(())
    }

    /// Handle read registers command
    fn handle_read_registers(&self, conn: &mut GdbConnection) -> VmResult<()> {
        if let Some(thread_id) = conn.current_thread {
            // Get thread registers
            let mut response = String::new();
            
            // Read general purpose registers
            for i in 0..16 {
                if let Ok(value) = self.debugger.read_register(Some(thread_id), &format!("r{}", i)) {
                    response.push_str(&format!("{:016x}", value));
                }
            }
            
            // Add PC and status registers
            if let Ok(pc) = self.debugger.read_register(Some(thread_id), "pc") {
                response.push_str(&format!("{:016x}", pc));
            }
            if let Ok(status) = self.debugger.read_register(Some(thread_id), "status") {
                response.push_str(&format!("{:016x}", status));
            }
            
            let _ = self.send_packet(conn, response.as_bytes());
        } else {
            let response = "E01"; // No current thread
            let _ = self.send_packet(conn, response.as_bytes());
        }

        Ok(())
    }

    /// Handle write registers command
    fn handle_write_registers(&self, conn: &mut GdbConnection, data: Vec<u8>) -> VmResult<()> {
        if let Some(thread_id) = conn.current_thread {
            // Parse register data (simplified)
            // In a real implementation, this would parse the GDB register format
            
            // For now, just acknowledge
            let response = "OK";
            let _ = self.send_packet(conn, response.as_bytes());
        } else {
            let response = "E01"; // No current thread
            let _ = self.send_packet(conn, response.as_bytes());
        }

        Ok(())
    }

    /// Handle read memory command
    fn handle_read_memory(&self, conn: &mut GdbConnection, address: GuestAddr, length: usize) -> VmResult<()> {
        // Read memory from debugger
        let data = self.debugger.read_memory(address, length)?;
        
        // Convert to hex string
        let mut hex_string = String::new();
        for byte in data {
            hex_string.push_str(&format!("{:02x}", byte));
        }
        
        let _ = self.send_packet(conn, hex_string.as_bytes());
        Ok(())
    }

    /// Handle write memory command
    fn handle_write_memory(&self, conn: &mut GdbConnection, address: GuestAddr, data: &[u8]) -> VmResult<()> {
        // Write memory to debugger
        self.debugger.write_memory(address, data)?;
        
        // Send acknowledgment
        let response = "OK";
        let _ = self.send_packet(conn, response.as_bytes());
        Ok(())
    }

    /// Handle continue command
    fn handle_continue(&self, conn: &mut GdbConnection, signal: Option<u8>) -> VmResult<()> {
        if let Some(thread_id) = conn.current_thread {
            // Continue execution
            self.debugger.continue_execution(Some(thread_id))?;
            
            // Send acknowledgment
            let response = "OK";
            let _ = self.send_packet(conn, response.as_bytes());
        } else {
            let response = "E01"; // No current thread
            let _ = self.send_packet(conn, response.as_bytes());
        }

        Ok(())
    }

    /// Handle step command
    fn handle_step(&self, conn: &mut GdbConnection, signal: Option<u8>) -> VmResult<()> {
        if let Some(thread_id) = conn.current_thread {
            // Step execution
            self.debugger.step(Some(thread_id))?;
            
            // Send acknowledgment
            let response = "OK";
            let _ = self.send_packet(conn, response.as_bytes());
        } else {
            let response = "E01"; // No current thread
            let _ = self.send_packet(conn, response.as_bytes());
        }

        Ok(())
    }

    /// Handle breakpoint commands
    fn handle_breakpoint(&self, conn: &mut GdbConnection, action: BreakpointAction, address: GuestAddr, kind: Option<String>) -> VmResult<()> {
        match action {
            BreakpointAction::Set => {
                // Set breakpoint
                let condition = if let Some(ref k) = kind {
                    match k.as_str() {
                        "hw" => Some(BreakpointCondition::Always), // Hardware breakpoint
                        "sw" => Some(BreakpointCondition::Always), // Software breakpoint
                        _ => Some(BreakpointCondition::Always),
                    }
                } else {
                    Some(BreakpointCondition::Always)
                };
                
                let bp_id = self.debugger.set_breakpoint(address, BreakpointType::Execution, condition, conn.current_thread)?;
                
                // Send breakpoint ID
                let response = format!("{}", bp_id);
                let _ = self.send_packet(conn, response.as_bytes());
            }
            BreakpointAction::Remove => {
                // Remove breakpoint
                // Find breakpoint at address
                let breakpoints = self.debugger.breakpoint_manager.get_breakpoints_at(address);
                for bp in breakpoints {
                    self.debugger.remove_breakpoint(bp.id)?;
                }
                
                let response = "OK";
                let _ = self.send_packet(conn, response.as_bytes());
            }
            BreakpointAction::Query => {
                // Query breakpoints
                let breakpoints = self.debugger.breakpoint_manager.get_all_breakpoints();
                let mut response = String::new();
                
                for (i, bp) in breakpoints.iter().enumerate() {
                    if i > 0 {
                        response.push(',');
                    }
                    response.push_str(&format!(
                        "{:x},{:x},{:x}",
                        bp.number,
                        bp.address,
                        bp.kind
                    ));
                }
                
                let _ = self.send_packet(conn, response.as_bytes());
            }
        }

        Ok(())
    }

    /// Handle kill command
    fn handle_kill(&self, conn: &mut GdbConnection) -> VmResult<()> {
        // Kill all threads
        let threads = self.debugger.multi_thread_debugger.get_all_threads()?;
        for thread in threads {
            self.debugger.multi_thread_debugger.unregister_thread(thread.thread_id, 0)?;
        }
        
        // Send acknowledgment
        let response = "OK";
        let _ = self.send_packet(conn, response.as_bytes());
        
        // Close connection
        conn.state = GdbConnectionState::Closing;
        
        Ok(())
    }

    /// Handle detach command
    fn handle_detach(&self, conn: &mut GdbConnection) -> VmResult<()> {
        // Detach from current thread
        if let Some(thread_id) = conn.current_thread {
            // Clear current thread
            conn.current_thread = None;
            
            // Resume thread if it was paused
            self.debugger.continue_execution(Some(thread_id))?;
        }
        
        // Send acknowledgment
        let response = "OK";
        let _ = self.send_packet(conn, response.as_bytes());
        
        // Close connection
        conn.state = GdbConnectionState::Closing;
        
        Ok(())
    }

    /// Parse a GDB command
    fn parse_command(&self, packet: &str) -> VmResult<GdbCommand> {
        if packet.is_empty() {
            return Ok(GdbCommand::Unknown(String::new()));
        }

        let first_char = packet.chars().next().unwrap_or('\0');
        
        match first_char {
            'q' => {
                // Query command
                Ok(GdbCommand::Query(packet[1..].to_string()))
            }
            'Q' => {
                // Set command
                Ok(GdbCommand::Set(packet[1..].to_string()))
            }
            'g' => {
                // Read registers
                Ok(GdbCommand::ReadRegisters)
            }
            'G' => {
                // Write registers
                let data = packet[1..].bytes().map(|b| *b).collect();
                Ok(GdbCommand::WriteRegisters(data))
            }
            'm' => {
                // Read memory
                let parts: Vec<&str> = packet[1..].split(',').collect();
                if parts.len() >= 2 {
                    let address = parts[0].parse::<GuestAddr>()
                        .map_err(|_| VmError::Core(crate::error::CoreError::InvalidState {
                            message: "Invalid address format".to_string(),
                            current: packet.to_string(),
                            expected: "Valid address".to_string(),
                        }))?;
                    let length = parts[1].parse::<usize>()
                        .map_err(|_| VmError::Core(crate::error::CoreError::InvalidState {
                            message: "Invalid length format".to_string(),
                            current: packet.to_string(),
                            expected: "Valid length".to_string(),
                        }))?;
                    Ok(GdbCommand::ReadMemory { address, length })
                } else {
                    Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Invalid memory read command".to_string(),
                        current: packet.to_string(),
                        expected: "address,length".to_string(),
                    }))
                }
            }
            'M' => {
                // Write memory
                let parts: Vec<&str> = packet[1..].split(',').collect();
                if parts.len() >= 2 {
                    let address = parts[0].parse::<GuestAddr>()
                        .map_err(|_| VmError::Core(crate::error::CoreError::InvalidState {
                            message: "Invalid address format".to_string(),
                            current: packet.to_string(),
                            expected: "Valid address".to_string(),
                        }))?;
                    let data_str = parts[1];
                    let data = data_str.as_bytes().to_vec();
                    Ok(GdbCommand::WriteMemory { address, data })
                } else {
                    Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Invalid memory write command".to_string(),
                        current: packet.to_string(),
                        expected: "address,data".to_string(),
                    }))
                }
            }
            'c' => {
                // Continue command
                let signal = if packet.len() > 1 {
                    packet[1..].parse::<u8>().ok()
                } else {
                    None
                };
                Ok(GdbCommand::Continue { signal })
            }
            's' => {
                // Step command
                let signal = if packet.len() > 1 {
                    packet[1..].parse::<u8>().ok()
                } else {
                    None
                };
                Ok(GdbCommand::Step { signal })
            }
            'k' => {
                // Kill command
                Ok(GdbCommand::Kill)
            }
            'D' => {
                // Detach command
                Ok(GdbCommand::Detach)
            }
            'Z' | 'z' => {
                // Breakpoint commands
                let parts: Vec<&str> = packet[1..].split(',').collect();
                if parts.len() >= 2 {
                    let action = if packet.starts_with('Z') {
                        BreakpointAction::Set
                    } else {
                        BreakpointAction::Remove
                    };
                    
                    let address = parts[1].parse::<GuestAddr>()
                        .map_err(|_| VmError::Core(crate::error::CoreError::InvalidState {
                            message: "Invalid address format".to_string(),
                            current: packet.to_string(),
                            expected: "Valid address".to_string(),
                        }))?;
                    let kind = if parts.len() > 2 {
                        Some(parts[2].to_string())
                    } else {
                        None
                    };
                    
                    Ok(GdbCommand::Breakpoint { action, address, kind })
                } else {
                    Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: "Invalid breakpoint command".to_string(),
                        current: packet.to_string(),
                        expected: "Z,type,address,kind".to_string(),
                    }))
                }
            }
            _ => {
                Ok(GdbCommand::Unknown(packet.to_string()))
            }
        }
    }

    /// Send a packet to GDB
    fn send_packet(&self, conn: &mut GdbConnection, data: &[u8]) -> io::Result<()> {
        // Calculate checksum
        let checksum = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(*b));
        
        // Format packet: $<data>#<checksum>
        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.push(b'$');
        packet.extend_from_slice(data);
        packet.push(b'#');
        packet.push(format!("{:02x}", checksum).as_bytes()[0]);
        packet.push(format!("{:02x}", checksum).as_bytes()[1]);
        
        // Send packet
        conn.stream.write_all(&packet)?;
        conn.stream.flush()?;
        
        // Update last activity
        conn.last_activity = SystemTime::now();
        
        Ok(())
    }

    /// Receive a packet from GDB
    fn receive_packet(&self, conn: &mut GdbConnection) -> VmResult<Option<String>> {
        let mut buffer = [0u8; 4096];
        
        // Read packet start
        loop {
            match conn.stream.read(&mut buffer) {
                Ok(0) => {
                    // Connection closed
                    return Ok(None);
                }
                Ok(n) => {
                    // Look for packet start
                    let mut start = 0;
                    while start < n && buffer[start] != b'$' {
                        start += 1;
                    }
                    
                    if start >= n {
                        continue;
                    }
                    
                    // Look for packet end
                    let mut end = start + 1;
                    while end < n && buffer[end] != b'#' {
                        end += 1;
                    }
                    
                    if end >= n {
                        continue;
                    }
                    
                    // Extract packet data
                    let packet_data = &buffer[start + 1..end];
                    let packet_str = String::from_utf8_lossy(packet_data);
                    
                    // Verify checksum
                    let mut valid_packet = false;
                    if end + 3 <= n {
                        let received_checksum = u8::from_str_radix(&String::from_utf8_lossy(&buffer[end + 1..= end + 2]), 16).unwrap_or(0);
                        let calculated_checksum = packet_data.iter().fold(0u8, |acc, &b| acc.wrapping_add(*b));
                        
                        if received_checksum == calculated_checksum {
                            // Valid packet
                            conn.last_activity = SystemTime::now();
                            valid_packet = true;
                        }
                        
                        // Send acknowledgment or negative acknowledgment
                        let ack = if received_checksum == calculated_checksum { b'+' } else { b'-' };
                        let _ = conn.stream.write_all(&[ack, b'\0'])?;
                        
                        if valid_packet {
                            return Ok(Some(packet_str.to_string()));
                        }
                    }
                }
                Err(e) => {
                    return Err(VmError::Io(format!("Error reading from GDB: {}", e)));
                }
            }
        }
    }

    /// Generate a unique connection ID
    fn generate_connection_id(&self) -> VmResult<String> {
        let mut next_id = self.lock_next_connection_id_mut()?;
        let id = format!("conn_{}", *next_id);
        *next_id += 1;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gdb_server_creation() {
        let config = EnhancedGdbServerConfig::default();
        let debugger_config = UnifiedDebuggerConfig::default();

        let server = EnhancedGdbServer::new(config, debugger_config);
        assert!(server.is_ok());
    }

    #[test]
    fn test_command_parsing() {
        let server = EnhancedGdbServer::new(
            EnhancedGdbServerConfig::default(),
            UnifiedDebuggerConfig::default(),
        ).expect("Failed to create GDB server");

        // Test query command
        let query_cmd = server.parse_command("qSupported").expect("Failed to parse query command");
        match query_cmd {
            GdbCommand::Query(query) => assert_eq!(query, "Supported"),
            _ => panic!("Expected query command"),
        }

        // Test continue command
        let continue_cmd = server.parse_command("c").expect("Failed to parse continue command");
        match continue_cmd {
            GdbCommand::Continue { signal } => assert!(signal.is_none()),
            _ => panic!("Expected continue command"),
        }

        // Test step command
        let step_cmd = server.parse_command("s").expect("Failed to parse step command");
        match step_cmd {
            GdbCommand::Step { signal } => assert!(signal.is_none()),
            _ => panic!("Expected step command"),
        }

        // Test memory read command
        let read_mem_cmd = server.parse_command("m1000,10").expect("Failed to parse memory read command");
        match read_mem_cmd {
            GdbCommand::ReadMemory { address, length } => {
                assert_eq!(address, 0x1000);
                assert_eq!(length, 10);
            }
            _ => panic!("Expected memory read command"),
        }
    }
}
