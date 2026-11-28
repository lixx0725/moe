//! 根据焦点和终端状态路由原生输入。

mod context;
mod keyboard;

pub(crate) use context::InteractionContext;
pub(crate) use keyboard::{KeyboardInput, route_key};
