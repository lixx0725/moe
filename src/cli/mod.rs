//! 解析进程参数，并在需要时启动图形终端。

use clap::{Parser, error::ErrorKind};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

/// 一次命令行启动的参数。
#[derive(Parser)]
#[command(version, about = "Open a native terminal")]
struct Args {
    /// Shell 可以是 PATH 中的程序名，也可以是可执行文件路径。
    #[arg(long, value_name = "PROGRAM", help = "Shell executable name or path")]
    shell: Option<PathBuf>,
    /// 双横线后的每一项都按原有边界传给 Shell。
    #[arg(
        last = true,
        value_name = "SHELL_ARG",
        help = "Arguments passed to the shell"
    )]
    shell_args: Vec<OsString>,
}

/// 解析程序名和参数，帮助、版本及参数错误均在创建会话前处理。
pub(crate) fn run(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let args = match Args::try_parse_from(args) {
        Ok(args) => args,
        Err(error) => {
            let code = match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
                _ => ExitCode::from(2),
            };
            return if error.print().is_ok() {
                code
            } else {
                ExitCode::FAILURE
            };
        }
    };
    match crate::app::run(args.shell, args.shell_args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("moe: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arguments_keep_spaces_unicode_empty_values_and_option_prefixes() {
        let args = Args::try_parse_from([
            "moe",
            "--shell",
            "目录/测试 Shell",
            "--",
            "-NoLogo",
            "甲 乙",
            "",
            "--help",
        ])
        .expect("解析 Shell 及其参数");
        assert_eq!(args.shell, Some(PathBuf::from("目录/测试 Shell")));
        assert_eq!(args.shell_args, ["-NoLogo", "甲 乙", "", "--help"]);
    }

    #[test]
    fn shell_arguments_can_be_given_to_the_default_shell() {
        let args = Args::try_parse_from(["moe", "--", "-i"]).expect("为默认 Shell 解析参数");
        assert_eq!(args.shell, None);
        assert_eq!(args.shell_args, ["-i"]);
    }
}
