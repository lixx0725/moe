//! Windows 命令行入口。打包为 moe.com，与图形程序 moe.exe 放在同一目录。

use std::process::ExitCode;

#[cfg(windows)]
use std::{
    env, io,
    process::{Command, Stdio},
};

#[cfg(windows)]
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};

/// 保持等待方存活，让连接到控制台的 Moe 子进程处理 Ctrl+C 和 Ctrl+Break。
#[cfg(windows)]
unsafe extern "system" fn handle_interrupt(event: u32) -> i32 {
    i32::from(matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT))
}

/// 转发原始参数与退出码。无参数时打开图形窗口后立即归还命令行。
#[cfg(windows)]
fn launch() -> io::Result<ExitCode> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let mut command = Command::new(env::current_exe()?.with_file_name("moe.exe"));
    if args.is_empty() {
        // 让后台窗口与调用者的输入及输出管道独立。
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        return Ok(ExitCode::SUCCESS);
    }

    // SAFETY: 回调是静态函数，注册仅影响当前启动器进程。
    unsafe { SetConsoleCtrlHandler(Some(handle_interrupt), 1) };
    let status = command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from))
}

#[cfg(windows)]
fn main() -> ExitCode {
    launch().unwrap_or_else(|error| {
        eprintln!("moe: {error}");
        ExitCode::FAILURE
    })
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("moe: the console launcher requires Windows");
    ExitCode::FAILURE
}
