//! 文件系统访问抽象层
//!
//! 提供跨架构文件系统访问兼容性，包括：
//! - 文件路径转换（处理不同架构的路径格式）
//! - 文件权限映射
//! - 文件描述符管理
//! - 文件系统操作抽象

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vm_core::{GuestArch, GuestAddr, VmError};
use tracing::{debug, trace, warn};

/// 文件描述符类型
pub type FileDescriptor = i32;

/// 文件打开标志
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
    pub exclusive: bool,
    pub nonblock: bool,
    pub sync: bool,
    pub directory: bool,
    pub no_follow: bool,
}

impl OpenFlags {
    /// 从 POSIX open flags 创建
    pub fn from_posix(flags: u32) -> Self {
        // O_RDONLY = 0, O_WRONLY = 1, O_RDWR = 2
        let read = (flags & 0o3) != 0o1; // RDONLY or RDWR
        let write = (flags & 0o3) != 0o0; // WRONLY or RDWR

        Self {
            read,
            write,
            create: (flags & 0o100) != 0,      // O_CREAT
            truncate: (flags & 0o1000) != 0,   // O_TRUNC
            append: (flags & 0o2000) != 0,     // O_APPEND
            exclusive: (flags & 0o400) != 0,  // O_EXCL
            nonblock: (flags & 0o4000) != 0,  // O_NONBLOCK
            sync: (flags & 0o4010000) != 0,   // O_SYNC
            directory: (flags & 0o200000) != 0, // O_DIRECTORY
            no_follow: (flags & 0o400000) != 0, // O_NOFOLLOW
        }
    }

    /// 转换为 POSIX open flags
    pub fn to_posix(&self) -> u32 {
        let mut flags = 0u32;

        if self.read && self.write {
            flags |= 0o2; // O_RDWR
        } else if self.write {
            flags |= 0o1; // O_WRONLY
        }
        // else: O_RDONLY = 0

        if self.create {
            flags |= 0o100; // O_CREAT
        }
        if self.truncate {
            flags |= 0o1000; // O_TRUNC
        }
        if self.append {
            flags |= 0o2000; // O_APPEND
        }
        if self.exclusive {
            flags |= 0o400; // O_EXCL
        }
        if self.nonblock {
            flags |= 0o4000; // O_NONBLOCK
        }
        if self.sync {
            flags |= 0o4010000; // O_SYNC
        }
        if self.directory {
            flags |= 0o200000; // O_DIRECTORY
        }
        if self.no_follow {
            flags |= 0o400000; // O_NOFOLLOW
        }

        flags
    }
}

/// 文件模式（权限）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMode {
    pub owner_read: bool,
    pub owner_write: bool,
    pub owner_exec: bool,
    pub group_read: bool,
    pub group_write: bool,
    pub group_exec: bool,
    pub other_read: bool,
    pub other_write: bool,
    pub other_exec: bool,
    pub setuid: bool,
    pub setgid: bool,
    pub sticky: bool,
}

impl FileMode {
    /// 从 POSIX mode 创建
    pub fn from_posix(mode: u32) -> Self {
        Self {
            owner_read: (mode & 0o400) != 0,
            owner_write: (mode & 0o200) != 0,
            owner_exec: (mode & 0o100) != 0,
            group_read: (mode & 0o40) != 0,
            group_write: (mode & 0o20) != 0,
            group_exec: (mode & 0o10) != 0,
            other_read: (mode & 0o4) != 0,
            other_write: (mode & 0o2) != 0,
            other_exec: (mode & 0o1) != 0,
            setuid: (mode & 0o4000) != 0,
            setgid: (mode & 0o2000) != 0,
            sticky: (mode & 0o1000) != 0,
        }
    }

    /// 转换为 POSIX mode
    pub fn to_posix(&self) -> u32 {
        let mut mode = 0u32;

        if self.owner_read {
            mode |= 0o400;
        }
        if self.owner_write {
            mode |= 0o200;
        }
        if self.owner_exec {
            mode |= 0o100;
        }
        if self.group_read {
            mode |= 0o40;
        }
        if self.group_write {
            mode |= 0o20;
        }
        if self.group_exec {
            mode |= 0o10;
        }
        if self.other_read {
            mode |= 0o4;
        }
        if self.other_write {
            mode |= 0o2;
        }
        if self.other_exec {
            mode |= 0o1;
        }
        if self.setuid {
            mode |= 0o4000;
        }
        if self.setgid {
            mode |= 0o2000;
        }
        if self.sticky {
            mode |= 0o1000;
        }

        mode
    }
}

/// 文件信息
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub fd: FileDescriptor,
    pub flags: OpenFlags,
    pub mode: FileMode,
    pub size: u64,
    pub offset: u64,
}

/// 文件系统操作 trait
pub trait FilesystemOperations: Send + Sync {
    /// 打开文件
    fn open(&mut self, path: &Path, flags: OpenFlags, mode: FileMode) -> Result<FileDescriptor, VmError>;

    /// 关闭文件
    fn close(&mut self, fd: FileDescriptor) -> Result<(), VmError>;

    /// 读取文件
    fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, VmError>;

    /// 写入文件
    fn write(&mut self, fd: FileDescriptor, buf: &[u8]) -> Result<usize, VmError>;

    /// 定位文件
    fn seek(&mut self, fd: FileDescriptor, offset: i64, whence: i32) -> Result<u64, VmError>;

    /// 获取文件信息
    fn stat(&mut self, path: &Path) -> Result<FileInfo, VmError>;

    /// 获取文件描述符信息
    fn fstat(&mut self, fd: FileDescriptor) -> Result<FileInfo, VmError>;
}

/// 文件系统访问抽象层
pub struct FilesystemCompatibilityLayer {
    /// Guest 架构
    guest_arch: GuestArch,
    /// 文件描述符表
    file_descriptors: Arc<Mutex<HashMap<FileDescriptor, FileInfo>>>,
    /// 下一个可用的文件描述符
    next_fd: Arc<Mutex<FileDescriptor>>,
    /// 文件系统操作实现
    fs_ops: Arc<Mutex<Box<dyn FilesystemOperations>>>,
    /// 根目录路径
    root_path: PathBuf,
}

impl FilesystemCompatibilityLayer {
    /// 创建新的文件系统访问抽象层
    pub fn new(guest_arch: GuestArch, root_path: PathBuf, fs_ops: Box<dyn FilesystemOperations>) -> Self {
        let mut layer = Self {
            guest_arch,
            file_descriptors: Arc::new(Mutex::new(HashMap::new())),
            next_fd: Arc::new(Mutex::new(3)), // 0, 1, 2 are stdin, stdout, stderr
            fs_ops: Arc::new(Mutex::new(fs_ops)),
            root_path,
        };

        // 初始化标准文件描述符
        layer.init_standard_fds();
        layer
    }

    /// 初始化标准文件描述符
    fn init_standard_fds(&mut self) {
        let mut fds = self.file_descriptors.lock().expect("Failed to lock file descriptors during initialization");
        
        // stdin (0)
        fds.insert(0, FileInfo {
            path: PathBuf::from("/dev/stdin"),
            fd: 0,
            flags: OpenFlags {
                read: true,
                write: false,
                ..Default::default()
            },
            mode: FileMode::from_posix(0o666),
            size: 0,
            offset: 0,
        });

        // stdout (1)
        fds.insert(1, FileInfo {
            path: PathBuf::from("/dev/stdout"),
            fd: 1,
            flags: OpenFlags {
                read: false,
                write: true,
                ..Default::default()
            },
            mode: FileMode::from_posix(0o666),
            size: 0,
            offset: 0,
        });

        // stderr (2)
        fds.insert(2, FileInfo {
            path: PathBuf::from("/dev/stderr"),
            fd: 2,
            flags: OpenFlags {
                read: false,
                write: true,
                ..Default::default()
            },
            mode: FileMode::from_posix(0o666),
            size: 0,
            offset: 0,
        });
    }

    /// 转换 guest 路径到 host 路径
    pub fn convert_path(&self, guest_path: &str) -> PathBuf {
        let path = Path::new(guest_path);
        
        // 如果是绝对路径，相对于 root_path
        if path.is_absolute() {
            self.root_path.join(path.strip_prefix("/").unwrap_or(path))
        } else {
            // 相对路径直接使用
            self.root_path.join(path)
        }
    }

    /// 打开文件
    pub fn open(&self, guest_path: &str, flags: u32, mode: u32) -> Result<FileDescriptor, VmError> {
        let host_path = self.convert_path(guest_path);
        let open_flags = OpenFlags::from_posix(flags);
        let file_mode = FileMode::from_posix(mode);

        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        let fd = fs_ops.open(&host_path, open_flags, file_mode)?;

        // 记录文件描述符
        let mut fds = self.file_descriptors.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock file descriptors: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        let file_info = FileInfo {
            path: host_path.clone(),
            fd,
            flags: open_flags,
            mode: file_mode,
            size: 0, // Will be updated by stat
            offset: 0,
        };

        fds.insert(fd, file_info);
        debug!("Opened file: {} -> {} (fd: {})", guest_path, host_path.display(), fd);

        Ok(fd)
    }

    /// 关闭文件
    pub fn close(&self, fd: FileDescriptor) -> Result<(), VmError> {
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        fs_ops.close(fd)?;

        // 从文件描述符表中移除（标准文件描述符除外）
        if fd >= 3 {
            let mut fds = self.file_descriptors.lock().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to lock file descriptors: {:?}", e),
                    module: "FilesystemCompatibilityLayer".to_string(),
                })
            })?;

            fds.remove(&fd);
            debug!("Closed file descriptor: {}", fd);
        }

        Ok(())
    }

    /// 读取文件
    pub fn read(&self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, VmError> {
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        let size = fs_ops.read(fd, buf)?;
        trace!("Read {} bytes from fd {}", size, fd);

        Ok(size)
    }

    /// 写入文件
    pub fn write(&self, fd: FileDescriptor, buf: &[u8]) -> Result<usize, VmError> {
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        let size = fs_ops.write(fd, buf)?;
        trace!("Wrote {} bytes to fd {}", size, fd);

        Ok(size)
    }

    /// 定位文件
    pub fn seek(&self, fd: FileDescriptor, offset: i64, whence: i32) -> Result<u64, VmError> {
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        let new_offset = fs_ops.seek(fd, offset, whence)?;
        trace!("Seek fd {} to offset {}", fd, new_offset);

        Ok(new_offset)
    }

    /// 获取文件信息
    pub fn stat(&self, guest_path: &str) -> Result<FileInfo, VmError> {
        let host_path = self.convert_path(guest_path);
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        fs_ops.stat(&host_path)
    }

    /// 获取文件描述符信息
    pub fn fstat(&self, fd: FileDescriptor) -> Result<FileInfo, VmError> {
        let mut fs_ops = self.fs_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock filesystem operations: {:?}", e),
                module: "FilesystemCompatibilityLayer".to_string(),
            })
        })?;

        fs_ops.fstat(fd)
    }

    /// 获取文件描述符信息（从缓存）
    pub fn get_file_info(&self, fd: FileDescriptor) -> Option<FileInfo> {
        let fds = self.file_descriptors.lock().ok()?;
        fds.get(&fd).cloned()
    }
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self {
            read: true,
            write: false,
            create: false,
            truncate: false,
            append: false,
            exclusive: false,
            nonblock: false,
            sync: false,
            directory: false,
            no_follow: false,
        }
    }
}

/// 默认文件系统操作实现（使用标准库）
pub struct DefaultFilesystemOperations;

impl FilesystemOperations for DefaultFilesystemOperations {
    fn open(&mut self, path: &Path, flags: OpenFlags, _mode: FileMode) -> Result<FileDescriptor, VmError> {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let mut options = OpenOptions::new();
        options.read(flags.read);
        options.write(flags.write);
        options.create(flags.create);
        options.truncate(flags.truncate);
        options.append(flags.append);
        options.create_new(flags.exclusive);

        // 注意：这里返回一个占位符文件描述符
        // 实际实现应该使用真正的文件句柄
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::open".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }

    fn close(&mut self, _fd: FileDescriptor) -> Result<(), VmError> {
        Ok(())
    }

    fn read(&mut self, _fd: FileDescriptor, _buf: &mut [u8]) -> Result<usize, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::read".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }

    fn write(&mut self, _fd: FileDescriptor, _buf: &[u8]) -> Result<usize, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::write".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }

    fn seek(&mut self, _fd: FileDescriptor, _offset: i64, _whence: i32) -> Result<u64, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::seek".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }

    fn stat(&mut self, _path: &Path) -> Result<FileInfo, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::stat".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }

    fn fstat(&mut self, _fd: FileDescriptor) -> Result<FileInfo, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultFilesystemOperations::fstat".to_string(),
            module: "FilesystemCompatibilityLayer".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_flags_from_posix() {
        let flags = OpenFlags::from_posix(0o2 | 0o100 | 0o1000); // O_RDWR | O_CREAT | O_TRUNC
        assert!(flags.read);
        assert!(flags.write);
        assert!(flags.create);
        assert!(flags.truncate);
    }

    #[test]
    fn test_file_mode_from_posix() {
        let mode = FileMode::from_posix(0o755);
        assert!(mode.owner_read);
        assert!(mode.owner_write);
        assert!(mode.owner_exec);
        assert!(mode.group_read);
        assert!(!mode.group_write);
        assert!(mode.group_exec);
        assert!(mode.other_read);
        assert!(!mode.other_write);
        assert!(mode.other_exec);
    }

    #[test]
    fn test_path_conversion() {
        let layer = FilesystemCompatibilityLayer::new(
            GuestArch::X86_64,
            PathBuf::from("/tmp/guest_root"),
            Box::new(DefaultFilesystemOperations),
        );

        let host_path = layer.convert_path("/etc/passwd");
        assert!(host_path.to_string_lossy().contains("tmp/guest_root"));
        assert!(host_path.to_string_lossy().contains("etc/passwd"));
    }
}

