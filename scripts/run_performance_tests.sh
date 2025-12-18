#!/bin/bash

# 性能和压力测试运行脚本
# 
# 本脚本用于运行所有性能和压力测试，包括：
# - 单元测试中的性能测试
# - 基准测试
# - 压力测试
# - 长时间稳定性测试

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查是否安装了必要的工具
check_dependencies() {
    log_info "检查依赖项..."
    
    if ! command -v cargo &> /dev/null; then
        log_error "cargo 未安装，请先安装 Rust"
        exit 1
    fi
    
    if ! command -v jq &> /dev/null; then
        log_warning "jq 未安装，某些报告功能可能不可用"
    fi
    
    log_success "依赖项检查完成"
}

# 创建结果目录
setup_results_dir() {
    local RESULTS_DIR="performance_results"
    local TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
    local RUN_DIR="${RESULTS_DIR}/run_${TIMESTAMP}"
    
    mkdir -p "${RUN_DIR}"
    mkdir -p "${RUN_DIR}/unit_tests"
    mkdir -p "${RUN_DIR}/benchmarks"
    mkdir -p "${RUN_DIR}/stress_tests"
    mkdir -p "${RUN_DIR}/reports"
    
    echo "${RUN_DIR}"
}

# 运行单元测试中的性能测试
run_unit_performance_tests() {
    local RESULTS_DIR=$1
    log_info "运行单元测试中的性能测试..."
    
    # 运行性能回归测试
    log_info "运行性能回归测试..."
    cargo test --test performance_regression --release -- --nocapture 2>&1 | tee "${RESULTS_DIR}/unit_tests/performance_regression.log"
    
    # 运行性能压力测试
    log_info "运行性能压力测试..."
    cargo test --test performance_stress_tests --release -- --nocapture 2>&1 | tee "${RESULTS_DIR}/unit_tests/performance_stress_tests.log"
    
    log_success "单元测试中的性能测试完成"
}

# 运行基准测试
run_benchmarks() {
    local RESULTS_DIR=$1
    log_info "运行基准测试..."
    
    # 运行综合性能基准测试
    log_info "运行综合性能基准测试..."
    cargo bench --bench comprehensive_performance_benchmark -- --output-format json 2>&1 | tee "${RESULTS_DIR}/benchmarks/comprehensive_performance.json"
    
    # 运行JIT基准测试
    log_info "运行JIT基准测试..."
    cargo bench --bench comprehensive_jit_benchmark -- --output-format json 2>&1 | tee "${RESULTS_DIR}/benchmarks/jit_performance.json"
    
    # 运行跨架构基准测试
    log_info "运行跨架构基准测试..."
    cargo bench --bench cross_arch_benchmark -- --output-format json 2>&1 | tee "${RESULTS_DIR}/benchmarks/cross_arch_performance.json"
    
    # 运行内存基准测试
    log_info "运行内存基准测试..."
    cargo bench --bench memory_optimization_benchmark -- --output-format json 2>&1 | tee "${RESULTS_DIR}/benchmarks/memory_performance.json"
    
    log_success "基准测试完成"
}

# 运行压力测试
run_stress_tests() {
    local RESULTS_DIR=$1
    log_info "运行压力测试..."
    
    # 创建压力测试可执行文件
    log_info "构建压力测试可执行文件..."
    cargo build --release --package vm-stress-test-runner --bin stress_test_runner
    
    # 运行短期压力测试（5分钟）
    log_info "运行短期压力测试（5分钟）..."
    timeout 300 ./vm-stress-test-runner/target/release/stress_test_runner --duration 300 --threads 4 --output "${RESULTS_DIR}/stress_tests/short_term.json" 2>&1 | tee "${RESULTS_DIR}/stress_tests/short_term.log"
    
    # 运行中期压力测试（30分钟）
    log_info "运行中期压力测试（30分钟）..."
    timeout 1800 ./vm-stress-test-runner/target/release/stress_test_runner --duration 1800 --threads 8 --output "${RESULTS_DIR}/stress_tests/medium_term.json" 2>&1 | tee "${RESULTS_DIR}/stress_tests/medium_term.log"
    
    # 运行资源泄漏测试
    log_info "运行资源泄漏测试..."
    ./vm-stress-test-runner/target/release/stress_test_runner --test resource_leak --output "${RESULTS_DIR}/stress_tests/resource_leak.json" 2>&1 | tee "${RESULTS_DIR}/stress_tests/resource_leak.log"
    
    log_success "压力测试完成"
}

# 生成综合报告
generate_reports() {
    local RESULTS_DIR=$1
    log_info "生成综合报告..."
    
    # 创建Python脚本来生成报告
    cat > "${RESULTS_DIR}/reports/generate_report.py" << 'EOF'
#!/usr/bin/env python3
import json
import os
import sys
from datetime import datetime

def load_json_file(filepath):
    try:
        with open(filepath, 'r') as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return None

def extract_benchmark_data(json_data):
    """从Criterion JSON输出中提取基准测试数据"""
    results = {}
    if not json_data or 'groups' not in json_data:
        return results
    
    for group in json_data['groups']:
        group_name = group.get('group_id', 'unknown')
        benchmarks = []
        
        for benchmark in group.get('benchmarks', []):
            benchmark_name = benchmark.get('benchmark_id', 'unknown')
            avg_time = benchmark.get('mean', {}).get('estimate_point', 0)
            std_dev = benchmark.get('mean', {}).get('standard_error', 0)
            throughput = benchmark.get('throughput', {}).get('estimate_point', 0)
            
            benchmarks.append({
                'name': benchmark_name,
                'avg_time_ns': avg_time,
                'std_dev_ns': std_dev,
                'throughput': throughput
            })
        
        results[group_name] = benchmarks
    
    return results

def generate_html_report(results_dir, output_file):
    """生成HTML格式的报告"""
    # 加载所有基准测试数据
    benchmark_files = [
        'benchmarks/comprehensive_performance.json',
        'benchmarks/jit_performance.json',
        'benchmarks/cross_arch_performance.json',
        'benchmarks/memory_performance.json'
    ]
    
    all_benchmarks = {}
    for file_path in benchmark_files:
        full_path = os.path.join(results_dir, file_path)
        data = load_json_file(full_path)
        if data:
            benchmarks = extract_benchmark_data(data)
            all_benchmarks.update(benchmarks)
    
    # 加载压力测试数据
    stress_test_files = [
        'stress_tests/short_term.json',
        'stress_tests/medium_term.json',
        'stress_tests/resource_leak.json'
    ]
    
    stress_tests = {}
    for file_path in stress_test_files:
        full_path = os.path.join(results_dir, file_path)
        data = load_json_file(full_path)
        if data:
            test_name = os.path.basename(file_path).replace('.json', '')
            stress_tests[test_name] = data
    
    # 生成HTML报告
    html = f"""
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>VM性能测试报告</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            line-height: 1.6;
            margin: 0;
            padding: 20px;
            color: #333;
        }}
        h1, h2, h3 {{
            color: #2c3e50;
        }}
        table {{
            border-collapse: collapse;
            width: 100%;
            margin-bottom: 20px;
        }}
        th, td {{
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
        }}
        th {{
            background-color: #f2f2f2;
        }}
        tr:nth-child(even) {{
            background-color: #f9f9f9;
        }}
        .summary {{
            background-color: #f8f9fa;
            padding: 15px;
            border-radius: 5px;
            margin-bottom: 20px;
        }}
        .chart {{
            margin: 20px 0;
            text-align: center;
        }}
    </style>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
</head>
<body>
    <h1>VM性能测试报告</h1>
    <p>生成时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</p>
    
    <div class="summary">
        <h2>测试摘要</h2>
        <p>本报告包含以下测试结果：</p>
        <ul>
            <li>基准测试: {len(all_benchmarks)} 个测试组</li>
            <li>压力测试: {len(stress_tests)} 个测试</li>
        </ul>
    </div>
    
    <h2>基准测试结果</h2>
"""
    
    # 添加基准测试结果表格
    for group_name, benchmarks in all_benchmarks.items():
        html += f"<h3>{group_name}</h3>\n<table>\n"
        html += "<tr><th>测试名称</th><th>平均时间 (ns)</th><th>标准差 (ns)</th><th>吞吐量</th></tr>\n"
        
        for benchmark in benchmarks:
            html += f"<tr>"
            html += f"<td>{benchmark['name']}</td>"
            html += f"<td>{benchmark['avg_time_ns']:.2}</td>"
            html += f"<td>{benchmark['std_dev_ns']:.2}</td>"
            html += f"<td>{benchmark['throughput']:.2}</td>"
            html += f"</tr>\n"
        
        html += "</table>\n"
    
    # 添加压力测试结果
    html += "<h2>压力测试结果</h2>\n"
    for test_name, test_data in stress_tests.items():
        html += f"<h3>{test_name}</h3>\n<table>\n"
        html += "<tr><th>测试名称</th><th>执行时间 (ms)</th><th>操作数</th><th>错误数</th><th>吞吐量 (ops/s)</th></tr>\n"
        
        if 'results' in test_data:
            for result in test_data['results']:
                html += f"<tr>"
                html += f"<td>{result.get('name', 'unknown')}</td>"
                html += f"<td>{result.get('execution_time_ms', 0)}</td>"
                html += f"<td>{result.get('operations', 0)}</td>"
                html += f"<td>{result.get('errors', 0)}</td>"
                html += f"<td>{result.get('throughput_ops_per_sec', 0):.2}</td>"
                html += f"</tr>\n"
        
        html += "</table>\n"
    
    html += """
</body>
</html>
"""
    
    with open(output_file, 'w') as f:
        f.write(html)
    
    print(f"HTML报告已生成: {output_file}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("用法: generate_report.py <results_dir> <output_file>")
        sys.exit(1)
    
    results_dir = sys.argv[1]
    output_file = sys.argv[2]
    
    generate_html_report(results_dir, output_file)
EOF
    
    # 运行Python脚本生成报告
    python3 "${RESULTS_DIR}/reports/generate_report.py" "${RESULTS_DIR}" "${RESULTS_DIR}/reports/performance_report.html"
    
    log_success "综合报告已生成: ${RESULTS_DIR}/reports/performance_report.html"
}

# 主函数
main() {
    log_info "开始运行性能和压力测试..."
    
    # 检查依赖项
    check_dependencies
    
    # 设置结果目录
    RESULTS_DIR=$(setup_results_dir)
    log_info "结果将保存在: ${RESULTS_DIR}"
    
    # 运行测试
    run_unit_performance_tests "${RESULTS_DIR}"
    run_benchmarks "${RESULTS_DIR}"
    run_stress_tests "${RESULTS_DIR}"
    
    # 生成报告
    generate_reports "${RESULTS_DIR}"
    
    log_success "所有性能和压力测试已完成！"
    log_info "查看报告: ${RESULTS_DIR}/reports/performance_report.html"
    
    # 如果在CI环境中，将结果复制到固定位置
    if [ -n "$CI" ]; then
        mkdir -p ./performance_results_latest
        cp -r "${RESULTS_DIR}"/* ./performance_results_latest/
        log_info "结果已复制到 ./performance_results_latest/"
    fi
}

# 处理命令行参数
case "${1:-}" in
    --unit-only)
        RESULTS_DIR=$(setup_results_dir)
        run_unit_performance_tests "${RESULTS_DIR}"
        ;;
    --bench-only)
        RESULTS_DIR=$(setup_results_dir)
        run_benchmarks "${RESULTS_DIR}"
        ;;
    --stress-only)
        RESULTS_DIR=$(setup_results_dir)
        run_stress_tests "${RESULTS_DIR}"
        ;;
    --help|-h)
        echo "用法: $0 [选项]"
        echo "选项:"
        echo "  --unit-only    只运行单元测试中的性能测试"
        echo "  --bench-only   只运行基准测试"
        echo "  --stress-only  只运行压力测试"
        echo "  --help, -h     显示此帮助信息"
        echo ""
        echo "默认情况下，运行所有测试"
        exit 0
        ;;
    *)
        main
        ;;
esac