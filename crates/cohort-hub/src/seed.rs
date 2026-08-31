//! Idempotent seed mirroring the P0 prototype's fake data, so the app boots
//! looking like the design. Runs only when the users table is empty.

use crate::db::now_rfc3339;
use crate::domain::{ChatMsg, FileNode, LiveData};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;

fn ago(minutes: i64) -> String {
    (Utc::now() - Duration::minutes(minutes)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn node(name: &str, path: &str, children: Vec<FileNode>) -> FileNode {
    FileNode { name: name.into(), path: path.into(), children }
}

fn k8s_live_data() -> LiveData {
    let mut files = HashMap::new();
    files.insert(
        "k8s/payments/deployment.yaml".to_string(),
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: payments-api\n  namespace: payments\nspec:\n  replicas: 3\n  template:\n    spec:\n      containers:\n        - name: payments-api\n          image: registry.internal:5000/payments-api:1.9.4\n          ports:\n            - containerPort: 8080\n      # imagePullSecrets: (none at this ref)".to_string(),
    );
    files.insert(
        "k8s/payments/kustomization.yaml".to_string(),
        "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - deployment.yaml\n  - service.yaml\nimages:\n  - name: payments-api\n    newTag: \"1.9.4\"".to_string(),
    );
    files.insert(
        "charts/payments/values.yaml".to_string(),
        "image:\n  repository: registry.internal:5000/payments-api\n  tag: \"1.9.4\"\nimagePullSecrets: []\nresources:\n  limits:\n    memory: 512Mi".to_string(),
    );
    files.insert(
        "helmfile.yaml".to_string(),
        "releases:\n  - name: payments\n    namespace: payments\n    chart: ./charts/payments\n    values:\n      - charts/payments/values.yaml".to_string(),
    );
    files.insert(
        "rollout.log".to_string(),
        "14:22:01 rollout restarted (revision 7)\n14:24:40 progress deadline exceeded\n14:25:02 pod payments-api-7c9f: ImagePullBackOff".to_string(),
    );
    LiveData {
        file_tree: vec![
            node("k8s", "k8s", vec![node("payments", "k8s/payments", vec![
                node("deployment.yaml", "k8s/payments/deployment.yaml", vec![]),
                node("kustomization.yaml", "k8s/payments/kustomization.yaml", vec![]),
            ])]),
            node("charts", "charts", vec![node("payments", "charts/payments", vec![
                node("values.yaml", "charts/payments/values.yaml", vec![]),
            ])]),
            node("helmfile.yaml", "helmfile.yaml", vec![]),
            node("rollout.log", "rollout.log", vec![]),
        ],
        files,
        terminal_tabs: vec!["iTerm2 (payments)".into(), "VS Code (zsh)".into()],
        terminal_feed: vec![
            "$ kubectl rollout status deploy/payments-api".into(),
            "Waiting for deployment rollout to finish: 0 of 3 updated replicas are available...".into(),
            "error: deployment exceeded its progress deadline".into(),
            "$ kubectl get pods -l app=payments-api".into(),
            "NAME                          READY  STATUS             RESTARTS  AGE\npayments-api-7c9f-2xk4d       0/1    ImagePullBackOff   0         6m".into(),
            "$ helm upgrade payments ./charts/payments --dry-run".into(),
            "Release \"payments\" has been upgraded. Happy Helming!".into(),
            "$ kubectl get events --field-selector involvedObject.name=payments-api-7c9f".into(),
            "6m    Warning   Failed      pod/payments-api-7c9f   Error: ImagePullBackOff".into(),
            "$ git diff a3f9c1^ -- charts/payments/values.yaml".into(),
            "-  imagePullSecrets:\n-    - name: regcred\n+  imagePullSecrets: []".into(),
            "$ export REGISTRY_TOKEN=************  (redacted on egress)".into(),
        ],
        agent_chat: vec![
            ChatMsg { who: "Priya".into(), text: "Which registry is the deployment pulling from, and does the pinned values.yaml override it?".into() },
            ChatMsg { who: "Agent - owner machine".into(), text: "deployment.yaml uses image `registry.internal:5000/payments-api:1.9.4`. values.yaml sets imagePullSecrets: []. The secret referenced in the previous revision is absent from the pinned ref.".into() },
        ],
    }
}

fn pg_live_data() -> LiveData {
    let mut files = HashMap::new();
    files.insert(
        "migrations/0007_orders.sql".to_string(),
        "BEGIN;\nALTER TABLE orders ADD COLUMN settled_at timestamptz;\nUPDATE orders SET settled_at = updated_at WHERE state = 'settled';\nCREATE INDEX CONCURRENTLY idx_orders_settled ON orders(settled_at);\nCOMMIT;".to_string(),
    );
    files.insert(
        "src/db.rs".to_string(),
        "let pool = PgPoolOptions::new()\n    .max_connections(2) // harness caps the pool\n    .connect(&url)\n    .await?;".to_string(),
    );
    LiveData {
        file_tree: vec![
            node("migrations", "migrations", vec![node("0007_orders.sql", "migrations/0007_orders.sql", vec![])]),
            node("src", "src", vec![node("db.rs", "src/db.rs", vec![])]),
        ],
        files,
        terminal_tabs: vec!["cargo test (orders)".into()],
        terminal_feed: vec![
            "$ cargo test -p orders -- --test-threads=4".into(),
            "test settle_batch ... FAILED".into(),
            "Error: database error: deadlock detected".into(),
            "DETAIL: Process 4114 waits for ShareLock on transaction 9021; blocked by process 4117.".into(),
        ],
        agent_chat: vec![],
    }
}

pub async fn seed(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    if user_count > 0 {
        return Ok(());
    }

    for (id, name, initials) in [
        ("u-alex", "Alex", "A"),
        ("u-meera", "Meera N.", "M"),
        ("u-priya", "Priya", "P"),
        ("u-arun", "Arun", "A"),
        ("u-devansh", "Devansh R.", "D"),
        ("u-anika", "Anika S.", "A"),
    ] {
        sqlx::query("INSERT INTO users (id, name, initials) VALUES (?, ?, ?)")
            .bind(id).bind(name).bind(initials)
            .execute(pool)
            .await?;
    }

    // (ref, title, status, category, owner, created_at, description, insights, environment, live_data, closed_at)
    // Insights on seeds demo what the Cohort AI integration will produce.
    let assists: Vec<(&str, &str, &str, Option<&str>, &str, String, &str, &str, &str, Option<String>, Option<String>)> = vec![
        (
            "S-2411",
            "Need help with an image pull that keeps failing on staging",
            "open",
            Some("broken"),
            "u-meera",
            ago(22),
            "Trying to get **payments-api 1.9.4** rolled out to `staging` before the release cut. The rollout hangs and the pod never becomes ready.",
            "- intent: ship payments-api 1.9.4 to staging for the checkout release\n- the shared deployment pins image `registry.internal:5000/payments-api:1.9.4` at ref a3f9c1\n- values.yaml at that ref sets `imagePullSecrets: []`; the previous revision referenced `regcred`\n- most plausible direction: restore the pull secret and re-apply",
            r#"["Kubernetes 1.29","Helm 3.14","registry.internal:5000","Linux amd64"]"#,
            Some(serde_json::to_string(&k8s_live_data()).unwrap()),
            None,
        ),
        (
            "S-2409",
            "My migration deadlocks only under the test harness and I'm out of ideas",
            "open",
            Some("broken"),
            "u-alex",
            ago(60),
            "The **orders settlement migration** deadlocks under `cargo test`, but applies cleanly in dev. Only the harness, which caps the pool at 2 connections, hits it.",
            "- intent: land migration 0007 without deadlocking the orders table\n- the shared migration runs `CREATE INDEX CONCURRENTLY` inside a transaction\n- the harness caps the pool at 2 connections, which serializes the lock acquisition\n- most plausible direction: move the index build out of the transaction",
            r#"["Postgres 16","sqlx 0.8","Rust 1.88","Linux amd64"]"#,
            Some(serde_json::to_string(&pg_live_data()).unwrap()),
            None,
        ),
        (
            "S-2404",
            "Can't figure out why our Vite build OOMs in CI but passes locally",
            "dormant",
            Some("environment"),
            "u-devansh",
            ago(180),
            "`vite build` exits 137 on the CI runner; the same commit builds locally in about 90 seconds.",
            "- intent: get the CI build green again\n- exit 137 on a 4 GB runner points at the OOM killer, not the build itself\n- most plausible direction: compare Node heap limits between CI and local",
            r#"["Node 20","Vite 5","CI runner: 4 GB","Linux amd64"]"#,
            None,
            None,
        ),
        (
            "S-2398",
            "Our gRPC stream was closing at exactly 60s behind the proxy",
            "done",
            Some("broken"),
            "u-anika",
            ago(300),
            "Long-lived gRPC streams keep closing at exactly 60 seconds behind the Envoy sidecar.",
            "- intent: keep long-lived gRPC streams open through the sidecar\n- an exact 60s cutoff matches Envoy's default `stream_idle_timeout`\n- most plausible direction: set `idle_timeout: 0s` on the listener",
            r#"["Envoy 1.30","grpc-go 1.64","Kubernetes 1.29"]"#,
            None,
            Some(ago(240)),
        ),
    ];

    for (ref_, title, status, category, owner, created, description, insights, env, live, closed) in assists {
        sqlx::query(
            "INSERT INTO assists (ref, title, status, category, owner_id, anonymous, description, insights, environment, live_data, created_at, closed_at)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ref_).bind(title).bind(status).bind(category).bind(owner)
        .bind(description).bind(insights).bind(env).bind(live).bind(created).bind(closed)
        .execute(pool)
        .await?;
    }

    for (ref_, tags) in [
        ("S-2411", vec!["kubernetes", "helm", "registry-auth"]),
        ("S-2409", vec!["postgres", "rust", "sqlx"]),
        ("S-2404", vec!["ci", "node", "build"]),
        ("S-2398", vec!["networking", "envoy", "grpc"]),
    ] {
        for tag in tags {
            sqlx::query("INSERT INTO assist_tags (assist_ref, tag) VALUES (?, ?)")
                .bind(ref_).bind(tag)
                .execute(pool)
                .await?;
        }
    }

    // Artifacts the owners shared at open.
    for (ref_, id, kind, label, detail) in [
        ("S-2411", "f1", "file", "deployment.yaml", "k8s/payments - ref a3f9c1"),
        ("S-2411", "f2", "file", "kustomization.yaml", "k8s/payments - ref a3f9c1"),
        ("S-2411", "t1", "terminal", "iTerm2 (payments)", "kubectl - read-only stream"),
        ("S-2409", "f1", "file", "0007_orders.sql", "migrations - ref 71bd44"),
        ("S-2409", "t1", "terminal", "cargo test (orders)", "read-only stream"),
        ("S-2409", "a1", "ai_agent", "Claude Code", "agent session - 41 turns"),
    ] {
        sqlx::query(
            "INSERT INTO assist_artifacts (assist_ref, id, kind, label, detail) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(ref_).bind(id).bind(kind).bind(label).bind(detail)
        .execute(pool)
        .await?;
    }

    for (ref_, user, joined) in [
        ("S-2409", "u-priya", ago(41)),
        ("S-2409", "u-arun", ago(36)),
        ("S-2398", "u-alex", ago(290)),
    ] {
        sqlx::query("INSERT INTO responders (assist_ref, user_id, joined_at) VALUES (?, ?, ?)")
            .bind(ref_).bind(user).bind(joined)
            .execute(pool)
            .await?;
    }

    // Scope requests on Alex's own assist, so the owner "Responds" view has
    // data on first boot: one comment, one approved file grant, two pending.
    let rows: Vec<(&str, &str, &str, Option<&str>, &str, &str, Option<i64>, String, Option<String>)> = vec![
        ("S-2409", "u-priya", "comment", None,
         "Seen this before when the harness pins the pool to two connections. Checking the migration's lock order first.",
         "approved", None, ago(40), Some(ago(40))),
        ("S-2409", "u-priya", "file", Some("migrations/0007_orders.sql"),
         "want to see the lock order in that migration", "approved", Some(240), ago(38), Some(ago(35))),
        ("S-2409", "u-arun", "terminal", Some("cargo test (orders)"),
         "need the failing test output, not the summary", "pending", Some(240), ago(30), None),
        ("S-2409", "u-priya", "live_debug", None,
         "quicker to trace this together", "pending", None, ago(12), None),
    ];
    for (ref_, requester, kind, target, reason, status, ttl, created, decided) in rows {
        sqlx::query(
            "INSERT INTO scope_requests (assist_ref, requester_id, kind, target, reason, status, ttl_minutes, created_at, decided_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ref_).bind(requester).bind(kind).bind(target).bind(reason)
        .bind(status).bind(ttl).bind(created).bind(decided)
        .execute(pool)
        .await?;
    }

    // Closed assist: credit + resolution record (outbound record for Alex).
    sqlx::query(
        "INSERT INTO credits (assist_ref, from_owner_id, to_responder_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind("S-2398").bind("u-anika").bind("u-alex").bind(ago(240))
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO resolution_records (assist_ref, outcome, symptom, env_fingerprint, scopes_that_mattered, dead_ends, fix, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("S-2398")
    .bind("resolved")
    .bind("gRPC stream reset at exactly 60s behind the Envoy sidecar; normalised error hash 91bd02")
    .bind("envoy 1.30 - grpc-go 1.64 - k8s 1.29")
    .bind("read:path deploy/envoy.yaml - read:log envoy sidecar")
    .bind("Client keepalive tuning; server-side ping interval")
    .bind("stream_idle_timeout defaulted to 60s. Set idle_timeout: 0s on the listener and re-deploy.")
    .bind(ago(240))
    .execute(pool)
    .await?;

    tracing::info!(at = now_rfc3339(), "seeded database");
    Ok(())
}
