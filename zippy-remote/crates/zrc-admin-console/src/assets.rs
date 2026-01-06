use rust_embed::RustEmbed;
use axum::{
    response::{IntoResponse, Response},
    http::{header, StatusCode, Uri},
};

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();
        
        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => {
                // SPA Fallback: Return index.html for 404s if it's not a direct API call (handled by router order)
                // But for simplicity, if asset not found, we return index.html IF the path looks like a route (no extension)
                // Actually, the easiest way is to catch-all route.
                StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Asset::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            // SPA Fallback
             if path.starts_with("assets/") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                 match Asset::get("index.html") {
                    Some(content) => {
                        ([(header::CONTENT_TYPE, "text/html")], content.data).into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }
}
