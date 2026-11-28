//! 维护终端网格、协议输入和调色板。

pub(crate) mod input;
pub(crate) mod palette;

use alacritty_terminal::{
    event::{EventListener, WindowSize},
    sync::FairMutex,
    term::{Config, Term, test::TermSize},
};
use std::sync::Arc;

/// 供 PTY 写入和界面读取的共享屏幕。
pub(crate) struct Screen<L> {
    /// Alacritty 网格、光标、滚动位置和模式。
    inner: Arc<FairMutex<Term<L>>>,
}
impl<L: EventListener> Screen<L> {
    /// 创建默认配置的屏幕。
    pub(crate) fn new(size: WindowSize, events: L) -> Self {
        let grid = TermSize::new(usize::from(size.num_cols), usize::from(size.num_lines));
        Self {
            inner: Arc::new(FairMutex::new(Term::new(Config::default(), &grid, events))),
        }
    }

    /// 将同一个网格交给 PTY 循环。
    pub(crate) fn share(&self) -> Arc<FairMutex<Term<L>>> {
        self.inner.clone()
    }

    /// 从网格读取数据，闭包返回后释放锁。
    pub(crate) fn read<R>(&self, read: impl FnOnce(&Term<L>) -> R) -> R {
        read(&self.inner.lock())
    }

    /// 保持尺寸入队和网格更新的顺序。
    pub(crate) fn resize(&self, size: WindowSize, send: impl FnOnce() -> bool) -> bool {
        let mut grid = self.inner.lock();
        if !send() {
            return false;
        }
        grid.resize(TermSize::new(
            usize::from(size.num_cols),
            usize::from(size.num_lines),
        ));
        true
    }
}
