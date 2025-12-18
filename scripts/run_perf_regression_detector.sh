#!/bin/bash

# VM Performance Regression Detector Runner
# This script runs the performance regression detector with various configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DATABASE_PATH="performance_data.db"
OUTPUT_DIR="performance_reports"
CONFIG_FILE=""
TEST_NAME=""
SOURCE_ARCH=""
TARGET_ARCH=""
ITERATIONS=10
COLLECT_ONLY=false
DETECT_ONLY=false
GENERATE_CHARTS=false
REPORT_FORMAT="text"

# Function to print usage
print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -d, --database PATH      Database file path (default: performance_data.db)"
    echo "  -o, --output DIR         Output directory for reports (default: performance_reports)"
    echo "  -c, --config FILE        Configuration file path"
    echo "  -t, --test-name NAME     Test name for this run"
    echo "  -s, --source-arch ARCH   Source architecture (default: x86_64)"
    echo "  -r, --target-arch ARCH   Target architecture (default: arm64)"
    echo "  -i, --iterations NUM     Number of test iterations (default: 10)"
    echo "  --collect-only           Only collect metrics, don't detect regressions"
    echo "  --detect-only            Only detect regressions, don't collect new metrics"
    echo "  --generate-charts        Generate charts in HTML reports"
    echo "  --format FORMAT          Report format: text, json, html, markdown (default: text)"
    echo "  -h, --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 -t basic_translation -s x86_64 -r arm64 -i 20"
    echo "  $0 --detect-only --format html --generate-charts"
    echo "  $0 --collect-only -t memory_stress_test -i 50"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--database)
            DATABASE_PATH="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -c|--config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        -t|--test-name)
            TEST_NAME="$2"
            shift 2
            ;;
        -s|--source-arch)
            SOURCE_ARCH="$2"
            shift 2
            ;;
        -r|--target-arch)
            TARGET_ARCH="$2"
            shift 2
            ;;
        -i|--iterations)
            ITERATIONS="$2"
            shift 2
            ;;
        --collect-only)
            COLLECT_ONLY=true
            shift
            ;;
        --detect-only)
            DETECT_ONLY=true
            shift
            ;;
        --generate-charts)
            GENERATE_CHARTS=true
            shift
            ;;
        --format)
            REPORT_FORMAT="$2"
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Validate arguments
if [[ "$COLLECT_ONLY" == true && "$DETECT_ONLY" == true ]]; then
    echo -e "${RED}Error: Cannot specify both --collect-only and --detect-only${NC}"
    exit 1
fi

if [[ ! "$REPORT_FORMAT" =~ ^(text|json|html|markdown)$ ]]; then
    echo -e "${RED}Error: Invalid report format: $REPORT_FORMAT${NC}"
    echo "Valid formats: text, json, html, markdown"
    exit 1
fi

# Create output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# Generate timestamp for report file
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="$OUTPUT_DIR/performance_report_$TIMESTAMP.$REPORT_FORMAT"

# Build the command
CMD="cargo run --bin vm-perf-regression-detector --package vm-perf-regression-detector --"

if [[ -n "$CONFIG_FILE" ]]; then
    CMD="$CMD --config $CONFIG_FILE"
fi

CMD="$CMD --database $DATABASE_PATH"
CMD="$CMD --output $REPORT_FILE"
CMD="$CMD --format $REPORT_FORMAT"

if [[ -n "$TEST_NAME" ]]; then
    CMD="$CMD --test-name $TEST_NAME"
fi

if [[ -n "$SOURCE_ARCH" ]]; then
    CMD="$CMD --source-arch $SOURCE_ARCH"
fi

if [[ -n "$TARGET_ARCH" ]]; then
    CMD="$CMD --target-arch $TARGET_ARCH"
fi

if [[ -n "$ITERATIONS" ]]; then
    CMD="$CMD --iterations $ITERATIONS"
fi

if [[ "$COLLECT_ONLY" == true ]]; then
    CMD="$CMD --collect-only"
fi

if [[ "$DETECT_ONLY" == true ]]; then
    CMD="$CMD --detect-only"
fi

if [[ "$GENERATE_CHARTS" == true ]]; then
    CMD="$CMD --generate-charts"
fi

# Print configuration
echo -e "${BLUE}VM Performance Regression Detector${NC}"
echo -e "${BLUE}=====================================${NC}"
echo "Database: $DATABASE_PATH"
echo "Output: $REPORT_FILE"
echo "Format: $REPORT_FORMAT"
if [[ -n "$TEST_NAME" ]]; then
    echo "Test Name: $TEST_NAME"
fi
if [[ -n "$SOURCE_ARCH" ]]; then
    echo "Source Architecture: $SOURCE_ARCH"
fi
if [[ -n "$TARGET_ARCH" ]]; then
    echo "Target Architecture: $TARGET_ARCH"
fi
if [[ -n "$ITERATIONS" ]]; then
    echo "Iterations: $ITERATIONS"
fi
if [[ -n "$CONFIG_FILE" ]]; then
    echo "Config File: $CONFIG_FILE"
fi
if [[ "$COLLECT_ONLY" == true ]]; then
    echo "Mode: Collect Only"
elif [[ "$DETECT_ONLY" == true ]]; then
    echo "Mode: Detect Only"
else
    echo "Mode: Collect and Detect"
fi
if [[ "$GENERATE_CHARTS" == true ]]; then
    echo "Charts: Enabled"
fi
echo ""

# Execute the command
echo -e "${GREEN}Executing: $CMD${NC}"
echo ""

eval $CMD

# Check if the command was successful
if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}Performance regression detection completed successfully!${NC}"
    echo -e "${GREEN}Report saved to: $REPORT_FILE${NC}"
    
    # If HTML report was generated, offer to open it
    if [[ "$REPORT_FORMAT" == "html" && -f "$REPORT_FILE" ]]; then
        echo ""
        read -p "Do you want to open the HTML report? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if command -v open >/dev/null 2>&1; then
                open "$REPORT_FILE"
            elif command -v xdg-open >/dev/null 2>&1; then
                xdg-open "$REPORT_FILE"
            else
                echo -e "${YELLOW}Cannot open HTML report automatically. Please open it manually: $REPORT_FILE${NC}"
            fi
        fi
    fi
else
    echo ""
    echo -e "${RED}Performance regression detection failed!${NC}"
    exit 1
fi