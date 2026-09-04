//! Integration tests against the full router with an in-memory database.
//!
//! The hub ships with no data, so every test builds exactly the state it
//! asserts on through the API. There are no shared fixtures beyond the
//! helpers below, and no test depends on another test's rows.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use cohort_hub::{build_router, config::Config, db};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        bind: "127.0.0.1:0".into(),
        db: "sqlite::memory:".into(),
        allowed_origins: vec!["http://localhost:1420".into()],
        log_dir: None,
        anthropic_api_key: None,
        anthropic_model: "claude-sonnet-5".into(),
    }
}

async fn app() -> Router {
    let config = test_config();
    let pool = db::pool(&config.db).await.expect("pool");
    build_router(pool, config)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    user: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(u) = user {
        builder = builder.header("x-user-id", u);
    }
    let request = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

// ---- Fixtures: state is built through the API, never inserted behind it ----

/// Register a user and return their id.
async fn user(app: &Router, name: &str) -> String {
    let (status, v) = call(app, "POST", "/api/users", None, Some(json!({ "name": name }))).await;
    assert_eq!(status, StatusCode::OK, "registering {name}");
    v["id"].as_str().unwrap().to_string()
}

/// Open an assist owned by `owner` and return its ref. `fields` is the
/// create body; only `title` is required.
async fn assist(app: &Router, owner: &str, fields: Value) -> String {
    let (status, v) = call(app, "POST", "/api/assists", Some(owner), Some(fields)).await;
    assert_eq!(status, StatusCode::OK, "creating an assist for {owner}");
    v["ref"].as_str().unwrap().to_string()
}

async fn join(app: &Router, ref_: &str, responder: &str) {
    let (status, _) = call(app, "POST", &format!("/api/assists/{ref_}/responders"), Some(responder), None).await;
    assert_eq!(status, StatusCode::OK, "{responder} joining {ref_}");
}

/// File a scope request and return its id.
async fn request_scope(app: &Router, ref_: &str, requester: &str, body: Value) -> i64 {
    let (status, v) =
        call(app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(requester), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{requester} requesting a scope on {ref_}");
    v["id"].as_i64().unwrap()
}

async fn approve(app: &Router, id: i64, owner: &str) {
    let (status, _) = call(app, "POST", &format!("/api/scope-requests/{id}/approve"), Some(owner), None).await;
    assert_eq!(status, StatusCode::OK, "approving request {id}");
}

async fn close(app: &Router, ref_: &str, owner: &str, body: Value) {
    let (status, _) = call(app, "POST", &format!("/api/assists/{ref_}/close"), Some(owner), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "closing {ref_}");
}

/// The shape most tests need: an owner, a responder who has joined, and one
/// open assist. Returns (owner, responder, assist ref).
async fn owner_responder_assist(app: &Router, title: &str) -> (String, String, String) {
    let owner = user(app, "Owner").await;
    let responder = user(app, "Responder").await;
    let ref_ = assist(app, &owner, json!({ "title": title })).await;
    join(app, &ref_, &responder).await;
    (owner, responder, ref_)
}

/// The cursor comparison is inclusive (`>=`) so boundary events are never
/// lost; the client dedupes by id. A short settle before reading a cursor
/// keeps timestamps strictly ordered where a test asserts non-repetition.
async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
}

// ---- Identity ----

#[tokio::test]
async fn requests_without_an_identity_are_refused() {
    let app = app().await;
    // Every route that acts as someone needs the header; there is no default
    // identity to fall back on.
    for (method, path) in [("GET", "/api/assists"), ("GET", "/api/my-record"), ("GET", "/api/notifications")] {
        let (status, v) = call(&app, method, path, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        assert!(v["error"].as_str().unwrap().contains("X-User-Id"));
    }
    // An unknown identity is refused just as clearly.
    let (status, _) = call(&app, "GET", "/api/assists", Some("u-nobody"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Registration and the identity picker stay open: they are how a machine
    // gets an identity in the first place.
    let (status, _) = call(&app, "GET", "/api/users", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "POST", "/api/users", None, Some(json!({ "name": "Newcomer" }))).await;
    assert_eq!(status, StatusCode::OK);
}

/// The whole first-run contract, in order: a freshly installed hub has nobody
/// on it, refuses every request that needs an identity, and the only way in is
/// to register. A second machine then signs in as an existing user.
#[tokio::test]
async fn first_run_requires_creating_or_picking_a_user() {
    let app = app().await;

    // 1. Nobody exists yet.
    let (status, users) = call(&app, "GET", "/api/users", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users.as_array().unwrap().len(), 0);

    // 2. Nothing can be done without an identity.
    let (status, _) = call(&app, "GET", "/api/assists", None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, "POST", "/api/assists", None, Some(json!({ "title": "Anything" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 3. Registering is open, and it is the only way in.
    let (status, me) = call(&app, "POST", "/api/users", None, Some(json!({ "name": "Ira K." }))).await;
    assert_eq!(status, StatusCode::OK);
    let me = me["id"].as_str().unwrap().to_string();

    // 4. That identity now works everywhere.
    let (status, assists) = call(&app, "GET", "/api/assists", Some(&me), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(assists.as_array().unwrap().len(), 0);
    let ref_ = assist(&app, &me, json!({ "title": "My first assist" })).await;
    assert_eq!(ref_, "S-1");

    // 5. A second machine sees that user in the picker and signs in as them,
    //    with no registration of its own.
    let (_, users) = call(&app, "GET", "/api/users", None, None).await;
    let listed: Vec<&str> = users.as_array().unwrap().iter().map(|u| u["id"].as_str().unwrap()).collect();
    assert_eq!(listed, vec![me.as_str()]);
    let (status, _) = call(&app, "GET", "/api/my-record", Some(&me), None).await;
    assert_eq!(status, StatusCode::OK);

    // 6. An identity this hub does not know stays refused, which is what
    //    sends a machine with a stale stored identity back to setup.
    let (status, v) = call(&app, "GET", "/api/assists", Some("u-from-another-hub"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(v["error"].as_str().unwrap().contains("unknown user"));
}

#[tokio::test]
async fn a_fresh_hub_is_empty() {
    let app = app().await;
    let (status, users) = call(&app, "GET", "/api/users", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users.as_array().unwrap().len(), 0, "no users ship with the hub");

    let viewer = user(&app, "First").await;
    let (_, assists) = call(&app, "GET", "/api/assists", Some(&viewer), None).await;
    assert_eq!(assists.as_array().unwrap().len(), 0, "no assists ship with the hub");
    let (_, record) = call(&app, "GET", "/api/my-record", Some(&viewer), None).await;
    assert_eq!(record["credits_earned"], 0);
    assert_eq!(record["my_assists"].as_array().unwrap().len(), 0);
}

// ---- Asymmetry rule: help given accumulates, help received never does ----

#[tokio::test]
async fn my_record_has_no_inbound_aggregate() {
    let app = app().await;
    let viewer = user(&app, "Viewer").await;
    let helper = user(&app, "Helper").await;
    let other_owner = user(&app, "Other Owner").await;

    // Inbound: the viewer owns an assist that someone joined.
    let owned = assist(&app, &viewer, json!({ "title": "The viewer's own assist" })).await;
    join(&app, &owned, &helper).await;

    // Outbound: the viewer responded elsewhere and was credited for it.
    let helped = assist(&app, &other_owner, json!({ "title": "Someone else's assist" })).await;
    join(&app, &helped, &viewer).await;
    close(
        &app, &helped, &other_owner,
        json!({ "outcome": "resolved", "credited_user_ids": [&viewer], "record": {} }),
    ).await;

    let (status, v) = call(&app, "GET", "/api/my-record", Some(&viewer), None).await;
    assert_eq!(status, StatusCode::OK);

    let keys: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    let mut expected = vec![
        "user", "credits_earned", "responses_count", "records_count",
        "my_assists", "credits_rows", "ai_usage",
    ];
    let mut got = keys.clone();
    expected.sort();
    got.sort();
    assert_eq!(got, expected, "my-record must expose exactly the outbound fields");

    fn assert_no_inbound_keys(v: &Value, path: &str) {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    let lk = k.to_lowercase();
                    assert!(
                        !lk.contains("received") && !lk.contains("inbound") && !lk.contains("helped"),
                        "inbound-aggregate-looking key '{k}' at {path}"
                    );
                    assert_no_inbound_keys(child, &format!("{path}.{k}"));
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    assert_no_inbound_keys(child, &format!("{path}[{i}]"));
                }
            }
            _ => {}
        }
    }
    assert_no_inbound_keys(&v, "my-record");

    // Inbound help appears only as names per assist, never as a number.
    let owned_row = v["my_assists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role"] == "owner")
        .expect("the viewer owns an assist");
    assert_eq!(owned_row["responder_names"], json!(["Helper"]));

    // Outbound accumulates.
    assert_eq!(v["credits_earned"], 1);
    assert_eq!(v["responses_count"], 1);
}

#[tokio::test]
async fn my_record_invents_no_ai_usage() {
    let app = app().await;
    let viewer = user(&app, "Owner").await;
    let (status, v) = call(&app, "GET", "/api/my-record", Some(&viewer), None).await;
    assert_eq!(status, StatusCode::OK);
    // Nothing measures token spend yet (the detector is P1), so the hub
    // reports nothing. Numbers here are never fabricated.
    assert_eq!(v["ai_usage"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_shows_responder_names_never_counts() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let first = user(&app, "First Responder").await;
    let second = user(&app, "Second Responder").await;
    let ref_ = assist(&app, &owner, json!({ "title": "Two responders joined" })).await;
    join(&app, &ref_, &first).await;
    join(&app, &ref_, &second).await;

    let (status, v) = call(&app, "GET", "/api/assists", Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    for row in v.as_array().unwrap() {
        assert!(row["responder_names"].is_array());
        for key in row.as_object().unwrap().keys() {
            assert!(!key.to_lowercase().contains("count"), "count-like key '{key}'");
        }
    }
    // Join order, by name, never a count.
    let row = v.as_array().unwrap().iter().find(|r| r["ref"] == ref_.as_str()).unwrap();
    assert_eq!(row["responder_names"], json!(["First Responder", "Second Responder"]));
}

// ---- Close flow: a resolution record on EVERY close ----

#[tokio::test]
async fn close_writes_record_even_when_abandoned() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let ref_ = assist(&app, &owner, json!({
        "title": "Migration deadlocks only under the test harness",
        "environment": ["Postgres 16", "Rust 1.98"]
    })).await;

    let body = json!({ "outcome": "abandoned", "credited_user_ids": [], "record": {} });
    let (status, v) = call(&app, "POST", &format!("/api/assists/{ref_}/close"), Some(&owner), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "done");

    let (status, rec) = call(&app, "GET", &format!("/api/assists/{ref_}/record"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rec["outcome"], "abandoned");
    // Empty fields were server-drafted, not left blank.
    assert!(!rec["symptom"].as_str().unwrap().is_empty());
    assert!(!rec["env_fingerprint"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn close_records_credits_for_responders_only() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Credit only the people who helped").await;
    let bystander = user(&app, "Bystander").await;

    close(&app, &ref_, &owner, json!({
        "outcome": "resolved",
        "credited_user_ids": [&responder, &bystander],
        "record": { "fix": "cap the harness pool at 4" }
    })).await;

    let (_, credited) = call(&app, "GET", "/api/my-record", Some(&responder), None).await;
    assert_eq!(credited["credits_earned"], 1);
    let (_, uncredited) = call(&app, "GET", "/api/my-record", Some(&bystander), None).await;
    assert_eq!(uncredited["credits_earned"], 0, "a non-responder cannot be credited");
}

#[tokio::test]
async fn close_is_owner_only_and_single_shot() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Only the owner closes").await;
    let body = json!({ "outcome": "resolved", "credited_user_ids": [], "record": {} });

    let (status, _) =
        call(&app, "POST", &format!("/api/assists/{ref_}/close"), Some(&responder), Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) =
        call(&app, "POST", &format!("/api/assists/{ref_}/close"), Some(&owner), Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) =
        call(&app, "POST", &format!("/api/assists/{ref_}/close"), Some(&owner), Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---- List filters ----

#[tokio::test]
async fn list_filters() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let other = user(&app, "Other").await;

    let tagged = assist(&app, &owner, json!({
        "title": "Image pull fails on staging", "tags": ["kubernetes", "helm"]
    })).await;
    let plain = assist(&app, &owner, json!({ "title": "Vite build OOMs in CI", "tags": ["ci"] })).await;
    let closed = assist(&app, &owner, json!({ "title": "gRPC stream closes at 60s" })).await;
    close(&app, &closed, &owner, json!({ "outcome": "resolved", "credited_user_ids": [], "record": {} })).await;
    let theirs = assist(&app, &other, json!({ "title": "Owned by someone else" })).await;
    join(&app, &theirs, &owner).await;

    let (_, v) = call(&app, "GET", "/api/assists", Some(&owner), None).await;
    assert_eq!(v.as_array().unwrap().len(), 4);

    let (_, v) = call(&app, "GET", "/api/assists?status=open", Some(&owner), None).await;
    assert_eq!(v.as_array().unwrap().len(), 3);

    let (_, v) = call(&app, "GET", "/api/assists?status=done", Some(&owner), None).await;
    let refs: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    assert_eq!(refs, vec![closed.as_str()]);

    // A CSV of statuses is a union; nothing sets 'dormant' yet, so it adds
    // no rows (see the dormancy note in the status enum).
    let (_, v) = call(&app, "GET", "/api/assists?status=open,dormant", Some(&owner), None).await;
    assert_eq!(v.as_array().unwrap().len(), 3);

    let (_, v) = call(&app, "GET", "/api/assists?tag=kubernetes", Some(&owner), None).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["ref"], tagged.as_str());

    // mine = owner or responder, newest first.
    let (_, v) = call(&app, "GET", "/api/assists?mine=true", Some(&owner), None).await;
    let refs: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    assert_eq!(refs, vec![theirs.as_str(), closed.as_str(), plain.as_str(), tagged.as_str()]);
    let (_, v) = call(&app, "GET", "/api/assists?mine=true", Some(&other), None).await;
    let refs: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    assert_eq!(refs, vec![theirs.as_str()]);
}

#[tokio::test]
async fn list_orders_newest_first_past_ten_assists() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    // Refs are 'S-<n>' with no padding, so nothing about the listing may
    // depend on them sorting as text. (The same-millisecond ref tiebreak in
    // the query cannot be reached through the API: every create lands on its
    // own timestamp. It is numeric for correctness, not because this covers
    // it.)
    let mut refs = Vec::new();
    for i in 1..=11 {
        refs.push(assist(&app, &owner, json!({ "title": format!("Assist {i}") })).await);
    }
    assert_eq!(refs[0], "S-1");
    assert_eq!(refs[10], "S-11");

    let (_, v) = call(&app, "GET", "/api/assists", Some(&owner), None).await;
    let listed: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    let expected: Vec<&str> = refs.iter().rev().map(|s| s.as_str()).collect();
    assert_eq!(listed, expected);
}

// ---- Scope request lifecycle, including live_debug ----

#[tokio::test]
async fn scope_request_lifecycle() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Pool sizing in the harness").await;
    let outsider = user(&app, "Outsider").await;

    // A non-member cannot request anything.
    let req = json!({ "kind": "file", "target": "src/db.rs", "reason": "check pool sizing" });
    let (status, _) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&outsider), Some(req.clone()),
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, created) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&responder), Some(req),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["status"], "pending");
    let id = created["id"].as_i64().unwrap();

    // Only the owner decides.
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/approve"), Some(&outsider), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, approved) = call(&app, "POST", &format!("/api/scope-requests/{id}/approve"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(approved["status"], "approved");

    // Deciding twice fails.
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/deny"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The grant is now derived and visible on the detail.
    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&responder), None).await;
    let grants = detail["grants"].as_array().unwrap();
    assert!(grants.iter().any(|g| g["scope_request_id"] == id && g["kind"] == "file"));
}

#[tokio::test]
async fn live_debug_request_and_approval_unlocks_grant() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Quicker to trace this together").await;

    let id = request_scope(
        &app, &ref_, &responder,
        json!({ "kind": "live_debug", "reason": "quicker to trace this together" }),
    ).await;

    // Pending is not a grant.
    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&owner), None).await;
    assert!(!detail["grants"].as_array().unwrap().iter().any(|g| g["kind"] == "live_debug"));

    approve(&app, id, &owner).await;

    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&responder), None).await;
    assert!(detail["grants"].as_array().unwrap().iter()
        .any(|g| g["kind"] == "live_debug" && g["granted_to_id"] == responder.as_str()));
}

#[tokio::test]
async fn catalog_and_ssh_key_flow() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Inspect the harness box").await;

    // Owner publishes what their engine sees; responders read it.
    let catalog = json!({ "items": [
        { "id": "w-4410", "kind": "window", "label": "Google Chrome",
          "detail": "Grafana - payments", "pid": null },
        { "id": "a-claude", "kind": "ai_agent", "label": "Claude Code",
          "detail": "agent session active", "pid": 3229 }
    ]});
    let (status, _) = call(&app, "POST", &format!("/api/assists/{ref_}/catalog"), Some(&responder), Some(catalog.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN); // responders cannot publish
    let (status, _) = call(&app, "POST", &format!("/api/assists/{ref_}/catalog"), Some(&owner), Some(catalog)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&responder), None).await;
    assert_eq!(detail["catalog"].as_array().unwrap().len(), 2);
    assert!(detail["catalog_at"].is_string());

    // The responder's ssh request carries their public key; the owner's
    // approval supplies the connection target, which lands on the grant.
    let (status, created) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&responder),
        Some(json!({
            "kind": "ssh",
            "reason": "need to inspect the harness box",
            "payload": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA responder@laptop",
            "ttl_minutes": 240
        })),
    ).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_i64().unwrap();
    assert!(created["payload"].as_str().unwrap().starts_with("ssh-ed25519"));

    let (status, approved) = call(
        &app, "POST", &format!("/api/scope-requests/{id}/approve"), Some(&owner),
        Some(json!({ "target": "owner@build-host.local" })),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(approved["target"], "owner@build-host.local");

    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&responder), None).await;
    let grant = detail["grants"].as_array().unwrap().iter()
        .find(|g| g["kind"] == "ssh").expect("ssh grant");
    assert_eq!(grant["target"], "owner@build-host.local");
}

#[tokio::test]
async fn window_grants_frames_and_revoke() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Show me the dashboard you see").await;
    let outsider = user(&app, "Outsider").await;

    async fn frame_put(app: &Router, user: &str, path: &str, bytes: Vec<u8>) -> StatusCode {
        let request = Request::builder()
            .method("PUT")
            .uri(path)
            .header("x-user-id", user)
            .header("content-type", "image/jpeg")
            .body(Body::from(bytes))
            .unwrap();
        app.clone().oneshot(request).await.unwrap().status()
    }

    let id = request_scope(&app, &ref_, &responder, json!({
        "kind": "window",
        "target": "w-771|Google Chrome: Grafana - payments",
        "reason": "want to see the dashboard you see"
    })).await;

    // No frames before the grant exists.
    let frame_path = format!("/api/assists/{ref_}/frames/{id}");
    assert_eq!(frame_put(&app, &owner, &frame_path, vec![1, 2, 3]).await, StatusCode::BAD_REQUEST);

    approve(&app, id, &owner).await;

    // Only the owner uploads; frames are size-capped.
    assert_eq!(frame_put(&app, &responder, &frame_path, vec![1]).await, StatusCode::FORBIDDEN);
    // Oversized frames are rejected by axum's default 2MB body limit.
    assert_eq!(
        frame_put(&app, &owner, &frame_path, vec![0; 3 * 1024 * 1024]).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(frame_put(&app, &owner, &frame_path, vec![9, 9, 9]).await, StatusCode::OK);

    // The grant holder reads the frame; other users do not.
    let request = Request::builder()
        .method("GET").uri(&frame_path).header("x-user-id", &responder)
        .body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.as_ref(), &[9, 9, 9]);
    let (status, _) = call(&app, "GET", &frame_path, Some(&outsider), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Revoke: owner-only, one click; grant and frame disappear, responder
    // is notified with "revoked ... access".
    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    let cursor = boot["now"].as_str().unwrap().to_string();
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/revoke"), Some(&responder), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, revoked) = call(&app, "POST", &format!("/api/scope-requests/{id}/revoke"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked["status"], "revoked");

    let (_, detail) = call(&app, "GET", &format!("/api/assists/{ref_}"), Some(&responder), None).await;
    assert!(detail["grants"].as_array().unwrap().iter().all(|g| g["kind"] != "window"));
    let (status, _) = call(&app, "GET", &frame_path, Some(&responder), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // no active grant anymore
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/revoke"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // cannot revoke twice

    settle().await;
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={cursor}"), Some(&responder), None,
    ).await;
    assert!(v["notifications"].as_array().unwrap().iter().any(|n| {
        let m = n["message"].as_str().unwrap();
        m.contains("revoked your application window view") && m.ends_with("access")
    }));
}

// ---- Create and join ----

#[tokio::test]
async fn create_assist_persists_artifacts_and_tags() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let body = json!({
        "title": "Webpack chunk hashes differ between two identical builds",
        "tags": ["ci", "build"],
        "category": "broken",
        "anonymous": false,
        "description": "Two builds of the same commit produce different chunk hashes.",
        "insights": "",
        "environment": ["Node 20", "webpack 5"],
        "artifacts": [
            { "id": "a1", "kind": "ai_agent", "label": "Claude Code", "detail": "agent session active",
              "icon": "data:image/png;base64,AAA", "pid": 4211 },
            { "id": "f1", "kind": "file", "label": "webpack.config.js", "detail": "repo root" }
        ]
    });
    let (status, v) = call(&app, "POST", "/api/assists", Some(&owner), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["ref"], "S-1", "the first assist on a fresh hub");
    let artifacts = v["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 2);
    // Icon and pid persist for the Shared Artifacts list; absent stays null.
    let agent = artifacts.iter().find(|a| a["id"] == "a1").unwrap();
    assert_eq!(agent["icon"], "data:image/png;base64,AAA");
    assert_eq!(agent["pid"], 4211);
    let file = artifacts.iter().find(|a| a["id"] == "f1").unwrap();
    assert!(file["icon"].is_null());
    assert!(file["pid"].is_null());
    assert_eq!(v["insights"], "");
    assert_eq!(v["description"], "Two builds of the same commit produce different chunk hashes.");
    assert_eq!(v["tags"], json!(["build", "ci"]));
    assert_eq!(v["status"], "open");

    let (status, _) = call(&app, "POST", "/api/assists", Some(&owner), Some(json!({ "title": "  " }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_assist_rules_and_cascade() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let responder = user(&app, "Responder").await;
    let other_owner = user(&app, "Other").await;

    // A closed assist where the owner earned a credit as a responder.
    let credited = assist(&app, &other_owner, json!({ "title": "Closed with a credit" })).await;
    join(&app, &credited, &owner).await;
    close(&app, &credited, &other_owner, json!({
        "outcome": "resolved", "credited_user_ids": [&owner], "record": { "fix": "set idle_timeout to 0s" }
    })).await;

    // An open assist with responders, requests, tags and artifacts.
    let open = assist(&app, &owner, json!({
        "title": "Open with everything hanging off it",
        "tags": ["postgres"],
        "artifacts": [{ "id": "f1", "kind": "file", "label": "schema.sql", "detail": "repo root" }]
    })).await;
    join(&app, &open, &responder).await;
    request_scope(&app, &open, &responder, json!({
        "kind": "file", "target": "schema.sql", "reason": "want to see the lock order"
    })).await;

    // Only the owner deletes.
    let (status, _) = call(&app, "DELETE", &format!("/api/assists/{open}"), Some(&responder), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Deleting a closed assist also removes its resolution record and the
    // credits it granted.
    let (_, before) = call(&app, "GET", "/api/my-record", Some(&owner), None).await;
    assert_eq!(before["credits_earned"], 1);
    let (status, _) = call(&app, "DELETE", &format!("/api/assists/{credited}"), Some(&other_owner), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "GET", &format!("/api/assists/{credited}/record"), Some(&other_owner), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, after) = call(&app, "GET", "/api/my-record", Some(&owner), None).await;
    assert_eq!(after["credits_earned"], 0);

    let (status, v) = call(&app, "DELETE", &format!("/api/assists/{open}"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "deleted");

    let (status, _) = call(&app, "GET", &format!("/api/assists/{open}"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = call(&app, "GET", "/api/assists", Some(&owner), None).await;
    assert_eq!(list.as_array().unwrap().len(), 0);

    // Nothing dangling: my-record and notifications still work for a
    // responder of the deleted assist.
    let (status, v) = call(&app, "GET", "/api/my-record", Some(&responder), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["my_assists"].as_array().unwrap().iter().all(|r| r["ref"] != open.as_str()));
    let (status, _) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn join_rules() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let responder = user(&app, "Responder").await;
    let open = assist(&app, &owner, json!({ "title": "Anyone but the owner may join" })).await;

    let (status, v) = call(&app, "POST", &format!("/api/assists/{open}/responders"), Some(&responder), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["viewer_is_responder"], true);

    let (status, _) = call(&app, "POST", &format!("/api/assists/{open}/responders"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // owner cannot join

    let closed = assist(&app, &owner, json!({ "title": "Already closed" })).await;
    close(&app, &closed, &owner, json!({ "outcome": "resolved", "credited_user_ids": [], "record": {} })).await;
    let (status, _) = call(&app, "POST", &format!("/api/assists/{closed}/responders"), Some(&responder), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // closed
}

#[tokio::test]
async fn owner_uploads_live_data_snapshot() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let responder = user(&app, "Responder").await;
    let ref_ = assist(&app, &owner, json!({ "title": "A real snapshot" })).await;

    let (_, empty) = call(&app, "GET", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), None).await;
    assert_eq!(empty["files"].as_object().unwrap().len(), 0);

    let snapshot = json!({
        "file_tree": [{ "name": "app.yaml", "path": "/work/app.yaml", "children": [] }],
        "files": { "/work/app.yaml": "replicas: 3" },
        "agent_chat": []
    });
    // Only the owner may upload.
    let (status, _) = call(&app, "POST", &format!("/api/assists/{ref_}/artifacts"), Some(&responder), Some(snapshot.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, "POST", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), Some(snapshot)).await;
    assert_eq!(status, StatusCode::OK);

    // A responder now sees the snapshot.
    join(&app, &ref_, &responder).await;
    let (_, v) = call(&app, "GET", &format!("/api/assists/{ref_}/artifacts"), Some(&responder), None).await;
    assert_eq!(v["files"]["/work/app.yaml"], "replicas: 3");
    assert_eq!(v["file_tree"][0]["name"], "app.yaml");

    // Replace semantics: a second upload fully supersedes the first.
    call(&app, "POST", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), Some(json!({
        "file_tree": [], "files": {}, "agent_chat": []
    }))).await;
    let (_, v) = call(&app, "GET", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), None).await;
    assert_eq!(v["files"].as_object().unwrap().len(), 0);

    // Closed assists reject uploads.
    close(&app, &ref_, &owner, json!({ "outcome": "resolved", "credited_user_ids": [], "record": {} })).await;
    let (status, _) = call(&app, "POST", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), Some(json!({
        "file_tree": [], "files": {}, "agent_chat": []
    }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn live_data_requires_membership() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let outsider = user(&app, "Outsider").await;
    let ref_ = assist(&app, &owner, json!({ "title": "Shared only with members" })).await;
    call(&app, "POST", &format!("/api/assists/{ref_}/artifacts"), Some(&owner), Some(json!({
        "file_tree": [{ "name": "deployment.yaml", "path": "k8s/deployment.yaml", "children": [] }],
        "files": { "k8s/deployment.yaml": "replicas: 3" },
        "agent_chat": []
    }))).await;

    let (status, _) = call(&app, "GET", &format!("/api/assists/{ref_}/artifacts"), Some(&outsider), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    join(&app, &ref_, &outsider).await;
    let (status, v) = call(&app, "GET", &format!("/api/assists/{ref_}/artifacts"), Some(&outsider), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["files"].as_object().unwrap().contains_key("k8s/deployment.yaml"));
}

// ---- Insights drafting fallback ----

#[tokio::test]
async fn draft_brief_without_api_key_invents_nothing() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let body = json!({
        "title": "Rollout stuck on image pull",
        "description": "The pod never becomes ready.",
        "artifacts": [
            { "id": "a1", "kind": "ai_agent", "label": "Claude Code", "detail": "running - /work/payments" },
            { "id": "f1", "kind": "file", "label": "deployment.yaml", "detail": "k8s/payments - ref a3f9c1" }
        ]
    });
    let (status, v) = call(&app, "POST", "/api/assists/draft-brief", Some(&owner), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    // No AI -> empty draft, never fabricated content. The UI shows N/A.
    assert_eq!(v["insights"], "");
    assert_eq!(v["environment"].as_array().unwrap().len(), 0);
    assert!(v.get("failures").is_none());
    assert!(v.get("goal").is_none());
}

// ---- User registration (per-machine identity) ----

#[tokio::test]
async fn register_user_creates_distinct_ids() {
    let app = app().await;
    let (status, first) = call(&app, "POST", "/api/users", None, Some(json!({ "name": "Ira K." }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["id"], "u-ira-k");
    assert_eq!(first["initials"], "IK");

    let (status, second) = call(&app, "POST", "/api/users", None, Some(json!({ "name": "Ira K." }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], "u-ira-k-2");

    let (status, _) = call(&app, "POST", "/api/users", None, Some(json!({ "name": "   " }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The new user works as an identity, and both appear in the picker.
    let (status, v) = call(&app, "GET", "/api/assists", Some("u-ira-k"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v.as_array().unwrap().len(), 0);
    let (_, users) = call(&app, "GET", "/api/users", None, None).await;
    assert_eq!(users.as_array().unwrap().len(), 2);
}

// ---- Notifications: the owner/responder event loop ----

#[tokio::test]
async fn notifications_cover_request_decision_join_and_credit() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let responder = user(&app, "Responder").await;
    let ref_ = assist(&app, &owner, json!({ "title": "Image pull fails on staging" })).await;

    // Bootstrap cursors (no `since` = no backlog).
    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&owner), None).await;
    assert_eq!(boot["notifications"].as_array().unwrap().len(), 0);
    let owner_cursor = boot["now"].as_str().unwrap().to_string();
    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    let responder_cursor = boot["now"].as_str().unwrap().to_string();

    // The responder joins and requests live debug.
    join(&app, &ref_, &responder).await;
    let request_id = request_scope(
        &app, &ref_, &responder,
        json!({ "kind": "live_debug", "reason": "quicker to trace this together" }),
    ).await;
    settle().await;

    // The owner is notified of the join and the request.
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={owner_cursor}"), Some(&owner), None,
    ).await;
    let kinds: Vec<&str> = v["notifications"]
        .as_array().unwrap().iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"responder_joined"));
    assert!(kinds.contains(&"scope_requested"));
    let owner_cursor = v["now"].as_str().unwrap().to_string();

    // The owner approves; the requester is notified.
    approve(&app, request_id, &owner).await;
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={responder_cursor}"), Some(&responder), None,
    ).await;
    let decided: Vec<&Value> = v["notifications"]
        .as_array().unwrap().iter().filter(|n| n["kind"] == "scope_decided").collect();
    assert_eq!(decided.len(), 1);
    assert!(decided[0]["message"].as_str().unwrap().contains("approved"));
    assert_eq!(decided[0]["assist_ref"], ref_.as_str());

    // The cursor advances: polling again shows no repeats of old events.
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={owner_cursor}"), Some(&owner), None,
    ).await;
    assert!(v["notifications"]
        .as_array().unwrap().iter().all(|n| n["kind"] != "responder_joined"));

    // Closing with credit notifies the credited responder.
    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    let responder_cursor = boot["now"].as_str().unwrap().to_string();
    close(&app, &ref_, &owner, json!({
        "outcome": "resolved", "credited_user_ids": [&responder], "record": {}
    })).await;
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={responder_cursor}"), Some(&responder), None,
    ).await;
    assert!(v["notifications"].as_array().unwrap().iter().any(|n| n["kind"] == "credited"));
}

#[tokio::test]
async fn conversation_flows_both_ways() {
    let app = app().await;
    let (owner, responder, ref_) = owner_responder_assist(&app, "Pool bumped to 4 on a branch").await;
    let outsider = user(&app, "Outsider").await;

    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    let responder_cursor = boot["now"].as_str().unwrap().to_string();

    // The owner comments on their own assist.
    let (status, comment) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&owner),
        Some(json!({ "kind": "comment", "reason": "pushed a branch with the pool bumped to 4" })),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment["status"], "approved"); // comments auto-approve

    // A non-member cannot comment; the owner cannot request scopes.
    let (status, _) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&outsider),
        Some(json!({ "kind": "comment", "reason": "drive-by" })),
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&owner),
        Some(json!({ "kind": "file", "target": "src/db.rs", "reason": "why not" })),
    ).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Responders are notified of the owner's comment; the owner is not
    // notified of their own.
    settle().await;
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={responder_cursor}"), Some(&responder), None,
    ).await;
    let comments: Vec<&Value> = v["notifications"]
        .as_array().unwrap().iter().filter(|n| n["kind"] == "comment").collect();
    assert_eq!(comments.len(), 1);
    assert!(comments[0]["message"].as_str().unwrap().contains("pool bumped to 4"));

    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={responder_cursor}"), Some(&owner), None,
    ).await;
    assert!(v["notifications"].as_array().unwrap().iter().all(|n| n["kind"] != "comment"));
}

#[tokio::test]
async fn notifications_do_not_echo_your_own_actions() {
    let app = app().await;
    let (_owner, responder, ref_) = owner_responder_assist(&app, "Checking the pool config").await;

    let (_, boot) = call(&app, "GET", "/api/notifications", Some(&responder), None).await;
    let cursor = boot["now"].as_str().unwrap().to_string();

    // The responder comments; she must not be notified of it herself.
    call(
        &app, "POST", &format!("/api/assists/{ref_}/scope-requests"), Some(&responder),
        Some(json!({ "kind": "comment", "reason": "checking the pool config" })),
    ).await;
    let (_, v) = call(
        &app, "GET", &format!("/api/notifications?since={cursor}"), Some(&responder), None,
    ).await;
    // Comments auto-approve; that must not surface as a decision either.
    assert_eq!(v["notifications"].as_array().unwrap().len(), 0);
}

// ---- Record draft prefill ----

#[tokio::test]
async fn record_draft_prefills_from_brief_and_scopes() {
    let app = app().await;
    let owner = user(&app, "Owner").await;
    let responder = user(&app, "Responder").await;
    let ref_ = assist(&app, &owner, json!({
        "title": "Migration deadlocks only under the test harness",
        "environment": ["Postgres 16", "sqlx 0.9"],
        "artifacts": [{ "id": "f1", "kind": "file", "label": "0007_orders.sql", "detail": "repo root" }]
    })).await;
    join(&app, &ref_, &responder).await;
    let id = request_scope(&app, &ref_, &responder, json!({
        "kind": "file", "target": "db/0007_orders.sql", "reason": "want to see the lock order"
    })).await;
    approve(&app, id, &owner).await;

    let (status, v) = call(&app, "GET", &format!("/api/assists/{ref_}/record-draft"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    // The title is the symptom until the detector captures real failures.
    assert!(v["symptom"].as_str().unwrap().contains("Migration deadlocks"));
    assert!(v["env_fingerprint"].as_str().unwrap().contains("Postgres 16"));
    // The approved file scope and the artifact shared at open both show up.
    let scopes = v["scopes_that_mattered"].as_str().unwrap();
    assert!(scopes.contains("db/0007_orders.sql"));
    assert!(scopes.contains("shared at open: 0007_orders.sql"));
}
