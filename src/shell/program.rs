//! 在显式路径或 PATH 中定位 Shell，并保留各平台的默认选择。

use std::{
    env, fs, io,
    path::{self, Component, Path, PathBuf},
};

/// 裸程序名在 PATH 中查找，带目录的路径从 Shell 工作目录解析。
pub(super) fn resolve(requested: &Path, working_directory: Option<&Path>) -> io::Result<PathBuf> {
    if requested.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the shell executable name or path is empty",
        ));
    }

    let mut components = requested.components();
    if matches!(components.next(), Some(Component::Normal(name)) if name == requested.as_os_str())
        && components.next().is_none()
    {
        let search_path = env::var_os("PATH")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PATH is not set"))?;
        return env::split_paths(&search_path)
            .find_map(|dir| {
                let candidate = absolute_path(&dir.join(requested), working_directory).ok()?;
                executable(candidate).ok()
            })
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no executable was found in PATH")
            });
    }

    executable(absolute_path(requested, working_directory)?)
}

/// 将候选路径转换为绝对路径，保留符号链接和调用者选择的程序名。
fn absolute_path(path: &Path, working_directory: Option<&Path>) -> io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    match working_directory {
        Some(directory) => path::absolute(directory.join(path)),
        None => path::absolute(path),
    }
}

/// Windows 优先补上原生程序扩展名，避免把命令解释脚本当作 Shell 程序。
fn executable(path: PathBuf) -> io::Result<PathBuf> {
    #[cfg(windows)]
    if path.extension().is_none() {
        for extension in ["exe", "com"] {
            let candidate = path.with_extension(extension);
            if require_executable(&candidate).is_ok() {
                return Ok(candidate);
            }
        }
    }
    require_executable(&path)?;
    Ok(path)
}

/// 路径必须指向文件，Unix 上还必须有执行权限位。
fn require_executable(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not a file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "file has no execute permission",
            ));
        }
    }
    Ok(())
}

/// 优先选择 PowerShell 7，其次选择系统自带的 Windows PowerShell。
#[cfg(windows)]
pub(super) fn default_shell() -> io::Result<(PathBuf, Vec<&'static str>)> {
    let mut pwsh = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        pwsh.extend(env::split_paths(&path).map(|dir| dir.join("pwsh.exe")));
    }
    if let Some(program_files) = env::var_os("ProgramFiles") {
        pwsh.push(PathBuf::from(program_files).join(r"PowerShell\7\pwsh.exe"));
    }
    if let Some(root) = env::var_os("SystemRoot") {
        pwsh.push(PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    }
    pwsh.into_iter()
        .find(|path| path.is_absolute() && path.is_file())
        .map(|program| (program, vec!["-NoLogo"]))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PowerShell was not found"))
}

/// 从账户配置选择 Shell，并按平台顺序回退。
#[cfg(unix)]
pub(super) fn default_shell() -> io::Result<(PathBuf, Vec<&'static str>)> {
    #[cfg(target_os = "macos")]
    let fallback = "/bin/zsh";
    #[cfg(not(target_os = "macos"))]
    let fallback = "/bin/sh";

    crate::platform::account_shell()
        .into_iter()
        .chain([PathBuf::from(fallback), PathBuf::from("/bin/sh")])
        .find(|path| path.is_absolute() && require_executable(path).is_ok())
        .map(|path| (path, Vec::new()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no executable login shell was found",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_shell_path_preserves_spaces_and_unicode() {
        let root = tempfile::tempdir().expect("创建临时工作目录");
        let relative = Path::new("工具 目录").join(if cfg!(windows) {
            "测试.exe"
        } else {
            "测试"
        });
        let program = root.path().join(&relative);
        fs::create_dir_all(program.parent().expect("路径包含父目录")).expect("创建测试目录");
        fs::write(&program, "").expect("创建测试程序文件");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("设置执行权限");
        }
        assert_eq!(
            resolve(&relative, Some(root.path())).expect("解析 Shell 路径"),
            program
        );
    }
}
