# Cargo.lock 变更摘要

## 文件大小变化

- **更新前**: 8820行
- **更新后**: 8764行
- **变化**: -56行 (-0.6%)

## 主要版本变更

### Random生态升级

```
rand 0.8.5 → 0.9.2
rand_core 0.6.4 → 0.9.3
rand_chacha 0.3.1 → 0.9.0
getrandom 0.2.16 → 0.3.4
```

### 工具库升级

```
miniz_oxide 0.7.4 → 0.8.9
log 0.4.29 → 0.4.22 (锁定版本)
time 0.3.36 → 0.3.44
criterion 0.5.1 → 0.8.1
```

### 数据结构升级

```
hashbrown 0.14.5 → 0.16.1 (新项目默认)
indexmap 1.9.3 → 2.12.1
itertools 0.10.5 → 0.14.0
```

### 其他统一

```
bitflags 1.3.2 → 2.10.0 (新项目)
base64 0.21.7 → 0.22.1 (新项目)
syn 1.0.109 → 2.0.111
semver 1.0.23 → 1.0.27
```

## 传递依赖说明

虽然我们统一了工作空间级别的依赖版本，但以下依赖仍保持多版本：

1. **hashbrown**: 0.12.3, 0.13.2, 0.14.5, 0.15.5, 0.16.1
   - 原因: cranelift、indexmap等第三方库的严格要求

2. **rand**: 0.7.3, 0.8.5, 0.9.2
   - 原因: 一些中间件库尚未升级到0.9

3. **bitflags**: 1.3.2, 2.10.0
   - 原因: winit、smoltcp使用1.x版本

4. **base64**: 0.21.7, 0.22.1
   - 原因: tauri的swift-rs依赖要求0.21.x

## 备份信息

原始Cargo.lock已备份至: `Cargo.lock.before_unification`

恢复方法（如需要）:
```bash
mv Cargo.lock.before_unification Cargo.lock
```

---

生成时间: 2025-12-30
