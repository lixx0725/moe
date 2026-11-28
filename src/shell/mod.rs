//! 解析本次启动描述，并将程序、参数和目录传递给 PTY。

mod program;

use alacritty_terminal::tty::{Options, Shell};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// 解析或启动 Shell 时可以报告给用户的错误。
#[derive(Debug, Error)]
pub(crate) enum LaunchError {
    /// 当前平台找不到可用的默认 Shell。
    #[error("cannot select a default shell: {0}")]
    Default(#[source] io::Error),
    /// 用户显式选择的程序无法解析。
    #[error("cannot use shell {}: {source}", .program.display())]
    Selection {
        /// 用户给出的程序名或路径。
        program: PathBuf,
        /// 文件检查或路径解析失败的原因。
        #[source]
        source: io::Error,
    },
    /// PTY 后端的 Shell 路径参数要求 UTF-8。
    #[error("shell path is not valid UTF-8: {}", .0.display())]
    ProgramEncoding(PathBuf),
    /// PTY 后端的某个 Shell 参数无法无损转换为 UTF-8。
    #[error("argument {position} for shell {} is not valid UTF-8", .program.display())]
    ArgumentEncoding {
        /// 即将启动的 Shell 路径。
        program: PathBuf,
        /// 从 1 开始的 Shell 参数位置。
        position: usize,
    },
    /// Shell 已选定，但 PTY 或子进程启动失败。
    #[error("could not start shell {}: {source}", .program.display())]
    Start {
        /// 已解析的 Shell 路径。
        program: PathBuf,
        /// PTY 或操作系统返回的错误。
        #[source]
        source: io::Error,
    },
}

/// 一次未持久化的 Shell 启动，供 CLI 和后续窗口入口共用。
#[derive(Debug)]
pub(crate) struct Launch {
    /// 已解析的可执行文件路径。
    program: PathBuf,
    /// 分开的进程参数，进入 PTY 之前保持原始字符表示。
    args: Vec<OsString>,
    /// 传给子进程的起始目录，缺省时由后端继承。
    working_directory: Option<PathBuf>,
}

impl Launch {
    /// 解析默认或显式程序。默认参数只随平台默认选择启用。
    pub(crate) fn resolve(
        requested: Option<PathBuf>,
        mut args: Vec<OsString>,
        working_directory: Option<PathBuf>,
    ) -> Result<Self, LaunchError> {
        let program =
            match requested {
                Some(requested) => program::resolve(&requested, working_directory.as_deref())
                    .map_err(|source| LaunchError::Selection {
                        program: requested,
                        source,
                    })?,
                None => {
                    let (program, defaults) =
                        program::default_shell().map_err(LaunchError::Default)?;
                    if !defaults.is_empty() {
                        args.splice(..0, defaults.into_iter().map(OsString::from));
                    }
                    program
                }
            };
        Ok(Self {
            program,
            args,
            working_directory,
        })
    }

    /// 返回已解析的程序路径，启动失败时可报告给用户。
    pub(crate) fn program(&self) -> &Path {
        &self.program
    }

    /// 转换为 Alacritty PTY 的启动配置，在边界校验后端字符串表示。
    pub(crate) fn options(&self) -> Result<Options, LaunchError> {
        let program = self
            .program
            .to_str()
            .ok_or_else(|| LaunchError::ProgramEncoding(self.program.clone()))?;
        // Windows PTY 后端直接拼接程序名。把它引用为一个参数，保留路径中的空格。
        #[cfg(windows)]
        let program = format!("\"{program}\"");
        #[cfg(not(windows))]
        let program = program.to_owned();
        let args = self
            .args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                arg.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| LaunchError::ArgumentEncoding {
                        program: self.program.clone(),
                        position: index + 1,
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Options {
            shell: Some(Shell::new(program, args)),
            working_directory: self.working_directory.clone(),
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
}
