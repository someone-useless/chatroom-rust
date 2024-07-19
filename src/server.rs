use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use futures::SinkExt;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use tokio::sync::Mutex;
use warp::filters::ws::{self, WebSocket};

use crate::{game::Game, player::Player};

#[derive(Debug, Clone, Default)]
pub struct Server {
    games: Arc<Mutex<HashMap<String, Game>>>,
}

impl Server {
    pub async fn new_game(&self) -> String {
        let mut game_code = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(3)
            .map(|c| c as char)
            .collect::<String>();
        let mut games = self.games.lock().await;
        while games.contains_key(&game_code) {
            game_code = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(4)
                .map(|c| c as char)
                .collect::<String>();
        }
        let server = self.clone();
        let game_code_removing = game_code.clone();
        let remover = move || {
            let server = server;
            server.destroy_game(game_code_removing)
        };
        games.insert(game_code.clone(), Game::new(remover));
        game_code
    }

    pub async fn destroy_game(self, game_code: String) {
        let mut games = self.games.lock().await;
        games.remove(&game_code);
    }

    pub async fn add_player_to_game(
        &self,
        mut player_socket: WebSocket,
        player_name: impl Into<Arc<str>>,
        game_code: &str,
    ) -> Result<()> {
        let mut games = self.games.lock().await;
        let game = if let Some(game) = games.get_mut(game_code) {
            game
        } else {
            player_socket
                .send(ws::Message::close_with(1000u16, "Game Not Found"))
                .await
                .expect("Should successfully send");
            player_socket
                .close()
                .await
                .expect("Should successfully close");
            return Err(anyhow!("Game Not Found"));
        };
        let (message_sender, message_recviver) = tokio::sync::mpsc::channel(3);
        let handle = tokio::spawn(crate::player::handle_one_player(
            player_socket,
            game.action_sender.clone(),
            message_recviver,
        ));
        let new_player = Player::new(handle, message_sender, player_name.into());
        game.message_sender
            .send(crate::game::ExternalMessage::NewPlayer(new_player))
            .await?;
        Ok(())
    }
}
