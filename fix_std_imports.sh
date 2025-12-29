#!/bin/bash

# 这个脚本将vm-core中的条件std编译改为直接使用std

FILES=(
  "vm-core/src/domain_event_bus.rs"
  "vm-core/src/syscall.rs"
  "vm-core/src/value_objects.rs"
  "vm-core/src/gdb.rs"
  "vm-core/src/event_sourcing.rs"
  "vm-core/src/aggregate_root.rs"
  "vm-core/src/unified_event_bus.rs"
  "vm-core/src/domain_events.rs"
  "vm-core/src/device_emulation.rs"
)

for file in "${FILES[@]}"; do
  if [ -f "$file" ]; then
    echo "Processing $file..."
    # 移除 #[cfg(feature = "std")] 和对应的 #[cfg(not(feature = "std")]
    # 但保留 #[cfg(feature = "async")] 等其他特性
    sed -i.bak \
      -e '/^#\[cfg(feature = "std")\]$/d' \
      -e '/^#\[cfg(not(feature = "std"))\]$/d' \
      "$file"
    echo "Fixed $file"
  fi
done

echo "Done!"



