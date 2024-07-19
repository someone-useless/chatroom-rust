use std::{
    convert::Infallible,
    net::{Ipv4Addr, SocketAddrV4},
};

use server::Server;
use warp::{filters::ws::Ws, Filter};

pub mod game;
pub mod player;
pub mod server;
pub mod stack;

#[tokio::main]
async fn main() {
    let server = Server::default();
    let create_game = warp::path("create-game")
        .and(warp::path::end())
        .and(warp::post())
        .and_then({
            let server = server.clone();
            move || {
                let server = server.clone();
                async move {
                    let game_code = server.new_game().await;
                    Ok::<String, Infallible>(game_code)
                }
            }
        });

    let join_game = warp::path("game")
        .and(warp::path::param())
        .and(warp::header("Player-Name"))
        .and(warp::ws())
        .map({
            let server = server.clone();
            move |game_code: String, player_name: String, ws: Ws| {
                let server = server.clone();
                ws.on_upgrade(|socket| async move {
                    let _ = server.add_player_to_game(socket, player_name, &game_code).await;
                })
            }
        });

    warp::serve(create_game.or(join_game))
        .run(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080))
        .await;
}
