use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

use axum::{routing::get, Router, Json, extract::Path};
use tokio::time::sleep;
use reqwest::Client;
use tonic::transport::Server;

use unicron_core::*;
use unicron_wal::Wal;

mod replication;
pub mod wal {
    tonic::include_proto!("wal");
}

use replication::ReplicationService;
use wal::wal_replication_server::WalReplicationServer;

#[derive(Clone)]
struct AppState {
    cluster: Arc<Mutex<ClusterState>>,
    node_id: String,
    mode: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {

    let node_id = env::var("CYBERTRON_NODE_ID")
        .unwrap_or("UNKNOWN".to_string());

    let mode = env::var("CYBERTRON_MODE")
        .unwrap_or("REPLICA".to_string());

    println!("Starting UNICRON on {} in {} mode", node_id, mode);

    let wal_path = PathBuf::from("/cybertron_wal/unicron.log");
    let mut wal = Wal::new(wal_path.clone());
    let state = wal.replay();

    let cluster_state = Arc::new(Mutex::new(state));

    // ===== LIVENESS EVALUATOR LOOP =====
    {
        let cluster = cluster_state.clone();
        tokio::spawn(async move {
            loop {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                {
                    let mut locked = cluster.lock().unwrap();
                    locked.evaluate_liveness(now);
                }

                sleep(Duration::from_secs(1)).await;
            }
        });
    }

    // ===== OUTBOUND HEARTBEAT LOOP =====
    {
        let client = Client::new();
        let node_clone = node_id.clone();

        tokio::spawn(async move {

            let peers = [
                "100.111.195.96",
                "100.71.121.15",
                "100.110.206.128",
            ];

            loop {
                for peer in peers {
                    let url = format!(
                        "http://{}:8080/heartbeat/{}",
                        peer, node_clone
                    );

                    let _ = client.get(&url).send().await;
                }

                sleep(Duration::from_secs(1)).await;
            }
        });
    }

    // ===== START gRPC SERVER (CONTROL ONLY) =====
    if mode == "CONTROL" {

        let replication_service = ReplicationService {
            cluster: cluster_state.clone(),
        };

        let grpc_addr = "0.0.0.0:50051".parse().unwrap();

        tokio::spawn(async move {
            println!("gRPC replication server running on 0.0.0.0:50051");

            Server::builder()
                .add_service(
                    WalReplicationServer::new(replication_service)
                )
                .serve(grpc_addr)
                .await
                .unwrap();
        });
    }

    async fn heartbeat_handler(
        Path(sender): Path<String>,
        state: axum::extract::State<AppState>
    ) -> Json<String> {

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut locked = state.cluster.lock().unwrap();
        locked.record_heartbeat(&sender, now);

        Json(format!("Heartbeat recorded from {}", sender))
    }

    async fn status_handler(
        state: axum::extract::State<AppState>
    ) -> Json<String> {
        let locked = state.cluster.lock().unwrap();
        Json(format!(
            "Node: {} | Mode: {} | Leader: {:?} | Jobs: {}",
            state.node_id,
            state.mode,
            locked.leadership.leader_id,
            locked.jobs.len()
        ))
    }

    let app_state = AppState {
        cluster: cluster_state.clone(),
        node_id: node_id.clone(),
        mode: mode.clone(),
    };

    let app = Router::new()
        .route("/status", get(status_handler))
        .route("/heartbeat/:node", get(heartbeat_handler))
        .with_state(app_state);

    println!("HTTP API running on 0.0.0.0:8080");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    axum::serve(listener, app)
        .await
        .unwrap();
}
