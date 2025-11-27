# GDB/LLDB 调试指南

## 概述

本指南介绍如何使用 GDB 或 LLDB 调试器连接到虚拟机，进行源码级调试。这对于操作系统开发、驱动调试和逆向工程非常有用。

## 快速开始

### 1. 启动虚拟机并启用 GDB 服务器

在虚拟机启动时，添加 `--gdb` 参数来启用 GDB 服务器：

```bash
vm-cli --kernel kernel.bin --gdb 1234
```

这将在端口 1234 上启动 GDB 服务器，等待调试器连接。

### 2. 使用 GDB 连接

在另一个终端中，启动 GDB 并连接到虚拟机：

```bash
# 启动 GDB（可以指定要调试的二进制文件）
gdb kernel.elf

# 连接到虚拟机
(gdb) target remote localhost:1234
Remote debugging using localhost:1234

# 现在可以开始调试了
(gdb) info registers
(gdb) break main
(gdb) continue
```

### 3. 使用 LLDB 连接

如果你更喜欢使用 LLDB：

```bash
# 启动 LLDB
lldb kernel.elf

# 连接到虚拟机
(lldb) gdb-remote localhost:1234
Process 1 stopped

# 现在可以开始调试了
(lldb) register read
(lldb) breakpoint set --name main
(lldb) continue
```

## 常用调试命令

### GDB 命令

#### 断点管理

```gdb
# 在地址设置断点
(gdb) break *0x80000000

# 在函数设置断点
(gdb) break main

# 在文件的特定行设置断点
(gdb) break kernel.c:42

# 列出所有断点
(gdb) info breakpoints

# 删除断点
(gdb) delete 1

# 禁用/启用断点
(gdb) disable 1
(gdb) enable 1
```

#### 执行控制

```gdb
# 继续执行
(gdb) continue
(gdb) c

# 单步执行（汇编级别）
(gdb) stepi
(gdb) si

# 单步执行（源码级别）
(gdb) step
(gdb) s

# 执行到下一行
(gdb) next
(gdb) n

# 执行到函数返回
(gdb) finish
```

#### 查看状态

```gdb
# 查看所有寄存器
(gdb) info registers
(gdb) info all-registers

# 查看特定寄存器
(gdb) print $pc
(gdb) print $sp

# 查看内存（十六进制）
(gdb) x/16x 0x80000000

# 查看内存（指令）
(gdb) x/16i 0x80000000

# 查看内存（字符串）
(gdb) x/s 0x80001000

# 查看调用栈
(gdb) backtrace
(gdb) bt
```

#### 修改状态

```gdb
# 修改寄存器
(gdb) set $pc = 0x80000100

# 修改内存
(gdb) set {int}0x80000000 = 0x12345678
(gdb) set {char}0x80000000 = 'A'
```

### LLDB 命令

#### 断点管理

```lldb
# 在地址设置断点
(lldb) breakpoint set --address 0x80000000
(lldb) br s -a 0x80000000

# 在函数设置断点
(lldb) breakpoint set --name main
(lldb) br s -n main

# 在文件的特定行设置断点
(lldb) breakpoint set --file kernel.c --line 42
(lldb) br s -f kernel.c -l 42

# 列出所有断点
(lldb) breakpoint list
(lldb) br l

# 删除断点
(lldb) breakpoint delete 1
(lldb) br del 1

# 禁用/启用断点
(lldb) breakpoint disable 1
(lldb) breakpoint enable 1
```

#### 执行控制

```lldb
# 继续执行
(lldb) continue
(lldb) c

# 单步执行（汇编级别）
(lldb) thread step-inst
(lldb) si

# 单步执行（源码级别）
(lldb) thread step-in
(lldb) s

# 执行到下一行
(lldb) thread step-over
(lldb) n

# 执行到函数返回
(lldb) thread step-out
(lldb) finish
```

#### 查看状态

```lldb
# 查看所有寄存器
(lldb) register read
(lldb) re r

# 查看特定寄存器
(lldb) register read pc
(lldb) register read sp

# 查看内存
(lldb) memory read 0x80000000 --count 16
(lldb) mem read 0x80000000 -c 16

# 查看内存（格式化）
(lldb) memory read --format hex --size 4 --count 16 0x80000000
(lldb) x/16xw 0x80000000

# 反汇编
(lldb) disassemble --start-address 0x80000000 --count 16
(lldb) di -s 0x80000000 -c 16

# 查看调用栈
(lldb) thread backtrace
(lldb) bt
```

#### 修改状态

```lldb
# 修改寄存器
(lldb) register write pc 0x80000100

# 修改内存
(lldb) memory write 0x80000000 0x12 0x34 0x56 0x78
```

## 高级调试技巧

### 1. 条件断点

**GDB**:
```gdb
# 只有当条件满足时才触发断点
(gdb) break *0x80000000 if $rax == 0x42
```

**LLDB**:
```lldb
(lldb) breakpoint set --address 0x80000000 --condition '$rax == 0x42'
```

### 2. 观察点（Watchpoints）

观察特定内存地址的变化：

**GDB**:
```gdb
# 监视内存地址
(gdb) watch *0x80001000

# 监视变量
(gdb) watch my_variable
```

**LLDB**:
```lldb
# 监视内存地址
(lldb) watchpoint set expression -- 0x80001000

# 监视变量
(lldb) watchpoint set variable my_variable
```

### 3. 自动化脚本

**GDB 脚本** (`.gdbinit`):
```gdb
# 连接到虚拟机
target remote localhost:1234

# 设置断点
break *0x80000000

# 定义自定义命令
define dump_regs
    info registers
    x/16x $sp
end

# 继续执行
continue
```

**LLDB 脚本** (`.lldbinit`):
```lldb
# 连接到虚拟机
gdb-remote localhost:1234

# 设置断点
breakpoint set --address 0x80000000

# 定义自定义命令
command alias dump_regs register read

# 继续执行
continue
```

### 4. 远程文件加载

如果调试符号文件在本地，但内核在虚拟机中运行：

**GDB**:
```gdb
# 加载符号文件
(gdb) file kernel.elf

# 连接到远程目标
(gdb) target remote localhost:1234

# 设置源码路径
(gdb) set substitute-path /build/kernel /home/user/kernel
```

**LLDB**:
```lldb
# 加载符号文件
(lldb) file kernel.elf

# 连接到远程目标
(lldb) gdb-remote localhost:1234

# 设置源码路径
(lldb) settings set target.source-map /build/kernel /home/user/kernel
```

## 调试操作系统内核

### 调试启动过程

```gdb
# 连接到虚拟机（此时虚拟机暂停在入口点）
(gdb) target remote localhost:1234

# 在内核入口点设置断点
(gdb) break _start

# 继续执行
(gdb) continue

# 单步执行启动代码
(gdb) si
(gdb) si
(gdb) si
```

### 调试中断和异常

```gdb
# 在中断处理程序设置断点
(gdb) break interrupt_handler

# 在异常处理程序设置断点
(gdb) break page_fault_handler

# 查看中断发生时的状态
(gdb) info registers
(gdb) backtrace
```

### 调试多核系统

```gdb
# 查看所有线程（vCPU）
(gdb) info threads

# 切换到特定线程
(gdb) thread 2

# 在所有线程上应用命令
(gdb) thread apply all backtrace
```

## 故障排查

### 连接失败

如果无法连接到 GDB 服务器：

1. 检查虚拟机是否已启动 GDB 服务器
2. 检查端口是否正确
3. 检查防火墙设置
4. 尝试使用 `telnet localhost 1234` 测试连接

### 符号不匹配

如果调试符号与实际代码不匹配：

1. 确保使用的符号文件与虚拟机中运行的二进制文件完全一致
2. 检查编译选项，确保包含调试信息（`-g`）
3. 避免编译器优化（`-O0`）

### 断点不触发

如果断点设置后不触发：

1. 检查断点地址是否正确
2. 确认代码确实执行到了该位置
3. 尝试使用硬件断点（如果支持）

## 参考资料

- [GDB 官方文档](https://sourceware.org/gdb/documentation/)
- [LLDB 官方文档](https://lldb.llvm.org/)
- [GDB Remote Serial Protocol](https://sourceware.org/gdb/current/onlinedocs/gdb/Remote-Protocol.html)
