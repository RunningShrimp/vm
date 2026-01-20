#!/bin/bash
# Ubuntu 25.10 Desktop VM with Automated Installation
# Apple M4 (ARM64) → x86_64 (AMD64) Cross-Architecture Emulation

set -e

VM_NAME="ubuntu-25.10-desktop-auto"
ISO_PATH="$HOME/vm-images/ubuntu-25.10-desktop-amd64.iso"
DISK_PATH="$HOME/vm-images/ubuntu-25.10-disk-auto.qcow2"
SEED_PATH="$HOME/vm-images/ubuntu-seed.iso"
VM_DIR="$HOME/vm-images"
LOG_FILE="$VM_DIR/vm-autoinstall.log"

# Create new disk for autoinstall
echo "Creating new 64GB disk for automated installation..."
rm -f "$DISK_PATH"
qemu-img create -f qcow2 "$DISK_PATH" 64G

# Resources
VCPUS=4
MEMORY=8192
VRAM=128

echo "=====================================" | tee "$LOG_FILE"
echo "Ubuntu 25.10 Autoinstall on Apple M4" | tee -a "$LOG_FILE"
echo "Date: $(date)" | tee -a "$LOG_FILE"
echo "=====================================" | tee -a "$LOG_FILE"

# Verify files
for file in "$ISO_PATH" "$DISK_PATH" "$SEED_PATH"; do
    if [ ! -f "$file" ]; then
        echo "ERROR: File not found: $file" | tee -a "$LOG_FILE"
        exit 1
    fi
done

echo "Starting automated installation..." | tee -a "$LOG_FILE"
echo "This will take 20-30 minutes" | tee -a "$LOG_FILE"

# QEMU with autoinstall kernel parameters
# Note: We use the seed ISO as a CD-ROM and let the installer find it
/opt/homebrew/bin/qemu-system-x86_64 \
    -name "$VM_NAME" \
    -machine q35,accel=tcg \
    -cpu qemu64 \
    -smp cpus="$VCPUS",sockets=1,cores="$VCPUS",threads=1 \
    -m "$MEMORY" \
    -drive file="$DISK_PATH",if=virtio,format=qcow2 \
    -drive file="$ISO_PATH",media=cdrom,readonly=on \
    -drive file="$SEED_PATH",if=virtio,readonly=on \
    -device virtio-vga,xres=1920,yres=1080 \
    -display cocoa \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-tablet \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -device virtio-net-pci,netdev=net0 \
    -rtc base=localtime,clock=host \
    -boot d \
    2>&1 | tee -a "$LOG_FILE"

echo "Installation completed!" | tee -a "$LOG_FILE"
echo "You can now boot the installed system with:" | tee -a "$LOG_FILE"
echo "qemu-system-x86_64 -drive file=$DISK_PATH,if=virtio -m 8192 -smp 4 -device virtio-vga -display cocoa" | tee -a "$LOG_FILE"
