use axum::{routing::get, Json, Router};
use serde::Serialize;
use tower_http::cors::CorsLayer;

#[derive(Serialize)]
struct Message {
    message: String,
}

async fn hello() -> Json<Message> {
    Json(Message {
        message: "여기에서는 api로 신호만 보낼 예정입니다.".to_string(),
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/hello", get(hello))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3002")
        .await
        .unwrap();

    println!("백엔드 실행 중: http://127.0.0.1:3002");
    axum::serve(listener, app).await.unwrap();
}
