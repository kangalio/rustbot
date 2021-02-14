#![allow(clippy::new_without_default)]

mod api;
pub use api::*;
mod commands;
pub use commands::*;
mod command_history;
pub use command_history::*;
mod events;
pub use events::*;

use serenity::{model::prelude::*, prelude::*};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

struct BotUserIdKey;
impl TypeMapKey for BotUserIdKey {
    type Value = UserId;
}
