//! 组装应用生命周期内使用的服务。

use super::paths::AppPaths;

/// 启动入口组装的应用级服务。
pub(super) struct AppServices {
    /// 配置、数据、缓存、日志和 Shell 启动路径。
    pub(super) paths: AppPaths,
}
impl AppServices {
    /// 发现当前用户的应用路径。
    pub(super) fn new() -> Self {
        Self {
            paths: AppPaths::discover(),
        }
    }
}
