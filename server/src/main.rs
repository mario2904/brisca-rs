use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{sse, Filter};
use rand::seq::SliceRandom;
use rand::thread_rng;
use api::{self, Card, GameInfo, Suit};


/// Our global unique game id counter.
static NEXT_GAME_ID: AtomicUsize = AtomicUsize::new(1);
// Deck of cards
static CARDS: [Card; 40] = [
    Card {number: 1, suit: Suit::Coin},
    Card {number: 2, suit: Suit::Coin},
    Card {number: 3, suit: Suit::Coin},
    Card {number: 4, suit: Suit::Coin},
    Card {number: 5, suit: Suit::Coin},
    Card {number: 6, suit: Suit::Coin},
    Card {number: 7, suit: Suit::Coin},
    Card {number: 10, suit: Suit::Coin},
    Card {number: 11, suit: Suit::Coin},
    Card {number: 12, suit: Suit::Coin},
    Card {number: 1, suit: Suit::Cup},
    Card {number: 2, suit: Suit::Cup},
    Card {number: 3, suit: Suit::Cup},
    Card {number: 4, suit: Suit::Cup},
    Card {number: 5, suit: Suit::Cup},
    Card {number: 6, suit: Suit::Cup},
    Card {number: 7, suit: Suit::Cup},
    Card {number: 10, suit: Suit::Cup},
    Card {number: 11, suit: Suit::Cup},
    Card {number: 12, suit: Suit::Cup},
    Card {number: 1, suit: Suit::Baton},
    Card {number: 2, suit: Suit::Baton},
    Card {number: 3, suit: Suit::Baton},
    Card {number: 4, suit: Suit::Baton},
    Card {number: 5, suit: Suit::Baton},
    Card {number: 6, suit: Suit::Baton},
    Card {number: 7, suit: Suit::Baton},
    Card {number: 10, suit: Suit::Baton},
    Card {number: 11, suit: Suit::Baton},
    Card {number: 12, suit: Suit::Baton},
    Card {number: 1, suit: Suit::Sword},
    Card {number: 2, suit: Suit::Sword},
    Card {number: 3, suit: Suit::Sword},
    Card {number: 4, suit: Suit::Sword},
    Card {number: 5, suit: Suit::Sword},
    Card {number: 6, suit: Suit::Sword},
    Card {number: 7, suit: Suit::Sword},
    Card {number: 10, suit: Suit::Sword},
    Card {number: 11, suit: Suit::Sword},
    Card {number: 12, suit: Suit::Sword},
];

#[derive(Clone, Debug, Default)]
struct GameState {
    deck: Vec<Card>,
    played: Vec<Card>,
    turn: u8,
    round: u8,
    trump: Card
}

#[derive(Debug, Default, Clone)]
struct Game {
    num_players: u8,
    players: Vec<Player>,
    state: GameState
}

#[derive(Clone, Debug)]
struct Player {
    id: String,
    cards: Vec<Card>,
    score: u8,
    sender: UnboundedSender<api::Event>
}

#[tokio::main]
async fn main() {

    // Registry of all games
    let games: Arc<Mutex<HashMap<usize, Game>>> = Arc::new(Mutex::new(HashMap::new()));
    // Turn our "state" into a new Filter...
    let games = warp::any().map(move || games.clone());


    // POST /game/:num_players -> create a game and return game_id
    let create = warp::path("game")
        .and(warp::post())
        .and(warp::path::param::<u8>())
        .and(games.clone())
        .map(|num_players: u8, games: Arc<Mutex<HashMap<usize, Game>>>| {
            // Generate new game_id
            let game_id = NEXT_GAME_ID.fetch_add(1, Ordering::Relaxed);
            // Create new game and add to registry
            games.lock().unwrap().insert(game_id, Game { num_players, ..Default::default() });
            println!("Game {}: Created with num_players {}", game_id, num_players);
            // Return game_id to user
            game_id.to_string()
        });

    // GET /game/:game_id -> join game and get event stream
    let join = warp::path("game")
        .and(warp::get())
        .and(warp::path::param::<usize>())
        .and(warp::header::<String>("authorization")) // TODO: Implement proper auth
        .and(games.clone())
        .map(|game_id, player_id, games: Arc<Mutex<HashMap<usize, Game>>>| {
            // Get game
            // TODO: Sanity check that game exists
            let mut games = games.lock().unwrap();
            let game = games.get_mut(&game_id).unwrap();

            if game.num_players == game.players.len() as u8 {
                // TODO: Sanity check that the game is not full / has started already
                eprintln!("Attempting to join an ongoing game.")
            }
            println!("Game {}: {} joined the game", game_id, player_id);

            // Create player channel game event stream
            // Use an unbounded channel to handle buffering and flushing of messages
            // to the event source...
            let (tx, rx) = unbounded_channel();
            let rx = UnboundedReceiverStream::new(rx);

            // Create Player
            let player = Player {
                id: player_id,
                cards: Vec::new(),
                score: 0,
                sender: tx.clone()
            };

            // Add player to game registry
            game.players.push(player);

            // Get list of players
            let players: Vec<String> = game.players.iter().map(|p| p.id.clone()).collect();
            // Send to all players the updated list of players
            for player in &game.players {
                player.sender.send(api::Event::Connected(players.clone())).unwrap();
            }

            // If all needed players have joined, start the game.
            if game.num_players == game.players.len() as u8 {
                println!("Game {}: All {} players have joined. Start Game", game_id, game.num_players);
                // Break out new deck of cards and shuffle them
                let mut rng = thread_rng();
                game.state.deck = CARDS.to_vec();
                game.state.deck.shuffle(&mut rng);
                // Get the trump card from the top of the deck
                // The trump card should stay in the deck as the last card to be dealt
                game.state.trump  = game.state.deck[0].clone();

                for player in &mut game.players {
                    // Deal 3 cards to each player
                    for _ in 0..3 {
                        let card = game.state.deck.pop().unwrap();
                        player.cards.push(card.clone());
                        player.sender.send(api::Event::NewCard(card)).unwrap();
                    }
                    player.sender.send(api::Event::GameStart(game.state.trump.clone())).unwrap();
                }
            }

            // Convert messages into Server-Sent Events and return resulting stream.
            let stream = create_sse_stream(rx);

            // Return game stream
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    // PUT /game/:game_id -> play card
    let play = warp::path("game")
        .and(warp::put())
        .and(warp::path::param::<usize>())
        .and(warp::header::<String>("authorization")) // TODO: Implement proper auth
        .and(warp::body::json())
        .and(games.clone())
        .map(|game_id, player_id: String, card: Card, games: Arc<Mutex<HashMap<usize, Game>>>| {
            println!("Game {}: {} plays {:?}", game_id, player_id, card);

            // Get game
            // TODO: Sanity check that game exists
            let mut games = games.lock().unwrap();
            let game = games.get_mut(&game_id).unwrap();

            let player_idx = game.players.iter().position(|p| *p.id == player_id).unwrap();
            if player_idx as u8 != game.state.turn {
                // TODO: Sanity check that it is currently that players turn
            }

            // Remove card from player's hand
            game.players[player_idx].cards.retain(|c| *c != card);
            // TODO: Sanity check that the card exists in his hand

            // Update other player with card played
            for i in 0..game.players.len() {
                if i != player_idx {
                    game.players[i].sender.send(api::Event::PlayedCard(card.clone())).unwrap();
                }
            }

            // Save card played
            game.state.played.push(card);

            // Check if all players have played
            if game.state.played.len() == game.players.len() {
                // End of the round. Compute result.

                let mut winner_idx: u8 = 0;
                let mut score = 0;
                // First card played dictates the round's winning suit, unless another player plays card with the trump suit
                let mut best_card = &game.state.played[0];
                // Look-up table of points values awarded for each card number value
                // Map to the following numbers [x, 1,  2,  3, 4, 5, 6, 7, x, x, 10, 11, 12]
                let points_per_card = [0, 11, 0, 10, 0, 0, 0, 0, 0, 0,  2,  3,  4];
                // Look-up table of heirarchical ordering 0..10 based on points and number
                let order = [0, 9, 0, 8, 1, 2, 3, 4, 0, 0, 5, 6, 7];

                score += points_per_card[best_card.number as usize];

                for i in 1..game.state.played.len() {
                    let card = &game.state.played[i];
                    score += points_per_card[card.number as usize];

                    if card.suit == best_card.suit {
                        if order[card.number as usize] > order[best_card.number as usize] {
                            best_card = card;
                            winner_idx = i as u8;
                        }
                    } else if card.suit == game.state.trump.suit {
                        best_card = card;
                        winner_idx = i as u8;
                    }
                }

                // Shift based on which player started the round (turn + 1)
                winner_idx = (winner_idx + game.state.turn + 1) % game.players.len() as u8;

                for player in &game.players {
                    player.sender.send(api::Event::RoundEnd(winner_idx, score)).unwrap();
                }

                // Update player score
                game.players[winner_idx as usize].score += score;

                // Update who plays first turn in next round based on the winner
                game.state.turn = winner_idx;

                // Update round counter
                game.state.round += 1;

                // Reset cards played
                game.state.played = Vec::with_capacity(game.players.len());

                // Check if there's more cards.
                if game.state.deck.len() != 0 {
                    // Deal new card to all players. Starting from the round winner

                    for i in 0..game.players.len() {
                        let card = game.state.deck.pop().unwrap();
                        let idx = (winner_idx as usize + i) % game.players.len();
                        let player = &mut game.players[idx];
                        player.cards.push(card.clone());
                        player.sender.send(api::Event::NewCard(card)).unwrap();
                    }
                } else if game.state.round as usize == CARDS.len() / game.players.len() {
                    // We've reached the last round of the game

                    // Find max score player
                    let winner = game.players.iter().max_by_key(|p| p.score).unwrap();
                    // TODO: FIXME: Edgecase - there is a draw

                    // Send who won to all players
                    for player in &game.players {
                        player.sender.send(api::Event::GameEnd(winner.id.clone())).unwrap();
                    }

                    // TODO: Close game event streams

                }
            } else {
                // Advance to next player's turn
                game.state.turn = (game.state.turn + 1) % game.players.len() as u8;
            }
            warp::reply()
        });

    // GET /game/ -> List all awaiting games
    // TODO: Add query string param to be able to filter: ongoing vs all
    let list_games = warp::path("game")
        .and(warp::get())
        .and(games.clone())
        .map(|games: Arc<Mutex<HashMap<usize, Game>>>| {
            let games = games.lock().unwrap();
            let active_games: Vec<GameInfo> = games
                .iter()
                .filter_map(|(id, game)| {
                    if game.num_players > game.players.len() as u8 {
                        Some(GameInfo {id: id.to_string(), num_players: game.num_players})
                    } else {
                        None
                    }
                })
                .collect();
            warp::reply::json(&active_games)
        });

    let routes = create
        .or(join)
        .or(play)
        .or(list_games);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}



fn create_sse_stream(rx: UnboundedReceiverStream<api::Event>) -> impl Stream<Item = Result<sse::Event, warp::Error>> + Send + 'static {
    // Transforms API events to SSE event
    rx.map(|event| Ok(sse::Event::default().data(serde_json::to_string(&event).unwrap())))
}