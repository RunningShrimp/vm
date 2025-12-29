# SQLx Upgrade Status Report
**Date**: 2025-12-28
**Task**: Upgrade sqlx from 0.6.3 to 0.8.x

---

## Completed Tasks ✅

### 1. Phase 0: Quick Fixes
- ✅ **Format Issues**: All Rust code is properly formatted (verified with `cargo fmt --check`)
- ✅ **thiserror Upgrade**: Workspace uses thiserror 2.0.17 (latest 2.x version)
- ✅ **Git Tag Created**: `pre-phase1-sqlx-upgrade` for rollback capability

### 2. Phase 1: Dependency Modernization (Partial)
- ✅ **Workspace Cargo.toml Updated**:
  ```toml
  # Before:
  sqlx = { version = "0.6", features = ["runtime-tokio-rustls", "postgres", "chrono", "json", "uuid"] }

  # After:
  sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "chrono", "json", "uuid"] }
  ```
  File: `/Users/wangbiao/Desktop/project/vm/Cargo.toml:134`

---

## Remaining Tasks ⚠️

### 3. Update Cargo.lock
**Status**: ❌ Blocked by network connectivity issues

**Issue**:
- DNS resolution failures for index.crates.io
- Rate limiting (429 errors) on USTC mirror
- Timeout errors on all mirror sources

**Attempted Solutions**:
1. ✗ USTC mirror (rate limiting)
2. ✗ rsproxy mirror (repository not found)
3. ✗ Tsinghua mirror (not yet tested)
4. ✗ Official crates.io (DNS resolution failure)

**Required Action**:
Once network connectivity is restored, run:
```bash
# Option 1: Update entire lock file
cargo update

# Option 2: Update only sqlx and related packages
cargo update -p sqlx -p sqlx-core -p sqlx-macros
```

### 4. Fix Breaking Changes (Estimated: 50-100 errors)

**Affected Packages** (16 total):
1. vm-runtime
2. vm-boot
3. vm-service
4. vm-core
5. vm-cross-arch
6. vm-engine-jit
7. vm-engine-interpreter
8. vm-device
9. vm-mem
10. vm-accel
11. vm-platform
12. vm-plugin
13. vm-monitor
14. vm-desktop
15. vm-cli
16. vm-interface

**Key Breaking Changes**:

#### A. Type Signature Changes
```rust
// ❌ Old API (sqlx 0.6)
use sqlx::{PgPool, Executor};
let rows = sqlx::query("SELECT * FROM users")
    .fetch_all(&pool)
    .await?;

// ✅ New API (sqlx 0.8)
use sqlx::{PgPool, Executor};
let rows: Vec<User> = sqlx::query_as("SELECT * FROM users")
    .fetch_all(&pool)
    .await?;
```

#### B. Derive Macro Required
```rust
// ✅ Add this derive
#[derive(sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
}
```

#### C. Error Handling Changes
```rust
// ❌ Old API
use sqlx::Error as SqlxError;
match err {
    SqlxError::Database(db_err) => { }
    _ => { }
}

// ✅ New API (more specific error types)
use sqlx::Error as SqlxError;
match err {
    SqlxError::Database(db_err) => {
        // db_err now provides more information
    }
    SqlxError::RowNotFound => { }
    _ => { }
}
```

**Fix Strategy**:
```bash
# 1. Build to see all errors
cargo build --workspace 2>&1 | tee build_errors.txt

# 2. Count errors
grep "error:" build_errors.txt | wc -l

# 3. Fix by package
for pkg in vm-runtime vm-boot vm-service vm-core vm-cross-arch vm-engine-jit vm-engine-interpreter vm-device vm-mem vm-accel vm-platform vm-plugin vm-monitor vm-desktop vm-cli vm-interface; do
    echo "Fixing $pkg..."
    cargo build -p $pkg 2>&1 | grep "error:" > ${pkg}_errors.txt
    # Manually fix errors in ${pkg}_errors.txt
done
```

**Key Files to Modify**:
1. `vm-runtime/src/lib.rs` - GC persistence queries
2. `vm-boot/src/runtime_service.rs` - Runtime service queries
3. `vm-service/src/vm_service.rs` - VM state persistence
4. `vm-core/src/event_store/mod.rs` - Event sourcing queries

### 5. Testing
**Required Tests**:
```bash
# Unit tests
cargo test --workspace --lib

# Integration tests
cargo test --workspace --test '*'

# Documentation tests
cargo test --workspace --doc

# All features
cargo test --workspace --all-features

# Expected: 339 unit tests + 196 integration tests
```

---

## Network Troubleshooting

### Option 1: Use VPN
If in China, connect to a VPN to access crates.io:
```bash
# Test connectivity
curl -I https://index.crates.io/config.json

# Then run cargo update
cargo update -p sqlx
```

### Option 2: Try Different Mirror
Edit `~/.cargo/config.toml`:
```toml
[source.crates-io]
replace-with = 'tuna'

[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"
```

### Option 3: Wait for Network Recovery
The USTC mirror may be temporarily unavailable. Wait and retry later.

### Option 4: Manual Crate Download (Advanced)
1. Download sqlx 0.8 crate manually
2. Place in `~/.cargo/registry/src/`
3. Manually edit Cargo.lock

---

## Current State

**Repository Status**:
- Working branch: master
- Modified files:
  - `/Users/wangbiao/Desktop/project/vm/Cargo.toml` (sqlx 0.6 → 0.8)
  - `Cargo.lock` (needs update)

**Git Status**:
```bash
# Check current changes
git status

# View the diff
git diff Cargo.toml

# Rollback if needed
git checkout Cargo.toml
```

**Rollback Plan**:
If issues arise, rollback using the git tag:
```bash
# Reset to pre-upgrade state
git checkout pre-phase1-sqlx-upgrade

# Or revert just the Cargo.toml change
git checkout HEAD -- Cargo.toml
```

---

## Next Steps (Once Network is Restored)

1. **Update Cargo.lock**:
   ```bash
   cargo update -p sqlx -p sqlx-core -p sqlx-macros
   ```

2. **Identify Compilation Errors**:
   ```bash
   cargo build --workspace 2>&1 | grep "error:" | wc -l
   ```

3. **Fix Errors Systematically**:
   - Start with vm-runtime (main sqlx user)
   - Then vm-boot, vm-service, vm-core
   - Finally, remaining packages

4. **Run Tests**:
   ```bash
   cargo test --workspace --all-features
   ```

5. **Verify**:
   ```bash
   cargo tree | grep sqlx  # Should show 0.8.x
   cargo test --workspace   # All tests pass
   ```

---

## Success Criteria

- [ ] sqlx upgraded to 0.8.x in Cargo.lock
- [ ] Zero compilation errors
- [ ] All 535 tests pass (339 unit + 196 integration)
- [ ] Zero runtime panics
- [ ] No breaking changes in public APIs

---

## Resources

- **sqlx 0.8 Migration Guide**: https://github.com/launchbadge/sqlx/blob/main/CHANGELOG.md#080
- **sqlx Documentation**: https://docs.rs/sqlx/
- **Implementation Plan**: `/Users/wangbiao/.claude/plans/gentle-hopping-pearl.md`
- **Architecture Review**: `/Users/wangbiao/Desktop/project/vm/COMPREHENSIVE_ARCHITECTURE_REVIEW_2025-12-28.md`

---

**Last Updated**: 2025-12-28
**Status**: ⚠️ Blocked by network connectivity
**Next Action**: Restore network connectivity, then run `cargo update -p sqlx`
