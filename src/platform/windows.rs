//! 提供 Windows 的控制台连接及安装进程协调。

use std::{
    ffi::OsStr,
    io,
    os::windows::{ffi::OsStrExt, io::OwnedHandle},
    ptr,
};

use std::os::windows::io::FromRawHandle;
use windows_sys::Win32::{
    Foundation::ERROR_FILE_NOT_FOUND,
    System::Console::{ATTACH_PARENT_PROCESS, AttachConsole},
    System::Threading::{CreateMutexW, OpenMutexW},
};

/// 与 Inno Setup 的 AppMutex 对应，所有打开的 Moe 窗口共同保持对象存活。
const APP_MUTEX: &str = r"Local\io.github.lixx0725.moe-app";
/// 与 Inno Setup 的 SetupMutex 对应，安装期间暂停启动新的终端会话。
const SETUP_MUTEX: &str = r"Local\io.github.lixx0725.moe-setup";
/// OpenMutexW 所需的最小访问权，用于检查对象是否存在。
const SYNCHRONIZE: u32 = 0x0010_0000;

/// 带参数时尝试使用父进程的现有控制台，保留启动器显式传入的重定向句柄。
pub(crate) fn attach_parent_console() {
    // SAFETY: 只连接现有控制台，无控制台的父进程会让此调用直接失败。
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

/// 注册当前终端实例。调用方持有句柄到窗口和会话退出。
pub(crate) fn hold_installation_guard() -> io::Result<OwnedHandle> {
    let name = wide_name(APP_MUTEX);
    // SAFETY: 名称以 NUL 结尾，空指针选择默认安全属性，进程只持有句柄而不锁定对象。
    let app = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    if app.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 成功的 CreateMutexW 返回由调用者负责关闭的独立句柄。
    let app = unsafe { OwnedHandle::from_raw_handle(app) };

    let setup_name = wide_name(SETUP_MUTEX);
    // SAFETY: 名称缓冲区在调用期间有效，返回的非空句柄由本进程负责关闭。
    let setup = unsafe { OpenMutexW(SYNCHRONIZE, 0, setup_name.as_ptr()) };
    if !setup.is_null() {
        // SAFETY: 成功的 OpenMutexW 返回了独立句柄。
        drop(unsafe { OwnedHandle::from_raw_handle(setup) });
        return Err(io::Error::other(
            "Moe setup is running. Close it and try again.",
        ));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_FILE_NOT_FOUND as i32) {
        return Err(error);
    }

    Ok(app)
}

/// 将固定的内核对象名编码为 Win32 使用的 NUL 结尾字符串。
fn wide_name(name: &str) -> Vec<u16> {
    OsStr::new(name).encode_wide().chain(Some(0)).collect()
}
