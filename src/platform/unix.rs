//! Unix 用户账户查询。

use std::path::PathBuf;

/// 查询当前账户登记的登录 Shell 路径。
pub(crate) fn account_shell() -> Option<PathBuf> {
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
