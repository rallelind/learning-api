use async_mongodb_session::MongodbSessionStore;
use async_session::{async_trait, Session, SessionStore};
use axum::{
    extract::{rejection::TypedHeaderRejectionReason, FromRef, FromRequestParts, Query, State},
    headers::Cookie,
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    RequestPartsExt, TypedHeader, Extension,
};
use http::{header, request::Parts};
use oauth2::{basic::BasicClient, reqwest::async_http_client, AuthorizationCode, TokenResponse};
use serde::{Deserialize, Serialize};

use crate::{repository::mongodb_repo::MongoRepo, errors::AppError};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthedUser {
    pub id: String,
    pub picture: String,
    pub name: String,
    pub email: String
}

pub static COOKIE_NAME: &str = "SESSION";

pub async fn login_authorized(
    Extension(db): Extension<MongoRepo>,
    Query(query): Query<AuthRequest>,
    State(store): State<MongodbSessionStore>,
    State(oauth_client): State<BasicClient>,
) -> (HeaderMap, axum::response::Redirect) {
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let user_data = client
        .get("https://www.googleapis.com/oauth2/v1/userinfo?alt=json")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .unwrap()
        .json::<AuthedUser>()
        .await
        .unwrap();

    let create_user_result = db.create_user(&user_data);

    if let Err(err) = create_user_result {
        eprintln!("Error creating user: {:?}", err);
    }

    let mut session = Session::new();

    session.insert("user", &user_data).unwrap();

    let cookie = store.store_session(session).await.unwrap().unwrap();

    let cookie = format!("{}={}; SameSite=Lax; Path=/", COOKIE_NAME, cookie);

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().unwrap());

    (headers, Redirect::to("http://localhost:3002/application"))
}



#[async_trait]
impl<S> FromRequestParts<S> for AuthedUser
where
    MongodbSessionStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MongodbSessionStore::from_ref(state);

        let cookies =
            parts
                .extract::<TypedHeader<Cookie>>()
                .await
                .map_err(|e| match *e.name() {
                    header::COOKIE => match e.reason() {
                        TypedHeaderRejectionReason::Missing => AppError::UserNotLoggedIn,
                        _ => panic!("unexpected error getting Cookie header(s): {}", e),
                    },
                    _ => panic!("unexpected error getting cookies: {}", e),
                })?;

        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AppError::UserNotLoggedIn)?;

        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AppError::UserNotLoggedIn)?;

            let user = session.get::<AuthedUser>("user").ok_or(AppError::UserNotLoggedIn)?;

            Ok(user)    
        }
}
