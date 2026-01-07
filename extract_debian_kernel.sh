#!/bin/bash
# Extract Debian kernel and initrd from ISO using standard tools

ISO_FILE="/Users/didi/Downloads/debian-13.2.0-amd64-netinst.iso"
EXTRACT_DIR="/tmp/debian_iso_extracted"
mkdir -p "$EXTRACT_DIR"

echo "Extracting kernel and initrd from Debian ISO..."

# Method 1: Use 7z if available
if command -v 7z &> /dev/null; then
    echo "Using 7z to extract..."
    cd "$EXTRACT_DIR"
    7z x -y "$ISO_FILE" 'install.amd/vmlinuz' 'install.amd/initrd.gz' 2>/dev/null

    if [ -f "$EXTRACT_DIR/install.amd/vmlinuz" ]; then
        echo "✓ Extracted: install.amd/vmlinuz"
        cp "$EXTRACT_DIR/install.amd/vmlinuz" "$EXTRACT_DIR/vmlinuz"
    fi

    if [ -f "$EXTRACT_DIR/install.amd/initrd.gz" ]; then
        echo "✓ Extracted: install.amd/initrd.gz"
        cp "$EXTRACT_DIR/install.amd/initrd.gz" "$EXTRACT_DIR/initrd.gz"
    fi
fi

# Method 2: Use isoinfo if available
if command -v isoinfo &> /dev/null && [ ! -f "$EXTRACT_DIR/vmlinuz" ]; then
    echo "Using isoinfo to extract..."

    # Extract kernel
    isoinfo -i "$ISO_FILE" -R -x /install.amd/vmlinuz -o "$EXTRACT_DIR/vmlinuz" 2>/dev/null
    if [ -f "$EXTRACT_DIR/vmlinuz" ]; then
        echo "✓ Extracted: vmlinuz"
    fi

    # Extract initrd
    isoinfo -i "$ISO_FILE" -R -x /install.amd/initrd.gz -o "$EXTRACT_DIR/initrd.gz" 2>/dev/null
    if [ -f "$EXTRACT_DIR/initrd.gz" ]; then
        echo "✓ Extracted: initrd.gz"
    fi
fi

# List what we got
echo ""
echo "Extracted files:"
ls -lh "$EXTRACT_DIR"/vmlinuz "$EXTRACT_DIR"/initrd.gz 2>/dev/null

# Analyze kernel
if [ -f "$EXTRACT_DIR/vmlinuz" ]; then
    echo ""
    echo "Kernel analysis:"
    file "$EXTRACT_DIR/vmlinuz"
    echo ""
    echo "First 64 bytes (hex):"
    xxd -l 64 "$EXTRACT_DIR/vmlinuz"
fi
