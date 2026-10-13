use crate::{
    auth::Tokens,
    service::{Error, User, UserService},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
#[derive(Clone)]
pub struct AppState {
    pub service: Arc<Mutex<UserService>>,
    pub tokens: Tokens,
}
#[derive(Deserialize, ToSchema)]
pub struct Register {
    pub nickname: String,
    pub password: String,
    pub color: u8,
    pub character: String,
}
#[derive(Deserialize, ToSchema)]
pub struct Login {
    pub nickname: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct Update {
    pub nickname: String,
    pub color: u8,
    pub character: String,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Error::Invalid(_) => StatusCode::BAD_REQUEST,
            Error::Conflict => StatusCode::CONFLICT,
            Error::Unauthorized => StatusCode::UNAUTHORIZED,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::Storage => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorBody {
                error: self.message().into(),
            }),
        )
            .into_response()
    }
}
async fn service<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&mut UserService) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let s = state.service.clone();
    tokio::task::spawn_blocking(move || f(&mut *s.lock().map_err(|_| Error::Storage)?))
        .await
        .map_err(|_| Error::Storage)?
}
fn identity(state: &AppState, headers: &HeaderMap) -> Result<u64, Error> {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(Error::Unauthorized)?;
    state.tokens.verify(token).map_err(|_| {
        tracing::warn!("JWT rejected");
        Error::Unauthorized
    })
}
#[utoipa::path(post,path="/api/register",tag="Authentication",request_body=Register,responses((status=201,body=User),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
async fn register(
    State(s): State<AppState>,
    Json(r): Json<Register>,
) -> Result<(StatusCode, Json<User>), Error> {
    let user = service(&s, move |x| {
        x.register(&r.nickname, &r.password, r.color, &r.character)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(user)))
}
#[utoipa::path(post,path="/api/login",tag="Authentication",request_body=Login,responses((status=200,body=TokenResponse),(status=401,body=ErrorBody)))]
async fn login(
    State(s): State<AppState>,
    Json(r): Json<Login>,
) -> Result<Json<TokenResponse>, Error> {
    let user = service(&s, move |x| x.login(&r.nickname, &r.password)).await?;
    let token = s
        .tokens
        .issue(user.id, jsonwebtoken::get_current_timestamp())
        .map_err(|_| Error::Storage)?;
    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "Bearer".into(),
        expires_in: 900,
    }))
}
#[utoipa::path(get,path="/api/users/me",tag="Profile",security(("bearerAuth"=[])),responses((status=200,body=User),(status=401,body=ErrorBody),(status=404,body=ErrorBody)))]
async fn read(State(s): State<AppState>, headers: HeaderMap) -> Result<Json<User>, Error> {
    let id = identity(&s, &headers)?;
    Ok(Json(service(&s, move |x| x.read(id)).await?))
}
#[utoipa::path(put,path="/api/users/me",tag="Profile",request_body=Update,security(("bearerAuth"=[])),responses((status=200,body=User),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=409,body=ErrorBody)))]
async fn update(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<Update>,
) -> Result<Json<User>, Error> {
    let id = identity(&s, &headers)?;
    Ok(Json(
        service(&s, move |x| {
            x.update(id, &r.nickname, r.color, &r.character)
        })
        .await?,
    ))
}
#[utoipa::path(delete,path="/api/users/me",tag="Profile",security(("bearerAuth"=[])),responses((status=204,description="Profile deleted"),(status=401,body=ErrorBody),(status=404,body=ErrorBody)))]
async fn delete(State(s): State<AppState>, headers: HeaderMap) -> Result<StatusCode, Error> {
    let id = identity(&s, &headers)?;
    service(&s, move |x| x.delete(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/characters",tag="Catalog",responses((status=200,body=Vec<String>)))]
async fn characters() -> Json<Vec<&'static str>> {
    Json(vec!["default", "keqing", "sonic", "cheburashka"])
}
struct Bearer;
impl Modify for Bearer {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi.components.as_mut().unwrap().add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}
#[derive(OpenApi)]
#[openapi(info(title="Boltrunner Player Profile API",version="0.1.0"),paths(register,login,read,update,delete,characters),components(schemas(User,Register,Login,Update,TokenResponse,ErrorBody)),modifiers(&Bearer))]
pub struct ApiDoc;
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { Html(include_str!("ui.html")) }))
        .route(
            "/docs",
            get(|| async { Html(include_str!("swagger.html")) }),
        )
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .route("/api/register", post(register))
        .route("/api/login", post(login))
        .route("/api/users/me", get(read).put(update).delete(delete))
        .route("/api/characters", get(characters))
        .with_state(state)
        .layer(axum::middleware::from_fn(
            |request: axum::extract::Request, next: axum::middleware::Next| async move {
                let method = request.method().clone();
                let response = next.run(request).await;
                tracing::info!(%method,status=response.status().as_u16(),"HTTP request handled");
                response
            },
        ))
}
