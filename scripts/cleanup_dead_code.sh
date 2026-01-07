#!/bin/bash
# 死代码和未使用依赖清理脚本
# 会话15 - P0任务#5

set -e  # 遇到错误立即退出

echo "================================================"
echo "  VM项目 - 死代码和未使用依赖清理脚本"
echo "  会话15 - P0任务#5"
echo "================================================"
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 1. 备份当前状态
echo -e "${YELLOW}步骤 1/6: 备份当前状态${NC}"
echo "----------------------------------------"
git stash save "Auto-backup before dead code cleanup - Session 15"
echo "✅ 备份完成"
echo ""

# 2. 创建清理分支
echo -e "${YELLOW}步骤 2/6: 创建清理分支${NC}"
echo "----------------------------------------"
BRANCH_NAME="cleanup/dead-code-session15"
if git rev-parse --verify "$BRANCH_NAME" >/dev/null 2>&1; then
    echo "分支 $BRANCH_NAME 已存在,切换到该分支"
    git checkout "$BRANCH_NAME"
else
    echo "创建新分支 $BRANCH_NAME"
    git checkout -b "$BRANCH_NAME"
fi
echo "✅ 分支准备完成"
echo ""

# 3. 分析未使用依赖
echo -e "${YELLOW}步骤 3/6: 分析未使用依赖${NC}"
echo "----------------------------------------"
if ! command -v cargo-machete &> /dev/null; then
    echo -e "${RED}错误: cargo-machete 未安装${NC}"
    echo "请运行: cargo install cargo-machete"
    exit 1
fi

echo "运行 cargo machete 分析..."
cargo machete > /tmp/machete_output.txt 2>&1
echo "✅ 分析完成,结果保存到 /tmp/machete_output.txt"
echo ""

# 4. 显示将要清理的依赖
echo -e "${YELLOW}步骤 4/6: 显示清理计划${NC}"
echo "----------------------------------------"
echo "发现的未使用依赖 (排除vm-build-deps):"
grep -v "vm-build-deps" /tmp/machete_output.txt | grep -v "IO error" | grep -v "Analyzing" | grep "\-\-" -A 10 | head -50
echo ""
TOTAL=$(grep -v "vm-build-deps" /tmp/machete_output.txt | grep -v "IO error" | grep -v "Analyzing" | grep "\-\-" -A 10 | grep "^  [a-z]" | wc -l | tr -d ' ')
echo "总计: $TOTAL 个未使用依赖"
echo ""

# 询问用户确认
read -p "是否继续清理? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}取消清理操作${NC}"
    exit 0
fi

# 5. 执行清理 (仅安全清理)
echo -e "${YELLOW}步骤 5/6: 执行安全清理${NC}"
echo "----------------------------------------"

# 定义安全清理的crate和依赖
SAFE_CLEANUP=(
    "vm-smmu:anyhow"
    "vm-smmu:log"
    "vm-smmu:parking_lot"
    "vm-smmu:serde"
    "vm-smmu:vm-platform"
    "vm-service:anyhow"
    "vm-service:uuid"
    "vm-plugin:parking_lot"
    "vm-plugin:reqwest"
    "vm-passthrough:serde"
    "vm-passthrough:serde_json"
    "vm-passthrough:vm-core"
    "vm-osal:vm-core"
    "vm-graphics:serde"
    "vm-graphics:vm-core"
    "vm-platform:serde_json"
)

CLEANED_COUNT=0

for item in "${SAFE_CLEANUP[@]}"; do
    IFS=':' read -r crate dependency <<< "$item"
    cargo_file="$crate/Cargo.toml"

    if [ -f "$cargo_file" ]; then
        echo "清理 $crate 中的 $dependency ..."

        # 使用sed移除依赖行
        sed -i.bak "/^$dependency = /d" "$cargo_file"

        # 清理备份文件
        rm -f "${cargo_file}.bak"

        CLEANED_COUNT=$((CLEANED_COUNT + 1))
    else
        echo -e "${RED}警告: $cargo_file 不存在${NC}"
    fi
done

echo ""
echo "✅ 安全清理完成,清理了 $CLEANED_COUNT 个依赖"
echo ""

# 6. 验证编译
echo -e "${YELLOW}步骤 6/6: 验证编译${NC}"
echo "----------------------------------------"
echo "运行编译验证..."
if cargo build --workspace 2>&1 | tee /tmp/build_output.txt; then
    echo -e "${GREEN}✅ 编译成功!${NC}"

    # 显示编译时间对比
    echo ""
    echo "编译时间统计:"
    grep "Finished" /tmp/build_output.txt | tail -1

else
    echo -e "${RED}❌ 编译失败!${NC}"
    echo "请检查 /tmp/build_output.txt 获取详细错误信息"
    echo ""
    echo "恢复备份..."
    git stash pop
    exit 1
fi

# 7. 运行测试
echo ""
echo -e "${YELLOW}步骤 7/7: 运行测试套件${NC}"
echo "----------------------------------------"
echo "运行快速测试..."
if cargo test --workspace --lib 2>&1 | tee /tmp/test_output.txt; then
    echo -e "${GREEN}✅ 测试通过!${NC}"
else
    echo -e "${YELLOW}⚠️  部分测试失败,可能需要手动调整${NC}"
    echo "请检查 /tmp/test_output.txt 获取详细错误信息"
fi

# 8. 总结
echo ""
echo "================================================"
echo -e "${GREEN}  清理完成!${NC}"
echo "================================================"
echo ""
echo "清理统计:"
echo "  - 清理的依赖数: $CLEANED_COUNT"
echo "  - 分支: $BRANCH_NAME"
echo "  - 备份: git stash"
echo ""
echo "下一步:"
echo "  1. 检查清理结果: git diff"
echo "  2. 运行完整测试: cargo test --workspace"
echo "  3. 提交更改: git add . && git commit -m 'cleanup: Remove unused dependencies'"
echo "  4. 如需回滚: git stash pop"
echo ""
