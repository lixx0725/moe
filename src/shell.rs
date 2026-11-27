//! 根据当前系统选择默认 Shell，向 PTY 传递启动目录和参数。

use alacritty_terminal::tty::{Options, Shell};
use std::{env, io, path::PathBuf};

/// 每次启动从调用者所在目录开始；通过桌面启动时可回退到用户目录。
pub(crate) fn options() -> io::Result<Options> {
    let (program, args) = default_shell()?;
    let program = program.into_os_string().into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "default shell path is not valid UTF-8",
        )
    })?;
    Ok(Options {
        shell: Some(Shell::new(program, args)),
        working_directory: env::current_dir()
            .ok()
            .filter(|path| path.is_dir())
            .or_else(dirs::home_dir),
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

    account_shell()
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

#[cfg(unix)]
fn account_shell() -> Option<PathBuf> {
    use std::{
        ffi::{CStr, OsStr},
        mem::MaybeUninit,
        os::unix::ffi::OsStrExt,
        ptr,
    };

    let mut entry = MaybeUninit::<libc::passwd>::uninit();
    let mut buffer = vec![0u8; 16 * 1024];
    let mut found = ptr::null_mut();
    // SAFETY: passwd 及缓冲区在查询期间有效；成功时在释放缓冲区前复制路径。
    unsafe {
        let result = libc::getpwuid_r(
            libc::getuid(),
            entry.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut found,
        );
        if result != 0 || found.is_null() || (*found).pw_shell.is_null() {
            return None;
        }
        Some(PathBuf::from(OsStr::from_bytes(
            CStr::from_ptr((*found).pw_shell).to_bytes(),
        )))
    }
}
