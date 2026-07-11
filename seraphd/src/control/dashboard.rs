use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../dashboard/dist/"]
struct Assets;

pub async fn serve_asset(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => {
            // SPA routing fallback: always serve index.html for unknown paths
            match Assets::get("index.html") {
                Some(content) => {
                    Response::builder()
                        .header(header::CONTENT_TYPE, "text/html")
                        .body(Body::from(content.data))
                        .unwrap()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
