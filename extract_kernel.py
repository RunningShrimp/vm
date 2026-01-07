#!/usr/bin/env python3
"""
Simple ISO 9660 parser to extract Debian kernel and initrd.
For use with VM project - Debian ISO testing.
"""

import struct
import sys
import os

def parse_iso9660_primary_volume_descriptor(iso_data):
    """Parse ISO 9660 Primary Volume Descriptor"""
    # Volume descriptors start at sector 16 (0x8000)
    offset = 0x8000

    while True:
        descriptor = iso_data[offset:offset+2048]
        type_code = descriptor[0]

        if type_code == 0:  # Boot Descriptor
            pass
        elif type_code == 1:  # Primary Volume Descriptor
            print(f"Found Primary Volume Descriptor at offset 0x{offset:x}")
            pvd = descriptor

            # Parse PVD fields
            system_id = pvd[8:40].decode('ascii').strip()
            volume_id = pvd[40:72].decode('ascii').strip()
            volume_space_size = struct.unpack('<I', pvd[80:84])[0]
            volume_set_size = struct.unpack('<H', pvd[120:122])[0]
            volume_sequence_number = struct.unpack('<H', pvd[124:126])[0]
            logical_block_size = struct.unpack('<H', pvd[128:130])[0]
            path_table_size = struct.unpack('<I', pvd[132:136])[0]

            print(f"  System ID: {system_id}")
            print(f"  Volume ID: {volume_id}")
            print(f"  Volume Space Size: {volume_space_size} blocks")
            print(f"  Logical Block Size: {logical_block_size} bytes")

            # Root Directory Record is at offset 156
            root_dir_record = pvd[156:190]
            return root_dir_record, logical_block_size

        elif type_code == 255:  # Volume Descriptor Set Terminator
            print("Reached Volume Descriptor Set Terminator")
            break

        offset += 2048

    return None, None

def parse_directory_record(record_data):
    """Parse a directory record"""
    if not record_data or record_data[0] == 0:
        return None

    length = record_data[0]
    ext_attr_length = record_data[1]
    extent_location = struct.unpack('<I', record_data[2:6] + b'\x00\x00\x00\x00')[0]
    data_length = struct.unpack('<I', record_data[10:14] + b'\x00\x00\x00\x00')[0]

    # Flags and file unit size skip
    # File identifier starts at offset 33 in directory record
    name_length = record_data[32]
    name = record_data[33:33+name_length].decode('ascii').strip()

    is_dir = (record_data[25] & 0x02) != 0

    return {
        'name': name,
        'extent': extent_location,
        'length': data_length,
        'is_dir': is_dir,
        'record': record_data[:length]
    }

def find_file_in_path(iso_data, root_record, block_size, path_components):
    """Recursively find a file by path"""
    if not path_components:
        return None

    current = root_record
    current_path = []

    for component in path_components:
        if not current['is_dir']:
            return None

        # Read directory extent
        extent_offset = current['extent'] * block_size
        dir_data = iso_data[extent_offset:extent_offset + current['length']]

        # Parse directory entries
        offset = 0
        found = False

        while offset < len(dir_data):
            record_length = dir_data[offset]
            if record_length == 0:
                # Padding to end of sector
                sector_remainder = (block_size - (offset % block_size))
                if sector_remainder < block_size:
                    offset += sector_remainder
                continue

            record = dir_data[offset:offset+record_length]
            entry = parse_directory_record(record)

            if entry and entry['name'] == component:
                current = entry
                current_path.append(component)
                found = True
                break

            offset += record_length

        if not found:
            return None

    return current

def extract_file(iso_data, file_record, block_size, output_path):
    """Extract a file from the ISO"""
    extent_offset = file_record['extent'] * block_size
    file_data = iso_data[extent_offset:extent_offset + file_record['length']]

    with open(output_path, 'wb') as f:
        f.write(file_data)

    print(f"Extracted: {output_path} ({file_record['length']} bytes)")
    return file_data

def main():
    iso_path = '/Users/didi/Downloads/debian-13.2.0-amd64-netinst.iso'
    output_dir = '/tmp/debian_iso_extracted'

    # Create output directory
    os.makedirs(output_dir, exist_ok=True)

    # Read ISO
    print(f"Reading ISO: {iso_path}")
    with open(iso_path, 'rb') as f:
        iso_data = f.read()

    # Parse Primary Volume Descriptor
    root_record, block_size = parse_iso9660_primary_volume_descriptor(iso_data)

    if not root_record:
        print("ERROR: Could not find Primary Volume Descriptor")
        return 1

    root_record = parse_directory_record(root_record)

    # Look for Debian kernel
    # Typical path: /install.amd/vmlinuz or /install/vmlinuz
    possible_paths = [
        ['install.amd', 'vmlinuz'],
        ['install', 'vmlinuz'],
        ['boot', 'vmlinuz'],
        ['isolinux', 'vmlinuz'],
    ]

    kernel_record = None
    for path in possible_paths:
        print(f"Searching for: /{'/'.join(path)}")
        kernel_record = find_file_in_path(iso_data, root_record, block_size, path)
        if kernel_record:
            print(f"FOUND: /{'/'.join(path)}")
            break

    if not kernel_record:
        print("ERROR: Could not find kernel in ISO")
        print("Trying alternative search...")

        # Brute force search for kernel magic
        kernel_magic = b'\xb8\xc0\x07\x8e'  # Typical x86 kernel start
        pos = iso_data.find(kernel_magic)
        if pos != -1:
            print(f"Found kernel-like code at offset 0x{pos:x}")
            # Use this as kernel
            kernel_data = iso_data[pos:pos+20*1024*1024]  # Up to 20MB
            with open(f'{output_dir}/vmlinuz', 'wb') as f:
                f.write(kernel_data)
            print(f"Extracted kernel: {output_dir}/vmlinuz ({len(kernel_data)} bytes)")
        else:
            return 1
    else:
        extract_file(iso_data, kernel_record, block_size, f'{output_dir}/vmlinuz')

    # Look for initrd
    possible_initrd = [
        ['install.amd', 'initrd.gz'],
        ['install', 'initrd.gz'],
        ['boot', 'initrd.gz'],
    ]

    initrd_record = None
    for path in possible_initrd:
        print(f"Searching for: /{'/'.join(path)}")
        initrd_record = find_file_in_path(iso_data, root_record, block_size, path)
        if initrd_record:
            print(f"FOUND: /{'/'.join(path)}")
            break

    if initrd_record:
        extract_file(iso_data, initrd_record, block_size, f'{output_dir}/initrd.gz')
    else:
        print("WARNING: Could not find initrd.gz")

    print(f"\nExtracted files available in: {output_dir}")
    print("Kernel: vmlinuz")
    if initrd_record:
        print("Initrd: initrd.gz")

    return 0

if __name__ == '__main__':
    sys.exit(main())
