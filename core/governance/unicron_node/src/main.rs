use std::collections::BTreeSet;
use std::path::PathBuf;

use unicron_core::*;
use unicron_wal::Wal;
use hardware_monitor::HardwareSnapshot;
use execution_kernel::*;
use titan::PropConstraints;

struct ResearchProject;
struct TradingProject;

impl ProjectInterface for ResearchProject {
    fn id(&self) -> ProjectId { "RESEARCH".to_string() }

    fn generate_tasks(&self) -> Vec<TaskSpec> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::HeavyCompute);

        vec![TaskSpec{
            project_id: self.id(),
            required_capabilities: caps,
            compute_profile: ComputeProfile{
                estimated_cpu_cost: 40.0,
                parallelizable: true,
                max_effective_units: 6,
            },
            capital_impact: CapitalImpactEstimate{ expected_return_score: 2.0 },
            urgency: UrgencyLevel{ urgency_score: 3.0 },
            manual_weight: 0.0,
        }]
    }
}

impl ProjectInterface for TradingProject {
    fn id(&self) -> ProjectId { "TRADING".to_string() }

    fn generate_tasks(&self) -> Vec<TaskSpec> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::HeavyCompute);

        vec![TaskSpec{
            project_id: self.id(),
            required_capabilities: caps,
            compute_profile: ComputeProfile{
                estimated_cpu_cost: 30.0,
                parallelizable: false,
                max_effective_units: 1,
            },
            capital_impact: CapitalImpactEstimate{ expected_return_score: 8.0 },
            urgency: UrgencyLevel{ urgency_score: 9.0 },
            manual_weight: 0.0,
        }]
    }
}

fn main() {

    let wal_path = PathBuf::from("/cybertron_wal/unicron.log");
    let mut wal = Wal::new(wal_path);
    let mut state = wal.replay();

    // -------- CONTROL PLANE NODE --------
    let mut control_caps = BTreeSet::new();
    control_caps.insert(Capability::CommandAuthority);
    control_caps.insert(Capability::ExecutionPrimary);

    wal.append_event(
        &mut state,
        GovernanceEvent::RegisterNode(Node{
            id: "UNICRON".to_string(),
            role: NodeRole::Follower,
            health: NodeHealth::Healthy,
            capabilities: control_caps,
        })
    );

    // -------- COMPUTE NODES --------
    let mut compute_caps = BTreeSet::new();
    compute_caps.insert(Capability::HeavyCompute);

    for id in ["MEGATRON", "OPTIMUS", "STARSCREAM", "BEE"] {
        wal.append_event(
            &mut state,
            GovernanceEvent::RegisterNode(Node{
                id: id.to_string(),
                role: NodeRole::Follower,
                health: NodeHealth::Healthy,
                capabilities: compute_caps.clone(),
            })
        );
    }

    println!("Leader after registration: {:?}", state.leadership.leader_id);

    // Create Account
    wal.append_event(
        &mut state,
        GovernanceEvent::AccountCreated{
            firm: FirmId::MFF,
            size: 50_000,
            cycle: 1,
            constraints: PropConstraints{
                starting_balance: 50_000.0,
                trailing_drawdown: 2_000.0,
                daily_loss_limit: 1_000.0,
                profit_target: 3_000.0,
            },
        }
    );

    let account_id = state.accounts.accounts.keys().next().unwrap().clone();

    let hw = HardwareSnapshot::collect();

    let execution_mode = ExecutionMode{
        trading_active: true,
        pi_mode: PiMode::Base6,
        optimus_reserve_units: 4,
    };

    let mut kernel = ExecutionKernel::new();
    kernel.register_project(Box::new(ResearchProject));
    kernel.register_project(Box::new(TradingProject));

    let events = kernel.execute_cycle(&hw, &execution_mode);

    for event in events {
        if let GovernanceEvent::CreateJob { job_id, required, preferred_node, .. } = event {
            wal.append_event(
                &mut state,
                GovernanceEvent::CreateJob{
                    job_id,
                    account_id: account_id.clone(),
                    required,
                    preferred_node,
                }
            );
        }
    }

    state.schedule();

    println!("\nAssigned Jobs:");
    for (id, job) in state.jobs.iter() {
        println!(
            "{} → {:?} pref={:?} assigned={:?}",
            id,
            job.status,
            job.preferred_node,
            job.assigned_node
        );
    }
}
