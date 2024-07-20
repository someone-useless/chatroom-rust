use std::{
    convert::Infallible,
    net::{Ipv4Addr, SocketAddrV4},
};

use serde_json::json;
use server::Server;
use warp::{filters::ws::Ws, Filter};

pub mod game;
pub mod player;
pub mod server;
pub mod stack;

#[tokio::main]
async fn main() {
    let server = Server::default();
    let cors = warp::cors::cors()
        .allow_any_origin()
        .build();
    let create_game = warp::path("create-game")
        .and(warp::path::end())
        .and(warp::post())
        .and_then({
            let server = server.clone();
            move || {
                let server = server.clone();
                async move {
                    let game_code = server.new_game().await;
                    Ok::<_, Infallible>(json!({ "game_code": game_code }).to_string())
                }
            }
        })
        .with(&cors);

    let game_exist = warp::path("game-exist")
        .and(warp::path::param())
        .and(warp::path::end())
        .and(warp::get())
        .and_then({
            let server = server.clone();
            move |game_code: String| {
                let server = server.clone();
                async move {
                    let is_game_exist = server.is_game_exist(&game_code).await;
                    Ok::<_, Infallible>(json!({ "game_exist": is_game_exist }).to_string())
                }
            }
        })
        .with(&cors);

    let join_game = warp::path("game")
        .and(warp::path::param())
        .and(warp::header("Player-Name"))
        .and(warp::ws())
        .map({
            let server = server.clone();
            move |game_code: String, player_name: String, ws: Ws| {
                let server = server.clone();
                ws.on_upgrade(|socket| async move {
                    let _ = server
                        .add_player_to_game(socket, player_name, &game_code)
                        .await;
                })
            }
        })
        .with(&cors);

    warp::serve(create_game.or(game_exist).or(join_game))
        .run(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080))
        .await;
}
