//! 根据当前系统选择默认 Shell，向 PTY 传递启动目录和参数。

use alacritty_terminal::tty::{Options, Shell};
#[cfg(windows)]
use std::env;
use std::{io, path::PathBuf};

/// 以应用选择的目录启动默认 Shell。
pub(crate) fn options(working_directory: Option<PathBuf>) -> io::Result<Options> {
    let (program, args) = default_shell()?;
    launch_options(program, args, working_directory)
}

/// 将 Shell 路径和参数转换为 PTY 配置。
fn launch_options(
    program: PathBuf,
    args: Vec<String>,
    working_directory: Option<PathBuf>,
) -> io::Result<Options> {
    let program = program.into_os_string().into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "default shell path is not valid UTF-8",
        )
    })?;
    Ok(Options {
        shell: Some(Shell::new(program, args)),
        working_directory,
        drain_on_exit: true,
        env: [
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
        ]
        .into(),
        #[cfg(windows)]
        escape_args: true,
    })
}

/// 在 Windows 测试中跳过本机 PowerShell 启动脚本。
#[cfg(all(test, windows))]
pub(crate) fn test_options(working_directory: Option<PathBuf>) -> io::Result<Options> {
    let (program, mut args) = default_shell()?;
    args.push("-NoProfile".to_owned());
    launch_options(program, args, working_directory)
}

#[cfg(windows)]
fn default_shell() -> io::Result<(PathBuf, Vec<String>)> {
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
        .map(|program| (program, vec!["-NoLogo".into()]))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PowerShell was not found"))
}

#[cfg(unix)]
fn default_shell() -> io::Result<(PathBuf, Vec<String>)> {
    use std::os::unix::fs::PermissionsExt;

    #[cfg(target_os = "macos")]
    let fallback = "/bin/zsh";
    #[cfg(not(target_os = "macos"))]
    let fallback = "/bin/sh";

    crate::platform::account_shell()
        .into_iter()
        .chain([PathBuf::from(fallback), PathBuf::from("/bin/sh")])
        .find(|path| {
            path.is_absolute()
                && path
                    .metadata()
                    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .map(|path| (path, Vec::new()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no executable login shell was found",
            )
        })
}
