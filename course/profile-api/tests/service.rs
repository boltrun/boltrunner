use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boltrunner_profile_api::{
    auth::{Claims, Tokens},
    http::{AppState, router},
    logging::RollingLog,
    service::{Error, UserService, validate_profile},
};
use http_body_util::BodyExt;
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tower::ServiceExt;
static COUNTER: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "boltrunner-api-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn key() -> Tokens {
    Tokens::new(b"test-only-secret-32-bytes-minimum-unused-outside-tests").unwrap()
}
#[test]
fn profile_validation_categories() {
    for (nick, color, character, valid) in [
        ("Alice", 0, "default", true),
        ("Игрок-2_", 7, "sonic", true),
        ("A B", 1, "keqing", true),
        ("", 0, "default", false),
        ("   ", 0, "default", false),
        (" Alice", 0, "default", false),
        ("Alice ", 0, "default", false),
        ("abcdefghijklmnopqrstu", 0, "default", false),
        ("A<script>", 0, "default", false),
        ("A\nB", 0, "default", false),
        ("Alice", 8, "default", false),
        ("Alice", 0, "unknown", false),
    ] {
        assert_eq!(
            validate_profile(nick, color, character).is_ok(),
            valid,
            "{nick:?}/{color}/{character}"
        );
    }
}
#[test]
fn password_boundaries() {
    let t = Temp::new();
    let mut s = UserService::open(t.0.join("users.json")).unwrap();
    for p in ["", "1234567", &"x".repeat(129)] {
        assert!(matches!(
            s.register("Alice", p, 0, "default"),
            Err(Error::Invalid(_))
        ));
    }
    assert!(s.register("Alice", "12345678", 0, "default").is_ok());
    assert!(s.register("Bob", &"x".repeat(128), 0, "default").is_ok());
}
#[test]
fn crud_persists_and_hashes_passwords() {
    let t = Temp::new();
    let path = t.0.join("users.json");
    let mut s = UserService::open(&path).unwrap();
    let u = s.register("Alice", "test-pass-123", 2, "sonic").unwrap();
    assert_eq!(s.read(u.id).unwrap(), u);
    assert_eq!(s.login("alice", "test-pass-123").unwrap(), u);
    assert!(matches!(
        s.register("ALICE", "test-pass-123", 0, "default"),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        s.login("Alice", "wrong"),
        Err(Error::Unauthorized)
    ));
    assert!(matches!(
        s.login("Missing", "test-pass-123"),
        Err(Error::Unauthorized)
    ));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("test-pass-123"));
    assert!(text.contains("$argon2id$"));
    let changed = s.update(u.id, "Alex", 3, "cheburashka").unwrap();
    drop(s);
    let mut s = UserService::open(&path).unwrap();
    assert_eq!(s.read(u.id).unwrap(), changed);
    s.delete(u.id).unwrap();
    assert!(matches!(s.read(u.id), Err(Error::NotFound)));
    assert!(matches!(s.delete(u.id), Err(Error::NotFound)));
    assert!(matches!(
        s.update(u.id, "Alice", 0, "default"),
        Err(Error::NotFound)
    ));
}
#[test]
fn updates_cannot_take_other_nickname() {
    let t = Temp::new();
    let mut s = UserService::open(t.0.join("users.json")).unwrap();
    let a = s.register("Alice", "password-A", 0, "default").unwrap();
    s.register("Bob", "password-B", 0, "default").unwrap();
    assert!(matches!(
        s.update(a.id, "BOB", 0, "default"),
        Err(Error::Conflict)
    ));
    assert_eq!(s.read(a.id).unwrap().nickname, "Alice");
}
#[test]
fn failed_write_does_not_mutate_service() {
    let t = Temp::new();
    let path = t.0.join("users.json");
    let mut s = UserService::open(&path).unwrap();
    let u = s.register("Alice", "password-A", 0, "default").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(
        s.update(u.id, "Changed", 0, "default"),
        Err(Error::Storage)
    ));
    assert_eq!(s.read(u.id).unwrap().nickname, "Alice");
}
#[test]
fn corrupted_storage_is_rejected() {
    let t = Temp::new();
    let p = t.0.join("users.json");
    std::fs::write(&p, "broken json").unwrap();
    assert!(matches!(UserService::open(p), Err(Error::Storage)));
}
#[test]
fn jwt_valid_tampered_expired_and_wrong_context() {
    let tokens = key();
    let now = jsonwebtoken::get_current_timestamp();
    let token = tokens.issue(9, now).unwrap();
    assert_eq!(tokens.verify(&token).unwrap(), 9);
    let other = Tokens::new(b"different-test-only-secret-at-least-32-bytes").unwrap();
    assert!(other.verify(&token).is_err());
    assert!(tokens.verify("a.b.c").is_err());
    for (exp, iss, aud, sub) in [
        (
            now - 1,
            "boltrunner-profile-api",
            "boltrunner-profile-ui",
            "9",
        ),
        (now + 900, "wrong", "boltrunner-profile-ui", "9"),
        (now + 900, "boltrunner-profile-api", "wrong", "9"),
        (
            now + 900,
            "boltrunner-profile-api",
            "boltrunner-profile-ui",
            "0",
        ),
    ] {
        let token = tokens
            .issue_claims(Claims {
                sub: sub.into(),
                exp,
                iat: now,
                iss: iss.into(),
                aud: aud.into(),
            })
            .unwrap();
        assert!(tokens.verify(&token).is_err());
    }
    assert!(Tokens::new(b"short").is_err());
}
#[test]
fn log_rotation_keeps_two_backups() {
    let t = Temp::new();
    let path = t.0.join("log.txt");
    let mut logger = RollingLog::new(path.clone(), 10).unwrap();
    logger.write_all(b"event-one\n").unwrap();
    logger.write_all(b"event-two\n").unwrap();
    logger.write_all(b"event-three\n").unwrap();
    logger.flush().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "event-three\n");
    assert_eq!(
        std::fs::read_to_string(path.with_extension("1.txt")).unwrap(),
        "event-two\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.with_extension("2.txt")).unwrap(),
        "event-one\n"
    );
}
async fn send(
    app: axum::Router,
    method: &str,
    path: &str,
    data: serde_json::Value,
    token: Option<&str>,
) -> axum::response::Response {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    app.oneshot(b.body(Body::from(data.to_string())).unwrap())
        .await
        .unwrap()
}
#[tokio::test]
async fn http_authentication_and_profile_lifecycle() {
    let t = Temp::new();
    let state = AppState {
        service: Arc::new(Mutex::new(
            UserService::open(t.0.join("users.json")).unwrap(),
        )),
        tokens: key(),
    };
    let app = router(state);
    assert_eq!(
        send(
            app.clone(),
            "GET",
            "/api/users/me",
            serde_json::Value::Null,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let body = serde_json::json!({"nickname":"Alice","password":"password-A","color":2,"character":"sonic"});
    assert_eq!(
        send(app.clone(), "POST", "/api/register", body.clone(), None)
            .await
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        send(app.clone(), "POST", "/api/register", body, None)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let r = send(
        app.clone(),
        "POST",
        "/api/login",
        serde_json::json!({"nickname":"Alice","password":"password-A"}),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    let body = r.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = v["access_token"].as_str().unwrap();
    assert_eq!(
        send(
            app.clone(),
            "GET",
            "/api/users/me",
            serde_json::Value::Null,
            Some(token)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            app.clone(),
            "PUT",
            "/api/users/me",
            serde_json::json!({"nickname":"Alex","color":7,"character":"keqing"}),
            Some(token)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            app.clone(),
            "DELETE",
            "/api/users/me",
            serde_json::Value::Null,
            Some(token)
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            app,
            "GET",
            "/api/users/me",
            serde_json::Value::Null,
            Some(token)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn openapi_matches_routes_and_security() {
    let t = Temp::new();
    let app = router(AppState {
        service: Arc::new(Mutex::new(
            UserService::open(t.0.join("users.json")).unwrap(),
        )),
        tokens: key(),
    });
    let r = send(app, "GET", "/openapi.json", serde_json::Value::Null, None).await;
    assert_eq!(r.status(), StatusCode::OK);
    let b = r.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert!(v["paths"]["/api/login"]["post"].is_object());
    assert!(v["paths"]["/api/users/me"]["put"].is_object());
    assert_eq!(
        v["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
}

#[test]
fn structured_events_rotate_without_splitting_json() {
    let t = Temp::new();
    let path = t.0.join("log.txt");
    let writer = RollingLog::new(path.clone(), 200).unwrap();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(writer)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        for i in 0..5 {
            tracing::info!(event_id = i, "complete structured event");
        }
    });
    for p in [
        path.clone(),
        path.with_extension("1.txt"),
        path.with_extension("2.txt"),
    ] {
        let text = std::fs::read_to_string(p).unwrap();
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["fields"]["message"], "complete structured event");
        }
    }
}
