use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::env;
use std::time::Duration;

use axum::{routing::get, Router, Json};
use tokio::time::sleep;

use unicron_core::*;
use unicron_wal::Wal;
use hardware_monitor::HardwareSnapshot;
use execution_kernel::*;

struct ResearchProject;
impl ProjectInterface for ResearchProject {
    fn id(&self) -> ProjectId { "RESEARCH".to_string() }
    fn generate_tasks(&self) -> Vec<TaskSpec> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::HeavyCompute);

        vec![TaskSpec {
            project_id: self.id(),
            required_capabilities: caps,
            compute_profile: ComputeProfile {
                estimated_cpu_cost: 40.0,
                parallelizable: true,
                max_effective_units: 4,
            },
            capital_impact: CapitalImpactEstimate { expected_return_score: 2.0 },
            urgency: UrgencyLevel { urgency_score: 3.0 },
            manual_weight: 0.0,
        }]
    }
}

struct TradingProject;
impl ProjectInterface for TradingProject {
    fn id(&self) -> ProjectId { "TRADING".to_string() }
    fn generate_tasks(&self) -> Vec<TaskSpec> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::HeavyCompute);

        vec![TaskSpec {
            project_id: self.id(),
            required_capabilities: caps,
            compute_profile: ComputeProfile {
                estimated_cpu_cost: 30.0,
                parallelizable: false,
                max_effective_units: 1,
            },
            capital_impact: CapitalImpactEstimate { expected_return_score: 8.0 },
            urgency: UrgencyLevel { urgency_score: 9.0 },
            manual_weight: 0.0,
        }]
    }
}

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
    let mut state = wal.replay();

    if mode == "CONTROL" {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::HeavyCompute);
        caps.insert(Capability::ExecutionPrimary);
        caps.insert(Capability::CommandAuthority);

        for id in ["MEGATRON", "OPTIMUS", "STARSCREAM", "BEE"] {
            wal.append_event(
                &mut state,
                GovernanceEvent::RegisterNode(Node {
                    id: id.to_string(),
                    role: NodeRole::Follower,
                    health: NodeHealth::Healthy,
                    capabilities: caps.clone(),
                })
            );
        }
    }

    let cluster_state = Arc::new(Mutex::new(state));

    let app_state = AppState {
        cluster: cluster_state.clone(),
        node_id: node_id.clone(),
        mode: mode.clone(),
    };

    let local = tokio::task::LocalSet::new();

    local.spawn_local(async move {

        let mut kernel = ExecutionKernel::new();
        kernel.register_project(Box::new(ResearchProject));
        kernel.register_project(Box::new(TradingProject));

        loop {
            let hw = HardwareSnapshot::collect();

            if mode == "CONTROL" {
                let execution_mode = ExecutionMode {
                    trading_active: true,
                    pi_mode: PiMode::Base6,
                    optimus_reserve_units: 4,
                };

                let events = kernel.execute_cycle(&hw, &execution_mode);

                let mut locked = cluster_state.lock().unwrap();
                let mut wal = Wal::new(PathBuf::from("/cybertron_wal/unicron.log"));

                for event in events {
                    wal.append_event(&mut locked, event);
                }
            }

            {
                let mut locked = cluster_state.lock().unwrap();
                locked.schedule();
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

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

    let app = Router::new()
        .route("/status", get(status_handler))
        .with_state(app_state);

    println!("HTTP API running on 0.0.0.0:8080");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    local.run_until(async {
        axum::serve(listener, app)
            .await
            .unwrap();
    }).await;
}
