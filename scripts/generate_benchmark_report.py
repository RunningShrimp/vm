#!/usr/bin/env python3
"""
Benchmark Report Generator

This script generates comprehensive benchmark reports from Criterion output.
It can generate reports from scratch or compare against previous runs.
"""

import json
import os
import sys
from pathlib import Path
from datetime import datetime
from argparse import ArgumentParser

# Configuration
CRITERION_DIR = "target/criterion"
OUTPUT_FILE = "benchmark-report.md"
PREVIOUS_DIR = "target/criterion/previous"

def parse_criterion_benchmark(benchmark_path):
    """Parse a single Criterion benchmark directory"""
    estimates_file = benchmark_path / "new" / "estimates.json"
    
    if not estimates_file.exists():
        return None
    
    try:
        with open(estimates_file, 'r') as f:
            data = json.load(f)
    except (json.JSONDecodeError, FileNotFoundError):
        return None
    
    # Extract key metrics
    point_estimate = data.get('PointEstimate', {}).get('PointEstimate', 0)
    unit = data.get('PointEstimate', {}).get('Unit', 'ns')
    
    # Get confidence interval
    confidence_interval = data.get('PointEstimate', {}).get('ConfidenceInterval', {})
    lower_bound = confidence_interval.get('LowerBound', 0)
    upper_bound = confidence_interval.get('UpperBound', 0)
    
    # Get iterations
    iterations = data.get('Iterations', {}).get('PointEstimates', [])
    
    return {
        'point_estimate': point_estimate,
        'unit': unit,
        'lower_bound': lower_bound,
        'upper_bound': upper_bound,
        'iterations': len(iterations) if iterations else 0
    }

def parse_comparison(benchmark_path):
    """Parse comparison data if available"""
    change_file = benchmark_path / "change" / "estimates.json"
    
    if not change_file.exists():
        return None
    
    try:
        with open(change_file, 'r') as f:
            data = json.load(f)
    except (json.JSONDecodeError, FileNotFoundError):
        return None
    
    point_estimate = data.get('PointEstimate', {}).get('PointEstimate', 0)
    unit = data.get('PointEstimate', {}).get('Unit', '%')
    
    # Get significance
    significance = data.get('Change', {}).get('Significance', 'None')
    
    return {
        'change_percent': point_estimate,
        'unit': unit,
        'significance': significance
    }

def format_time(value, unit):
    """Format time value in appropriate units"""
    if unit == 'ns':
        if value < 1000:
            return f"{value:.2f} ns"
        elif value < 1_000_000:
            return f"{value/1000:.2f} μs"
        else:
            return f"{value/1_000_000:.3f} ms"
    elif unit == 'us':
        if value < 1000:
            return f"{value:.2f} μs"
        else:
            return f"{value/1000:.3f} ms"
    elif unit == 'ms':
        return f"{value:.3f} ms"
    elif unit == 's':
        return f"{value:.2f} s"
    else:
        return f"{value} {unit}"

def generate_comparison_summary(previous_dir):
    """Generate comparison summary with previous run"""
    if not os.path.exists(previous_dir):
        return None
    
    # This would compare against previous benchmark results
    # For now, return a placeholder
    return {
        'has_comparison': True,
        'note': 'Comparison with previous run available'
    }

def categorize_benchmark(name):
    """Categorize benchmark by type"""
    name_lower = name.lower()
    
    if any(x in name_lower for x in ['jit', 'compile', 'code_gen', 'tier']):
        return 'JIT Compilation'
    elif any(x in name_lower for x in ['cross_arch', 'translation', 'x86_64', 'arm64']):
        return 'Cross-Architecture'
    elif any(x in name_lower for x in ['memory', 'alloc', 'mmu', 'tlb']):
        return 'Memory Management'
    elif any(x in name_lower for x in ['gc', 'garbage']):
        return 'Garbage Collection'
    elif any(x in name_lower for x in ['async', 'executor', 'future']):
        return 'Async Operations'
    elif any(x in name_lower for x in ['device', 'io', 'block']):
        return 'Device I/O'
    elif any(x in name_lower for x in ['lock', 'mutex', 'sync']):
        return 'Concurrency'
    else:
        return 'General'

def generate_report(compare=False):
    """Generate comprehensive benchmark report"""
    criterion_path = Path(CRITERION_DIR)
    
    if not criterion_path.exists():
        print(f"Error: {CRITERION_DIR} not found. Run benchmarks first.")
        return None
    
    # Collect all benchmark data
    benchmarks = {}
    
    for benchmark_dir in criterion_path.iterdir():
        if benchmark_dir.is_dir() and not benchmark_dir.name.startswith('.'):
            data = parse_criterion_benchmark(benchmark_dir)
            if data:
                comparison = parse_comparison(benchmark_dir)
                category = categorize_benchmark(benchmark_dir.name)
                
                benchmarks[benchmark_dir.name] = {
                    **data,
                    'comparison': comparison,
                    'category': category
                }
    
    if not benchmarks:
        print("No benchmark results found")
        return None
    
    # Group by category
    categories = {}
    for name, data in benchmarks.items():
        cat = data['category']
        if cat not in categories:
            categories[cat] = []
        categories[cat].append((name, data))
    
    # Generate report
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    report_lines = [
        "# Performance Benchmark Report",
        "",
        f"**Generated:** {timestamp}",
        f"**Total Benchmarks:** {len(benchmarks)}",
        "",
    ]
    
    # Summary section
    report_lines.extend([
        "## Summary",
        "",
    ])
    
    for category in sorted(categories.keys()):
        count = len(categories[category])
        report_lines.append(f"- **{category}:** {count} benchmark(s)")
    
    report_lines.append("")
    
    # Detailed results by category
    for category in sorted(categories.keys()):
        report_lines.extend([
            f"## {category}",
            "",
        ])
        
        # Sort by name
        sorted_benchmarks = sorted(categories[category], key=lambda x: x[0])
        
        for name, data in sorted_benchmarks:
            time_str = format_time(data['point_estimate'], data['unit'])
            
            report_lines.extend([
                f"### {name}",
                "",
                f"- **Mean:** {time_str}",
            ])
            
            # Add confidence interval
            if data['lower_bound'] and data['upper_bound']:
                lower = format_time(data['lower_bound'], data['unit'])
                upper = format_time(data['upper_bound'], data['unit'])
                report_lines.append(f"- **95% CI:** [{lower}, {upper}]")
            
            # Add comparison data if available
            if data['comparison']:
                comp = data['comparison']
                if comp['significance'] != 'None':
                    change = comp['change_percent']
                    icon = "📈" if change > 0 else "📉"
                    report_lines.append(f"- **Change:** {icon} {change:+.2f}% ({comp['significance']})")
            
            report_lines.append("")
    
    # Performance insights
    report_lines.extend([
        "---",
        "",
        "## Performance Insights",
        "",
    ])
    
    # Find fastest and slowest in each category
    for category in sorted(categories.keys()):
        sorted_by_time = sorted(
            categories[category],
            key=lambda x: x[1]['point_estimate']
        )
        
        if len(sorted_by_time) > 0:
            fastest_name, fastest_data = sorted_by_time[0]
            fastest_time = format_time(fastest_data['point_estimate'], fastest_data['unit'])
            
            report_lines.append(f"### {category}")
            report_lines.append(f"- **Fastest:** {fastest_name} ({fastest_time})")
            
            if len(sorted_by_time) > 1:
                slowest_name, slowest_data = sorted_by_time[-1]
                slowest_time = format_time(slowest_data['point_estimate'], slowest_data['unit'])
                report_lines.append(f"- **Slowest:** {slowest_name} ({slowest_time})")
            
            report_lines.append("")
    
    # Notes
    report_lines.extend([
        "---",
        "",
        "## Notes",
        "",
        "- All benchmarks use Criterion.rs for statistical analysis",
        "- Confidence intervals represent 95% confidence level",
        "- Comparisons show change from previous baseline (if available)",
        "",
        f"*Report generated by scripts/generate_benchmark_report.py*",
    ])
    
    return "\n".join(report_lines)

def main():
    parser = ArgumentParser(description='Generate benchmark reports')
    parser.add_argument(
        '--compare',
        action='store_true',
        help='Compare with previous benchmark results'
    )
    parser.add_argument(
        '--output',
        default=OUTPUT_FILE,
        help=f'Output file (default: {OUTPUT_FILE})'
    )
    
    args = parser.parse_args()
    
    print("Generating benchmark report...")
    
    report = generate_report(compare=args.compare)
    
    if report:
        with open(args.output, 'w') as f:
            f.write(report)
        
        print(f"✓ Report saved to {args.output}")
        return 0
    else:
        print("✗ Failed to generate report")
        return 1

if __name__ == "__main__":
    sys.exit(main())
