# 质数计算 - RISC-V汇编示例
#
# 功能: 计算小于等于N的所有质数
#
# 算法: 埃拉托斯特尼筛法 (Sieve of Eratosthenes)

.section .data
    # 筛数组 (0表示质数, 1表示合数)
    sieve: .skip 1000    # 可以计算到1000以内的质数

    # 上限N
    limit: .word 100

    # 质数计数
    prime_count: .word 0

.section .text
    .globl _start

_start:
    # 初始化栈
    addi sp, sp, -32

    # 加载上限
    la   t0, limit
    lw   a0, 0(t0)       # a0 = N

    # 初始化筛数组(全部设为0)
    la   t1, sieve
    mv   t2, a0          # t2 = N

init_loop:
    beqz t2, init_done   # if t2 == 0, 完成
    sb   zero, 0(t1)     # sieve[i] = 0
    addi t1, t1, 1
    addi t2, t2, -1
    j    init_loop

init_done:
    # 埃拉托斯特尼筛法
    li   t0, 2           # 从2开始

outer_loop:
    # 计算i*i
    mul  t1, t0, t0
    la   t2, limit
    lw   t2, 0(t2)       # t2 = N
    bgt  t1, t2, sieve_done  # if i*i > N, 完成

    # 检查sieve[i]是否为0
    la   t3, sieve
    add  t3, t3, t0
    lbu  t4, 0(t3)
    bnez t4, next_i      # if sieve[i] != 0, 跳到下一个i

    # 标记所有i的倍数为合数
    mv   t5, t1          # j = i*i

mark_loop:
    la   t6, limit
    lw   t6, 0(t6)       # t6 = N
    bgt  t5, t6, mark_done  # if j > N, 完成

    # 标记sieve[j] = 1
    la   t6, sieve
    add  t6, t6, t5
    li   t7, 1
    sb   t7, 0(t6)       # sieve[j] = 1

    # j += i
    add  t5, t5, t0
    j    mark_loop

mark_done:
next_i:
    addi t0, t0, 1       # i++
    j    outer_loop

sieve_done:
    # 计算质数个数
    la   t0, sieve
    la   t1, limit
    lw   t1, 0(t1)       # t1 = N
    mv   t2, zero        # count = 0

count_loop:
    blez t1, count_done  # if N <= 0, 完成
    lbu  t3, 0(t0)
    beqz t3, is_prime    # if sieve[i] == 0, 是质数
    j    next_count

is_prime:
    addi t2, t2, 1       # count++

next_count:
    addi t0, t0, 1
    addi t1, t1, -1
    j    count_loop

count_done:
    # 保存质数计数
    la   t0, prime_count
    sw   t2, 0(t0)

    # 退出
    addi sp, sp, 32
    li   a0, 0
    ret

# 辅助函数: 检查一个数是否为质数
# 参数: a0 = 数字
# 返回: a0 = 1(是质数) 或 0(不是质数)
is_prime:
    # 保存寄存器
    addi sp, sp, -16
    sd   ra, 8(sp)
    sd   s0, 0(sp)

    blez a0, not_prime   # <= 0不是质数
    li   t0, 1
    beq  a0, t0, not_prime  # 1不是质数
    li   t0, 2
    beq  a0, t0, is_prime_2  # 2是质数

    # 检查是否能被2整除
    rem  t1, a0, t0
    beqz t1, not_prime

    # 检查奇数因子
    li   t0, 3
    mv   t1, a0

check_divisor:
    # 如果i*i > n, 是质数
    mul  t2, t0, t0
    bgt  t2, t1, prime_yes

    # 检查n % i == 0
    rem  t2, t1, t0
    beqz t2, not_prime

    # i += 2
    addi t0, t0, 2
    j    check_divisor

prime_yes:
    li   a0, 1
    j    prime_exit

not_prime:
    li   a0, 0

prime_exit:
    # 恢复寄存器
    ld   ra, 8(sp)
    ld   s0, 0(sp)
    addi sp, sp, 16
    ret
