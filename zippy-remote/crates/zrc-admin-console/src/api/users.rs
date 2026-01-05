use axum::{
    Json,
    Extension,
};
use crate::db::schema::User;

pub async fn get_current_user(
    Extension(user): Extension<User>,
) -> Json<User> {
    Json(user)
}
