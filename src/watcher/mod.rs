mod poll;
mod utils;

pub use self::poll::*;

use std::hash::RandomState;

thread_local! {
    pub static MAIN_HASHER: RandomState = RandomState::default();
}
