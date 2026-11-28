//! 统一应用目录规则，解析时不创建目录。

use std::{env, path::PathBuf};

/// 当前用户的 Moe 应用目录与默认启动位置。
pub(crate) struct AppPaths {
    /// 配置及快捷键目录。
    #[cfg_attr(not(test), expect(dead_code, reason = "配置模块接入时使用"))]
    pub(crate) config_dir: Option<PathBuf>,
    /// 会话和数据库等持久数据的目录。
    #[cfg_attr(not(test), expect(dead_code, reason = "存储模块接入时使用"))]
    pub(crate) data_dir: Option<PathBuf>,
    /// 可重建的本地缓存目录。
    #[cfg_attr(not(test), expect(dead_code, reason = "缓存功能接入时使用"))]
    pub(crate) cache_dir: Option<PathBuf>,
    /// 本地日志目录。优先使用系统状态目录。
    #[cfg_attr(not(test), expect(dead_code, reason = "日志功能接入时使用"))]
    pub(crate) log_dir: Option<PathBuf>,
    /// 当前目录无效时使用的 Shell 启动位置。
    home_dir: Option<PathBuf>,
}

impl AppPaths {
    /// 获取平台目录并为 Moe 分配应用子目录。
    pub(crate) fn discover() -> Self {
        let data_dir = dirs::data_local_dir();
        let cache_dir = dirs::cache_dir();
        let log_dir = dirs::state_dir().or_else(|| data_dir.clone());
        // Windows 的数据和缓存可能共享系统根，使用独立的缓存子目录。
        let shared_cache_root = cache_dir == data_dir;
        Self {
            config_dir: dirs::config_dir().map(|root| root.join("moe")),
            data_dir: data_dir.map(|root| root.join("moe")),
            cache_dir: cache_dir.map(|root| {
                let directory = root.join("moe");
                if shared_cache_root {
                    directory.join("cache")
                } else {
                    directory
                }
            }),
            log_dir: log_dir.map(|root| root.join("moe").join("logs")),
            home_dir: dirs::home_dir(),
        }
    }

    /// 优先使用调用者当前目录，再尝试用户主目录。
    pub(crate) fn working_directory(&self) -> Option<PathBuf> {
        env::current_dir()
            .ok()
            .filter(|path| path.is_dir())
            .or_else(|| self.home_dir.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_platform_roots_without_mixing_cache_and_data() {
        let paths = AppPaths::discover();
        assert_eq!(
            paths.config_dir,
            dirs::config_dir().map(|root| root.join("moe"))
        );
        assert_eq!(
            paths.data_dir,
            dirs::data_local_dir().map(|root| root.join("moe"))
        );
        if let Some(root) = dirs::cache_dir() {
            let expected = if Some(&root) == dirs::data_local_dir().as_ref() {
                root.join("moe").join("cache")
            } else {
                root.join("moe")
            };
            assert_eq!(paths.cache_dir, Some(expected));
            assert_ne!(paths.cache_dir, paths.data_dir);
        }
        assert_eq!(
            paths.log_dir,
            dirs::state_dir()
                .or_else(dirs::data_local_dir)
                .map(|root| root.join("moe").join("logs"))
        );
        assert_eq!(paths.working_directory(), env::current_dir().ok());
    }
}
