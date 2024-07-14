use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

use game::Game;
use tokio::sync::Mutex;
use warp::{reject::Rejection, Filter};
use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub mod game;
pub mod player;
pub mod stack;

#[tokio::main]
async fn main() {
    let games: Arc<Mutex<HashMap<String, Game>>> = Arc::new(Mutex::new(HashMap::new()));
    let create_game = warp::path("create-game")
        .and(warp::path::end())
        .and(warp::post())
        .and_then(|| async { 
            let game_code = thread_rng().sample_iter(&Alphanumeric).take(5).map(|c| c as char).collect::<String>();
            Ok::<String, Rejection>(game_code)
        });
    warp::serve(create_game)
        .run(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080))
        .await;
}
