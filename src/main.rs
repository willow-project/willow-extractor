//! Willow extractor server

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use willow_extractor::{extract_from_url, resolve_identifier, Config, Item};

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
}

#[derive(Deserialize)]
struct ExtractRequest {
    url: String,
}

#[derive(Deserialize)]
struct SearchRequest {
    identifier: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn health() -> &'static str {
    "ok"
}

async fn extract(
    State(state): State<AppState>,
    Json(req): Json<ExtractRequest>,
) -> Result<Json<Vec<Item>>, (StatusCode, Json<ErrorResponse>)> {
    println!("=> {}", req.url);

    match extract_from_url(&req.url, &state.config).await {
        Ok(items) => {
            for item in &items {
                println!(
                    "   [{}] {} - {}",
                    item.source.as_deref().unwrap_or("?"),
                    item.title.as_deref().unwrap_or("No title"),
                    item.authors.join(", ")
                );
            }
            Ok(Json(items))
        }
        Err(e) => {
            println!("   ERROR: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            ))
        }
    }
}

async fn search(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Json<Option<Item>> {
    println!("=> search: {}", req.identifier);

    let item = resolve_identifier(&req.identifier, &state.config).await;

    if let Some(ref i) = item {
        println!(
            "   [{}] {} - {}",
            i.source.as_deref().unwrap_or("?"),
            i.title.as_deref().unwrap_or("No title"),
            i.authors.join(", ")
        );
    } else {
        println!("   Not found");
    }

    Json(item)
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8783".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let state = AppState {
        config: Arc::new(Config::default()),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/add", post(extract))
        .route("/search", post(search))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Willow running on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
