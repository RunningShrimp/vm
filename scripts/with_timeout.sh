#!/bin/bash
# 带超时的命令执行包装脚本
#
# 用法: ./with_timeout.sh <超时秒数> <命令...>
# 示例: ./with_timeout.sh 300 cargo test --workspace

set -e

if [ $# -lt 2 ]; then
    echo "用法: $0 <超时秒数> <命令...>"
    echo "示例: $0 300 cargo test --workspace"
    exit 1
fi

TIMEOUT_SECONDS=$1
shift
COMMAND="$@"

echo "执行命令（超时: ${TIMEOUT_SECONDS}秒）: $COMMAND"

# 检查系统是否有 timeout 命令
if command -v timeout >/dev/null 2>&1; then
    # Linux/Unix 系统使用 timeout 命令
    timeout ${TIMEOUT_SECONDS}s $COMMAND
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -eq 124 ]; then
        echo "错误: 命令超时（超过 ${TIMEOUT_SECONDS} 秒）"
        exit 124
    fi
    
    exit $EXIT_CODE
elif command -v gtimeout >/dev/null 2>&1; then
    # macOS 使用 gtimeout (GNU coreutils)
    gtimeout ${TIMEOUT_SECONDS}s $COMMAND
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -eq 124 ]; then
        echo "错误: 命令超时（超过 ${TIMEOUT_SECONDS} 秒）"
        exit 124
    fi
    
    exit $EXIT_CODE
else
    # 如果没有 timeout 命令，使用 Perl 实现（macOS 默认）
    perl -e '
        use POSIX;
        $SIG{ALRM} = sub { die "timeout\n" };
        alarm shift;
        exec @ARGV;
    ' ${TIMEOUT_SECONDS} $COMMAND
    
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -ne 0 ]; then
        echo "错误: 命令执行失败或超时"
        exit $EXIT_CODE
    fi
fi

