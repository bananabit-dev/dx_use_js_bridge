use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub players: Vec<String>,
    pub max_players: u32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub name: String,
    pub max_players: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct JoinRoomRequest {
    pub player_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ListRoomsQuery {
    pub status: Option<String>,
}

type AppState = Arc<RwLock<HashMap<String, Room>>>;

#[tokio::main]
async fn main() {
    // Initialize shared state
    let state: AppState = Arc::new(RwLock::new(HashMap::new()));

    // Create a sample room for testing
    {
        let mut rooms = state.write().await;
        let sample_room = Room {
            id: "room_001".to_string(),
            name: "Sample Room".to_string(),
            players: vec!["player1".to_string()],
            max_players: 4,
            status: "waiting".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        rooms.insert(sample_room.id.clone(), sample_room);
    }

    // Build the router
    let app = Router::new()
        .route("/api/rooms", get(list_rooms))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/:room_id/join", post(join_room))
        .route("/api/health", get(health_check))
        .with_state(state)
        .layer(CorsLayer::permissive());

    // Start the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("Failed to bind to port 3001");

    println!("Lobby server starting on port 3001");
    println!("Available endpoints:");
    println!("  GET  /api/rooms      - List all rooms");
    println!("  POST /api/rooms      - Create a new room");
    println!("  POST /api/rooms/:id/join - Join a room");
    println!("  GET  /api/health     - Health check");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "lobby-server",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn list_rooms(
    State(state): State<AppState>,
    Query(params): Query<ListRoomsQuery>,
) -> Json<Vec<Room>> {
    let rooms = state.read().await;
    let mut room_list: Vec<Room> = rooms.values().cloned().collect();

    // Filter by status if provided
    if let Some(status) = params.status {
        room_list.retain(|room| room.status == status);
    }

    Json(room_list)
}

async fn create_room(
    State(state): State<AppState>,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<Room>, StatusCode> {
    let room_id = format!("room_{}", uuid::Uuid::new_v4().to_string().replace("-", "_"));
    
    let room = Room {
        id: room_id.clone(),
        name: request.name,
        players: Vec::new(),
        max_players: request.max_players.unwrap_or(4),
        status: "waiting".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut rooms = state.write().await;
    rooms.insert(room_id, room.clone());

    Ok(Json(room))
}

async fn join_room(
    State(state): State<AppState>,
    axum::extract::Path(room_id): axum::extract::Path<String>,
    Json(request): Json<JoinRoomRequest>,
) -> Result<Json<Room>, StatusCode> {
    let mut rooms = state.write().await;
    
    match rooms.get_mut(&room_id) {
        Some(room) => {
            if room.players.len() >= room.max_players as usize {
                return Err(StatusCode::BAD_REQUEST);
            }
            
            if !room.players.contains(&request.player_name) {
                room.players.push(request.player_name);
            }
            
            // Update status if room is full
            if room.players.len() >= room.max_players as usize {
                room.status = "full".to_string();
            }
            
            Ok(Json(room.clone()))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}