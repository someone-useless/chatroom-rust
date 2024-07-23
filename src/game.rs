use std::{collections::BTreeMap, sync::Arc, time::Duration};

use futures::{stream_select, Future, Stream, StreamExt};
use rand::{rngs::StdRng, Rng, SeedableRng};
use tokio::{
    sync::mpsc::Sender as MpscSender,
    task::JoinHandle,
    time::{interval_at, Instant},
};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};

use crate::{
    player::{Player, PlayerAction, PlayerMessage},
    stack::{Card, CardDistribution, Overflow, Stack},
};

use anyhow::{anyhow, Result};

pub enum ExternalMessage {
    NewPlayer(Player),
}

#[derive(Debug)]
pub struct Game {
    _handle: JoinHandle<()>,
    pub action_sender: MpscSender<(PlayerAction, usize)>,
}

enum Message {
    Internal(PlayerAction, usize),
    CheckAlive,
}

impl Game {
    pub fn new<F>(game_code: String, remover: impl FnOnce() -> F + Send + 'static) -> Self
    where
        F: Future<Output = ()> + Send,
    {
        let (action_sender, action_receiver) = tokio::sync::mpsc::channel(3);
        let handle = tokio::spawn(async move {
            let mut data = GameData::default();
            let game_func = || async {
                let mut message_stream = stream_select!(
                    ReceiverStream::new(action_receiver)
                        .map(|(msg, id)| Message::Internal(msg, id)),
                    IntervalStream::new(interval_at(
                        Instant::now() + Duration::from_secs(60),
                        Duration::from_secs(60)
                    ))
                    .map(|_| Message::CheckAlive)
                );
                Self::waiting_for_start(&mut data, &mut message_stream).await?;
                let mut game_data = Self::start(&mut data).await?;
                Self::game_loop(&mut data, &mut game_data, &mut message_stream).await?;
                Self::game_end(&mut game_data, &mut data).await?;
                Ok::<(), anyhow::Error>(())
            };
            if let Err(err) = game_func().await {
                eprintln!("ERROR: {err}");
            }
            Self::clean_up(&mut data).await;
            remover().await;
        });
        Self {
            _handle: handle,
            action_sender,
        }
    }

    async fn waiting_for_start(
        data: &mut GameData,
        message_stream: &mut (impl Stream<Item = Message> + Unpin),
    ) -> Result<()> {
        while let Some(message) = message_stream.next().await {
            match message {
                Message::CheckAlive => {
                    if data.players.is_empty() {
                        return Err(anyhow!("Game Not Alive: {}", data.code()));
                    };
                }
                Message::Internal(PlayerAction::Join { .. }, _ ) => {
                    return Err(anyhow!("Join Action should not be sent"));
                }
                Message::Internal(PlayerAction::JoinWithPlayer { player: new_player, name }, _) => {
                    let id = *data
                        .players
                        .last_key_value()
                        .map(|(id, _)| id)
                        .unwrap_or(&0)
                        + 1;
                    for player in data.all_players() {
                        player
                            .send(PlayerMessage::NewPlayer {
                                name: name.clone(),
                            })
                            .await?;
                    }
                    new_player.send(PlayerMessage::Register(id)).await?;
                    new_player
                        .send(PlayerMessage::Joined {
                            players_name: data
                                .all_players_name()
                                .collect(),
                        })
                        .await?;
                    if data.players.is_empty() {
                        new_player.send(PlayerMessage::HostStart).await?;
                    }
                    data.players.insert(id, (new_player, name));
                }
                Message::Internal(PlayerAction::Start, id) => {
                    if data.players.len() > 1 {
                        break;
                    } else {
                        data.send_player(&id, PlayerMessage::StartFailed).await?;
                    }
                }
                Message::Internal(PlayerAction::Quit, id) => {
                    data.players.remove(&id);
                    if data.players.is_empty() {
                       return Err(anyhow!("Player all quit: {}", data.code()));
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    async fn start(data: &mut GameData) -> Result<InGameData> {
        let mut game_data = InGameData::new();

        for (id, player) in data.all_players_and_ids() {
            game_data
                .player_state
                .insert(id, PlayerState { point: 10 });

            player.send(PlayerMessage::Start { point: 10 }).await?;
        }
        Ok(game_data)
    }

    async fn game_loop(
        player_data: &mut GameData,
        game_data: &mut InGameData,
        message_stream: &mut (impl Stream<Item = Message> + Unpin),
    ) -> Result<()> {
        let mut playing_id = *game_data
            .player_state
            .keys()
            .nth(game_data.rng.gen_range(0..game_data.player_state.len()))
            .unwrap();
        while !game_data.game_ended() {
            let cards = game_data.gen_cards();
            let playing_player_name = player_data
                .get_player_name(&playing_id)
                .ok_or_else(|| anyhow!("Should be in player_data"))?;
            for (id, player) in player_data.all_players_and_ids() {
                player
                    .send(PlayerMessage::RoundStart {
                        player_name: playing_player_name.clone(),
                        stack: game_data.stack.clone(),
                        point: game_data.get_state(&id).map(|data| data.point),
                    })
                    .await?;
                if playing_id == id {
                    player
                        .send(PlayerMessage::NewRound {
                            cards: cards.clone(),
                            stack: game_data.stack.clone(),
                        })
                        .await?;
                }
            }
            while let Some(message) = message_stream.next().await {
                match message {
                    Message::CheckAlive => (),
                    Message::Internal(PlayerAction::Join { .. }, _) => {
                        return Err(anyhow!("Join Action should not be sent"));
                    }
                    Message::Internal(PlayerAction::JoinWithPlayer { player, .. }, _) => {
                        player.send(PlayerMessage::GameStarted).await?;
                    }
                    Message::Internal(PlayerAction::Start, id) => {
                        player_data
                            .send_player(&id, PlayerMessage::GameStarted)
                            .await?;
                    }
                    Message::Internal(PlayerAction::UseCard { card_index }, id)
                        if id == playing_id =>
                    {
                        let card = &cards[card_index];
                        let overflows = game_data.stack.use_card(card);
                        if !overflows.is_empty() {
                            Self::handle_overflow(overflows, playing_id, player_data, game_data)
                                .await?;
                        }
                        for (id, player) in player_data.all_players_and_ids() {
                            if playing_id != id {
                                player
                                    .send(PlayerMessage::OtherUseCard { card: card.clone() })
                                    .await?;
                            }
                        }
                        playing_id = game_data.next_id(playing_id);
                        break;
                    }
                    Message::Internal(PlayerAction::UseCard { .. }, id) => {
                        player_data
                            .send_player(&id, PlayerMessage::InvalidOperation)
                            .await?;
                    }
                    Message::Internal(PlayerAction::Quit, id) => {
                        player_data.players.remove(&id);
                        game_data.player_state.remove(&id);
                        if !player_data.players.is_empty() && game_data.player_state.is_empty() {
                            break;
                        } else if player_data.players.is_empty() {
                            return Err(anyhow!("All player quit"));
                        }
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }

    async fn handle_overflow(
        overflows: Vec<Overflow>,
        playing_id: usize,
        player_data: &mut GameData,
        game_data: &mut InGameData,
    ) -> Result<()> {
        let (gain, lose) = overflows
            .into_iter()
            .fold((0, 0), |(gain, lose), overflow| {
                (gain + overflow.self_gain, lose + overflow.other_lost)
            });
        let mut lost_players = Vec::new();
        game_data.player_state.retain(|id, state| {
            if playing_id == *id {
                state.point += gain;
            } else {
                state.point -= lose;
            }
            if state.point <= 0 {
                lost_players.push(
                    player_data
                        .get_player(id)
                        .ok_or(anyhow!("Should be in player")),
                );
            }
            state.point > 0
        });
        for lose in lost_players {
            lose?.send(PlayerMessage::Lose).await?;
        }
        Ok(())
    }

    async fn game_end(game_data: &mut InGameData, player_data: &mut GameData) -> Result<()> {
        if !game_data.game_ended() {
            return Err(anyhow!("Game should end"));
        }
        let winner_id = game_data.player_state.first_key_value();
        let winner_name = winner_id.and_then(|(id, _)| player_data.get_player_name(id));
        if let Some((id, _)) = winner_id {
            player_data
                .send_player(id, PlayerMessage::Win)
                .await?;
        }
        for player in player_data.all_players() {
            player
                .send(PlayerMessage::GameEnd {
                    winner_name: winner_name.clone(),
                })
                .await?;
        }
        Ok(())
    }

    async fn clean_up(data: &mut GameData) {
        for player in data.all_players() {
            player
                .send(PlayerMessage::GameEnded)
                .await
                .expect("Should successfully send");
        }
    }
}

#[derive(Debug, Default)]
struct GameData {
    players: BTreeMap<usize, (Player, Arc<str>)>,
    code: String,
}

impl GameData {
    fn code(&self) -> &str {
        &self.code
    }

    fn all_players(&self) -> impl Iterator<Item = &Player> {
        self.players.values().map(|(player, _)| player)
    }

    fn all_players_name(&self) -> impl Iterator<Item = Arc<str>> + '_ {
        self.players.values().map(|(_, name)| name.clone())
    }

    fn all_players_and_ids(&self) -> impl Iterator<Item = (usize, &Player)> {
        self.players.iter().map(|(id, (player, _))| (*id, player))
    }

    #[inline]
    fn get_player(&self, id: &usize) -> Option<&Player> {
        self.players.get(id).map(|(player, _)| player)
    }

    #[inline]
    async fn send_player(&self, id: &usize, msg: PlayerMessage) -> Result<()> {
        let player = self
            .get_player(id)
            .ok_or_else(|| anyhow!("Player not found"))?;
        player.send(msg).await
    }

    fn get_player_name(&self, id: &usize) -> Option<Arc<str>> {
        self.players.get(id).map(|(_, name)| name).cloned()
    }
}

struct InGameData {
    player_state: BTreeMap<usize, PlayerState>,
    rng: StdRng,
    card_distribution: CardDistribution,
    stack: Stack,
}

impl InGameData {
    #[inline]
    fn new() -> Self {
        Self {
            player_state: BTreeMap::new(),
            rng: StdRng::from_entropy(),
            card_distribution: CardDistribution::default(),
            stack: Stack::new(10),
        }
    }

    #[inline]
    fn game_ended(&self) -> bool {
        self.player_state.len() > 1
    }

    #[inline]
    fn get_state(&self, id: &usize) -> Option<&PlayerState> {
        self.player_state.get(id)
    }

    #[inline]
    fn next_id(&self, id: usize) -> usize {
        *self
            .player_state
            .range(id + 1..)
            .next()
            .or_else(|| self.player_state.first_key_value())
            .unwrap()
            .0
    }

    #[inline]
    fn gen_cards(&mut self) -> Vec<Card> {
        (0..3)
            .map(|_| self.rng.sample(&self.card_distribution))
            .collect()
    }
}

#[derive(Default, PartialEq, Eq, Debug)]
struct PlayerState {
    point: i32,
}
