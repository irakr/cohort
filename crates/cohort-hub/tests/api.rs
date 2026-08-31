//! Integration tests against the full router with an in-memory database.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use cohort_hub::{build_router, config::Config, db, seed};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        bind: "127.0.0.1:0".into(),
        db: "sqlite::memory:".into(),
        allowed_origins: vec!["http://localhost:1420".into()],
        anthropic_api_key: None,
        anthropic_model: "claude-sonnet-5".into(),
    }
}

async fn app() -> Router {
    let config = test_config();
    let pool = db::pool(&config.db).await.expect("pool");
    seed::seed(&pool).await.expect("seed");
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

// ---- Asymmetry rule: help given accumulates, help received never does ----

#[tokio::test]
async fn my_record_has_no_inbound_aggregate() {
    let app = app().await;
    let (status, v) = call(&app, "GET", "/api/my-record", Some("u-alex"), None).await;
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
    let owned = v["my_assists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role"] == "owner")
        .expect("alex owns an assist");
    assert!(owned["responder_names"].is_array());

    // Outbound accumulates: seeded credit from Anika.
    assert_eq!(v["credits_earned"], 1);
    assert_eq!(v["responses_count"], 1);
}

#[tokio::test]
async fn list_shows_responder_names_never_counts() {
    let app = app().await;
    let (status, v) = call(&app, "GET", "/api/assists", Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);
    for row in v.as_array().unwrap() {
        assert!(row["responder_names"].is_array());
        for key in row.as_object().unwrap().keys() {
            assert!(!key.to_lowercase().contains("count"), "count-like key '{key}'");
        }
    }
    let s2409 = v.as_array().unwrap().iter().find(|r| r["ref"] == "S-2409").unwrap();
    let names: Vec<&str> = s2409["responder_names"]
        .as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
    assert_eq!(names, vec!["Priya", "Arun"]);
}

// ---- Close flow: a resolution record on EVERY close ----

#[tokio::test]
async fn close_writes_record_even_when_abandoned() {
    let app = app().await;
    let body = json!({ "outcome": "abandoned", "credited_user_ids": [], "record": {} });
    let (status, v) = call(&app, "POST", "/api/assists/S-2409/close", Some("u-alex"), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "done");

    let (status, rec) = call(&app, "GET", "/api/assists/S-2409/record", Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rec["outcome"], "abandoned");
    // Empty fields were server-drafted, not left blank.
    assert!(!rec["symptom"].as_str().unwrap().is_empty());
    assert!(!rec["env_fingerprint"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn close_records_credits_for_responders_only() {
    let app = app().await;
    let body = json!({
        "outcome": "resolved",
        "credited_user_ids": ["u-priya", "u-devansh"],  // devansh is not a responder
        "record": { "fix": "cap the harness pool at 4" }
    });
    let (status, _) = call(&app, "POST", "/api/assists/S-2409/close", Some("u-alex"), Some(body)).await;
    assert_eq!(status, StatusCode::OK);

    let (_, priya) = call(&app, "GET", "/api/my-record", Some("u-priya"), None).await;
    assert_eq!(priya["credits_earned"], 1);
    let (_, devansh) = call(&app, "GET", "/api/my-record", Some("u-devansh"), None).await;
    assert_eq!(devansh["credits_earned"], 0);
}

#[tokio::test]
async fn close_is_owner_only_and_single_shot() {
    let app = app().await;
    let body = json!({ "outcome": "resolved", "credited_user_ids": [], "record": {} });
    let (status, _) =
        call(&app, "POST", "/api/assists/S-2411/close", Some("u-priya"), Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) =
        call(&app, "POST", "/api/assists/S-2409/close", Some("u-alex"), Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) =
        call(&app, "POST", "/api/assists/S-2409/close", Some("u-alex"), Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---- List filters ----

#[tokio::test]
async fn list_filters() {
    let app = app().await;
    let (_, v) = call(&app, "GET", "/api/assists", Some("u-alex"), None).await;
    assert_eq!(v.as_array().unwrap().len(), 4);

    let (_, v) = call(&app, "GET", "/api/assists?status=open", Some("u-alex"), None).await;
    assert_eq!(v.as_array().unwrap().len(), 2);

    let (_, v) = call(&app, "GET", "/api/assists?status=open,dormant", Some("u-alex"), None).await;
    assert_eq!(v.as_array().unwrap().len(), 3);

    let (_, v) = call(&app, "GET", "/api/assists?tag=kubernetes", Some("u-alex"), None).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["ref"], "S-2411");

    // mine = owner or responder
    let (_, v) = call(&app, "GET", "/api/assists?mine=true", Some("u-alex"), None).await;
    let refs: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    assert_eq!(refs, vec!["S-2409", "S-2398"]);
    let (_, v) = call(&app, "GET", "/api/assists?mine=true", Some("u-priya"), None).await;
    let refs: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["ref"].as_str().unwrap()).collect();
    assert_eq!(refs, vec!["S-2409"]);
}

// ---- Scope request lifecycle, including live_debug ----

#[tokio::test]
async fn scope_request_lifecycle() {
    let app = app().await;

    // Devansh is not a responder on S-2409.
    let req = json!({ "kind": "file", "target": "src/db.rs", "reason": "check pool sizing" });
    let (status, _) = call(
        &app, "POST", "/api/assists/S-2409/scope-requests", Some("u-devansh"), Some(req.clone()),
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Priya (responder) requests a file scope.
    let (status, created) = call(
        &app, "POST", "/api/assists/S-2409/scope-requests", Some("u-priya"), Some(req),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["status"], "pending");
    let id = created["id"].as_i64().unwrap();

    // Only the owner decides.
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/approve"), Some("u-arun"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, approved) = call(&app, "POST", &format!("/api/scope-requests/{id}/approve"), Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(approved["status"], "approved");

    // Deciding twice fails.
    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/deny"), Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The grant is now derived and visible on the detail.
    let (_, detail) = call(&app, "GET", "/api/assists/S-2409", Some("u-priya"), None).await;
    let grants = detail["grants"].as_array().unwrap();
    assert!(grants.iter().any(|g| g["scope_request_id"] == id && g["kind"] == "file"));
}

#[tokio::test]
async fn live_debug_request_and_approval_unlocks_grant() {
    let app = app().await;
    // Seeded pending live_debug from Priya on S-2409.
    let (_, detail) = call(&app, "GET", "/api/assists/S-2409", Some("u-alex"), None).await;
    let pending = detail["scope_requests"].as_array().unwrap().iter()
        .find(|r| r["kind"] == "live_debug" && r["status"] == "pending")
        .expect("seeded live_debug request");
    let id = pending["id"].as_i64().unwrap();
    assert!(!detail["grants"].as_array().unwrap().iter().any(|g| g["kind"] == "live_debug"));

    let (status, _) = call(&app, "POST", &format!("/api/scope-requests/{id}/approve"), Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);

    let (_, detail) = call(&app, "GET", "/api/assists/S-2409", Some("u-priya"), None).await;
    assert!(detail["grants"].as_array().unwrap().iter()
        .any(|g| g["kind"] == "live_debug" && g["granted_to_id"] == "u-priya"));
}

// ---- Create and join ----

#[tokio::test]
async fn create_assist_persists_artifacts_and_tags() {
    let app = app().await;
    let body = json!({
        "title": "Webpack chunk hashes differ between two identical builds",
        "tags": ["ci", "build"],
        "category": "broken",
        "anonymous": false,
        "goal": "Make builds reproducible.",
        "failures": [{ "label": "diff dist/ between runs", "note": "exit 1" }],
        "environment": ["Node 20", "webpack 5"],
        "artifacts": [
            { "id": "t1", "kind": "terminal", "label": "iTerm2", "detail": "npm run build" },
            { "id": "f1", "kind": "file", "label": "webpack.config.js", "detail": "repo root" }
        ]
    });
    let (status, v) = call(&app, "POST", "/api/assists", Some("u-meera"), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["ref"], "S-2412");
    assert_eq!(v["artifacts"].as_array().unwrap().len(), 2);
    assert_eq!(v["tags"], json!(["build", "ci"]));
    assert_eq!(v["status"], "open");

    let (status, _) = call(&app, "POST", "/api/assists", Some("u-meera"), Some(json!({ "title": "  " }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn join_rules() {
    let app = app().await;
    let (status, v) = call(&app, "POST", "/api/assists/S-2411/responders", Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["viewer_is_responder"], true);

    let (status, _) = call(&app, "POST", "/api/assists/S-2411/responders", Some("u-meera"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // owner cannot join
    let (status, _) = call(&app, "POST", "/api/assists/S-2398/responders", Some("u-priya"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST); // closed
}

#[tokio::test]
async fn live_data_requires_membership() {
    let app = app().await;
    let (status, _) = call(&app, "GET", "/api/assists/S-2411/artifacts", Some("u-priya"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    call(&app, "POST", "/api/assists/S-2411/responders", Some("u-priya"), None).await;
    let (status, v) = call(&app, "GET", "/api/assists/S-2411/artifacts", Some("u-priya"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["files"].as_object().unwrap().contains_key("k8s/payments/deployment.yaml"));
    assert!(!v["terminal_feed"].as_array().unwrap().is_empty());
}

// ---- Brief drafting fallback ----

#[tokio::test]
async fn draft_brief_falls_back_without_api_key() {
    let app = app().await;
    let body = json!({
        "title": "Rollout stuck on image pull",
        "artifacts": [
            { "id": "t1", "kind": "terminal", "label": "iTerm2 (payments)", "detail": "kubectl" },
            { "id": "f1", "kind": "file", "label": "deployment.yaml", "detail": "k8s/payments - ref a3f9c1" }
        ]
    });
    let (status, v) = call(&app, "POST", "/api/assists/draft-brief", Some("u-meera"), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["goal"].as_str().unwrap().contains("Rollout stuck on image pull"));
    assert!(!v["failures"].as_array().unwrap().is_empty());
    assert!(!v["environment"].as_array().unwrap().is_empty());
}

// ---- Record draft prefill ----

#[tokio::test]
async fn record_draft_prefills_from_brief_and_scopes() {
    let app = app().await;
    let (status, v) = call(&app, "GET", "/api/assists/S-2409/record-draft", Some("u-alex"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["symptom"].as_str().unwrap().contains("cargo test"));
    assert!(v["env_fingerprint"].as_str().unwrap().contains("Postgres 16"));
    // Seeded approved file scope shows up.
    assert!(v["scopes_that_mattered"].as_str().unwrap().contains("migrations/0007_orders.sql"));
}
