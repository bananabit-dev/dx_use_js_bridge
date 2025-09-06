use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use warp::Filter;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Room {
    id: String,
    name: String,
    players: Vec<String>,
    max_players: u32,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RoomResponse {
    rooms: Vec<Room>,
    total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateRoomRequest {
    name: String,
    max_players: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JoinRoomRequest {
    room_id: String,
    player_name: String,
}

#[tokio::main]
async fn main() {
    // Initialize with some sample rooms
    let mut rooms = HashMap::new();
    
    rooms.insert("room_1".to_string(), Room {
        id: "room_1".to_string(),
        name: "Lobby Room 1".to_string(),
        players: vec!["player_1".to_string()],
        max_players: 4,
        created_at: "2024-01-01T00:00:00Z".to_string(),
    });
    
    rooms.insert("room_2".to_string(), Room {
        id: "room_2".to_string(),
        name: "Lobby Room 2".to_string(),
        players: vec!["player_2".to_string(), "player_3".to_string()],
        max_players: 6,
        created_at: "2024-01-01T01:00:00Z".to_string(),
    });

    // CORS filter to allow cross-origin requests
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

    // GET /api/rooms - Get all rooms
    let get_rooms = warp::path!("api" / "rooms")
        .and(warp::get())
        .map(move || {
            let room_list: Vec<Room> = rooms.values().cloned().collect();
            let response = RoomResponse {
                total: room_list.len(),
                rooms: room_list,
            };
            warp::reply::json(&response)
        });

    // POST /api/rooms - Create a new room
    let create_room = warp::path!("api" / "rooms")
        .and(warp::post())
        .and(warp::body::json())
        .map(|create_req: CreateRoomRequest| {
            let room_id = format!("room_{}", chrono::Utc::now().timestamp());
            let room = Room {
                id: room_id.clone(),
                name: create_req.name,
                players: vec![],
                max_players: create_req.max_players.unwrap_or(4),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            warp::reply::json(&room)
        });

    // POST /api/rooms/:id/join - Join a room
    let join_room = warp::path!("api" / "rooms" / String / "join")
        .and(warp::post())
        .and(warp::body::json())
        .map(|room_id: String, join_req: JoinRoomRequest| {
            // In a real implementation, you'd update the room state
            let response = serde_json::json!({
                "success": true,
                "room_id": room_id,
                "player_name": join_req.player_name,
                "message": "Successfully joined room"
            });
            warp::reply::json(&response)
        });

    // Health check endpoint
    let health = warp::path!("health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "service": "lobby-server"
            }))
        });

    let routes = get_rooms
        .or(create_room)
        .or(join_room)
        .or(health)
        .with(cors);

    println!("Lobby server starting on port 3001...");
    println!("Available endpoints:");
    println!("  GET  /api/rooms     - List all rooms");
    println!("  POST /api/rooms     - Create a new room");
    println!("  POST /api/rooms/:id/join - Join a room");
    println!("  GET  /health        - Health check");

    warp::serve(routes)
        .run(([0, 0, 0, 0], 3001))
        .await;
}