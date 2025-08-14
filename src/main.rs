use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use minesweeper::{Minesweeper, TileValue};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower_http::{cors::CorsLayer, services::ServeDir};

type GameStorage = Arc<Mutex<HashMap<String, Minesweeper>>>;

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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    
    println!("Server running on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
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
            document.getElementById('game-status').textContent = 'Game in progress';
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
    let mine_locations = generate_random_mines(req.size, req.mine_count);
    let game = Minesweeper::new(req.size, mine_locations);
    
    let response = GameResponse {
        game_id: game_id.clone(),
        size: game.get_size(),
        mine_count: game.get_bomb_count(),
        game_state: format!("{:?}", game.get_game_state()),
        board: serialize_board(&game),
    };
    
    games.lock().unwrap().insert(game_id, game);
    Ok(Json(response))
}

async fn get_game_state(
    State(games): State<GameStorage>,
    Path(game_id): Path<String>,
) -> Result<Json<GameResponse>, StatusCode> {
    let games = games.lock().unwrap();
    let game = games.get(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
    let response = GameResponse {
        game_id,
        size: game.get_size(),
        mine_count: game.get_bomb_count(),
        game_state: format!("{:?}", game.get_game_state()),
        board: serialize_board(game),
    };
    
    Ok(Json(response))
}

async fn click_tile(
    State(games): State<GameStorage>,
    Path((game_id, x, y)): Path<(String, usize, usize)>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let mut games = games.lock().unwrap();
    let game = games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
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
    let game = games.get_mut(&game_id).ok_or(StatusCode::NOT_FOUND)?;
    
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

fn generate_game_id() -> String {
    use rand::distributions::Alphanumeric;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

fn generate_random_mines(size: usize, mine_count: usize) -> Vec<(usize, usize)> {
    let mut rng = rand::thread_rng();
    let mut mines = Vec::new();
    
    while mines.len() < mine_count {
        let x = rng.gen_range(0..size);
        let y = rng.gen_range(0..size);
        
        if !mines.contains(&(x, y)) {
            mines.push((x, y));
        }
    }
    
    mines
}
