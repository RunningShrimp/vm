# Hello World - RISC-V汇编示例
#
# 功能: 在控制台输出"Hello, World!"
#
# 说明:
# - 这是一个简化的示例,假设存在一个字符输出设备
# - 实际输出取决于VM的设备配置
# - 演示基本的字符串操作和循环

.section .data
    # 字符串数据
    msg: .asciz "Hello, World!\n"
    msg_len = . - msg

.section .text
    .globl _start

_start:
    # 准备参数
    la   a0, msg        # 加载消息地址到a0
    li   a1, msg_len    # 加载消息长度到a1

    # 调用输出函数(假设存在)
    jal  ra, print_string

    # 退出
    li   a0, 0          # 返回码 0
    ret

# 字符串输出函数
# 参数: a0 = 字符串地址, a1 = 长度
print_string:
    # 保存寄存器
    addi sp, sp, -16
    sd   ra, 8(sp)
    sd   s0, 0(sp)

    mv   s0, a0         # 保存字符串地址
    mv   s1, a1         # 保存长度
    mv   t0, zero       # 循环计数器 = 0

loop:
    bge  t0, s1, end    # 如果计数 >= 长度,退出

    # 加载字符
    add  t1, s0, t0     # 计算字符地址
    lbu  a0, 0(t1)      # 加载字节到a0

    # 输出字符(假设设备在特定地址)
    # lui  t2, 0x10000   # 设备基址
    # sb   a0, 0(t2)     # 写字符到设备

    # 增加计数器
    addi t0, t0, 1
    j    loop

end:
    # 恢复寄存器
    ld   ra, 8(sp)
    ld   s0, 0(sp)
    addi sp, sp, 16

    ret
