use std::sync::Arc;

use warp::filters::ws::WebSocket as Socket;

use crate::game::Game;

#[derive(Debug)]
pub struct Player {
    ws: Socket,
    game: Arc<Game>,
}