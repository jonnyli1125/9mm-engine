mod engine;
mod board;

use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use clap::Parser;
use log::{info, error};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::protocol::Message;
use crate::board::{Board, Move};
//use crate::engine::Engine;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "localhost")]
    host: String,
    #[arg(long, default_value_t = 999)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    let args = Args::parse();
    let address = format!("{}:{}", args.host, args.port);

    // Bind the server to a local port
    let listener = TcpListener::bind(address.clone()).await.expect("Failed to bind");
    info!("Listening on: {}", address);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(stream));
    }

    Ok(())
}

struct Game {
    started: bool,
    player_is_black: bool,
    board: Board,
}

impl Game {
    fn new() -> Self {
        Self {
            started: false,
            player_is_black: true,
            board: Board::new(),
        }
    }
}

async fn accept_connection(stream: TcpStream) -> Result<(), Error> {
    let addr = stream.peer_addr()?;
    info!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("New WebSocket connection: {}", addr);

    let (mut write, mut read) = ws_stream.split();

    let game_mutex = Arc::new(Mutex::new(Game::new()));
    //let engine = Arc::new(Engine::new());

    while let Some(raw_message) = read.next().await {
        match raw_message {
            Ok(text_message) => {
                if !text_message.is_text() && !text_message.is_binary() { continue; }
                match serde_json::from_slice::<Value>(&text_message.into_data()) {
                    Ok(data) => {
                        info!("Received: {}", data);
                        let result: Result<Value, Error> = handle_message(&game_mutex, data).await;
                        let response = match result {
                            Ok(resp) => resp,
                            Err(e) => {
                                error!("Error handling message: {:?}", e);
                                json!({"error": format!("{:?}", e)})
                            }
                        };
                        let response_str = response.to_string();
                        write.send(Message::text(response_str.clone())).await
                                    .expect(&format!("Failed to send message: {}", response_str));
                        info!("Sent: {}", response_str);
                    },
                    Err(e) => { error!("Error parsing JSON: {:?}", e); }
                }
            }
            Err(e) => { error!("Error reading websocket message: {:?}", e); }
        }
    }

    Ok(())
}

async fn handle_message(game_mutex: &Arc<Mutex<Game>>, data: Value) -> Result<Value, Error> {
    let mut game = game_mutex.lock().unwrap();

    let map = data.as_object()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Expected a dict"))?;

    // client message protocol: "start", "move"
    // server message protocol: "move", "legal_moves", "error", "end"
    if map.contains_key("start") {
        let player_is_black = data["start"].as_bool().ok_or_else(
            || Error::new(ErrorKind::InvalidInput, "Expected boolean field: start")
        )?;
        let response = handle_start(&mut game, player_is_black)?;
        Ok(response)
    } else if map.contains_key("move") {
        if !game.started {
            return Err(Error::new(ErrorKind::InvalidInput, "Game has not started yet"));
        }
        let maybe_move: Option<Move> = serde_json::from_value(data["move"].clone())?;
        let response = handle_move(&mut game, maybe_move)?;
        Ok(response)
    } else {
        Err(Error::new(ErrorKind::InvalidInput, format!("Invalid message: {}", data)))
    }
}

fn handle_start(game: &mut Game, player_is_black: bool) -> Result<Value, Error> {
    game.started = true;
    game.player_is_black = player_is_black;
    if player_is_black {
        Ok(json!({ "legal_moves": game.board.legal_moves() }))
    } else {
        make_engine_move(game)
    }
}

fn handle_move(game: &mut Game, maybe_move: Option<Move>) -> Result<Value, Error> {
    game.board = game.board.make_move(maybe_move)?;
    match check_game_over(game) {
        Some(game_over) => Ok(game_over),
        None => make_engine_move(game)
    }
}

fn make_engine_move(game: &mut Game) -> Result<Value, Error> {
    // TODO: select best engine move
    //   let engine find best move in new thread with rayon::spawn and Engine::best_move(Arc::as_ref(&board))
    let selected_move = game.board.legal_moves().first().cloned(); // debug: select a random move
    game.board = game.board.make_move(selected_move.clone())?;
    match check_game_over(game) {
        Some(game_over) => Ok(game_over),
        None => Ok(json!({ "move": selected_move, "legal_moves": game.board.legal_moves() }))
    }
}

fn check_game_over(game: &Game) -> Option<Value> {
    if game.board.is_black_winner() {
        Some(json!({ "end": true }))
    } else if game.board.is_white_winner() {
        Some(json!({ "end": false }))
    } else {
        None
    }
}
