# 矩阵乘法 - RISC-V汇编示例
#
# 功能: 计算两个矩阵的乘积 C = A * B
#
# 矩阵大小: 3x3
# 算法: 标准的三重循环矩阵乘法

.section .data
    # 矩阵A (3x3)
    # 1 2 3
    # 4 5 6
    # 7 8 9
    matrix_a:
        .quad 1, 2, 3
        .quad 4, 5, 6
        .quad 7, 8, 9

    # 矩阵B (3x3)
    # 9 8 7
    # 6 5 4
    # 3 2 1
    matrix_b:
        .quad 9, 8, 7
        .quad 6, 5, 4
        .quad 3, 2, 1

    # 结果矩阵C (3x3)
    matrix_c: .skip 9 * 8

    # 矩阵维度
    SIZE: .word 3

.section .text
    .globl _start

_start:
    # 初始化栈
    addi sp, sp, -64

    # 加载矩阵维度
    la   t0, SIZE
    lw   s0, 0(t0)       # s0 = SIZE (n)

    # 矩阵基址
    la   s1, matrix_a    # s1 = &A
    la   s2, matrix_b    # s2 = &B
    la   s3, matrix_c    # s3 = &C

    # i循环(行)
    li   t1, 0           # i = 0

i_loop:
    bge  t1, s0, i_done  # if i >= n, 退出

    # j循环(列)
    li   t2, 0           # j = 0

j_loop:
    bge  t2, s0, j_next  # if j >= n, j循环结束

    # 初始化累加器
    li   t3, 0           # sum = 0

    # k循环
    li   t4, 0           # k = 0

k_loop:
    bge  t4, s0, k_done  # if k >= n, k循环结束

    # 计算A[i][k]的地址: &A + (i*n + k) * 8
    mul  t5, t1, s0      # t5 = i * n
    add  t5, t5, t4      # t5 = i*n + k
    slli t5, t5, 3       # t5 = (i*n + k) * 8
    add  t5, s1, t5      # t5 = &A[i][k]
    ld   a0, 0(t5)       # a0 = A[i][k]

    # 计算B[k][j]的地址: &B + (k*n + j) * 8
    mul  t6, t4, s0      # t6 = k * n
    add  t6, t6, t2      # t6 = k*n + j
    slli t6, t6, 3       # t6 = (k*n + j) * 8
    add  t6, s2, t6      # t6 = &B[k][j]
    ld   a1, 0(t6)       # a1 = B[k][j]

    # 累加: sum += A[i][k] * B[k][j]
    mul  a2, a0, a1      # a2 = A[i][k] * B[k][j]
    add  t3, t3, a2      # sum += a2

    # k++
    addi t4, t4, 1
    j    k_loop

k_done:
    # 存储结果到C[i][j]
    # 地址: &C + (i*n + j) * 8
    mul  t5, t1, s0      # t5 = i * n
    add  t5, t5, t2      # t5 = i*n + j
    slli t5, t5, 3       # t5 = (i*n + j) * 8
    add  t5, s3, t5      # t5 = &C[i][j]
    sd   t3, 0(t5)       # C[i][j] = sum

    # j++
    addi t2, t2, 1
    j    j_loop

j_next:
    addi t1, t1, 1
    j    i_loop

i_done:
    # 计算完成,结果在matrix_c中

    # 退出
    addi sp, sp, 64
    li   a0, 0           # 返回码 0
    ret

# 辅助函数: 打印矩阵(调试用)
# 参数: a0 = 矩阵地址, a1 = 维度
print_matrix:
    # 保存寄存器
    addi sp, sp, -32
    sd   ra, 24(sp)
    sd   s0, 16(sp)
    sd   s1, 8(sp)
    sd   s2, 0(sp)

    mv   s0, a0          # 保存矩阵地址
    mv   s1, a1          # 保存维度

    li   t0, 0           # 行i = 0

print_i_loop:
    bge  t0, s1, print_i_done

    li   t1, 0           # 列j = 0

print_j_loop:
    bge  t1, s1, print_j_done

    # 计算元素地址
    mul  t2, t0, s1
    add  t2, t2, t1
    slli t2, t2, 3
    add  t2, s0, t2

    # 加载元素(这里应该调用输出函数)
    ld   a0, 0(t2)

    # 在实际实现中,这里会将a0转换为字符串并输出
    # ...

    addi t1, t1, 1
    j    print_j_loop

print_j_done:
    # 输出换行
    # ...

    addi t0, t0, 1
    j    print_i_loop

print_i_done:
    # 恢复寄存器
    ld   ra, 24(sp)
    ld   s0, 16(sp)
    ld   s1, 8(sp)
    ld   s2, 0(sp)
    addi sp, sp, 32
    ret
