use std::collections::{BTreeMap, BTreeSet};
use serde::{Serialize, Deserialize};
use titan::{CapitalState, CapitalEvent, PropConstraints};

pub type NodeId = String;
pub type JobId = String;
pub type Term = u64;
pub type AccountId = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    HeavyCompute,
    ExecutionPrimary,
    AlwaysOn,
    CommandAuthority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealth {
    Healthy,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafeMode {
    Normal,
    Restricted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Assigned,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FirmId {
    MFF,
    TOPSTEP,
    APEX,
    TRADEDAY,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub account_id: AccountId,
    pub assigned_node: Option<NodeId>,
    pub preferred_node: Option<NodeId>,
    pub required_capabilities: BTreeSet<Capability>,
    pub status: JobStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub role: NodeRole,
    pub health: NodeHealth,
    pub capabilities: BTreeSet<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Leadership {
    pub current_term: Term,
    pub leader_id: Option<NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub firm: FirmId,
    pub size: u64,
    pub cycle: u32,
    pub capital: CapitalState,
    pub safe_mode: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountRegistry {
    pub accounts: BTreeMap<AccountId, Account>,
    pub global_serial: u64,
}

impl AccountRegistry {

    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
            global_serial: 0,
        }
    }

    fn generate_account_id(
        &mut self,
        firm: &FirmId,
        size: u64,
        cycle: u32,
    ) -> AccountId {

        self.global_serial += 1;

        format!(
            "{:?}-{:03}K-{:04}-{:06}",
            firm,
            size / 1000,
            cycle,
            self.global_serial
        )
    }

    pub fn create_account(
        &mut self,
        firm: FirmId,
        size: u64,
        cycle: u32,
        constraints: PropConstraints,
    ) -> AccountId {

        let id = self.generate_account_id(&firm, size, cycle);

        let mut capital = CapitalState::new();
        capital.apply(CapitalEvent::Initialize(constraints));

        let account = Account {
            id: id.clone(),
            firm,
            size,
            cycle,
            capital,
            safe_mode: false,
        };

        self.accounts.insert(id.clone(), account);

        id
    }

    pub fn apply_capital_event(
        &mut self,
        account_id: &AccountId,
        event: CapitalEvent,
    ) {
        if let Some(account) = self.accounts.get_mut(account_id) {
            account.capital.apply(event);

            if account.capital.safe_mode {
                account.safe_mode = true;
            }
        }
    }

    pub fn is_account_active(&self, account_id: &AccountId) -> bool {
        self.accounts
            .get(account_id)
            .map(|a| !a.safe_mode)
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterState {
    pub nodes: BTreeMap<NodeId, Node>,
    pub jobs: BTreeMap<JobId, Job>,
    pub leadership: Leadership,
    pub safe_mode: SafeMode,
    pub accounts: AccountRegistry,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GovernanceEvent {
    RegisterNode(Node),
    UpdateHealth { node_id: NodeId, health: NodeHealth },
    CreateJob {
        job_id: JobId,
        account_id: AccountId,
        required: BTreeSet<Capability>,
        preferred_node: Option<NodeId>,
    },
    AccountCreated {
        firm: FirmId,
        size: u64,
        cycle: u32,
        constraints: PropConstraints,
    },
    AccountCapitalEvent {
        account_id: AccountId,
        event: CapitalEvent,
    },
    ForceLeader { node_id: NodeId },
}

impl ClusterState {

    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            jobs: BTreeMap::new(),
            leadership: Leadership {
                current_term: 0,
                leader_id: None,
            },
            safe_mode: SafeMode::Normal,
            accounts: AccountRegistry::new(),
        }
    }

    fn elect_leader(&mut self) {

        let mut eligible: Vec<String> = self.nodes
            .values()
            .filter(|n| n.health == NodeHealth::Healthy)
            .filter(|n| n.capabilities.contains(&Capability::CommandAuthority))
            .map(|n| n.id.clone())
            .collect();

        if eligible.is_empty() {
            self.leadership.leader_id = None;
            return;
        }

        eligible.sort();

        let new_leader = eligible[0].clone();

        self.leadership.current_term += 1;
        self.leadership.leader_id = Some(new_leader.clone());

        for node in self.nodes.values_mut() {
            node.role = if node.id == new_leader {
                NodeRole::Leader
            } else {
                NodeRole::Follower
            };
        }
    }

    pub fn apply(&mut self, event: GovernanceEvent) {

        match event {

            GovernanceEvent::RegisterNode(node) => {
                self.nodes.insert(node.id.clone(), node);
                self.elect_leader();
            }

            GovernanceEvent::UpdateHealth { node_id, health } => {

                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.health = health.clone();
                }

                if health == NodeHealth::Offline {
                    self.elect_leader();
                }
            }

            GovernanceEvent::ForceLeader { node_id } => {

                if self.nodes.contains_key(&node_id) {
                    self.leadership.current_term += 1;
                    self.leadership.leader_id = Some(node_id.clone());

                    for node in self.nodes.values_mut() {
                        node.role = if node.id == node_id {
                            NodeRole::Leader
                        } else {
                            NodeRole::Follower
                        };
                    }
                }
            }

            GovernanceEvent::CreateJob { job_id, account_id, required, preferred_node } => {
                self.jobs.insert(job_id.clone(), Job {
                    id: job_id,
                    account_id,
                    assigned_node: None,
                    preferred_node,
                    required_capabilities: required,
                    status: JobStatus::Queued,
                });
            }

            GovernanceEvent::AccountCreated { firm, size, cycle, constraints } => {
                self.accounts.create_account(firm, size, cycle, constraints);
            }

            GovernanceEvent::AccountCapitalEvent { account_id, event } => {
                self.accounts.apply_capital_event(&account_id, event);
            }
        }
    }

    fn heavy_compute_priority() -> Vec<NodeId> {
        vec![
            "MEGATRON".to_string(),
            "OPTIMUS".to_string(),
            "STARSCREAM".to_string(),
            "BEE".to_string(),
        ]
    }

    pub fn schedule(&mut self) {

        if self.safe_mode != SafeMode::Normal {
            return;
        }

        let leader = match &self.leadership.leader_id {
            Some(id) => id.clone(),
            None => return,
        };

        let leader_node = match self.nodes.get(&leader) {
            Some(node) => node,
            None => return,
        };

        if leader_node.role != NodeRole::Leader {
            return;
        }

        for job in self.jobs.values_mut() {

            if job.status != JobStatus::Queued {
                continue;
            }

            if !self.accounts.is_account_active(&job.account_id) {
                continue;
            }

            if let Some(pref) = &job.preferred_node {
                if let Some(node) = self.nodes.get(pref) {
                    if node.health == NodeHealth::Healthy
                        && job.required_capabilities.iter().all(|c| node.capabilities.contains(c))
                    {
                        job.assigned_node = Some(node.id.clone());
                        job.status = JobStatus::Assigned;
                        continue;
                    }
                }
            }

            let mut candidate_nodes: Vec<&Node> = self.nodes
                .values()
                .filter(|n| n.health == NodeHealth::Healthy)
                .filter(|n| job.required_capabilities.iter().all(|c| n.capabilities.contains(c)))
                .collect();

            if job.required_capabilities.contains(&Capability::HeavyCompute) {
                let priority = Self::heavy_compute_priority();
                candidate_nodes.sort_by_key(|n| {
                    priority.iter().position(|id| id == &n.id).unwrap_or(usize::MAX)
                });
            }

            if let Some(node) = candidate_nodes.first() {
                job.assigned_node = Some(node.id.clone());
                job.status = JobStatus::Assigned;
            }
        }
    }
}
