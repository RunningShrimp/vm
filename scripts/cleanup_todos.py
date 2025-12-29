#!/usr/bin/env python3
"""
TODO/FIXME Cleanup Script

This script helps clean up TODO and FIXME comments by:
1. Removing implemented features
2. Replacing TODOs with GitHub issue references
3. Creating backups before modifications
"""

import os
import re
import sys
import shutil
from datetime import datetime
from pathlib import Path
from typing import List, Tuple, Dict

# Configuration
PROJECT_ROOT = Path("/Users/wangbiao/Desktop/project/vm")
BACKUP_DIR = PROJECT_ROOT / ".backup_todo_cleanup"
TODO_FILE = PROJECT_ROOT / "TODO_FIXME_GITHUB_ISSUES.md"

# Colors
class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    NC = '\033[0m'

def log_info(msg: str):
    print(f"{Colors.BLUE}[INFO]{Colors.NC} {msg}")

def log_success(msg: str):
    print(f"{Colors.GREEN}[SUCCESS]{Colors.NC} {msg}")

def log_warning(msg: str):
    print(f"{Colors.YELLOW}[WARNING]{Colors.NC} {msg}")

def log_error(msg: str):
    print(f"{Colors.RED}[ERROR]{Colors.NC} {msg}")

def create_backup(file_path: Path) -> Path:
    """Create a backup of the file"""
    BACKUP_DIR.mkdir(exist_ok=True)
    backup_path = BACKUP_DIR / f"{file_path.name}.backup.{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    shutil.copy2(file_path, backup_path)
    log_info(f"Backed up: {file_path} -> {backup_path}")
    return backup_path

def find_todos(file_path: Path) -> List[Tuple[int, str, str]]:
    """
    Find all TODO/FIXME comments in a file
    Returns: List of (line_number, marker_type, content)
    """
    todos = []
    pattern = re.compile(r'//\s*(TODO|FIXME):\s*(.+?)$')

    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            for line_num, line in enumerate(f, 1):
                match = pattern.search(line)
                if match:
                    marker_type = match.group(1)
                    content = match.group(2).strip()
                    todos.append((line_num, marker_type, content))
    except Exception as e:
        log_error(f"Error reading {file_path}: {e}")

    return todos

def replace_todo_with_issue(file_path: Path, line_num: int, issue_num: int, issue_title: str):
    """Replace a TODO comment with a GitHub issue reference"""
    create_backup(file_path)

    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    if line_num <= len(lines):
        # Replace TODO with issue reference
        lines[line_num - 1] = re.sub(
            r'//\s*(TODO|FIXME):.*',
            f'// See: Issue #{issue_num} - {issue_title}\n',
            lines[line_num - 1]
        )

        with open(file_path, 'w', encoding='utf-8') as f:
            f.writelines(lines)

        log_success(f"Replaced TODO in {file_path}:{line_num} with Issue #{issue_num}")
    else:
        log_error(f"Line number {line_num} out of range for {file_path}")

def remove_todo(file_path: Path, line_num: int):
    """Remove a TODO comment completely"""
    create_backup(file_path)

    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    if line_num <= len(lines):
        # Remove the line if it only contains the TODO
        line = lines[line_num - 1].strip()
        if line.startswith('//') and ('TODO' in line or 'FIXME' in line):
            del lines[line_num - 1]
        else:
            # Remove just the TODO part
            lines[line_num - 1] = re.sub(r'\s*//\s*(TODO|FIXME):.*\n?', '\n', lines[line_num - 1])

        with open(file_path, 'w', encoding='utf-8') as f:
            f.writelines(lines)

        log_success(f"Removed TODO from {file_path}:{line_num}")
    else:
        log_error(f"Line number {line_num} out of range for {file_path}")

def get_file_statistics() -> Dict[str, int]:
    """Get TODO/FIXME statistics for all Rust files"""
    stats = {}
    total = 0

    for rs_file in PROJECT_ROOT.rglob("*.rs"):
        # Skip target directory and examples
        if 'target' in str(rs_file) or 'examples/todo' in str(rs_file):
            continue

        todos = find_todos(rs_file)
        if todos:
            stats[str(rs_file.relative_to(PROJECT_ROOT))] = len(todos)
            total += len(todos)

    return stats, total

def show_statistics():
    """Display TODO/FIXME statistics"""
    log_info("TODO/FIXME Statistics")
    print()

    stats, total = get_file_statistics()

    print(f"Total files with TODOs: {len(stats)}")
    print(f"Total TODO/FIXME comments: {total}")
    print()

    if stats:
        print("Top files with TODOs:")
        sorted_stats = sorted(stats.items(), key=lambda x: x[1], reverse=True)[:10]
        for file, count in sorted_stats:
            print(f"  {count:3d} {file}")

def cleanup_file_interactive(file_path: Path):
    """Interactively clean up TODOs in a file"""
    if not file_path.exists():
        log_error(f"File not found: {file_path}")
        return

    log_info(f"Processing: {file_path.relative_to(PROJECT_ROOT)}")
    print()

    todos = find_todos(file_path)

    if not todos:
        log_info(f"No TODO/FIXME found in {file_path.name}")
        return

    log_info(f"Found {len(todos)} TODO/FIXME comments")
    print()

    for line_num, marker_type, content in todos:
        print(f"Line {line_num}: [{marker_type}] {content}")
        print("Choose action:")
        print("  1) Replace with GitHub issue reference")
        print("  2) Remove completely")
        print("  3) Keep as-is")
        print("  4) Skip to next file")
        print("  q) Quit")

        choice = input("Action [1/2/3/4/q]: ").strip().lower()

        if choice == '1':
            issue_num = input("Enter issue number: ").strip()
            issue_title = input("Enter issue title: ").strip()
            replace_todo_with_issue(file_path, line_num, issue_num, issue_title)
        elif choice == '2':
            remove_todo(file_path, line_num)
        elif choice == '3':
            log_info(f"Keeping TODO at {file_path.name}:{line_num}")
        elif choice == '4':
            log_info(f"Skipping rest of {file_path.name}")
            break
        elif choice == 'q':
            log_info("Quitting")
            sys.exit(0)
        else:
            log_warning("Invalid choice, keeping TODO")

        print()

def cleanup_batch(dry_run: bool = False):
    """Batch cleanup of all TODOs"""
    log_info(f"Batch cleanup mode (dry_run={dry_run})")
    print()

    # List of files to process (from our analysis)
    files_to_process = [
        "vm-engine-jit/src/translation_optimizer.rs",
        "vm-engine-jit/src/x86_codegen.rs",
        "vm-engine-jit/src/domain/compilation.rs",
        "vm-platform/src/runtime.rs",
        "vm-platform/src/boot.rs",
        "vm-platform/src/gpu.rs",
        "vm-platform/src/iso.rs",
        "vm-platform/src/sriov.rs",
        "vm-service/src/vm_service.rs",
        "vm-common/src/lockfree/hash_table.rs",
        "vm-common/src/lib.rs",
        "vm-ir/src/lift/semantics.rs",
        "vm-ir/src/lift/mod.rs",
        "vm-mem/src/tlb/tlb_concurrent.rs",
        "vm-mem/src/memory/memory_pool.rs",
        "vm-mem/src/lib.rs",
    ]

    total_changes = 0

    for file_rel in files_to_process:
        file_path = PROJECT_ROOT / file_rel

        if not file_path.exists():
            log_warning(f"File not found: {file_rel}")
            continue

        todos = find_todos(file_path)

        if not todos:
            log_info(f"No TODOs in {file_rel}")
            continue

        if dry_run:
            print(f"\n{file_rel}: {len(todos)} TODOs")
            for line_num, marker_type, content in todos:
                print(f"  Line {line_num}: [{marker_type}] {content}")
        else:
            cleanup_file_interactive(file_path)

    log_success(f"Batch cleanup complete. Total files processed: {len(files_to_process)}")

def generate_cleanup_report():
    """Generate a comprehensive cleanup report"""
    report_file = PROJECT_ROOT / "TODO_CLEANUP_REPORT.md"

    log_info(f"Generating cleanup report: {report_file}")

    stats, total = get_file_statistics()

    with open(report_file, 'w', encoding='utf-8') as f:
        f.write("# TODO/FIXME Cleanup Report\n\n")
        f.write(f"**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        f.write(f"**Total TODO/FIXME comments**: {total}\n")
        f.write(f"**Files affected**: {len(stats)}\n\n")
        f.write("---\n\n")

        f.write("## Files by Priority\n\n")

        # High priority (>3 TODOs)
        f.write("### High Priority (more than 3 TODOs)\n\n")
        for file, count in sorted(stats.items(), key=lambda x: x[1], reverse=True):
            if count > 3:
                f.write(f"- **{file}**: {count} TODOs\n")

        f.write("\n### Medium Priority (2-3 TODOs)\n\n")
        for file, count in sorted(stats.items(), key=lambda x: x[1], reverse=True):
            if 2 <= count <= 3:
                f.write(f"- **{file}**: {count} TODOs\n")

        f.write("\n### Low Priority (1 TODO)\n\n")
        for file, count in sorted(stats.items(), key=lambda x: x[1], reverse=True):
            if count == 1:
                f.write(f"- **{file}**: {count} TODO\n")

        f.write("\n---\n\n")
        f.write("## Detailed TODO List\n\n")

        for file, count in sorted(stats.items(), key=lambda x: x[1], reverse=True):
            file_path = PROJECT_ROOT / file
            todos = find_todos(file_path)

            f.write(f"### {file} ({count} TODOs)\n\n")
            for line_num, marker_type, content in todos:
                f.write(f"- Line {line_num}: **[{marker_type}]** {content}\n")
            f.write("\n")

    log_success(f"Report saved to: {report_file}")

def main():
    """Main entry point"""
    if not PROJECT_ROOT.exists():
        log_error(f"Project root not found: {PROJECT_ROOT}")
        sys.exit(1)

    os.chdir(PROJECT_ROOT)

    if len(sys.argv) > 1:
        arg = sys.argv[1]

        if arg == "--stats":
            show_statistics()
        elif arg == "--report":
            generate_cleanup_report()
        elif arg == "--help":
            print("Usage: cleanup_todos.py [OPTION]")
            print()
            print("Options:")
            print("  --stats      Show TODO/FIXME statistics")
            print("  --report     Generate detailed cleanup report")
            print("  --help       Show this help message")
            print()
            print("Interactive mode (no args):")
            print("  Run the script without arguments for interactive mode")
        else:
            log_error(f"Unknown option: {arg}")
            sys.exit(1)
    else:
        # Interactive mode
        print("\nTODO/FIXME Cleanup Script")
        print("=" * 40)
        print()
        print("1) Show statistics")
        print("2) Generate cleanup report")
        print("3) Clean up specific file (interactive)")
        print("4) Batch cleanup (interactive)")
        print("5) Exit")
        print()

        choice = input("Choose an option [1-5]: ").strip()

        if choice == '1':
            show_statistics()
        elif choice == '2':
            generate_cleanup_report()
        elif choice == '3':
            file_rel = input("Enter file path (relative to project root): ").strip()
            file_path = PROJECT_ROOT / file_rel
            cleanup_file_interactive(file_path)
        elif choice == '4':
            dry_run = input("Dry run? [y/N]: ").strip().lower() == 'y'
            cleanup_batch(dry_run)
        elif choice == '5':
            log_info("Exiting")
            sys.exit(0)
        else:
            log_error("Invalid option")
            sys.exit(1)

if __name__ == "__main__":
    main()
