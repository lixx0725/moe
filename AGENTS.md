# moe 开发约定

- moe 是面向 Windows、Linux 和 macOS 的原生终端。构建、默认 Shell 及验证方式见 [README](README.md)。
- `src/app/` 组装应用服务，统一应用路径。`src/cli/` 负责命令行入口。
- `src/workspace/` 决定会话在窗口中的组织方式。`src/session/` 管理 PTY 和会话生命周期。`src/terminal/` 管理网格与协议。`src/shell/` 选择 Shell，`src/platform/` 处理系统查询。
- `src/interaction/` 负责输入路由。`src/ui/` 按窗口、界面区域和组件组织，跨区复用组件归 `ui/components/`，会话与布局规则由业务模块负责。
- 自有源码的结构体、字段、枚举、枚举变体和重要方法均需中文注释。写清设计原因、单位、平台差异和生命周期。修改行为时同步更新注释。
- 新增或升级第三方依赖时，优先选择兼容固定工具链和目标平台的最新稳定版。核对许可证并更新 `Cargo.lock`。
- Git 提交使用中文 Conventional Commits。版本变更单独提交，推送、打 tag 和发布按用户明确指示执行。
- 提交前检查格式、Clippy 和测试。分别记录平台构建、真实 PTY 测试和图形实机验收结果。
