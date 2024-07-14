use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{player::Player, stack::Stack};

#[derive(Debug, Clone, Default)]
pub struct Game(Arc<Mutex<GameInner>>);

#[derive(Debug, Default)]
struct GameInner {
    players: Vec<Player>,
    stack: Stack,
}