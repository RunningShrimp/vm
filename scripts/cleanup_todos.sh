#!/bin/bash
#
# TODO/FIXME Cleanup Script
# This script helps clean up TODO and FIXME comments by:
# 1. Removing implemented features
# 2. Replacing TODOs with GitHub issue references
# 3. Creating backup before modifications
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKUP_DIR="$PROJECT_ROOT/.backup_todo_cleanup"
TODO_FILE="$PROJECT_ROOT/TODO_FIXME_GITHUB_ISSUES.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Create backup directory
create_backup() {
    local file=$1
    local backup_path="$BACKUP_DIR/$(basename $file).backup"

    mkdir -p "$BACKUP_DIR"
    cp "$file" "$backup_path"
    log_info "Backed up: $file -> $backup_path"
}

# Check if file exists and is readable
check_file() {
    local file=$1
    if [ ! -f "$file" ]; then
        log_error "File not found: $file"
        return 1
    fi
    if [ ! -r "$file" ]; then
        log_error "File not readable: $file"
        return 1
    fi
    return 0
}

# Replace TODO with issue reference
replace_with_issue() {
    local file=$1
    local line_num=$2
    local issue_num=$3
    local issue_title=$4

    check_file "$file" || return 1

    create_backup "$file"

    # Use sed to replace the TODO comment on the specific line
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i '' "${line_num}s|// TODO.*|// See: Issue #${issue_num} - ${issue_title}|" "$file"
    else
        # Linux
        sed -i "${line_num}s|// TODO.*|// See: Issue #${issue_num} - ${issue_title}|" "$file"
    fi

    log_success "Replaced TODO in $file:$line_num with Issue #${issue_num}"
}

# Remove TODO comment entirely
remove_todo() {
    local file=$1
    local line_num=$2

    check_file "$file" || return 1

    create_backup "$file"

    # Remove the line
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "${line_num}d" "$file"
    else
        sed -i "${line_num}d" "$file"
    fi

    log_success "Removed TODO from $file:$line_num"
}

# Interactive cleanup for a specific file
cleanup_file_interactive() {
    local file=$1

    if ! check_file "$file"; then
        return 1
    fi

    log_info "Processing: $file"
    echo ""

    # Find all TODO/FIXME lines in the file
    local todo_lines=()
    while IFS= read -r line; do
        todo_lines+=("$line")
    done < <(grep -n "TODO\|FIXME" "$file" || true)

    if [ ${#todo_lines[@]} -eq 0 ]; then
        log_info "No TODO/FIXME found in $file"
        return 0
    fi

    log_info "Found ${#todo_lines[@]} TODO/FIXME comments"
    echo ""

    for todo_line in "${todo_lines[@]}"; do
        local line_num=$(echo "$todo_line" | cut -d: -f1)
        local content=$(echo "$todo_line" | cut -d: -f2-)

        echo "Line $line_num: $content"
        echo "Choose action:"
        echo "  1) Replace with GitHub issue reference"
        echo "  2) Remove completely"
        echo "  3) Keep as-is"
        echo "  4) Skip to next file"
        echo "  q) Quit"
        read -p "Action [1/2/3/4/q]: " choice

        case $choice in
            1)
                read -p "Enter issue number: " issue_num
                read -p "Enter issue title: " issue_title
                replace_with_issue "$file" "$line_num" "$issue_num" "$issue_title"
                ;;
            2)
                remove_todo "$file" "$line_num"
                ;;
            3)
                log_info "Keeping TODO at $file:$line_num"
                ;;
            4)
                log_info "Skipping rest of $file"
                break
                ;;
            q)
                log_info "Quitting"
                return 0
                ;;
            *)
                log_warning "Invalid choice, keeping TODO"
                ;;
        esac
        echo ""
    done
}

# Batch cleanup using the issues document
cleanup_batch() {
    local dry_run=${1:-false}

    log_info "Batch cleanup mode (dry_run=$dry_run)"
    log_info "Reading issues from: $TODO_FILE"

    if [ ! -f "$TODO_FILE" ]; then
        log_error "Issues file not found: $TODO_FILE"
        log_error "Run the analysis first to generate this file"
        return 1
    fi

    # Process each file that needs cleanup
    local files=(
        "/Users/wangbiao/Desktop/project/vm/vm-engine-jit/src/translation_optimizer.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-engine-jit/src/x86_codegen.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-engine-jit/src/domain/compilation.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-platform/src/runtime.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-platform/src/boot.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-platform/src/gpu.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-platform/src/iso.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-platform/src/sriov.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-service/src/vm_service.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-common/src/lockfree/hash_table.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-common/src/lib.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-ir/src/lift/semantics.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-ir/src/lift/mod.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-mem/src/tlb/tlb_concurrent.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-mem/src/memory/memory_pool.rs"
        "/Users/wangbiao/Desktop/project/vm/vm-mem/src/lib.rs"
    )

    local total_changes=0

    for file in "${files[@]}"; do
        if [ ! -f "$file" ]; then
            log_warning "File not found: $file"
            continue
        fi

        log_info "Processing: $file"

        if [ "$dry_run" = true ]; then
            echo "Would process TODOs in: $file"
        else
            cleanup_file_interactive "$file"
        fi
        echo ""
    done

    log_success "Batch cleanup complete. Total changes: $total_changes"
}

# Show statistics
show_stats() {
    log_info "TODO/FIXME Statistics"
    echo ""

    local total=$(grep -r "TODO\|FIXME" --include="*.rs" "$PROJECT_ROOT" 2>/dev/null | grep -v target | wc -l | tr -d ' ')
    local actual=$(grep -v "vm-codegen/examples/todo" /tmp/todo_comments.txt 2>/dev/null | wc -l | tr -d ' ')

    echo "Total TODO/FIXME comments: $total"
    echo "Actual code TODOs (excluding examples): $actual"
    echo ""
    echo "Top files with TODOs:"
    grep -r "TODO\|FIXME" --include="*.rs" "$PROJECT_ROOT" 2>/dev/null | \
        grep -v target | \
        grep -v "vm-codegen/examples/todo" | \
        cut -d: -f1 | \
        sort | uniq -c | sort -rn | head -10
}

# Restore from backup
restore_backup() {
    local file=$1
    local backup_path="$BACKUP_DIR/$(basename $file).backup"

    if [ ! -f "$backup_path" ]; then
        log_error "Backup not found: $backup_path"
        return 1
    fi

    cp "$backup_path" "$file"
    log_success "Restored: $file from backup"
}

# Main menu
main_menu() {
    echo ""
    echo "TODO/FIXME Cleanup Script"
    echo "========================="
    echo ""
    echo "1) Show statistics"
    echo "2) Clean up specific file (interactive)"
    echo "3) Batch cleanup (interactive)"
    echo "4) Batch cleanup (dry-run)"
    echo "5) Restore file from backup"
    echo "6) Exit"
    echo ""

    read -p "Choose an option [1-6]: " choice

    case $choice in
        1)
            show_stats
            ;;
        2)
            read -p "Enter file path: " file_path
            cleanup_file_interactive "$file_path"
            ;;
        3)
            cleanup_batch false
            ;;
        4)
            cleanup_batch true
            ;;
        5)
            read -p "Enter file path to restore: " file_path
            restore_backup "$file_path"
            ;;
        6)
            log_info "Exiting"
            exit 0
            ;;
        *)
            log_error "Invalid option"
            exit 1
            ;;
    esac
}

# Script entry point
main() {
    if [ ! -d "$PROJECT_ROOT" ]; then
        log_error "Project root not found: $PROJECT_ROOT"
        exit 1
    fi

    cd "$PROJECT_ROOT"

    if [ "$1" = "--stats" ]; then
        show_stats
    elif [ "$1" = "--help" ]; then
        echo "Usage: $0 [OPTION]"
        echo ""
        echo "Options:"
        echo "  --stats      Show TODO/FIXME statistics"
        echo "  --help       Show this help message"
        echo ""
        echo "Interactive mode (no args):"
        echo "  Run the script without arguments for interactive mode"
    else
        main_menu
    fi
}

main "$@"
