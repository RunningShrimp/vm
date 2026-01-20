//! # VM Build Dependencies
//!
//! 此包由[cargo-hakari](https://github.com/QNNI154/cargo-hakari)自动生成和管理。
//!
//! 它统一管理VM工作区的所有第三方依赖，优化编译时间。
//!
//! ## 关于
//!
//! cargo-hakari通过以下方式减少编译时间：
//! 1. 将所有第三方依赖集中在一个包中
//! 2. 避免在每个包中重复编译相同的依赖
//! 3. 优化依赖图的编译顺序
//!
//! ## 使用
//!
//! 这个包会自动被cargo-hakari管理。如果你需要更新依赖：
//!
//! ```bash
//! # 安装cargo-hakari
//! cargo install cargo-hakari
//!
//! # 生成或更新hakari依赖
//! cargo hakari generate
//!
//! # 验证hakari配置
//! cargo hakari verify
//! ```
//!
//! ## 注意
//!
//! 不要手动编辑此文件中的依赖，因为它们会被cargo-hakari覆盖。
//!
//! 如果需要添加新的第三方依赖到项目，请在根Cargo.toml的
//! `[workspace.dependencies]`部分添加，然后运行`cargo hakari generate`。

// 以下将由cargo-hakari自动填充
// 当运行 `cargo hakari generate` 时，这些pub use语句将自动生成
