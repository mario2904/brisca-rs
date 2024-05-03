use api::Event;

use bytes::Bytes;
use eventsource_stream::Eventsource;
use iced::futures::stream::BoxStream;
use iced::futures::{SinkExt, StreamExt};
use iced::command::{channel, Command};
use reqwest::{Client, Error};


enum State<'a> {
    Starting,
    Ready(BoxStream<'a, Result<Bytes, Error>>),
    Finished
}

pub fn connect(url: String, player_id: String) -> Command<Event> {
    channel(100, |mut output| async move {
        let mut state = State::Starting;
        loop {
            match &mut state {
                State::Starting => {
                    // Get game stream events
                    let stream = Client::new()
                        .get(&url)
                        .header("authorization", &player_id) // TODO: Implement proper auth
                        .send()
                        .await
                        .unwrap()
                        .bytes_stream()
                        .boxed();
                    state = State::Ready(stream);
                },
                State::Ready(stream) => {
                    if let Some(event) = stream.eventsource().next().await {
                        match event {
                            Ok(event) => {
                                let game_event: Event = serde_json::from_str(&event.data).unwrap();

                                if let Event::GameEnd(_) = game_event {
                                    // This is the last event for the game.
                                    state = State::Finished;
                                }

                                let _ = output.send(game_event).await;
                            },
                            Err(error) => {
                                // Error parsing the event
                                println!("{:?}", error);
                            }
                        }
                    }
                },
                State::Finished => {
                    println!("Game event stream has finished");
                    break;
                }
            }
        }
    })
}