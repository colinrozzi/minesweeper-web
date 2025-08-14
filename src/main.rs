use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use minesweeper::{Minesweeper, TileValue};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower_http::{cors::CorsLayer, services::ServeDir};
use axum::http::{Method, HeaderValue};
use axum::http::header::CONTENT_TYPE;

type GameStorage = Arc<Mutex<HashMap<String, GameInfo>>>;

#[derive(Debug)]
struct GameInfo {
    game: Option<Minesweeper>,
    size: usize,
    mine_count: usize,
    first_click_made: bool,
}

#[derive(Serialize, Deserialize)]
struct NewGameRequest {
    size: usize,
    mine_count: usize,
}

#[derive(Serialize)]
struct GameResponse {
    game_id: String,
    size: usize,
    mine_count: usize,
    game_state: String,
    board: Vec<Vec<TileResponse>>,
}

#[derive(Serialize)]
struct TileResponse {
    exposed: bool,
    flagged: bool,
    value: Option<String>, // "bomb", number as string, or None if not exposed
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    message: String,
    game_state: String,
    board: Vec<Vec<TileResponse>>,
}

#[tokio::main]
async fn main() {
    let games: GameStorage = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/new-game", post(new_game))
        .route("/api/game/{game_id}", get(get_game_state))
        .route("/api/game/{game_id}/click/{x}/{y}", post(click_tile))
        .route("/api/game/{game_id}/flag/{x}/{y}", post(toggle_flag))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(games);

    // Check if we should use HTTPS
    let use_https = std::env::var("USE_HTTPS").unwrap_or_else(|_| "false".to_string()) == "true";
    
    if use_https {
        // Production HTTPS
        let cert_path = std::env::var("CERT_PATH").expect("CERT_PATH must be set when USE_HTTPS=true");
        let key_path = std::env::var("KEY_PATH").expect("KEY_PATH must be set when USE_HTTPS=true");
        
        let config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .expect("Failed to load TLS certificates");

        let addr = "0.0.0.0:443".parse().unwrap();
        println!("HTTPS server running on https://0.0.0.0:443");
        
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        // Development HTTP
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
            .await
            .unwrap();
        
        println!("HTTP server running on http://127.0.0.1:3000");
        axum::serve(listener, app).await.unwrap();
    }
}

async fn serve_index() -> Html<String> {
    let html = tokio::fs::read_to_string("static/index.html")
        .await
        .unwrap_or_else(|_| {
            r#"
<!DOCTYPE html>
<html>
<head>
    <title>Minesweeper</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .board { display: inline-block; border: 2px solid #333; }
        .row { display: flex; }
        .tile { 
            width: 30px; height: 30px; 
            border: 1px solid #999; 
            display: flex; align-items: center; justify-content: center;
            cursor: pointer; font-weight: bold;
            background: #ddd;
        }
        .tile.exposed { background: #fff; }
        .tile.flagged { background: #ff9; }
        .tile.bomb { background: #f66; }
        .controls { margin: 20px 0; }
        button { padding: 10px 20px; margin: 5px; }
    </style>
</head>
<body>
    <h1>Minesweeper</h1>
    <div class="controls">
        <button onclick="newGame()">New Game</button>
        <span id="game-status">Ready to play!</span>
    </div>
    <div id="board"></div>
    <script>
        let currentGameId = null;
        
        async function newGame() {
            const response = await fetch('/api/new-game', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ size: 10, mine_count: 15 })
            });
            const game = await response.json();
            currentGameId = game.game_id;
            renderBoard(game);
            document.getElementById('game-status').textContent = 'Game ready - click any tile to start!';
        }
        
        async function clickTile(x, y) {
            if (!currentGameId) return;
            const response = await fetch(`/api/game/${currentGameId}/click/${x}/${y}`, { method: 'POST' });
            const result = await response.json();
            renderBoard(result);
            document.getElementById('game-status').textContent = result.game_state;
        }
        
        async function flagTile(x, y) {
            if (!currentGameId) return;
            const response = await fetch(`/api/game/${currentGameId}/flag/${x}/${y}`, { method: 'POST' });
            const result = await response.json();
            renderBoard(result);
        }
        
        function renderBoard(game) {
            const board = document.getElementById('board');
            board.innerHTML = '';
            board.className = 'board';
            
            game.board.forEach((row, x) => {
                const rowDiv = document.createElement('div');
                rowDiv.className = 'row';
                
                row.forEach((tile, y) => {
                    const tileDiv = document.createElement('div');
                    tileDiv.className = 'tile';
                    
                    if (tile.exposed) {
                        tileDiv.classList.add('exposed');
                        if (tile.value === 'bomb') {
                            tileDiv.classList.add('bomb');
                            tileDiv.textContent = '💣';
                        } else if (tile.value && tile.value !== '0') {
                            tileDiv.textContent = tile.value;
                        }
                    } else if (tile.flagged) {
                        tileDiv.classList.add('flagged');
                        tileDiv.textContent = '🚩';
                    }
                    
                    tileDiv.onclick = () => clickTile(x, y);
                    tileDiv.oncontextmenu = (e) => { e.preventDefault(); flagTile(x, y); };
                    
                    rowDiv.appendChild(tileDiv);
                });
                
                board.appendChild(rowDiv);
            });
        }
        
        // Start with a new game
        newGame();
    </script>
</body>
</html>
            "#.to_string()
        });
    Html(html)
}

async fn new_game(
    State(games): State<GameStorage>,
    Json(req): Json<NewGameRequest>,
) -> Result<Json<GameResponse>, StatusCode> {
    let game_id = generate_game_id();
    
    // Create a placeholder game info - the actual game will be created on first click
    let game_info = GameInfo {
        game: None,
        size: req.size,
        mine_count: req.mine_count,
        first_click_made: false,
    };
    
    let response = GameResponse {
        game_id: game_id.clone(),
        size: req.size,
        mine_count: req.mine_count,
        game_state: "InProgress".to_string(),
        board: create_empty_board_response(req.size),
    };
    
    games.lock().unwrap().insert(game_id, game_info);
    Ok(Json(response))
}

async fn get_game_state(
    State(games): State<GameStorage>,
    Path(game_id): Path<String>,
) -> Result<Json<GameResponse>, StatusCode> {
    let games = games.lock().unwrap();
    let game_info = games.get(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
    let (game_state, board) = if let Some(ref game) = game_info.game {
        (format!("{:?}", game.get_game_state()), serialize_board(game))
    } else {
        ("InProgress".to_string(), create_empty_board_response(game_info.size))
    };
    
    let response = GameResponse {
        game_id,
        size: game_info.size,
        mine_count: game_info.mine_count,
        game_state,
        board,
    };
    
    Ok(Json(response))
}

async fn click_tile(
    State(games): State<GameStorage>,
    Path((game_id, x, y)): Path<(String, usize, usize)>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let mut games = games.lock().unwrap();
    let game_info = games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
    // If this is the first click, create the game now
    if !game_info.first_click_made {
        game_info.game = Some(Minesweeper::new_with_first_click(
            game_info.size,
            game_info.mine_count,
            (x, y),
        ));
        game_info.first_click_made = true;
        
        // The first click is already processed by new_with_first_click
        let game = game_info.game.as_ref().unwrap();
        let response = ActionResponse {
            success: true,
            message: "First click processed! Game board generated.".to_string(),
            game_state: format!("{:?}", game.get_game_state()),
            board: serialize_board(game),
        };
        
        return Ok(Json(response));
    }
    
    // Normal click processing for subsequent clicks
    let game = game_info.game.as_mut().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = game.click_tile(x, y);
    
    let response = ActionResponse {
        success: result.is_ok(),
        message: result.err().unwrap_or_else(|| "Success".to_string()),
        game_state: format!("{:?}", game.get_game_state()),
        board: serialize_board(game),
    };
    
    Ok(Json(response))
}

async fn toggle_flag(
    State(games): State<GameStorage>,
    Path((game_id, x, y)): Path<(String, usize, usize)>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let mut games = games.lock().unwrap();
    let game_info = games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
    // Can't flag before first click
    if !game_info.first_click_made {
        let response = ActionResponse {
            success: false,
            message: "Make your first click before flagging!".to_string(),
            game_state: "InProgress".to_string(),
            board: create_empty_board_response(game_info.size),
        };
        return Ok(Json(response));
    }
    
    let game = game_info.game.as_mut().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = game.toggle_flag(x, y);
    
    let response = ActionResponse {
        success: result.is_ok(),
        message: result.err().unwrap_or_else(|| "Success".to_string()),
        game_state: format!("{:?}", game.get_game_state()),
        board: serialize_board(game),
    };
    
    Ok(Json(response))
}

fn serialize_board(game: &Minesweeper) -> Vec<Vec<TileResponse>> {
    let mut board = Vec::new();
    
    for x in 0..game.get_size() {
        let mut row = Vec::new();
        for y in 0..game.get_size() {
            if let Some(tile) = game.get_tile(x, y) {
                let value = if tile.exposed {
                    match &tile.value {
                        TileValue::Bomb => Some("bomb".to_string()),
                        TileValue::Number(n) => Some(n.to_string()),
                    }
                } else {
                    None
                };
                
                row.push(TileResponse {
                    exposed: tile.exposed,
                    flagged: tile.flagged,
                    value,
                });
            }
        }
        board.push(row);
    }
    
    board
}

fn create_empty_board_response(size: usize) -> Vec<Vec<TileResponse>> {
    let mut board = Vec::new();
    
    for _ in 0..size {
        let mut row = Vec::new();
        for _ in 0..size {
            row.push(TileResponse {
                exposed: false,
                flagged: false,
                value: None,
            });
        }
        board.push(row);
    }
    
    board
}

fn generate_game_id() -> String {
    use rand::distributions::Alphanumeric;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}
