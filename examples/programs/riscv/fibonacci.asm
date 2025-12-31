# Fibonacci计算 - RISC-V汇编示例
#
# 功能: 计算Fibonacci数列的前N项
#
# 算法:
#   fib[0] = 0
#   fib[1] = 1
#   fib[n] = fib[n-1] + fib[n-2]

.section .data
    # Fibonacci数组
    fib_array: .skip 100 * 8  # 预分配100个64位数的空间

    # 计数N
    n: .word 10

.section .text
    .globl _start

_start:
    # 初始化栈
    addi sp, sp, -16

    # 加载N
    la   t0, n
    lw   a0, 0(t0)       # a0 = N

    # 处理特殊情况: N <= 0
    blez a0, exit        # if N <= 0, 退出

    # 处理特殊情况: N == 1
    li   t1, 1
    bgt  a0, t1, init    # if N > 1, 跳转到初始化

    # N == 1, 只计算fib[0]
    la   t1, fib_array
    sd   zero, 0(t1)     # fib[0] = 0
    j    exit

init:
    # 初始化fib[0]和fib[1]
    la   t1, fib_array
    sd   zero, 0(t1)     # fib[0] = 0
    sd   t1, 8(t1)       # fib[1] = 1

    # 初始化循环
    li   t2, 2           # i = 2
    mv   t3, a0          # t3 = N (循环边界)

    # 加载fib[0]和fib[1]
    ld   t4, 0(t1)       # t4 = fib[i-2]
    ld   t5, 8(t1)       # t5 = fib[i-1]

loop:
    bge  t2, t3, done    # if i >= N, 完成

    # 计算fib[i] = fib[i-1] + fib[i-2]
    add  t6, t4, t5      # t6 = fib[i]

    # 存储结果: fib[i] = t6
    slli a1, t2, 3       # a1 = i * 8 (字节偏移)
    add  a1, t1, a1      # a1 = &fib[i]
    sd   t6, 0(a1)       # 存储fib[i]

    # 更新值: fib[i-2] = fib[i-1], fib[i-1] = fib[i]
    mv   t4, t5          # t4 = t5
    mv   t5, t6          # t5 = t6

    # 增加循环计数器
    addi t2, t2, 1
    j    loop

done:
    # 结果已在fib_array中
    # 在实际程序中,这里可以调用输出函数

exit:
    # 清理并退出
    addi sp, sp, 16
    li   a0, 0           # 返回码 0
    ret
