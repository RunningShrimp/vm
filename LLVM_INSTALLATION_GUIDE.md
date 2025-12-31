# LLVM 安装指南和环境变量配置

## 问题描述

项目当前依赖 LLVM 211 版本，但系统中没有找到合适的 LLVM 版本，导致编译失败：

```
error: No suitable version of LLVM was found system-wide or pointed to by LLVM_SYS_211_PREFIX.
```

## 解决方案

### 方案1：使用 Homebrew 安装 LLVM（推荐用于 macOS）

#### 步骤1：安装 LLVM

```bash
# 安装 LLVM 18（与 llvm-sys 211 兼容）
brew install llvm@18

# 或者安装最新版本
brew install llvm
```

#### 步骤2：设置环境变量

```bash
# 添加到 ~/.zshrc 或 ~/.bash_profile
export LLVM_SYS_211_PREFIX=$(brew --prefix llvm@18)
export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
export LD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$LD_LIBRARY_PATH"
export DYLD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$DYLD_LIBRARY_PATH"
```

#### 步骤3：重新加载环境变量

```bash
source ~/.zshrc  # 或 source ~/.bash_profile
```

#### 步骤4：验证安装

```bash
# 检查 LLVM 版本
llvm-config --version

# 检查 clang 版本
clang --version

# 检查环境变量
echo $LLVM_SYS_211_PREFIX
```

### 方案2：使用官方安装包

#### macOS

1. 访问 [LLVM 官方下载页面](https://releases.llvm.org/)
2. 下载适合 macOS 的 LLVM 18.x 版本
3. 解压到 `/usr/local/llvm18` 或其他目录
4. 设置环境变量：

```bash
export LLVM_SYS_211_PREFIX=/usr/local/llvm18
export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
export DYLD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$DYLD_LIBRARY_PATH"
```

#### Linux (Ubuntu/Debian)

```bash
# 更新包管理器
sudo apt update

# 安装 LLVM 18
sudo apt install llvm-18 llvm-18-dev clang-18

# 设置环境变量
export LLVM_SYS_211_PREFIX=/usr/lib/llvm-18
export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
export LD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$LD_LIBRARY_PATH"
```

#### Linux (CentOS/RHEL/Fedora)

```bash
# CentOS/RHEL
sudo yum install llvm18 llvm18-devel clang18

# Fedora
sudo dnf install llvm18 llvm18-devel clang18

# 设置环境变量
export LLVM_SYS_211_PREFIX=/usr/lib64/llvm18
export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
export LD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib64:$LD_LIBRARY_PATH"
```

#### Windows

1. 下载 LLVM for Windows [官方下载页面](https://releases.llvm.org/download.html)
2. 解压到 `C:\llvm18` 或其他目录
3. 设置环境变量：

```cmd
# 在系统环境变量中设置
LLVM_SYS_211_PREFIX=C:\llvm18
PATH=%LLVM_SYS_211_PREFIX%\bin;%PATH%
```

### 方案3：使用 llvmenv 工具

```bash
# 安装 llvmenv
cargo install llvmenv

# 初始化 llvmenv
llvmenv init

# 安装兼容的 LLVM 版本
llvmenv install 18.1.0

# 激活 LLVM 版本
llvmenv activate 18.1.0
```

## 验证安装

安装完成后，运行以下命令验证：

```bash
# 1. 检查环境变量
echo "LLVM_SYS_211_PREFIX: $LLVM_SYS_211_PREFIX"

# 2. 检查 LLVM 版本
if command -v llvm-config &> /dev/null; then
    echo "LLVM version: $(llvm-config --version)"
else
    echo "llvm-config not found in PATH"
fi

# 3. 尝试编译项目
cargo build
```

## 常见问题解决

### 问题1：仍然找不到 LLVM

**症状**：设置环境变量后仍然报错

**解决方案**：
1. 确认环境变量设置正确
2. 重启终端或重新加载 shell 配置
3. 检查 llvm-config 是否在 PATH 中

### 问题2：版本不匹配

**症状**：LLVM 版本与 llvm-sys 不兼容

**解决方案**：
- llvm-sys 211 通常对应 LLVM 18.x
- 如果使用其他版本，可能需要降级或升级 llvm-sys

### 问题3：链接错误

**症状**：编译时出现链接错误

**解决方案**：
1. 确保设置了正确的库路径变量（LD_LIBRARY_PATH/DYLD_LIBRARY_PATH）
2. 在 macOS 上可能需要设置 DYLD_LIBRARY_PATH
3. 在 Linux 上可能需要设置 LD_LIBRARY_PATH

### 问题4：权限问题

**症状**：无法访问 LLVM 安装目录

**解决方案**：
```bash
# 更改安装目录权限
sudo chown -R $USER:$USER $(brew --prefix llvm@18)

# 或者使用 sudo 安装到系统目录
sudo mv llvm-release /usr/local/llvm
```

## 临时解决方案

如果无法立即安装 LLVM，可以使用我们配置的可选功能：

```bash
# 不使用 LLVM 功能编译
cargo build --no-default-features

# 或者只编译不依赖 LLVM 的模块
cargo build -p vm-core -p vm-ir -p vm-mem --exclude vm-engine-jit --exclude aot-builder --exclude vm-cross-arch
```

## 项目特定配置

项目已经配置了可选的 LLVM 功能：

- `vm-ir-lift/llvm`: 启用 LLVM 集成功能
- `vm-engine-jit/llvm`: 启用 JIT 编译器的 LLVM 后端
- `aot-builder/llvm`: 启用 AOT 编译器的 LLVM 功能
- `vm-cross-arch/llvm`: 启用跨架构 LLVM 优化

要启用所有 LLVM 功能：

```bash
cargo build --features llvm
```

要禁用所有 LLVM 功能：

```bash
cargo build --no-default-features
```

## 推荐方案

对于 macOS 用户，推荐使用 Homebrew 安装 LLVM 18：

```bash
# 一键安装脚本
brew install llvm@18 && \
echo 'export LLVM_SYS_211_PREFIX=$(brew --prefix llvm@18)' >> ~/.zshrc && \
echo 'export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"' >> ~/.zshrc && \
echo 'export DYLD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$DYLD_LIBRARY_PATH"' >> ~/.zshrc && \
source ~/.zshrc && \
echo "LLVM 安装完成，版本: $(llvm-config --version)"
```

这个方案提供了完整的 LLVM 支持，同时保持了项目的所有功能。