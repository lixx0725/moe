# moe 开发约定

- moe 是面向 Windows、Linux 和 macOS 的原生终端。构建、默认 Shell 及验证方式见 [README](README.md)。
- `src/shell.rs` 选择平台默认 Shell，`src/session.rs` 管理 PTY 生命周期，`src/view.rs` 负责窗口输入与绘制。各模块保持易懂、易维护，按实际需求引入扩展点。
- 自有源码使用中文注释。写清设计原因、单位、平台差异和生命周期。修改行为时同步更新注释。
- 新增或升级第三方依赖时，优先选择兼容固定工具链和目标平台的最新稳定版。核对许可证并更新 `Cargo.lock`。
- Git 提交使用中文 Conventional Commits。版本变更单独提交，推送、打 tag 和发布按用户明确指示执行。
- 提交前检查格式、Clippy 和测试。分别记录平台构建、真实 PTY 测试和图形实机验收结果。
