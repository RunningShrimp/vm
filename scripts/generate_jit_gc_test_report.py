#!/usr/bin/env python3
"""
JIT编译器和GC功能测试报告生成脚本

该脚本用于生成vm-engine-jit模块的测试报告，包括：
- 测试覆盖率统计
- 性能基准数据
- 发现的问题和改进建议
"""

import os
import sys
import subprocess
import json
import re
from datetime import datetime
from pathlib import Path

class TestReportGenerator:
    def __init__(self, project_root):
        self.project_root = Path(project_root)
        self.vm_engine_jit_dir = self.project_root / "vm-engine-jit"
        self.tests_dir = self.vm_engine_jit_dir / "tests"
        self.benches_dir = self.project_root / "benches"
        
        # 测试结果
        self.test_results = {}
        self.coverage_data = {}
        self.benchmark_data = {}
        self.issues = []
        self.recommendations = []
        
    def run_tests(self):
        """运行所有测试并收集结果"""
        print("🔧 运行JIT和GC测试...")
        
        # 运行单元测试
        self._run_unit_tests()
        
        # 运行集成测试
        self._run_integration_tests()
        
        # 运行性能基准测试
        self._run_benchmarks()
        
        # 生成覆盖率报告
        self._generate_coverage_report()
        
    def _run_unit_tests(self):
        """运行单元测试"""
        test_files = [
            "jit_optimizing_compiler_comprehensive_tests",
            "gc_comprehensive_tests", 
            "hotspot_cache_comprehensive_tests",
            "register_allocator_tests",
            "ewma_hotspot_tests",
            "gc_module_tests",
            "unified_cache_tests",
            "jit_error_tests"
        ]
        
        for test_file in test_files:
            print(f"  📋 运行 {test_file}...")
            try:
                result = subprocess.run(
                    ["cargo", "test", "--package", "vm-engine-jit", "--test", test_file],
                    cwd=self.vm_engine_jit_dir,
                    capture_output=True,
                    text=True,
                    timeout=300  # 5分钟超时
                )
                
                self.test_results[test_file] = {
                    "exit_code": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "success": result.returncode == 0
                }
                
                if result.returncode == 0:
                    print(f"    ✅ {test_file} 通过")
                else:
                    print(f"    ❌ {test_file} 失败")
                    self._extract_test_errors(test_file, result.stderr)
                    
            except subprocess.TimeoutExpired:
                print(f"    ⏰ {test_file} 超时")
                self.test_results[test_file] = {
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": "Test timed out",
                    "success": False
                }
                self.issues.append(f"{test_file}: 测试超时")
            except Exception as e:
                print(f"    💥 {test_file} 异常: {e}")
                self.test_results[test_file] = {
                    "exit_code": -2,
                    "stdout": "",
                    "stderr": str(e),
                    "success": False
                }
                self.issues.append(f"{test_file}: 执行异常 - {e}")
    
    def _run_integration_tests(self):
        """运行集成测试"""
        integration_test_files = [
            "jit_gc_integration_tests",
            "aot_integration_tests",
            "task3_integration"
        ]
        
        for test_file in integration_test_files:
            print(f"  🔗 运行集成测试 {test_file}...")
            try:
                result = subprocess.run(
                    ["cargo", "test", "--package", "vm-engine-jit", "--test", test_file],
                    cwd=self.vm_engine_jit_dir,
                    capture_output=True,
                    text=True,
                    timeout=600  # 10分钟超时
                )
                
                self.test_results[f"integration_{test_file}"] = {
                    "exit_code": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "success": result.returncode == 0
                }
                
                if result.returncode == 0:
                    print(f"    ✅ 集成测试 {test_file} 通过")
                else:
                    print(f"    ❌ 集成测试 {test_file} 失败")
                    self._extract_test_errors(f"integration_{test_file}", result.stderr)
                    
            except subprocess.TimeoutExpired:
                print(f"    ⏰ 集成测试 {test_file} 超时")
                self.issues.append(f"集成测试 {test_file}: 测试超时")
            except Exception as e:
                print(f"    💥 集成测试 {test_file} 异常: {e}")
                self.issues.append(f"集成测试 {test_file}: 执行异常 - {e}")
    
    def _run_benchmarks(self):
        """运行性能基准测试"""
        print("  📊 运行性能基准测试...")
        
        benchmark_names = [
            "jit_gc_performance_benchmarks"
        ]
        
        for benchmark in benchmark_names:
            print(f"    📈 运行基准测试 {benchmark}...")
            try:
                result = subprocess.run(
                    ["cargo", "bench", "--package", "vm-engine-jit", benchmark],
                    cwd=self.project_root,
                    capture_output=True,
                    text=True,
                    timeout=600  # 10分钟超时
                )
                
                self.benchmark_data[benchmark] = {
                    "exit_code": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "success": result.returncode == 0
                }
                
                if result.returncode == 0:
                    print(f"    ✅ 基准测试 {benchmark} 完成")
                    self._extract_benchmark_results(benchmark, result.stdout)
                else:
                    print(f"    ❌ 基准测试 {benchmark} 失败")
                    self.issues.append(f"基准测试 {benchmark}: 执行失败")
                    
            except subprocess.TimeoutExpired:
                print(f"    ⏰ 基准测试 {benchmark} 超时")
                self.issues.append(f"基准测试 {benchmark}: 测试超时")
            except Exception as e:
                print(f"    💥 基准测试 {benchmark} 异常: {e}")
                self.issues.append(f"基准测试 {benchmark}: 执行异常 - {e}")
    
    def _generate_coverage_report(self):
        """生成测试覆盖率报告"""
        print("  📊 生成覆盖率报告...")
        
        try:
            # 检查是否安装了cargo-tarpaulin
            result = subprocess.run(
                ["cargo", "tarpaulin", "--version"],
                capture_output=True,
                text=True
            )
            
            if result.returncode != 0:
                print("    ⚠️  cargo-tarpaulin 未安装，跳过覆盖率报告")
                return
                
            # 运行覆盖率测试
            result = subprocess.run(
                [
                    "cargo", "tarpaulin",
                    "--out", "Html",
                    "--output-dir", "target/coverage",
                    "--package", "vm-engine-jit",
                    "test"
                ],
                cwd=self.vm_engine_jit_dir,
                capture_output=True,
                text=True,
                timeout=600
            )
            
            if result.returncode == 0:
                print("    ✅ 覆盖率报告生成成功")
                self.coverage_data = {
                    "success": True,
                    "output": result.stdout,
                    "error": result.stderr
                }
            else:
                print("    ❌ 覆盖率报告生成失败")
                self.coverage_data = {
                    "success": False,
                    "output": result.stdout,
                    "error": result.stderr
                }
                self.issues.append("覆盖率报告生成失败")
                
        except Exception as e:
            print(f"    💥 覆盖率报告生成异常: {e}")
            self.issues.append(f"覆盖率报告生成异常 - {e}")
    
    def _extract_test_errors(self, test_name, stderr):
        """从测试输出中提取错误信息"""
        error_patterns = [
            r"thread '.*' panicked at '.*'",
            r"test .* failed",
            r"error: .*",
            r"panicked at '.*'"
        ]
        
        for pattern in error_patterns:
            matches = re.findall(pattern, stderr)
            for match in matches:
                self.issues.append(f"{test_name}: {match}")
    
    def _extract_benchmark_results(self, benchmark_name, stdout):
        """从基准测试输出中提取性能数据"""
        # 尝试提取基准测试结果
        lines = stdout.split('\n')
        
        for line in lines:
            if "test result" in line.lower() or "benchmark" in line.lower():
                self.benchmark_data[benchmark_name]["results"] = line
                break
    
    def generate_report(self):
        """生成测试报告"""
        print("📝 生成测试报告...")
        
        report = {
            "timestamp": datetime.now().isoformat(),
            "project": "vm-engine-jit",
            "summary": self._generate_summary(),
            "test_results": self.test_results,
            "coverage": self.coverage_data,
            "benchmarks": self.benchmark_data,
            "issues": self.issues,
            "recommendations": self._generate_recommendations()
        }
        
        # 生成Markdown报告
        self._generate_markdown_report(report)
        
        # 生成JSON报告
        self._generate_json_report(report)
        
        print("✅ 测试报告生成完成")
        return report
    
    def _generate_summary(self):
        """生成测试摘要"""
        total_tests = len(self.test_results)
        passed_tests = sum(1 for result in self.test_results.values() if result.get("success", False))
        failed_tests = total_tests - passed_tests
        
        return {
            "total_tests": total_tests,
            "passed_tests": passed_tests,
            "failed_tests": failed_tests,
            "success_rate": (passed_tests / total_tests * 100) if total_tests > 0 else 0,
            "benchmarks_run": len(self.benchmark_data),
            "coverage_generated": self.coverage_data.get("success", False)
        }
    
    def _generate_recommendations(self):
        """生成改进建议"""
        recommendations = []
        
        # 基于测试结果生成建议
        failed_tests = [name for name, result in self.test_results.items() 
                      if not result.get("success", False)]
        
        if failed_tests:
            recommendations.append({
                "category": "测试失败",
                "priority": "高",
                "description": f"以下测试失败，需要修复: {', '.join(failed_tests)}",
                "action": "检查失败的测试用例，修复相关代码问题"
            })
        
        # 基于覆盖率生成建议
        if not self.coverage_data.get("success", False):
            recommendations.append({
                "category": "覆盖率",
                "priority": "中",
                "description": "测试覆盖率报告生成失败",
                "action": "安装cargo-tarpaulin: cargo install cargo-tarpaulin"
            })
        
        # 基于基准测试生成建议
        failed_benchmarks = [name for name, result in self.benchmark_data.items() 
                           if not result.get("success", False)]
        
        if failed_benchmarks:
            recommendations.append({
                "category": "性能基准",
                "priority": "中",
                "description": f"以下基准测试失败: {', '.join(failed_benchmarks)}",
                "action": "检查基准测试环境，确保系统资源充足"
            })
        
        # 通用建议
        if len(self.issues) > 5:
            recommendations.append({
                "category": "整体质量",
                "priority": "高",
                "description": f"发现 {len(self.issues)} 个问题，需要系统性改进",
                "action": "建议进行代码审查，改进测试策略"
            })
        
        return recommendations
    
    def _generate_markdown_report(self, report):
        """生成Markdown格式的测试报告"""
        report_content = f"""# JIT编译器和GC功能测试报告

## 测试概览

- **测试时间**: {report['timestamp']}
- **项目**: {report['project']}
- **总测试数**: {report['summary']['total_tests']}
- **通过测试数**: {report['summary']['passed_tests']}
- **失败测试数**: {report['summary']['failed_tests']}
- **成功率**: {report['summary']['success_rate']:.1f}%
- **基准测试数**: {report['summary']['benchmarks_run']}
- **覆盖率报告**: {'✅ 已生成' if report['summary']['coverage_generated'] else '❌ 未生成'}

## 测试结果详情

"""
        
        # 添加测试结果
        for test_name, result in report['test_results'].items():
            status = "✅ 通过" if result.get('success', False) else "❌ 失败"
            report_content += f"### {test_name}\n\n**状态**: {status}\n\n"
            
            if not result.get('success', False) and result.get('stderr'):
                report_content += f"**错误信息**:\n```\n{result['stderr']}\n```\n\n"
        
        # 添加基准测试结果
        if report['benchmarks']:
            report_content += "## 性能基准测试\n\n"
            for benchmark_name, result in report['benchmarks'].items():
                status = "✅ 完成" if result.get('success', False) else "❌ 失败"
                report_content += f"### {benchmark_name}\n\n**状态**: {status}\n\n"
                
                if result.get('results'):
                    report_content += f"**结果**: {result['results']}\n\n"
        
        # 添加问题列表
        if report['issues']:
            report_content += "## 发现的问题\n\n"
            for i, issue in enumerate(report['issues'], 1):
                report_content += f"{i}. {issue}\n"
            report_content += "\n"
        
        # 添加改进建议
        if report['recommendations']:
            report_content += "## 改进建议\n\n"
            for rec in report['recommendations']:
                priority_emoji = "🔴" if rec['priority'] == '高' else "🟡" if rec['priority'] == '中' else "🟢"
                report_content += f"### {priority_emoji} {rec['category']}\n\n"
                report_content += f"**描述**: {rec['description']}\n\n"
                report_content += f"**建议操作**: {rec['action']}\n\n"
        
        # 写入报告文件
        report_path = self.project_root / "vm_engine_jit_test_report.md"
        with open(report_path, 'w', encoding='utf-8') as f:
            f.write(report_content)
        
        print(f"📄 Markdown报告已生成: {report_path}")
    
    def _generate_json_report(self, report):
        """生成JSON格式的测试报告"""
        report_path = self.project_root / "vm_engine_jit_test_report.json"
        with open(report_path, 'w', encoding='utf-8') as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
        
        print(f"📄 JSON报告已生成: {report_path}")

def main():
    """主函数"""
    if len(sys.argv) != 2:
        print("用法: python generate_jit_gc_test_report.py <项目根目录>")
        sys.exit(1)
    
    project_root = sys.argv[1]
    
    if not os.path.exists(project_root):
        print(f"错误: 项目根目录不存在: {project_root}")
        sys.exit(1)
    
    # 创建报告生成器
    generator = TestReportGenerator(project_root)
    
    # 运行测试
    generator.run_tests()
    
    # 生成报告
    report = generator.generate_report()
    
    # 打印摘要
    summary = report['summary']
    print(f"\n📊 测试摘要:")
    print(f"   总测试数: {summary['total_tests']}")
    print(f"   通过测试数: {summary['passed_tests']}")
    print(f"   失败测试数: {summary['failed_tests']}")
    print(f"   成功率: {summary['success_rate']:.1f}%")
    print(f"   基准测试数: {summary['benchmarks_run']}")
    print(f"   覆盖率报告: {'已生成' if summary['coverage_generated'] else '未生成'}")
    print(f"   发现问题数: {len(report['issues'])}")
    print(f"   改进建议数: {len(report['recommendations'])}")

if __name__ == "__main__":
    main()