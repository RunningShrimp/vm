#!/bin/bash

# 后端检测和验证脚本
# 检测系统中可用的编译器后端并验证其功能

set -e

echo "🔍 后端检测和验证脚本"
echo "======================"

# 配置文件路径
CONFIG_FILE="scripts/backend_config.json"
DETECTION_RESULTS="scripts/backend_detection.json"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检测结果数组
declare -A BACKEND_STATUS
declare -A BACKEND_VERSION
declare -A BACKEND_PATH

# 检测LLVM后端
detect_llvm() {
    echo -e "${BLUE}🔍 检测 LLVM 后端...${NC}"
    
    local llvm_found=false
    local llvm_version=""
    local llvm_path=""
    
    # 检查 llvm-config
    if command -v llvm-config &> /dev/null; then
        llvm_found=true
        llvm_version=$(llvm-config --version 2>/dev/null || echo "未知")
        llvm_path=$(which llvm-config)
        echo -e "  ${GREEN}✅ llvm-config 找到: $llvm_path${NC}"
        echo -e "  ${GREEN}   版本: $llvm_version${NC}"
    else
        echo -e "  ${RED}❌ llvm-config 未找到${NC}"
        
        # 尝试常见的LLVM安装路径
        local common_paths=(
            "/usr/bin/llvm-config"
            "/usr/local/bin/llvm-config"
            "/opt/homebrew/bin/llvm-config"
            "/usr/lib/llvm-18/bin/llvm-config"
            "/usr/lib/llvm-17/bin/llvm-config"
            "/usr/lib/llvm-16/bin/llvm-config"
        )
        
        for path in "${common_paths[@]}"; do
            if [[ -f "$path" ]]; then
                llvm_found=true
                llvm_version=$("$path" --version 2>/dev/null || echo "未知")
                llvm_path="$path"
                echo -e "  ${YELLOW}⚠️  在常见路径找到: $path${NC}"
                echo -e "  ${YELLOW}   版本: $llvm_version${NC}"
                break
            fi
        done
    fi
    
    # 检查 clang
    if command -v clang &> /dev/null; then
        local clang_version=$(clang --version | head -n1)
        echo -e "  ${GREEN}✅ clang 找到: $(which clang)${NC}"
        echo -e "  ${GREEN}   版本: $clang_version${NC}"
    else
        echo -e "  ${RED}❌ clang 未找到${NC}"
    fi
    
    # 检查环境变量
    if [[ -n "$LLVM_SYS_211_PREFIX" ]]; then
        echo -e "  ${GREEN}✅ LLVM_SYS_211_PREFIX 设置: $LLVM_SYS_211_PREFIX${NC}"
    else
        echo -e "  ${YELLOW}⚠️  LLVM_SYS_211_PREFIX 未设置${NC}"
    fi
    
    # 检查LLVM库
    if [[ -n "$LLVM_SYS_211_PREFIX" && -d "$LLVM_SYS_211_PREFIX/lib" ]]; then
        local lib_count=$(find "$LLVM_SYS_211_PREFIX/lib" -name "libLLVM*.so" -o -name "libLLVM*.dylib" 2>/dev/null | wc -l)
        if [[ $lib_count -gt 0 ]]; then
            echo -e "  ${GREEN}✅ LLVM 库找到 ($lib_count 个)${NC}"
        else
            echo -e "  ${RED}❌ LLVM 库未找到${NC}"
        fi
    fi
    
    # 保存检测结果
    if [[ "$llvm_found" == true ]]; then
        BACKEND_STATUS["llvm"]="available"
        BACKEND_VERSION["llvm"]="$llvm_version"
        BACKEND_PATH["llvm"]="$llvm_path"
    else
        BACKEND_STATUS["llvm"]="unavailable"
        BACKEND_VERSION["llvm"]=""
        BACKEND_PATH["llvm"]=""
    fi
    
    echo ""
}

# 检测Cranelift后端
detect_cranelift() {
    echo -e "${BLUE}🔍 检测 Cranelift 后端...${NC}"
    
    local cranelift_available=false
    local cargo_version=""
    
    # 检查 Cargo
    if command -v cargo &> /dev/null; then
        cargo_version=$(cargo --version)
        echo -e "  ${GREEN}✅ Cargo 找到: $(which cargo)${NC}"
        echo -e "  ${GREEN}   版本: $cargo_version${NC}"
        
        # 检查 Cranelift crate 可用性
        if cargo search cranelift --limit 1 &> /dev/null; then
            echo -e "  ${GREEN}✅ Cranelift crate 可用${NC}"
            cranelift_available=true
        else
            echo -e "  ${YELLOW}⚠️  无法验证 Cranelift crate 可用性${NC}"
        fi
        
        # 检查网络连接
        if curl -s --head https://crates.io > /dev/null; then
            echo -e "  ${GREEN}✅ crates.io 可访问${NC}"
        else
            echo -e "  ${YELLOW}⚠️  crates.io 不可访问，可能影响 crate 下载${NC}"
        fi
    else
        echo -e "  ${RED}❌ Cargo 未找到${NC}"
        echo -e "  ${YELLOW}   请安装 Rust: https://rustup.rs/${NC}"
    fi
    
    # 保存检测结果
    if [[ "$cranelift_available" == true ]]; then
        BACKEND_STATUS["cranelift"]="available"
        BACKEND_VERSION["cranelift"]="latest"
        BACKEND_PATH["cranelift"]="cargo"
    else
        BACKEND_STATUS["cranelift"]="unavailable"
        BACKEND_VERSION["cranelift"]=""
        BACKEND_PATH["cranelift"]=""
    fi
    
    echo ""
}

# 检测系统信息
detect_system_info() {
    echo -e "${BLUE}🔍 检测系统信息...${NC}"
    
    local os=$(uname -s)
    local arch=$(uname -m)
    local kernel=$(uname -r)
    
    echo -e "  操作系统: $os"
    echo -e "  架构: $arch"
    echo -e "  内核版本: $kernel"
    
    # 检测内存
    if [[ "$os" == "Darwin" ]]; then
        local memory=$(sysctl -n hw.memsize | awk '{printf "%.1f GB", $1/1024/1024/1024}')
        echo -e "  内存: $memory"
    elif [[ "$os" == "Linux" ]]; then
        local memory=$(free -h | awk '/^Mem:/ {print $2}')
        echo -e "  内存: $memory"
    fi
    
    # 检测CPU核心数
    local cores=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo "未知")
    echo -e "  CPU核心数: $cores"
    
    echo ""
}

# 验证后端功能
verify_backend_functionality() {
    local backend="$1"
    
    echo -e "${BLUE}🧪 验证 $backend 后端功能...${NC}"
    
    case "$backend" in
        "llvm")
            # 尝试编译一个简单的LLVM程序
            if command -v clang &> /dev/null; then
                local test_file="/tmp/llvm_test.c"
                cat > "$test_file" << 'EOF'
#include <stdio.h>
int main() {
    printf("LLVM test successful\n");
    return 0;
}
EOF
                
                if clang -o "/tmp/llvm_test" "$test_file" &> /dev/null; then
                    if "/tmp/llvm_test" &> /dev/null; then
                        echo -e "  ${GREEN}✅ LLVM 编译测试通过${NC}"
                    else
                        echo -e "  ${RED}❌ LLVM 运行测试失败${NC}"
                    fi
                    rm -f "/tmp/llvm_test" "$test_file"
                else
                    echo -e "  ${RED}❌ LLVM 编译测试失败${NC}"
                fi
            fi
            ;;
        "cranelift")
            # 检查是否可以创建新的Rust项目
            if command -v cargo &> /dev/null; then
                local test_dir="/tmp/cranelift_test"
                if cargo new --bin "$test_dir" &> /dev/null; then
                    echo -e "  ${GREEN}✅ Cargo 项目创建测试通过${NC}"
                    rm -rf "$test_dir"
                else
                    echo -e "  ${RED}❌ Cargo 项目创建测试失败${NC}"
                fi
            fi
            ;;
    esac
    
    echo ""
}

# 生成检测报告
generate_detection_report() {
    echo -e "${BLUE}📝 生成检测报告...${NC}"
    
    # 创建JSON报告
    cat > "$DETECTION_RESULTS" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "system": {
    "os": "$(uname -s)",
    "arch": "$(uname -m)",
    "kernel": "$(uname -r)"
  },
  "backends": {
EOF
    
    first=true
    for backend in "llvm" "cranelift"; do
        if [[ "$first" == false ]]; then echo "," >> "$DETECTION_RESULTS"; fi
        cat >> "$DETECTION_RESULTS" << EOF
    "$backend": {
      "status": "${BACKEND_STATUS[$backend]}",
      "version": "${BACKEND_VERSION[$backend]}",
      "path": "${BACKEND_PATH[$backend]}"
    }
EOF
        first=false
    done
    
    cat >> "$DETECTION_RESULTS" << EOF
  },
  "recommendations": [
EOF
    
    # 生成建议
    local recommendations=()
    
    if [[ "${BACKEND_STATUS[llvm]}" == "unavailable" ]]; then
        recommendations+=("考虑安装LLVM以获得更好的性能")
    fi
    
    if [[ "${BACKEND_STATUS[cranelift]}" == "unavailable" ]]; then
        recommendations+=("考虑安装Rust和Cargo以使用Cranelift后端")
    fi
    
    if [[ "${BACKEND_STATUS[llvm]}" == "available" && "${BACKEND_STATUS[cranelift]}" == "available" ]]; then
        recommendations+=("系统支持多种后端，可以根据需求选择")
    fi
    
    first=true
    for rec in "${recommendations[@]}"; do
        if [[ "$first" == false ]]; then echo "," >> "$DETECTION_RESULTS"; fi
        echo "    \"$rec\"" >> "$DETECTION_RESULTS"
        first=false
    done
    
    cat >> "$DETECTION_RESULTS" << EOF
  ]
}
EOF
    
    echo -e "  ${GREEN}✅ 检测报告已生成: $DETECTION_RESULTS${NC}"
    echo ""
}

# 显示摘要
show_summary() {
    echo -e "${BLUE}📊 检测摘要${NC}"
    echo "============"
    
    for backend in "llvm" "cranelift"; do
        local status="${BACKEND_STATUS[$backend]}"
        local version="${BACKEND_VERSION[$backend]}"
        
        case "$status" in
            "available")
                echo -e "$backend: ${GREEN}✅ 可用${NC} (版本: $version)"
                ;;
            "unavailable")
                echo -e "$backend: ${RED}❌ 不可用${NC}"
                ;;
            *)
                echo -e "$backend: ${YELLOW}⚠️  未知状态${NC}"
                ;;
        esac
    done
    
    echo ""
    
    # 显示推荐的后端
    if [[ "${BACKEND_STATUS[cranelift]}" == "available" ]]; then
        echo -e "${GREEN}推荐: 使用 Cranelift 后端${NC}"
        echo "  命令: cargo build --features cranelift-backend"
    elif [[ "${BACKEND_STATUS[llvm]}" == "available" ]]; then
        echo -e "${GREEN}推荐: 使用 LLVM 后端${NC}"
        echo "  命令: cargo build --features llvm"
    else
        echo -e "${RED}警告: 没有可用的后端${NC}"
        echo "  请运行: ./scripts/install_backend.sh"
    fi
    
    echo ""
}

# 主函数
main() {
    # 检测系统信息
    detect_system_info
    
    # 检测各个后端
    detect_llvm
    detect_cranelift
    
    # 验证后端功能
    for backend in "llvm" "cranelift"; do
        if [[ "${BACKEND_STATUS[$backend]}" == "available" ]]; then
            verify_backend_functionality "$backend"
        fi
    done
    
    # 生成报告
    generate_detection_report
    
    # 显示摘要
    show_summary
}

# 运行主函数
main "$@"