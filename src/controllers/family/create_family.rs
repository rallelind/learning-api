use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    errors::AppError,
    models::{family::Family, users::User},
    repository::mongodb_repo::MongoRepo,
};

#[derive(Deserialize)]
pub struct FamilyPayload {
    family_members: Vec<User>,
    family_name: String,
}

#[axum_macros::debug_handler]
pub async fn create_family(
    Extension(db): Extension<MongoRepo>,
    Json(family_payload): Json<FamilyPayload>,
) -> Result<Json<Value>, AppError> {
    let FamilyPayload {
        family_members,
        family_name,
    } = family_payload;

    // There may be a prettier solution for this, it was made because id in users model, id is optional
    for member in &family_members {
        if member.id.is_none() {
            return Err(AppError::MissingUserIdForProvidedUsers);
        }
    }

    let new_family = Family {
        family_name,
        members: family_members,
        id: None,
    };

    let created_family = db.create_family(new_family);

    match created_family {
        Ok(family) => Ok(Json(serde_json::json!(family))),
        Err(_) => Err(AppError::InternalServerError),
    }
}
