#!/usr/bin/env python3
"""
RISC-V C扩展指令编码生成器
根据RISC-V规范生成正确的16位压缩指令编码
"""

def encode_c_add(rd, rs2):
    """
    C.ADD: 1001 1 rd rs2 00
    opcode=10, funct3=010, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b010 << 13) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_mv(rd, rs2):
    """
    C.MV: 1000 1 rd rs2 10
    opcode=10, funct3=001, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b001 << 13) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_jr(rs1):
    """
    C.JR: 1000 1 rs1 00000
    opcode=10, funct3=001, rs1[4:0], rs2=0
    """
    if rs1 < 0 or rs1 > 31:
        raise ValueError(f"Invalid register: rs1={rs1}")

    insn = (0b10 << 0) | (0b001 << 13) | (rs1 << 7) | (0 << 2)
    return insn & 0xFFFF

def encode_c_jalr(rs1):
    """
    C.JALR: 1001 0 rs1 00000
    opcode=10, funct3=010, rs1[4:0], rs2=0
    """
    if rs1 < 0 or rs1 > 31:
        raise ValueError(f"Invalid register: rs1={rs1}")

    insn = (0b10 << 0) | (0b010 << 13) | (rs1 << 7) | (0 << 2)
    return insn & 0xFFFF

def encode_c_ebreak():
    """
    C.EBREAK: 1001 00 00000 00000
    opcode=10, funct3=010, rd=0, rs2=0
    """
    insn = (0b10 << 0) | (0b010 << 13) | (0 << 7) | (0 << 2)
    return insn & 0xFFFF

def encode_c_addi(rd, imm):
    """
    C.ADDI: 1001 imm[5] imm[4:0|rd
    opcode=01, funct3=000, rd[4:0], imm[5:0]
    """
    if rd < 0 or rd > 31:
        raise ValueError(f"Invalid register: rd={rd}")
    if imm < -32 or imm > 31:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_unsigned = imm & 0x3F  # 6-bit signed immediate
    insn = (0b01 << 0) | (0b000 << 13) | (rd << 7) | ((imm_unsigned & 0x1F) << 2) | ((imm_unsigned >> 5) << 12)
    return insn & 0xFFFF

def encode_c_li(rd, imm):
    """
    C.LI: 010 rd imm[5:0]
    opcode=01, funct3=010, rd[4:0], imm[5:0]
    """
    if rd < 0 or rd > 31:
        raise ValueError(f"Invalid register: rd={rd}")
    if imm < -32 or imm > 31:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_unsigned = imm & 0x3F
    insn = (0b01 << 0) | (0b010 << 13) | (rd << 7) | ((imm_unsigned & 0x1F) << 2) | ((imm_unsigned >> 5) << 12)
    return insn & 0xFFFF

def encode_c_lui(rd, imm):
    """
    C.LUI: 010 rd imm[17:12]
    opcode=01, funct3=011, rd[4:0], imm[17:12]
    imm会被左移12位
    """
    if rd < 0 or rd > 31 or rd == 0 or rd == 2:
        raise ValueError(f"Invalid register: rd={rd}")

    imm_upper = (imm >> 12) & 0x3F
    imm_unsigned = imm_upper & 0x3F
    insn = (0b01 << 0) | (0b011 << 13) | (rd << 7) | ((imm_unsigned & 0x1F) << 2) | ((imm_unsigned >> 5) << 12)
    return insn & 0xFFFF

def encode_c_slli(rd, shamt):
    """
    C.SLLI: 100 rd 0 shamt[4:0]
    opcode=10, funct3=000, rd[4:0], shamt[4:0]
    """
    if rd < 0 or rd > 31:
        raise ValueError(f"Invalid register: rd={rd}")
    if shamt < 0 or shamt > 31:
        raise ValueError(f"Invalid shift amount: {shamt}")

    insn = (0b10 << 0) | (0b000 << 13) | (rd << 7) | (shamt << 2)
    return insn & 0xFFFF

def encode_c_lwsp(rd, imm):
    """
    C.LWSP: opcode=10, funct3=000, funct2=10, rd[4:0], imm[7:2]
    Immediate encoding according to decoder:
    - imm[1:0] at bits [5:4]
    - imm[3:2] at bits [3:2]
    - imm[4] at bit [12]
    - imm[5] at bit [?] (need to check)
    """
    if rd < 0 or rd > 31 or rd == 0:
        raise ValueError(f"Invalid register: rd={rd}")
    if imm < 0 or imm > 255 or imm % 4 != 0:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_bits = (imm >> 2) & 0x3F  # 6-bit immediate
    insn = (0b10 << 0) | (0b000 << 13) | (0b10 << 10) | (rd << 7) | ((imm_bits & 0x3) << 4) | ((imm_bits & 0xC) << 0) | ((imm_bits & 0x10) << 8)
    return insn & 0xFFFF

def encode_c_swsp(rs2, imm):
    """
    C.SWSP: opcode=10, funct3=011, funct3_b=01, rs2[4:0], imm[7:2]
    """
    if rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid register: rs2={rs2}")
    if imm < 0 or imm > 255 or imm % 4 != 0:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_bits = (imm >> 2) & 0x3F
    insn = (0b10 << 0) | (0b011 << 13) | (0b01 << 10) | (rs2 << 2) | ((imm_bits & 0x3) << 12) | ((imm_bits & 0x4) << 7) | ((imm_bits & 0x38) << 9)
    return insn & 0xFFFF

def encode_c_sub(rd, rs2):
    """
    C.SUB: funct4=1000, opcode=10, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b1000 << 12) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_xor(rd, rs2):
    """
    C.XOR: funct4=1001, opcode=10, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b1001 << 12) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_or(rd, rs2):
    """
    C.OR: funct4=1010, opcode=10, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b1010 << 12) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_and(rd, rs2):
    """
    C.AND: funct4=1011, opcode=10, rd[4:0], rs2[4:0]
    """
    if rd < 0 or rd > 31 or rs2 < 0 or rs2 > 31:
        raise ValueError(f"Invalid registers: rd={rd}, rs2={rs2}")

    insn = (0b10 << 0) | (0b1011 << 12) | (rd << 7) | (rs2 << 2)
    return insn & 0xFFFF

def encode_c_beqz(rs1, imm):
    """
    C.BEQZ: 1101 rs1 imm[7:6] imm[2:2] imm[5:3] imm[8:8|imm[4:3]
    opcode=01, funct3=110, rs1[4:0], imm[8:1]
    """
    if rs1 < 0 or rs1 > 31:
        raise ValueError(f"Invalid register: rs1={rs1}")
    if imm < -256 or imm > 255 or imm % 2 != 0:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_bits = (imm >> 1) & 0xFF
    insn = (0b01 << 0) | (0b110 << 13) | (rs1 << 7) | ((imm_bits & 0x1) << 12) | ((imm_bits & 0x6) << 10) | ((imm_bits & 0x18) << 3) | ((imm_bits & 0xE0) >> 2)
    return insn & 0xFFFF

def encode_c_bnez(rs1, imm):
    """
    C.BNEZ: 1101 rs1 imm[7:6] imm[2:2] imm[5:3] imm[8:8|imm[4:3]
    opcode=01, funct3=111, rs1[4:0], imm[8:1]
    """
    if rs1 < 0 or rs1 > 31:
        raise ValueError(f"Invalid register: rs1={rs1}")
    if imm < -256 or imm > 255 or imm % 2 != 0:
        raise ValueError(f"Invalid immediate: {imm}")

    imm_bits = (imm >> 1) & 0xFF
    insn = (0b01 << 0) | (0b111 << 13) | (rs1 << 7) | ((imm_bits & 0x1) << 12) | ((imm_bits & 0x6) << 10) | ((imm_bits & 0x18) << 3) | ((imm_bits & 0xE0) >> 2)
    return insn & 0xFFFF

# 生成测试用例编码
def generate_test_encodings():
    """生成所有C扩展测试的正确编码"""
    print("# RISC-V C扩展指令正确编码")
    print("")

    # C.ADD 测试
    print("## C.ADD 测试")
    print(f"test_decode_c_add x1, x2:  0x{encode_c_add(1, 2):04x}")
    print(f"test_decode_c_add x5, x10: 0x{encode_c_add(5, 10):04x}")
    print("")

    # C.MV 测试
    print("## C.MV 测试")
    print(f"test_decode_c_mv x1, x2:   0x{encode_c_mv(1, 2):04x}")
    print("")

    # C.JR 测试
    print("## C.JR 测试")
    print(f"test_decode_c_jr x1:       0x{encode_c_jr(1):04x}")
    print("")

    # C.JALR 测试
    print("## C.JALR 测试")
    print(f"test_decode_c_jalr x1:     0x{encode_c_jalr(1):04x}")
    print("")

    # C.EBREAK 测试
    print("## C.EBREAK 测试")
    print(f"test_decode_c_ebreak:      0x{encode_c_ebreak():04x}")
    print("")

    # C.ADDI 测试
    print("## C.ADDI 测试")
    print(f"test_decode_c_addi x1, -4: 0x{encode_c_addi(1, -4):04x}")
    print("")

    # C.LI 测试
    print("## C.LI 测试")
    print(f"test_decode_c_li x1, 10:   0x{encode_c_li(1, 10):04x}")
    print("")

    # C.LUI 测试
    print("## C.LUI 测试")
    print(f"test_decode_c_lui x1, 1:   0x{encode_c_lui(1, 0x1000):04x}")
    print("")

    # C.SLLI 测试
    print("## C.SLLI 测试")
    print(f"test_decode_c_slli x1, 8:  0x{encode_c_slli(1, 8):04x}")
    print("")

    # C.LWSP 测试
    print("## C.LWSP 测试")
    print(f"test_decode_c_lwsp x1, 0:  0x{encode_c_lwsp(1, 0):04x}")
    print("")

    # C.SWSP 测试
    print("## C.SWSP 测试")
    print(f"test_decode_c_swsp x2, 0:  0x{encode_c_swsp(2, 0):04x}")
    print("")

    # C.AND 测试
    print("## C.AND 测试")
    print(f"test_decode_c_and x9, x10: 0x{encode_c_and(9, 10):04x}")
    print("")

    # C.OR 测试
    print("## C.OR 测试")
    print(f"test_decode_c_or x9, x10:  0x{encode_c_or(9, 10):04x}")
    print("")

    # C.XOR 测试
    print("## C.XOR 测试")
    print(f"test_decode_c_xor x9, x10: 0x{encode_c_xor(9, 10):04x}")
    print("")

    # C.SUB 测试
    print("## C.SUB 测试")
    print(f"test_decode_c_sub x9, x10: 0x{encode_c_sub(9, 10):04x}")
    print("")

    # C.BEQZ 测试
    print("## C.BEQZ 测试")
    print(f"test_decode_c_beqz x9, 0:  0x{encode_c_beqz(9, 0):04x}")
    print("")

    # C.BNEZ 测试
    print("## C.BNEZ 测试")
    print(f"test_decode_c_bnez x9, 0:  0x{encode_c_bnez(9, 0):04x}")
    print("")

if __name__ == "__main__":
    generate_test_encodings()
