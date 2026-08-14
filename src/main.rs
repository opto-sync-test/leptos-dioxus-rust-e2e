use axum::{
    Json, Router,
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use leptos_dioxus_rust_e2e::{MutationEnvelope, dioxus_island, leptos_shell, optimistic_snapshot};
use tower_http::services::ServeFile;

async fn index() -> Html<String> {
    let snapshot = optimistic_snapshot().unwrap_or_else(|error| format!("sync error: {error}"));
    Html(format!(
        r#"<!doctype html>
<html lang="en">
  <head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>OptoSync Rust E2E</title></head>
  <body>{}{}<pre id="snapshot">{}</pre>
    <script>navigator.serviceWorker?.register('/service-worker.js');</script>
  </body>
</html>"#,
        leptos_shell(),
        dioxus_island(),
        snapshot
    ))
}

async fn sync_lane(
    Path(lane): Path<String>,
    Json(batch): Json<Vec<MutationEnvelope>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if lane != "upload" && lane != "realtime" {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(serde_json::json!({
        "lane": lane,
        "accepted": batch.iter().map(|mutation| &mutation.id).collect::<Vec<_>>()
    })))
}

pub fn app() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/sync/{lane}", post(sync_lane))
        .route_service(
            "/service-worker.js",
            ServeFile::new("public/service-worker.js"),
        )
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("server address must bind");
    axum::serve(listener, app()).await.expect("server must run");
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn http_shell_contains_both_renderers_and_worker_registration() {
        let response = super::app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("OptoSync Leptos shell"));
        assert!(html.contains("Dioxus cross-platform island"));
        assert!(html.contains("serviceWorker"));
        assert!(html.contains("edited while offline"));
    }
}
