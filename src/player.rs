use std::sync::Arc;

use anyhow::Result;
use futures::{stream_select, SinkExt, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
use warp::filters::ws::{self, WebSocket};

use crate::stack::{Card, Stack};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerMessage {
    #[serde(skip)]
    Register(usize),
    NewPlayer {
        name: Arc<str>,
    },
    HostStart,
    Joined {
        players_name: Vec<Arc<str>>,
    },
    GameEnded,
    GameStarted,
    Start {
        point: i32,
    },
    StartFailed,
    RoundStart {
        player_name: Arc<str>,
        stack: Stack,
        point: Option<i32>,
    },
    OtherUseCard {
        card: Card,
    },
    NewRound {
        cards: Vec<Card>,
        stack: Stack,
    },
    Lose,
    GameEnd {
        winner_name: Option<Arc<str>>,
    },
    Win,
    InvalidOperation,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerAction {
    #[serde(skip)]
    Error(anyhow::Error),
    Start,
    UseCard {
        card_index: usize,
    },
    Quit,
}

#[derive(Debug)]
pub struct Player {
    _handle: JoinHandle<()>,
    message_sender: MpscSender<PlayerMessage>,
    pub name: Arc<str>,
}

impl Player {
    pub fn new(
        handle: JoinHandle<()>,
        message_sender: MpscSender<PlayerMessage>,
        name: Arc<str>,
    ) -> Self {
        Self {
            _handle: handle,
            message_sender,
            name,
        }
    }

    pub async fn send(&self, msg: PlayerMessage) -> Result<()> {
        Ok(self.message_sender.send(msg).await?)
    }
}

enum Message {
    Frontend(ws::Message),
    Backend(PlayerMessage),
}

pub async fn handle_one_player(
    ws: WebSocket,
    action_sender: MpscSender<(PlayerAction, usize)>,
    message_receiver: MpscReceiver<PlayerMessage>,
) {
    let mut id = None;
    let message_stream = ReceiverStream::new(message_receiver).map(|msg| Ok(Message::Backend(msg)));
    let (mut ws_sender, ws_recv_stream) = ws.split();
    let ws_recv_stream = ws_recv_stream.map(|msg| msg.map(|msg| Message::Frontend(msg)));
    let mut message_stream = stream_select!(ws_recv_stream, message_stream);
    while let Some(message) = message_stream.next().await {
        if let Err(err) = message {
            action_sender
                .send((PlayerAction::Error(err.into()), id.unwrap()))
                .await
                .expect("Should successfully send");
            break;
        }
        match message.unwrap() {
            Message::Backend(PlayerMessage::Register(reg_id)) => {
                id = Some(reg_id);
            }
            Message::Backend(msg) => {
                ws_sender
                    .send(ws::Message::text(
                        serde_json::to_string(&msg).expect("Should successfully serialize"),
                    ))
                    .await
                    .expect("Should successfully send");
                if msg == PlayerMessage::GameEnded {
                    ws_sender
                        .send(ws::Message::close_with(1000u16, "Game ended"))
                        .await
                        .expect("Should successfully close");
                    ws_sender.close().await.expect("Should successfully close");
                    break;
                }
            }
            Message::Frontend(msg) => {
                if msg.is_close() {
                    action_sender
                        .send((PlayerAction::Quit, id.unwrap()))
                        .await
                        .expect("Should successfully send");
                    break;
                }
                let msg = msg.to_str();
                if let Err(_) = msg {
                    continue;
                }
                let msg = msg.unwrap();

                let player_action = serde_json::from_str(&msg);
                if let Err(_) = player_action {
                    continue;
                }
                let player_action = player_action.unwrap();

                action_sender
                    .send((player_action, id.unwrap()))
                    .await
                    .expect("Should successfully send");
            }
        }
    }
}
