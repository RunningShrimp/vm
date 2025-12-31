# RISC-V编程指南

本指南介绍如何为VM编写RISC-V汇编程序。

## 目录

- [RISC-V基础](#risc-v基础)
- [指令集](#指令集)
- [汇编语法](#汇编语法)
- [控制流](#控制流)
- [内存访问](#内存访问)
- [函数调用](#函数调用)
- [示例程序](#示例程序)
- [调试技巧](#调试技巧)
- [最佳实践](#最佳实践)

## RISC-V基础

### 什么是RISC-V?

RISC-V是一个开源的精简指令集计算机(RISC)架构。特点:
- **简洁**: 指令集规范清晰
- **模块化**: 可选的扩展指令集
- **开源**: 无需授权费

### VM支持的架构

VM项目支持RISC-V 64位(RV64IMAC):
- **RV64I**: 基础整数指令集
- **M**: 乘除法扩展
- **A**: 原子指令扩展
- **C**: 压缩指令扩展

### 寄存器

RISC-V有32个通用寄存器(x0-x31):

| 寄存器 | 别名   | 用途                 | 说明               |
|--------|--------|----------------------|--------------------|
| x0     | zero   | 常零                 | 硬编码为0          |
| x1     | ra     | 返回地址             | 函数调用           |
| x2     | sp     | 栈指针               | 指向栈顶           |
| x3     | gp     | 全局指针             | 指向全局数据       |
| x4     | tp     | 线程指针             | 线程局部存储       |
| x5-x7  | t0-t2  | 临时寄存器           | 函数调用不保存     |
| x8     | s0/fp  | 保存寄存器/帧指针    | 函数调用保存       |
| x9     | s1     | 保存寄存器           | 函数调用保存       |
| x10-x11| a0-a1  | 参数/返回值          | 函数参数/返回值    |
| x12-x17| a2-a7  | 参数                 | 函数参数           |
| x18-x27| s2-s11 | 保存寄存器           | 函数调用保存       |
| x28-x31| t3-t6  | 临时寄存器           | 函数调用不保存     |

### 调用约定

- **参数传递**: a0-a7 (前8个参数)
- **返回值**: a0, a1
- **栈**: 向下生长,16字节对齐
- **保存**: s0-s11由被调用者保存
- **临时**: t0-t6由调用者保存

## 指令集

### 立即数指令

#### 加载立即数

```assembly
li  rd, imm      # rd = imm (伪指令)
```

示例:
```assembly
li  x1, 42       # x1 = 42
li  x2, 0x100    # x2 = 256
```

#### 立即数运算

```assembly
addi  rd, rs1, imm    # rd = rs1 + imm
andi  rd, rs1, imm    # rd = rs1 & imm
ori   rd, rs1, imm    # rd = rs1 | imm
xori  rd, rs1, imm    # rd = rs1 ^ imm
slti  rd, rs1, imm    # rd = (rs1 < imm) ? 1 : 0
sltiu rd, rs1, imm    # rd = (rs1 < imm) ? 1 : 0 (无符号)
```

### 寄存器运算

```assembly
add  rd, rs1, rs2    # rd = rs1 + rs2
sub  rd, rs1, rs2    # rd = rs1 - rs2
and  rd, rs1, rs2    # rd = rs1 & rs2
or   rd, rs1, rs2    # rd = rs1 | rs2
xor  rd, rs1, rs2    # rd = rs1 ^ rs2
sll  rd, rs1, rs2    # rd = rs1 << rs2
srl  rd, rs1, rs2    # rd = rs1 >> rs2 (逻辑)
sra  rd, rs1, rs2    # rd = rs1 >> rs2 (算术)

slt  rd, rs1, rs2    # rd = (rs1 < rs2) ? 1 : 0
sltu rd, rs1, rs2    # rd = (rs1 < rs2) ? 1 : 0 (无符号)
```

### 乘除法 (M扩展)

```assembly
mul   rd, rs1, rs2    # rd = rs1 * rs2 (低32位)
mulh  rd, rs1, rs2    # rd = (rs1 * rs2) >> 32 (有符号)
mulhu rd, rs1, rs2    # rd = (rs1 * rs2) >> 32 (无符号)
mulhsu rd, rs1, rs2   # rd = (rs1 * rs2) >> 32 (混合)

div  rd, rs1, rs2     # rd = rs1 / rs2
rem  rd, rs1, rs2     # rd = rs1 % rs2
divu rd, rs1, rs2     # rd = rs1 / rs2 (无符号)
remu rd, rs1, rs2     # rd = rs1 % rs2 (无符号)
```

### 比较和分支

```assembly
beq  rs1, rs2, label    # if (rs1 == rs2) goto label
bne  rs1, rs2, label    # if (rs1 != rs2) goto label
blt  rs1, rs2, label    # if (rs1 < rs2) goto label (有符号)
bge  rs1, rs2, label    # if (rs1 >= rs2) goto label (有符号)
bltu rs1, rs2, label    # if (rs1 < rs2) goto label (无符号)
bgeu rs1, rs2, label    # if (rs1 >= rs2) goto label (无符号)
```

### 加载和存储

```assembly
# 加载
lb   rd, offset(rs1)    # rd = mem[rs1 + offset] (8位,符号扩展)
lh   rd, offset(rs1)    # rd = mem[rs1 + offset] (16位,符号扩展)
lw   rd, offset(rs1)    # rd = mem[rs1 + offset] (32位,符号扩展)
ld   rd, offset(rs1)    # rd = mem[rs1 + offset] (64位)

lbu  rd, offset(rs1)    # rd = mem[rs1 + offset] (8位,零扩展)
lhu  rd, offset(rs1)    # rd = mem[rs1 + offset] (16位,零扩展)
lwu  rd, offset(rs1)    # rd = mem[rs1 + offset] (32位,零扩展)

# 存储
sb   rs2, offset(rs1)   # mem[rs1 + offset] = rs2 (8位)
sh   rs2, offset(rs1)   # mem[rs1 + offset] = rs2 (16位)
sw   rs2, offset(rs1)   # mem[rs1 + offset] = rs2 (32位)
sd   rs2, offset(rs1)   # mem[rs1 + offset] = rs2 (64位)
```

### 跳转和函数调用

```assembly
jal  rd, offset    # rd = PC+4; PC += offset (跳转并链接)
jalr rd, offset(rs1)  # rd = PC+4; PC = rs1 + offset

ret                # PC = ra (返回,伪指令)
call offset        # 调用函数(伪指令)
jr  rs1            # 跳转到rs1(伪指令)
```

## 汇编语法

### 基本结构

```assembly
.section .data           # 数据段
    variable: .word 42   # 定义32位变量
    array:    .skip 100  # 分配100字节

.section .text           # 代码段
    .globl _start        # 导出符号

_start:                  # 标签
    li  x1, 10           # 指令
    ret
```

### 数据定义

```assembly
# 字节
.byte   0x12                # 1字节
.half   0x1234              # 2字节
.word   0x12345678          # 4字节
.dword  0x123456789abcdef0  # 8字节

# 字符串
.asciz  "Hello, World!"     # C字符串(以null结尾)
.string "Hello"             # 字符串

# 数组
array:  .word 1, 2, 3, 4, 5

# 未初始化
.skip   100                 # 分配100字节,未初始化
.zero   200                 # 分配200字节,清零
```

### 注释

```assembly
# 这是单行注释

/*
  这是
  多行
  注释
*/

li x1, 42   # 行尾注释
```

## 控制流

### 条件执行

```assembly
# if-else
li   x1, 10
li   x2, 20

blt  x1, x2, less_than     # if x1 < x2
    # x1 >= x2的代码
    j    endif
less_than:
    # x1 < x2的代码
endif:
```

### 循环

```assembly
# for循环: for(i=0; i<100; i++)
    li   x1, 0              # i = 0
    li   x2, 100            # limit = 100

loop:
    bge  x1, x2, end_loop   # if i >= 100, 退出

    # 循环体

    addi x1, x1, 1          # i++
    j    loop

end_loop:
```

```assembly
# while循环: while(x1 > 0)
loop:
    blez x1, end_loop       # if x1 <= 0, 退出

    # 循环体

    addi x1, x1, -1         # x1--
    j    loop

end_loop:
```

## 内存访问

### 数组访问

```assembly
# 初始化数组
la   x1, array             # x1 = &array
li   x2, 0                 # index = 0

loop:
    # 计算地址: &array[index]
    slli x3, x2, 3          # x3 = index * 8 (每个元素8字节)
    add  x3, x1, x3         # x3 = &array[index]

    ld   x4, 0(x3)          # x4 = array[index]
    # 处理x4

    addi x2, x2, 1
    blt  x2, x5, loop       # if index < length, 继续
```

### 结构体

```assembly
# C结构体:
# struct Point {
#     int64_t x;
#     int64_t y;
# };

# 访问
la   x1, point             # x1 = &point
ld   x2, 0(x1)             # x2 = point.x
ld   x3, 8(x1)             # x3 = point.y
```

## 函数调用

### 定义函数

```assembly
# 函数: int add(int a, int b)
# 参数: a0, a1
# 返回: a0
.globl add
add:
    # 函数体
    add  a0, a0, a1         # a0 = a0 + a1
    ret                    # 返回
```

### 调用函数

```assembly
# 调用: result = add(10, 20)
li   a0, 10                # 第一个参数
li   a1, 20                # 第二个参数
call add                   # 调用函数
# 结果在a0中
```

### 栈帧

```assembly
# 使用栈的函数
func:
    # 保存寄存器
    addi sp, sp, -32        # 分配栈帧
    sd   ra, 24(sp)         # 保存返回地址
    sd   s0, 16(sp)         # 保存保存寄存器
    sd   s1, 8(sp)

    # 函数体
    # ...

    # 恢复寄存器
    ld   ra, 24(sp)
    ld   s0, 16(sp)
    ld   s1, 8(sp)
    addi sp, sp, 32         # 释放栈帧

    ret
```

## 示例程序

### 最大值

```assembly
# int max(int a, int b)
.globl max
max:
    blt  a0, a1, return_b    # if a < b, 返回b
    mv   a0, a0              # 返回a
    ret
return_b:
    mv   a0, a1              # 返回b
    ret
```

### 绝对值

```assembly
# int64_t abs(int64_t x)
.globl abs
abs:
    blt  a0, zero, positive
    sub  a0, zero, a0        # a0 = -a0
    ret
positive:
    ret
```

### 字符串长度

```assembly
# size_t strlen(const char *str)
.globl strlen
strlen:
    mv   t0, a0              # t0 = str
    li   t1, 0               # len = 0

loop:
    lbu  t2, 0(t0)           # t2 = *str
    beqz t2, end             # if (*str == '\0') 完成
    addi t0, t0, 1           # str++
    addi t1, t1, 1           # len++
    j    loop

end:
    mv   a0, t1              # 返回长度
    ret
```

### 内存拷贝

```assembly
# void* memcpy(void *dst, const void *src, size_t n)
.globl memcpy
memcpy:
    mv   t0, a0              # 保存dst
    beqz a2, end             # if n == 0, 返回

loop:
    lbu  t1, 0(a1)           # t1 = *src
    sb   t1, 0(a0)           # *dst = t1
    addi a0, a0, 1           # dst++
    addi a1, a1, 1           # src++
    addi a2, a2, -1          # n--
    bnez a2, loop            # if n != 0, 继续

end:
    mv   a0, t0              # 返回原始dst
    ret
```

## 调试技巧

### 1. 使用模拟器

使用RISC-V模拟器测试程序:
```bash
# 使用spike
spike pk hello

# 使用qemu
qemu-riscv64 ./hello
```

### 2. 反汇编

```bash
# 反汇编二进制
riscv64-unknown-elf-objdump -d hello.elf

# 查看符号
riscv64-unknown-elf-objdump -t hello.elf
```

### 3. 添加调试输出

```assembly
# 打印寄存器(伪代码)
# 假设有输出设备在0x10000
.macro PRINT_REG reg
    lui  x5, 0x10000
    sw   \reg, 0(x5)
.endm

# 使用
PRINT_REG x1
```

### 4. 单步执行

在VM中启用单步调试:
```rust
for _ in 0..100 {
    engine.execute_step()?;

    let pc = engine.pc();
    println!("PC: 0x{:x}", pc);
}
```

## 最佳实践

### 1. 命名约定

- **标签**: 使用描述性名称
  ```assembly
  loop_start:          # 好
  L1:                  # 不好
  ```

- **函数**: 使用下划线分隔
  ```assembly
  calculate_fibonacci: # 好
  CalcFib:             # 不好
  ```

### 2. 注释

```assembly
# 好的注释
add  x3, x1, x2    # x3 = x1 + x2 (计算总和)

# 不好的注释
add  x3, x1, x2    # 加法
```

### 3. 代码组织

```assembly
.section .data
    # 常量定义
    MAX_SIZE: .word 100

    # 全局变量
    counter: .word 0

.section .text
    # 主函数
    .globl _start
_start:
    call main
    ret

    # 其他函数
    .globl main
main:
    # ...
    ret
```

### 4. 性能优化

- 使用移位代替乘除(2的幂)
  ```assembly
  slli x1, x2, 3    # 好: x1 = x2 * 8
  mul  x1, x2, 8    # 慢
  ```

- 循环展开
  ```assembly
  # 展开4次
  ld   x1, 0(x10)
  ld   x2, 8(x10)
  ld   x3, 16(x10)
  ld   x4, 24(x10)
  addi x10, x10, 32
  ```

- 寄存器重用
  ```assembly
  # 好: 重用寄存器
  add x1, x2, x3
  sub x1, x1, x4

  # 不好: 不必要的临时寄存器
  add x5, x2, x3
  sub x1, x5, x4
  ```

## 相关资源

- [RISC-V规范](https://riscv.org/technical/specifications/)
- [RISC-V汇编程序员手册](https://github.com/riscv/riscv-asm-manual)
- [示例程序](../../examples/programs/riscv/)
- [高级用法](./ADVANCED_USAGE.md)

---

祝你编程愉快!
