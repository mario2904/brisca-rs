use serde::{Deserialize, Serialize};


#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum Suit {
    #[default]
    Coin,
    Cup,
    Baton,
    Sword
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Card {
    pub number: u8,
    pub suit: Suit
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Event {
    Connected(Vec<String>),
    NewCard(Card),
    GameStart(Card),
    PlayedCard(Card),
    RoundEnd(u8, u8),
    GameEnd(String)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameInfo {
    pub id: String,
    pub num_players: u8
}