#!/bin/bash

# GitHub Issues 批量创建脚本
# 基于 docs/TODO_AUDIT.md 创建 GitHub Issues

set -e

# 配置
REPO_OWNER="your-org"  # 替换为你的组织名
REPO_NAME="vm"         # 替换为仓库名
TODO_AUDIT="docs/TODO_AUDIT.md"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查gh CLI是否安装
if ! command -v gh &> /dev/null; then
    echo -e "${RED}错误: gh CLI未安装${NC}"
    echo "请安装: https://cli.github.com/"
    exit 1
fi

# 检查是否已登录
if ! gh auth status &> /dev/null; then
    echo -e "${RED}错误: 未登录GitHub${NC}"
    echo "请运行: gh auth login"
    exit 1
fi

echo -e "${GREEN}=== GitHub Issues 批量创建工具 ===${NC}\n"

# 创建函数
create_issue() {
    local title="$1"
    local body="$2"
    local labels="$3"
    local milestone="$4"

    echo "创建Issue: $title"

    gh issue create \
        --repo "$REPO_OWNER/$REPO_NAME" \
        --title "$title" \
        --body "$body" \
        --label "$labels" \
        --milestone "$milestone" \
        --quiet || echo -e "${YELLOW}跳过: $title${NC}"
}

# P1任务 (GPU直通)
echo -e "${YELLOW}创建 P1: GPU直通 Issues...${NC}"

create_issue \
    "ROCm: 实现实际的流创建API" \
    "实现ROCm流创建功能

**位置**: vm-passthrough/src/rocm.rs:32

**描述**:
使用实际 ROCm API 创建流，替换当前的桩实现。

**技术方案**:
- 研究 ROCm API 文档
- 实现流创建函数
- 添加错误处理
- 编写单元测试

**工作量**: 2天

**相关TODO**: P1-001
**里程碑**: v0.2.0

**引用**: docs/TODO_AUDIT.md P1节" \
    "P1,enhancement,ROCm" \
    "v0.2.0"

create_issue \
    "CUDA: 实现实际的内核启动逻辑" \
    "实现CUDA内核启动功能

**位置**: vm-passthrough/src/cuda_compiler.rs:211

**描述**:
实现实际的CUDA内核启动逻辑，替换当前的桩实现。

**技术方案**:
- 使用 CUDA Runtime API
- 实现内核启动参数设置
- 添加异步执行支持
- 编写集成测试

**工作量**: 5天

**相关TODO**: P1-011
**里程碑**: v0.2.0" \
    "P1,enhancement,CUDA" \
    "v0.2.0"

create_issue \
    "ARM NPU: 实现NPU API集成" \
    "实现ARM NPU设备直通

**位置**: vm-passthrough/src/arm_npu.rs:76

**描述**:
集成ARM NPU实际API，实现模型加载和推理执行。

**技术方案**:
- 研究 ARM NPU API
- 实现设备初始化
- 实现模型加载
- 实现推理执行
- 添加测试

**工作量**: 13天

**相关TODO**: P1-015, P1-016, P1-017
**里程碑**: v0.3.0" \
    "P1,enhancement,ARM,NPU" \
    "v0.3.0"

create_issue \
    "JIT: 实现块链接优化" \
    "实现JIT块链接以减少间接跳转开销

**位置**: vm-engine-jit/src/lib.rs:67

**描述**:
实现JIT编译块的链接优化，减少间接跳转，提升性能。

**技术方案**:
- 设计块链接数据结构
- 实现链接算法
- 添加跳转优化
- 性能测试

**工作量**: 5天

**预期收益**: 10-15% JIT性能提升

**相关TODO**: P1-013
**里程碑**: v0.2.0" \
    "P1,enhancement,JIT,performance" \
    "v0.2.0"

create_issue \
    "SIMD: 扩展向量操作支持" \
    "在IR中添加更多向量操作变体

**位置**: vm-engine-jit/src/simd_integration.rs:442

**描述**:
扩展IROp以支持更多SIMD向量操作，提升SIMD工作负载性能。

**技术方案**:
- 分析缺失的向量操作
- 扩展IROp枚举
- 更新解码器
- 更新JIT后端
- 添加SIMD测试

**工作量**: 3天

**预期收益**: 20-30% SIMD工作负载性能提升

**相关TODO**: P1-014
**里程碑**: v0.2.0" \
    "P1,enhancement,SIMD,performance" \
    "v0.2.0"

# P2任务 (JIT增强)
echo -e "\n${YELLOW}创建 P2: JIT编译器增强 Issues...${NC}"

create_issue \
    "Cranelift: 实现跳转指令" \
    "在Cranelift后端实现跳转指令

**位置**: vm-engine-jit/src/cranelift_backend.rs:291

**描述**:
为Cranelift后端添加跳转指令支持。

**工作量**: 2天
**里程碑**: v0.3.0" \
    "P2,enhancement,JIT,Cranelift" \
    "v0.3.0"

create_issue \
    "Cranelift: 实现条件跳转" \
    "在Cranelift后端实现条件跳转指令

**位置**: vm-engine-jit/src/cranelift_backend.rs:295

**工作量**: 3天
**里程碑**: v0.3.0" \
    "P2,enhancement,JIT,Cranelift" \
    "v0.3.0"

create_issue \
    "Cranelift: 实现函数调用" \
    "在Cranelift后端实现函数调用指令

**位置**: vm-engine-jit/src/cranelift_backend.rs:302

**工作量**: 4天
**里程碑**: v0.3.0" \
    "P2,enhancement,JIT,Cranelift" \
    "v0.3.0"

create_issue \
    "JIT: 实现厂商优化策略" \
    "实现针对特定CPU厂商的优化策略

**位置**: vm-engine-jit/src/lib.rs:50,62

**描述**:
为Intel、AMD、ARM CPU实现特定的优化策略。

**工作量**: 8天
**预期收益**: 5-10% 特定厂商CPU性能提升
**里程碑**: v0.3.0" \
    "P2,enhancement,JIT,optimization" \
    "v0.3.0"

echo -e "\n${GREEN}=== Issues 创建完成 ===${NC}"
echo -e "${GREEN}总计: 创建了 10+ 个 GitHub Issues${NC}"
echo -e "\n下一步:"
echo "1. 访问: https://github.com/$REPO_OWNER/$REPO_NAME/issues"
echo "2. 审查创建的Issues"
echo "3. 分配给贡献者"
echo "4. 开始开发！"
