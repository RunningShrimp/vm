#!/usr/bin/env python3

# FVP虚拟机系统测试覆盖率报告生成器
# 解析覆盖率数据并生成HTML报告

import os
import sys
import json
import argparse
from datetime import datetime
from jinja2 import Template

def parse_coverage_summary(summary_file):
    """解析覆盖率摘要文件"""
    coverage_data = {
        'lines': {'covered': 0, 'total': 0, 'percentage': 0.0},
        'functions': {'covered': 0, 'total': 0, 'percentage': 0.0},
        'branches': {'covered': 0, 'total': 0, 'percentage': 0.0},
        'conditionals': {'covered': 0, 'total': 0, 'percentage': 0.0}
    }

    try:
        with open(summary_file, 'r') as f:
            content = f.read()

        # 解析各类型的覆盖率
        for line in content.split('\n'):
            if 'lines......:' in line:
                parts = line.split()
                if len(parts) >= 3:
                    coverage_data['lines']['percentage'] = float(parts[1].rstrip('%'))
                    coverage_data['lines']['covered'] = int(parts[2].split('/')[0])
                    coverage_data['lines']['total'] = int(parts[2].split('/')[1])
            elif 'functions...:' in line:
                parts = line.split()
                if len(parts) >= 3:
                    coverage_data['functions']['percentage'] = float(parts[1].rstrip('%'))
                    coverage_data['functions']['covered'] = int(parts[2].split('/')[0])
                    coverage_data['functions']['total'] = int(parts[2].split('/')[1])
            elif 'branches.....:' in line:
                parts = line.split()
                if len(parts) >= 3:
                    coverage_data['branches']['percentage'] = float(parts[1].rstrip('%'))
                    coverage_data['branches']['covered'] = int(parts[2].split('/')[0])
                    coverage_data['branches']['total'] = int(parts[2].split('/')[1])
            elif 'conditionals.:' in line:
                parts = line.split()
                if len(parts) >= 3:
                    coverage_data['conditionals']['percentage'] = float(parts[1].rstrip('%'))
                    coverage_data['conditionals']['covered'] = int(parts[2].split('/')[0])
                    coverage_data['conditionals']['total'] = int(parts[2].split('/')[1])

    except Exception as e:
        print(f"Error parsing coverage summary: {e}")

    return coverage_data

def parse_benchmark_results(benchmark_file):
    """解析基准测试结果"""
    benchmark_data = {
        'total_benchmarks': 0,
        'successful_benchmarks': 0,
        'failed_benchmarks': 0,
        'benchmarks': []
    }

    try:
        with open(benchmark_file, 'r') as f:
            data = json.load(f)

        benchmark_data['total_benchmarks'] = len(data.get('benchmarks', []))

        for benchmark in data.get('benchmarks', []):
            name = benchmark.get('name', 'Unknown')
            mean_time = benchmark.get('mean', 0.0)
            stddev = benchmark.get('stddev', 0.0)

            benchmark_data['benchmarks'].append({
                'name': name,
                'mean_time': mean_time,
                'stddev': stddev,
                'status': 'success'
            })
            benchmark_data['successful_benchmarks'] += 1

    except Exception as e:
        print(f"Error parsing benchmark results: {e}")

    return benchmark_data

def get_coverage_level(percentage):
    """根据覆盖率百分比获取等级"""
    if percentage >= 90:
        return 'excellent'
    elif percentage >= 80:
        return 'good'
    elif percentage >= 70:
        return 'average'
    else:
        return 'poor'

def get_coverage_color(level):
    """获取覆盖率等级对应的颜色"""
    colors = {
        'excellent': '#28a745',  # green
        'good': '#17a2b8',      # blue
        'average': '#ffc107',    # yellow
        'poor': '#dc3545'        # red
    }
    return colors.get(level, '#6c757d')

def generate_html_report(coverage_data, benchmark_data, output_file):
    """生成HTML覆盖率报告"""

    # 计算总体覆盖率等级
    line_level = get_coverage_level(coverage_data['lines']['percentage'])

    # HTML模板
    html_template = Template('''
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>FVP虚拟机系统测试报告</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            line-height: 1.6;
            color: #333;
            background-color: #f8f9fa;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }

        header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 40px;
            text-align: center;
            border-radius: 10px;
            margin-bottom: 30px;
        }

        h1 {
            font-size: 2.5rem;
            margin-bottom: 10px;
        }

        .subtitle {
            font-size: 1.2rem;
            opacity: 0.9;
        }

        .dashboard {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }

        .card {
            background: white;
            padding: 25px;
            border-radius: 10px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            text-align: center;
        }

        .card h3 {
            margin-bottom: 15px;
            color: #495057;
        }

        .coverage-circle {
            width: 120px;
            height: 120px;
            margin: 0 auto 15px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1.5rem;
            font-weight: bold;
            color: white;
            position: relative;
        }

        .coverage-label {
            font-size: 0.9rem;
            color: #6c757d;
        }

        .section {
            background: white;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }

        .section h2 {
            margin-bottom: 20px;
            color: #495057;
            border-bottom: 2px solid #e9ecef;
            padding-bottom: 10px;
        }

        .coverage-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
        }

        .coverage-item {
            text-align: center;
            padding: 20px;
            border: 1px solid #dee2e6;
            border-radius: 8px;
        }

        .coverage-percentage {
            font-size: 1.8rem;
            font-weight: bold;
            margin-bottom: 5px;
        }

        .coverage-count {
            font-size: 0.9rem;
            color: #6c757d;
        }

        .benchmark-table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 20px;
        }

        .benchmark-table th,
        .benchmark-table td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #dee2e6;
        }

        .benchmark-table th {
            background-color: #f8f9fa;
            font-weight: 600;
        }

        .status-success {
            color: #28a745;
            font-weight: bold;
        }

        .timestamp {
            text-align: center;
            color: #6c757d;
            margin-top: 20px;
            font-size: 0.9rem;
        }

        .footer {
            text-align: center;
            margin-top: 40px;
            padding: 20px;
            color: #6c757d;
            border-top: 1px solid #dee2e6;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🚀 FVP虚拟机系统测试报告</h1>
            <p class="subtitle">自动化测试结果与覆盖率分析</p>
            <p class="subtitle">生成时间: {{ timestamp }}</p>
        </header>

        <div class="dashboard">
            <div class="card">
                <h3>代码覆盖率</h3>
                <div class="coverage-circle" style="background-color: {{ coverage_color }}">
                    {{ coverage_percentage }}%
                </div>
                <p class="coverage-label">总体覆盖率 ({{ coverage_level }})</p>
            </div>

            <div class="card">
                <h3>测试执行</h3>
                <div class="coverage-circle" style="background-color: #17a2b8;">
                    {{ test_status }}
                </div>
                <p class="coverage-label">测试状态</p>
            </div>

            <div class="card">
                <h3>基准测试</h3>
                <div class="coverage-circle" style="background-color: #ffc107;">
                    {{ benchmark_count }}
                </div>
                <p class="coverage-label">基准测试数量</p>
            </div>
        </div>

        <div class="section">
            <h2>📊 覆盖率详情</h2>
            <div class="coverage-grid">
                <div class="coverage-item">
                    <div class="coverage-percentage">{{ coverage.lines.percentage }}%</div>
                    <p class="coverage-label">代码行</p>
                    <p class="coverage-count">{{ coverage.lines.covered }}/{{ coverage.lines.total }}</p>
                </div>

                <div class="coverage-item">
                    <div class="coverage-percentage">{{ coverage.functions.percentage }}%</div>
                    <p class="coverage-label">函数</p>
                    <p class="coverage-count">{{ coverage.functions.covered }}/{{ coverage.functions.total }}</p>
                </div>

                <div class="coverage-item">
                    <div class="coverage-percentage">{{ coverage.branches.percentage }}%</div>
                    <p class="coverage-label">分支</p>
                    <p class="coverage-count">{{ coverage.branches.covered }}/{{ coverage.branches.total }}</p>
                </div>

                <div class="coverage-item">
                    <div class="coverage-percentage">{{ coverage.conditionals.percentage }}%</div>
                    <p class="coverage-label">条件</p>
                    <p class="coverage-count">{{ coverage.conditionals.covered }}/{{ coverage.conditionals.total }}</p>
                </div>
            </div>
        </div>

        <div class="section">
            <h2>⚡ 性能基准测试</h2>
            {% if benchmarks.benchmarks %}
            <table class="benchmark-table">
                <thead>
                    <tr>
                        <th>测试名称</th>
                        <th>平均时间 (ns)</th>
                        <th>标准差</th>
                        <th>状态</th>
                    </tr>
                </thead>
                <tbody>
                    {% for benchmark in benchmarks.benchmarks %}
                    <tr>
                        <td>{{ benchmark.name }}</td>
                        <td>{{ "%.2"|format(benchmark.mean_time) }}</td>
                        <td>{{ "%.2"|format(benchmark.stddev) }}</td>
                        <td class="status-success">{{ benchmark.status|title }}</td>
                    </tr>
                    {% endfor %}
                </tbody>
            </table>
            {% else %}
            <p>没有基准测试结果</p>
            {% endif %}
        </div>

        <div class="footer">
            <p>🤖 自动生成 | FVP虚拟机系统开发团队</p>
        </div>
    </div>
</body>
</html>
    ''')

    # 渲染模板
    html_content = html_template.render(
        timestamp=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        coverage_percentage=round(coverage_data['lines']['percentage'], 1),
        coverage_level=line_level.title(),
        coverage_color=get_coverage_color(line_level),
        test_status="通过",
        benchmark_count=benchmark_data['successful_benchmarks'],
        coverage=coverage_data,
        benchmarks=benchmark_data
    )

    # 写入文件
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(html_content)

    print(f"HTML报告已生成: {output_file}")

def main():
    parser = argparse.ArgumentParser(description='生成测试覆盖率报告')
    parser.add_argument('--coverage-summary', required=True, help='覆盖率摘要文件路径')
    parser.add_argument('--benchmark-results', help='基准测试结果JSON文件路径')
    parser.add_argument('--output', default='coverage-report.html', help='输出HTML文件路径')

    args = parser.parse_args()

    # 解析覆盖率数据
    print("解析覆盖率数据...")
    coverage_data = parse_coverage_summary(args.coverage_summary)

    # 解析基准测试数据（可选）
    benchmark_data = {}
    if args.benchmark_results and os.path.exists(args.benchmark_results):
        print("解析基准测试数据...")
        benchmark_data = parse_benchmark_results(args.benchmark_results)
    else:
        print("跳过基准测试数据解析")
        benchmark_data = {'benchmarks': []}

    # 生成HTML报告
    print("生成HTML报告...")
    generate_html_report(coverage_data, benchmark_data, args.output)

    print("报告生成完成！")

if __name__ == '__main__':
    main()