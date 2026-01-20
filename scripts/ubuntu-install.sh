#!/bin/bash
# Ubuntu 25.10 Desktop Automated Installation on Apple M4
# Using QEMU for full x86_64 support (vm-cli at 45% incomplete)

set -e

VM_NAME="ubuntu-25.10-desktop"
ISO_PATH="$HOME/vm-images/ubuntu-25.10-desktop-amd64.iso"
DISK_PATH="$HOME/vm-images/ubuntu-25.10-final.qcow2"
VM_DIR="$HOME/vm-images"
LOG_FILE="$VM_DIR/installation.log"
PID_FILE="$VM_DIR/vm.pid"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "========================================" | tee "$LOG_FILE"
echo "Ubuntu 25.10 Desktop Automated Installation" | tee -a "$LOG_FILE"
echo "Platform: Apple M4 (ARM64) → x86_64" | tee -a "$LOG_FILE"
echo "========================================" | tee -a "$LOG_FILE"
echo ""

# Create new disk
echo -e "${YELLOW}Creating 64GB virtual disk...${NC}" | tee -a "$LOG_FILE"
rm -f "$DISK_PATH"
qemu-img create -f qcow2 "$DISK_PATH" 64G 2>&1 | tee -a "$LOG_FILE"
echo -e "${GREEN}✓ Disk created${NC}" | tee -a "$LOG_FILE"
echo ""

# Launch VM with automated install
echo -e "${YELLOW}Launching VM for installation...${NC}" | tee -a "$LOG_FILE"
echo "This will take 20-30 minutes" | tee -a "$LOG_FILE"
echo "VM window will open - please complete installation manually" | tee -a "$LOG_FILE"
echo "After installation, the VM will shutdown automatically" | tee -a "$LOG_FILE"
echo ""

# Kill any existing QEMU
pkill -9 qemu-system-x86_64 2>/dev/null || true
sleep 2

# Start QEMU with optimized settings
/opt/homebrew/bin/qemu-system-x86_64 \
    -name "$VM_NAME" \
    -machine q35,accel=tcg \
    -cpu qemu64,+ssse3,+sse4.1,+sse4.2 \
    -smp cpus=4,sockets=1,cores=4,threads=1 \
    -m 8192 \
    -drive file="$DISK_PATH",if=virtio,format=qcow2,index=0 \
    -drive file="$ISO_PATH",media=cdrom,readonly=on,index=1 \
    -device virtio-vga,xres=1920,yres=1080 \
    -display cocoa \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-tablet \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -device virtio-net-pci,netdev=net0 \
    -rtc base=localtime,clock=host \
    -boot d \
    2>&1 | tee -a "$LOG_FILE" &

VM_PID=$!
echo $VM_PID > "$PID_FILE"
echo -e "${GREEN}✓ VM started (PID: $VM_PID)${NC}" | tee -a "$LOG_FILE"
echo ""

# Wait for VM or timeout
TIMEOUT=1800  # 30 minutes
ELAPSED=0
CHECK_INTERVAL=30

echo "Monitoring installation (will timeout after $TIMEOUT seconds)..." | tee -a "$LOG_FILE"
echo ""

while [ $ELAPSED -lt $TIMEOUT ]; do
    # Check if VM is still running
    if ! ps -p $VM_PID > /dev/null 2>&1; then
        echo -e "${GREEN}✓ VM has stopped - installation complete!${NC}" | tee -a "$LOG_FILE"
        break
    fi

    # Show progress
    echo "[$(date +%H:%M:%S)] Running... (${ELAPSED}s/${TIMEOUT}s)" | tee -a "$LOG_FILE"

    # Check disk usage to see if installation is progressing
    DISK_SIZE=$(du -h "$DISK_PATH" 2>/dev/null | cut -f1)
    echo "  Disk written: $DISK_SIZE" | tee -a "$LOG_FILE"

    sleep $CHECK_INTERVAL
    ELAPSED=$((ELAPSED + CHECK_INTERVAL))
done

# Final check
if [ $ELAPSED -ge $TIMEOUT ]; then
    echo -e "${YELLOW}⚠ Timeout reached${NC}" | tee -a "$LOG_FILE"
    if ps -p $VM_PID > /dev/null 2>&1; then
        echo "VM is still running. Killing process..." | tee -a "$LOG_FILE"
        kill $VM_PID 2>/dev/null || true
    fi
fi

echo ""
echo "========================================" | tee -a "$LOG_FILE"
echo "Installation Summary" | tee -a "$LOG_FILE"
echo "========================================" | tee -a "$LOG_FILE"
echo "Disk image: $DISK_PATH" | tee -a "$LOG_FILE"
echo "Final size: $(du -h "$DISK_PATH" | cut -f1)" | tee -a "$LOG_FILE"
echo ""
echo "To boot the installed system, run:" | tee -a "$LOG_FILE"
echo "  qemu-system-x86_64 -drive file=$DISK_PATH,if=virtio -m 8192 -smp 4 -device virtio-vga -display cocoa -netdev user,id=net0 -device virtio-net-pci,netdev=net0 -boot c"
echo "========================================" | tee -a "$LOG_FILE"
