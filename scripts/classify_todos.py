#!/usr/bin/env python3
"""
TODO/FIXME标记分类脚本
根据关键词和上下文自动分类TODO标记的优先级
"""

import re
import os
from pathlib import Path
from collections import defaultdict
from typing import List, Dict, Tuple

# 高优先级关键词（影响核心功能）
HIGH_PRIORITY_KEYWORDS = [
    'core', 'critical', 'bug', 'crash', 'security', 'memory leak',
    'race condition', 'deadlock', 'panic', 'error', 'fail', 'broken',
    'fix', 'implement', 'missing', 'incomplete', 'not implemented',
    'jit', 'aot', 'gc', 'mmu', 'execution', 'compilation', 'runtime',
    'kvm', 'hvf', 'whpx', 'accel', 'device', 'io', 'interrupt'
]

# 中优先级关键词（重要功能）
MEDIUM_PRIORITY_KEYWORDS = [
    'optimize', 'performance', 'improve', 'enhance', 'refactor',
    'cleanup', 'documentation', 'test', 'coverage', 'monitoring',
    'metrics', 'logging', 'config', 'settings'
]

# 低优先级关键词（优化和增强）
LOW_PRIORITY_KEYWORDS = [
    'nice to have', 'future', 'later', 'optional', 'enhancement',
    'polish', 'ui', 'ux', 'cosmetic', 'minor', 'wait', 'depends'
]

def classify_priority(content: str, file_path: str) -> str:
    """根据内容和文件路径分类优先级"""
    content_lower = content.lower()
    file_lower = file_path.lower()
    
    # 检查高优先级关键词
    for keyword in HIGH_PRIORITY_KEYWORDS:
        if keyword in content_lower or keyword in file_lower:
            return 'high'
    
    # 检查低优先级关键词
    for keyword in LOW_PRIORITY_KEYWORDS:
        if keyword in content_lower:
            return 'low'
    
    # 检查中优先级关键词
    for keyword in MEDIUM_PRIORITY_KEYWORDS:
        if keyword in content_lower:
            return 'medium'
    
    # 根据文件路径判断
    if any(x in file_lower for x in ['test', 'example', 'bench', 'doc']):
        return 'low'
    
    if any(x in file_lower for x in ['core', 'engine', 'jit', 'gc', 'mmu', 'accel']):
        return 'high'
    
    # 默认中优先级
    return 'medium'

def scan_todos() -> Dict[str, List[Tuple[str, int, str]]]:
    """扫描所有TODO标记"""
    todos = defaultdict(list)
    
    # 扫描所有.rs和.md文件
    for root, dirs, files in os.walk('.'):
        # 跳过target、.git等目录
        if any(skip in root for skip in ['target', '.git', 'node_modules']):
            continue
        
        for file in files:
            if not file.endswith(('.rs', '.md')):
                continue
            
            file_path = os.path.join(root, file)
            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    for line_num, line in enumerate(f, 1):
                        # 查找TODO/FIXME等标记
                        for marker in ['TODO', 'FIXME', 'XXX', 'HACK', 'BUG']:
                            if marker in line:
                                # 提取TODO后的内容
                                match = re.search(rf'{marker}[:\s]*(.+)', line, re.IGNORECASE)
                                if match:
                                    content = match.group(1).strip()
                                    priority = classify_priority(content, file_path)
                                    todos[priority].append((file_path, line_num, marker, content))
            except Exception as e:
                print(f"Error reading {file_path}: {e}")
    
    return todos

def generate_report(todos: Dict[str, List[Tuple[str, int, str, str]]]) -> str:
    """生成分类报告"""
    report = []
    report.append("# TODO/FIXME标记分类报告\n")
    report.append(f"生成时间: {os.popen('date').read().strip()}\n")
    report.append("\n## 统计摘要\n")
    
    total = sum(len(items) for items in todos.values())
    report.append(f"- **总计**: {total} 个标记\n")
    report.append(f"- **高优先级**: {len(todos.get('high', []))} 个\n")
    report.append(f"- **中优先级**: {len(todos.get('medium', []))} 个\n")
    report.append(f"- **低优先级**: {len(todos.get('low', []))} 个\n")
    
    # 按优先级分组输出
    for priority in ['high', 'medium', 'low']:
        if priority not in todos or not todos[priority]:
            continue
        
        report.append(f"\n## {priority.upper()}优先级标记 ({len(todos[priority])} 个)\n")
        
        # 按文件分组
        by_file = defaultdict(list)
        for file_path, line_num, marker, content in todos[priority]:
            by_file[file_path].append((line_num, marker, content))
        
        for file_path in sorted(by_file.keys()):
            report.append(f"\n### {file_path}\n")
            for line_num, marker, content in sorted(by_file[file_path]):
                # 截断过长的内容
                content_display = content[:100] + '...' if len(content) > 100 else content
                report.append(f"- **行 {line_num}** [{marker}]: {content_display}\n")
    
    return ''.join(report)

if __name__ == '__main__':
    print("扫描TODO标记...")
    todos = scan_todos()
    
    print("生成分类报告...")
    report = generate_report(todos)
    
    output_file = 'TODO_CLASSIFIED.md'
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(report)
    
    print(f"\n分类完成！")
    print(f"高优先级: {len(todos.get('high', []))} 个")
    print(f"中优先级: {len(todos.get('medium', []))} 个")
    print(f"低优先级: {len(todos.get('low', []))} 个")
    print(f"\n详细报告已保存到: {output_file}")

