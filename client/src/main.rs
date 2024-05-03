mod game_event_stream;
use api;

use api::{Card, Event, GameInfo, Suit};
use iced::executor;
use iced::{Alignment, Application, Command, Element, Settings, Theme};
use iced::widget::{Button, column, Column, row, Row, Text, image::{Image, Handle}};
use std::env;

static API_URL: &str = "http://127.0.0.1:3030";

pub fn main() -> iced::Result {
    let mut args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprint!("{:?}", args);
        panic!("Run command: client [player_id]")
    }
    let player_id = args.pop().unwrap();

    App::run(Settings::with_flags(player_id))
}

#[derive(Clone, Debug, Default)]
enum State {
    #[default]
    Main,
    Waiting,
    Ongoing,
    Finished
}

#[derive(Clone, Debug, Default)]
struct Game {
    id: String,
    winner: String,
    turn: u8,
    round: u8,
    trump: Card,
    players: Vec<String>,
    played: Vec<Card>,
    score: Vec<u8>,
    cards: Vec<Card>
}

#[derive(Default)]
struct App {
    player_id: String,
    state: State,
    games: Vec<GameInfo>,
    game: Game
}

#[derive(Debug, Clone)]
enum Message {
    None,
    Navigate(State),
    RefreshGameList,
    GameList(Result<Vec<GameInfo>, Error>),
    CreateGame(u8),
    JoinGame(String),
    GameEvent(Event),
    PlayCard(usize)
}

#[derive(Debug, Clone)]
enum Error {
    APIError
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Error {
        dbg!(error);

        Error::APIError
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = String;

    fn new(flags: Self::Flags) -> (App, Command<Self::Message>) {
        let player_id = flags;
        (App {
            player_id: player_id.clone(),
            ..Default::default()
        }, Command::perform(get_games(), Message::GameList))
    }

    fn title(&self) -> String {
        format!("Brisca - {}", self.player_id)
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {

        match message {
            // Dummy message to do nothing.
            // Used mainly for Successful API response that require no state change
            Message::None => Command::none(),
            Message::Navigate(state) => {
                self.state = state.clone();

                match state {
                    State::Main => Command::perform(get_games(), Message::GameList),
                    _ => Command::none()
                }
            },
            Message::RefreshGameList => {
                Command::perform(get_games(), Message::GameList)
            },
            Message::GameList(result) => {
                match result {
                    Ok(games) => {
                        self.games = games;
                    }
                    Err(_) => {
                        println!("Error getting list of games.")
                    }
                }

                Command::none()
            },
            Message::CreateGame(num_players) => {
                Command::perform(create_game(num_players), |res| match res {
                    Ok(game_id) => Message::JoinGame(game_id),
                    Err(_) => Message::None // TODO: Handle this
                })
            },
            Message::JoinGame(game_id) => {
                self.state = State::Waiting;
                self.game = Game { id: game_id.clone(), ..Default::default() };

                game_event_stream::connect(format!("{API_URL}/game/{game_id}"),
                    self.player_id.clone())
                    .map(Message::GameEvent)
            },
            Message::PlayCard(card_idx) => {
                // Remove local copy
                let card = self.game.cards.remove(card_idx);

                // Set as played card
                self.game.played.push(card.clone());

                // Update turn counter if this round has not finished yet
                if self.game.played.len() < self.game.players.len() {
                    self.game.turn = (self.game.turn + 1) % self.game.players.len() as u8;
                }

                Command::perform(
                    play_card(
                        card.clone(),
                        self.player_id.clone(),
                        self.game.id.clone()),
                        |res| match res {
                            Ok(_) => Message::None,
                            Err(_) => Message::None // TODO: Handle this
                        })
            }
            Message::GameEvent(game_event) => {
                println!("Received GameEvent: {:?}", game_event);
                match game_event {
                    Event::Connected(players) => {
                        self.game.players = players;

                        Command::none()
                    },
                    Event::NewCard(card) => {
                        self.game.cards.push(card);

                        Command::none()
                    },
                    Event::GameStart(card) => {
                        self.state = State::Ongoing;
                        // Save trump card of this game
                        self.game.trump = card;
                        // Set initial round
                        self.game.round = 1;

                        // Initialize players score
                        for _ in 0..self.game.players.len() {
                            self.game.score.push(0);
                        }

                        Command::none()
                    },
                    Event::PlayedCard(card) => {
                        self.game.played.push(card);

                        // Update turn counter if this round has not finished yet
                        if self.game.played.len() < self.game.players.len() {
                            self.game.turn= (self.game.turn + 1) % self.game.players.len() as u8;
                        }

                        Command::none()
                    },
                    Event::RoundEnd(winner, round_score) => {
                        // Set next turn based on winner
                        self.game.turn = winner;
                        // Update score
                        self.game.score[winner as usize] += round_score;
                        // Update round counter
                        self.game.round += 1;
                        // Clear played cards
                        self.game.played = Vec::with_capacity(self.game.players.len());

                        Command::none()
                    },
                    Event::GameEnd(winner) => {
                        self.state = State::Finished;
                        self.game.winner = winner;

                        Command::none()
                    }
                }
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        match self.state {
            State::Main => {
                let games = Column::with_children(self.games
                    .iter()
                    .map(| GameInfo {id, num_players} | {
                        row![
                            Text::new(format!("game_id: {} - num_players: {}", id, num_players)),
                            Button::new("Join").on_press(Message::JoinGame(id.clone()))
                        ]
                        .spacing(20)
                        .align_items(Alignment::Center)
                    })
                    .map(Element::from)
                );
                column![
                    Text::new("Create game"),
                    Button::new("2 player").on_press(Message::CreateGame(2)),
                    Button::new("4 player").on_press(Message::CreateGame(4)),
                    Text::new("Available games"),
                    games,
                    Button::new("Refresh").on_press(Message::RefreshGameList)
                ]
                .spacing(10)
                .into()
            },
            State::Ongoing => {
                let round = Element::from(Text::new(format!("Round: {}", self.game.round)));
                let scores = Column::with_children(self.game.players
                    .iter()
                    .zip(self.game.score.iter())
                    .map(|(player, score)| Text::new(format!("{}: {}", player, score)))
                    .map(Element::from)
                );

                let hand;
                if self.game.players[self.game.turn as usize] == self.player_id {
                    // Let player have the option to play a card only when it's their turn
                    hand = Row::with_children(self.game.cards
                        .iter()
                        .enumerate()
                        .map(|(i, c)| column![
                            Image::<Handle>::new(get_image_path(c)),
                            Button::new("Play").on_press(Message::PlayCard(i))
                            ])
                        .map(Element::from)
                    );
                } else {
                    hand = Row::with_children(self.game.cards
                        .iter()
                        .map(|c| Image::<Handle>::new(get_image_path(c)))
                        .map(Element::from)
                    );
                }
                let trump = Element::from(Image::<Handle>::new(get_image_path(&self.game.trump)));

                // Show played cards
                let played = Row::with_children(self.game.played
                    .iter()
                    .map(|c| Image::<Handle>::new(get_image_path(c)))
                    .map(Element::from));

                Column::new()
                    .push(round)
                    .push(scores)
                    .push(trump)
                    .push(hand)
                    .push(played)
                    .into()
            }
            State::Finished => {
                let scores = Column::with_children(self.game.players
                    .iter()
                    .zip(self.game.score.iter())
                    .map(|(player, score)| Text::new(format!("{}: {}", player, score)))
                    .map(Element::from)
                );

                Column::new()
                    .push(scores)
                    .push(Element::from(Text::new(format!("Winner: {}", self.game.winner))))
                    .push(Button::new("Return to Main").on_press(Message::Navigate(State::Main)))
                    .into()
            }
            State::Waiting => {
                Text::new("Waiting for players to connect ...").into()
            }
        }
    }
}

fn get_image_path(Card {number, suit}: &Card) -> String {
    format!("{}/images/{number}{}.jpg",
        env!("CARGO_MANIFEST_DIR"),
        match suit {
            Suit::Coin => "o",
            Suit::Cup => "c",
            Suit::Baton => "b",
            Suit::Sword => "e"
        })
}

// API requests

async fn play_card(card: Card, player_id: String, game_id: String) -> Result<(), Error> {
    let url = format!("{API_URL}/game/{game_id}");
    reqwest::Client::new()
        .put(url)
        .header("authorization", player_id) // TODO: Implement proper auth
        .json(&card)
        .send()
        .await?;
    Ok(())
}

async fn get_games() -> Result<Vec<GameInfo>, Error> {
    let url = format!("{API_URL}/game");
    let games = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .json()
        .await?;
    Ok(games)
}

async fn create_game(num_players: u8) -> Result<String, Error> {
    let url = format!("{API_URL}/game/{num_players}");
    let game_id = reqwest::Client::new()
        .post(url)
        .send()
        .await?
        .text()
        .await?;
    Ok(game_id)
}