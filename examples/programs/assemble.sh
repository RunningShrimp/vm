#!/bin/bash
# RISC-V汇编程序编译脚本
#
# 功能: 将RISC-V汇编文件编译为二进制机器码
# 需要: RISC-V工具链 (riscv64-unknown-elf-*)

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查RISC-V工具链
check_toolchain() {
    echo -e "${YELLOW}检查RISC-V工具链...${NC}"

    if ! command -v riscv64-unknown-elf-as &> /dev/null; then
        echo -e "${RED}错误: 未找到RISC-V工具链${NC}"
        echo ""
        echo "请安装RISC-V工具链:"
        echo "  macOS:   brew install riscv-tools"
        echo "  Ubuntu:  sudo apt install gcc-riscv64-unknown-elf"
        echo "  或从源码编译: https://github.com/riscv-collab/riscv-gnu-toolchain"
        exit 1
    fi

    echo -e "${GREEN}✓ 找到RISC-V工具链${NC}"
    riscv64-unknown-elf-as --version | head -1
}

# 汇编单个文件
assemble_file() {
    local input=$1
    local output=${input%.asm}.o
    local bin=${input%.asm}.bin

    echo -e "${YELLOW}汇编: ${input}${NC}"

    # 汇编
    riscv64-unknown-elf-as -march=rv64imac -o "$output" "$input"

    # 转换为二进制
    riscv64-unknown-elf-objcopy -O binary "$output" "$bin"

    # 显示信息
    echo -e "${GREEN}✓ 生成: ${bin}${NC}"

    # 显示文件大小
    size=$(wc -c < "$bin")
    echo "  大小: ${size} 字节"

    # 清理中间文件
    rm -f "$output"

    echo ""
}

# 生成十六进制dump
generate_hexdump() {
    local bin=$1
    local hex=${bin%.bin}.hex

    echo -e "${YELLOW}生成hexdump: ${hex}${NC}"
    xxd -g 1 "$bin" > "$hex"
    echo -e "${GREEN}✓ 完成${NC}"
    echo ""
}

# 生成C数组(用于嵌入到Rust代码)
generate_c_array() {
    local bin=$1
    local array=${bin%.bin}.rs
    local name=$(basename "$bin" .bin)

    echo -e "${YELLOW}生成Rust数组: ${array}${NC}"

    cat > "$array" << EOF
// 自动生成的Rust数组
// 源文件: $(basename $bin)
// 大小: $(wc -c < $bin) 字节

pub const ${name^^}_CODE: &[u8] = &[
EOF

    xxd -i "$bin" | sed 's/0x/$0x/g' | sed 's/$/,/g' >> "$array"

    echo "];" >> "$array"

    echo -e "${GREEN}✓ 完成${NC}"
    echo ""
}

# 显示使用说明
show_usage() {
    cat << EOF
用法: $0 [选项] [文件...]

选项:
    -a, --all          编译所有.asm文件
    -h, --hex          生成hexdump
    -r, --rust         生成Rust数组
    -c, --check        仅检查工具链
    --help             显示此帮助信息

示例:
    $0 hello.asm              # 编译hello.asm
    $0 -a                      # 编译所有.asm文件
    $0 -a -h -r                # 编译所有并生成hex和rust数组
    $0 -c                      # 检查工具链

EOF
}

# 主函数
main() {
    local generate_hex=false
    local generate_rust=false
    local compile_all=false

    # 解析参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            -a|--all)
                compile_all=true
                shift
                ;;
            -h|--hex)
                generate_hex=true
                shift
                ;;
            -r|--rust)
                generate_rust=true
                shift
                ;;
            -c|--check)
                check_toolchain
                exit 0
                ;;
            --help)
                show_usage
                exit 0
                ;;
            *.asm)
                files+=("$1")
                shift
                ;;
            *)
                echo -e "${RED}未知选项: $1${NC}"
                show_usage
                exit 1
                ;;
        esac
    done

    # 检查工具链
    check_toolchain

    # 确定要编译的文件
    if [ "$compile_all" = true ]; then
        files=($(ls *.asm 2>/dev/null || true))
        if [ ${#files[@]} -eq 0 ]; then
            echo -e "${RED}错误: 未找到.asm文件${NC}"
            exit 1
        fi
    fi

    if [ ${#files[@]} -eq 0 ]; then
        echo -e "${RED}错误: 未指定文件${NC}"
        show_usage
        exit 1
    fi

    # 编译文件
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  RISC-V汇编编译器${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""

    for file in "${files[@]}"; do
        if [ ! -f "$file" ]; then
            echo -e "${RED}错误: 文件不存在: $file${NC}"
            continue
        fi

        local bin="${file%.asm}.bin"
        assemble_file "$file"

        if [ "$generate_hex" = true ]; then
            generate_hexdump "$bin"
        fi

        if [ "$generate_rust" = true ]; then
            generate_c_array "$bin"
        fi
    done

    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  编译完成!${NC}"
    echo -e "${GREEN}========================================${NC}"
}

# 运行
main "$@"
