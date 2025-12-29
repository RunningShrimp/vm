#!/usr/bin/env python3
"""
Performance Regression Detection Script

This script compares current benchmark results against baseline results
and detects performance regressions beyond a configured threshold.
"""

import json
import sys
import os
from pathlib import Path
from datetime import datetime

# Configuration
REGRESSION_THRESHOLD = 10.0  # 10% slowdown threshold
IMPROVEMENT_THRESHOLD = 5.0   # 5% improvement threshold
BASELINE_FILE = "benches/baselines/main.json"
CRITERION_DIR = "target/criterion"

# Colors for terminal output
class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    NC = '\033[0m'  # No Color

def print_colored(message, color=None):
    """Print a message with optional color"""
    if color:
        print(f"{color}{message}{Colors.NC}")
    else:
        print(message)

def load_json_file(filepath):
    """Load a JSON file, return None if it doesn't exist"""
    try:
        with open(filepath, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        return None
    except json.JSONDecodeError as e:
        print_colored(f"Error parsing {filepath}: {e}", Colors.RED)
        return None

def parse_criterion_results():
    """Parse Criterion benchmark results"""
    results = {}
    criterion_path = Path(CRITERION_DIR)
    
    if not criterion_path.exists():
        print_colored(f"Warning: {CRITERION_DIR} not found", Colors.YELLOW)
        return results
    
    # Look for benchmark directories
    for benchmark_dir in criterion_path.iterdir():
        if benchmark_dir.is_dir() and not benchmark_dir.name.startswith('.'):
            # Look for the estimates.json file in each benchmark
            estimates_file = benchmark_dir / "new" / "estimates.json"
            if estimates_file.exists():
                data = load_json_file(estimates_file)
                if data:
                    # Extract the mean estimate and confidence interval
                    point_estimate = data.get('PointEstimate', {}).get('PointEstimate', 0)
                    unit = data.get('PointEstimate', {}).get('Unit', 'ns')
                    
                    # Convert to milliseconds for easier reading
                    if unit == 'ns':
                        value = point_estimate / 1_000_000
                    elif unit == 'us':
                        value = point_estimate / 1_000
                    else:
                        value = point_estimate
                    
                    results[benchmark_dir.name] = {
                        'value': value,
                        'unit': 'ms',
                        'raw_value': point_estimate,
                        'raw_unit': unit
                    }
    
    return results

def compare_with_baseline(current_results, baseline):
    """Compare current results with baseline"""
    regressions = []
    improvements = []
    
    for metric, current_data in current_results.items():
        if metric in baseline:
            baseline_data = baseline[metric]
            
            # Handle different baseline formats
            if isinstance(baseline_data, dict):
                baseline_value = baseline_data.get('value', baseline_data.get('time_ms', 0))
            else:
                baseline_value = float(baseline_data)
            
            current_value = current_data['value']
            
            if baseline_value > 0:
                change_percent = ((current_value - baseline_value) / baseline_value) * 100
                
                if change_percent > REGRESSION_THRESHOLD:
                    regressions.append({
                        'metric': metric,
                        'baseline': baseline_value,
                        'current': current_value,
                        'change': change_percent,
                        'severity': 'high' if change_percent > 20 else 'medium'
                    })
                elif change_percent < -IMPROVEMENT_THRESHOLD:
                    improvements.append({
                        'metric': metric,
                        'baseline': baseline_value,
                        'current': current_value,
                        'change': change_percent
                    })
    
    return regressions, improvements

def generate_report(current_results, regressions, improvements):
    """Generate a comprehensive report"""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    report_lines = [
        f"# Performance Benchmark Report",
        f"",
        f"**Generated:** {timestamp}",
        f"",
        f"## Summary",
        f"",
        f"- Total benchmarks: {len(current_results)}",
        f"- Regressions detected: {len(regressions)}",
        f"- Improvements detected: {len(improvements)}",
        f"",
    ]
    
    if regressions:
        report_lines.extend([
            f"## ❌ Performance Regressions",
            f"",
            f"The following metrics show performance degradation beyond the {REGRESSION_THRESHOLD}% threshold:",
            f"",
        ])
        
        for reg in regressions:
            severity_icon = "🔴" if reg['severity'] == 'high' else "🟡"
            report_lines.extend([
                f"### {severity_icon} {reg['metric']}",
                f"",
                f"- **Baseline:** {reg['baseline']:.3f} ms",
                f"- **Current:** {reg['current']:.3f} ms",
                f"- **Change:** +{reg['change']:.1f}%",
                f"- **Severity:** {reg['severity'].capitalize()}",
                f"",
            ])
    
    if improvements:
        report_lines.extend([
            f"## ✅ Performance Improvements",
            f"",
            f"The following metrics show significant performance improvements:",
            f"",
        ])
        
        for imp in improvements:
            report_lines.extend([
                f"### {imp['metric']}",
                f"",
                f"- **Baseline:** {imp['baseline']:.3f} ms",
                f"- **Current:** {imp['current']:.3f} ms",
                f"- **Change:** {imp['change']:.1f}%",
                f"",
            ])
    
    if not regressions and not improvements:
        report_lines.extend([
            f"## ✅ No significant changes detected",
            f"",
            f"All benchmarks are within the acceptable performance range.",
            f"",
        ])
    
    report_lines.extend([
        f"## Benchmark Details",
        f"",
    ])
    
    for metric, data in sorted(current_results.items()):
        report_lines.append(
            f"- **{metric}:** {data['value']:.3f} {data['unit']}"
        )
    
    report_lines.append("")
    
    return "\n".join(report_lines)

def save_baseline(current_results, baseline_file):
    """Save current results as new baseline"""
    # Create directory if it doesn't exist
    os.makedirs(os.path.dirname(baseline_file), exist_ok=True)
    
    # Update existing baseline or create new one
    existing_baseline = load_json_file(baseline_file) or {}
    
    for metric, data in current_results.items():
        existing_baseline[metric] = {
            'value': data['value'],
            'unit': data['unit'],
            'date': datetime.now().isoformat()
        }
    
    with open(baseline_file, 'w') as f:
        json.dump(existing_baseline, f, indent=2)
    
    print_colored(f"Baseline saved to {baseline_file}", Colors.GREEN)

def main():
    print_colored("========================================", Colors.BLUE)
    print_colored("Performance Regression Detection", Colors.BLUE)
    print_colored("========================================", Colors.BLUE)
    print("")
    
    # Load baseline
    print("Loading baseline...")
    baseline = load_json_file(BASELINE_FILE)
    
    if not baseline:
        print_colored(f"Warning: Baseline file not found at {BASELINE_FILE}", Colors.YELLOW)
        print("This is expected for the first run. A baseline will be created.")
        baseline = {}
    else:
        print_colored(f"✓ Loaded baseline with {len(baseline)} metrics", Colors.GREEN)
    
    # Parse current results
    print("\nParsing current benchmark results...")
    current_results = parse_criterion_results()
    
    if not current_results:
        print_colored("Error: No benchmark results found", Colors.RED)
        print_colored(f"Expected results in {CRITERION_DIR}", Colors.RED)
        sys.exit(1)
    
    print_colored(f"✓ Found {len(current_results)} benchmark results", Colors.GREEN)
    
    # Compare with baseline
    if baseline:
        print("\nComparing with baseline...")
        regressions, improvements = compare_with_baseline(current_results, baseline)
        
        # Display results
        print("")
        if regressions:
            print_colored(f"❌ PERFORMANCE REGRESSIONS DETECTED:", Colors.RED)
            print_colored(f"   {len(regressions)} metrics degraded beyond {REGRESSION_THRESHOLD}% threshold", Colors.RED)
            print("")
            for reg in regressions:
                severity_icon = "🔴" if reg['severity'] == 'high' else "🟡"
                print_colored(
                    f"   {severity_icon} {reg['metric']}: +{reg['change']:.1f}% "
                    f"({reg['baseline']:.3f} → {reg['current']:.3f} ms)",
                    Colors.RED
                )
        
        if improvements:
            print("")
            print_colored(f"✅ PERFORMANCE IMPROVEMENTS:", Colors.GREEN)
            print_colored(f"   {len(improvements)} metrics improved beyond {IMPROVEMENT_THRESHOLD}%", Colors.GREEN)
            print("")
            for imp in improvements:
                print_colored(
                    f"   ✓ {imp['metric']}: {imp['change']:.1f}% "
                    f"({imp['baseline']:.3f} → {imp['current']:.3f} ms)",
                    Colors.GREEN
                )
        
        if not regressions and not improvements:
            print_colored("✅ No performance regressions detected", Colors.GREEN)
            print_colored("✅ All metrics within acceptable range", Colors.GREEN)
        
        # Generate report
        print("\nGenerating report...")
        report = generate_report(current_results, regressions, improvements)
        
        with open("benchmark-report.md", 'w') as f:
            f.write(report)
        
        print_colored("✓ Report saved to benchmark-report.md", Colors.GREEN)
        
        # Exit with error if regressions detected
        if regressions:
            print("")
            print_colored("========================================", Colors.RED)
            print_colored("REGRESSION DETECTED - Exiting with error", Colors.RED)
            print_colored("========================================", Colors.RED)
            sys.exit(1)
    
    else:
        # First run - save as baseline
        print("\nFirst run detected - saving as baseline...")
        save_baseline(current_results, BASELINE_FILE)
        print_colored("✓ Baseline established successfully", Colors.GREEN)
        print_colored("Future runs will compare against this baseline", Colors.GREEN)
    
    print("")
    print_colored("========================================", Colors.BLUE)
    print_colored("Detection Complete", Colors.BLUE)
    print_colored("========================================", Colors.BLUE)

if __name__ == "__main__":
    main()
