use axum::{routing::get, Json, Router};
use serde::Serialize;
use tower_http::cors::CorsLayer;

#[derive(Serialize)]
struct Message {
    message: String,
}

async fn hello() -> Json<Message> {
    Json(Message {
        message: "안녕하세요! axum 백엔드에서 보낸 메시지입니다 👋".to_string(),
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/hello", get(hello))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .unwrap();

    println!("🚀 백엔드 실행 중: http://127.0.0.1:3001");
    axum::serve(listener, app).await.unwrap();
}
