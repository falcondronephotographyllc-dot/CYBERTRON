use serde::{Serialize, Deserialize};
use std::collections::{BTreeMap, BTreeSet};

pub type NodeId = String;
pub type JobId = String;
pub type AccountId = String;
pub type FirmId = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum NodeHealth {
    Healthy,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    HeavyCompute,
    ExecutionPrimary,
    CommandAuthority,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub role: NodeRole,
    pub health: NodeHealth,
    pub capabilities: BTreeSet<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub assigned_node: Option<NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Leadership {
    pub current_term: u64,
    pub leader_id: Option<NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SafeMode {
    Normal,
    Restricted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountRegistry;

impl AccountRegistry {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterState {
    pub last_heartbeat: BTreeMap<NodeId, u64>,
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
        constraints: String,
    },

    AccountCapitalEvent {
        account_id: AccountId,
        event: String,
    },

    ForceLeader { node_id: NodeId },
}

impl ClusterState {

    pub fn new() -> Self {
        Self {
            last_heartbeat: BTreeMap::new(),
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

    pub fn record_heartbeat(&mut self, node_id: &str, timestamp: u64) {
        self.last_heartbeat.insert(node_id.to_string(), timestamp);
    }

    pub fn evaluate_liveness(&mut self, now: u64) {
        let timeout = 3;
        let mut changed = false;

        for (id, node) in self.nodes.iter_mut() {
            if let Some(last) = self.last_heartbeat.get(id) {
                if now.saturating_sub(*last) > timeout {
                    if node.health != NodeHealth::Offline {
                        node.health = NodeHealth::Offline;
                        changed = true;
                    }
                } else {
                    if node.health != NodeHealth::Healthy {
                        node.health = NodeHealth::Healthy;
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.elect_leader();
        }
    }

    fn elect_leader(&mut self) {

        let priority = ["MEGATRON", "OPTIMUS", "STARSCREAM", "BEE"];

        for id in priority {
            if let Some(node) = self.nodes.get(id) {
                if node.health == NodeHealth::Healthy &&
                   node.capabilities.contains(&Capability::CommandAuthority) {

                    self.leadership.current_term += 1;
                    self.leadership.leader_id = Some(id.to_string());

                    for n in self.nodes.values_mut() {
                        n.role = if n.id == id {
                            NodeRole::Leader
                        } else {
                            NodeRole::Follower
                        };
                    }

                    if id == "BEE" {
                        self.safe_mode = SafeMode::Restricted;
                    } else {
                        self.safe_mode = SafeMode::Normal;
                    }

                    return;
                }
            }
        }

        self.leadership.leader_id = None;
    }

    pub fn apply(&mut self, event: GovernanceEvent) {
        match event {

            GovernanceEvent::RegisterNode(node) => {
                self.nodes.insert(node.id.clone(), node);
                self.elect_leader();
            }

            GovernanceEvent::UpdateHealth { node_id, health } => {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.health = health;
                }
                self.elect_leader();
            }

            GovernanceEvent::ForceLeader { node_id } => {
                if self.nodes.contains_key(&node_id) {
                    self.leadership.current_term += 1;
                    self.leadership.leader_id = Some(node_id.clone());

                    for n in self.nodes.values_mut() {
                        n.role = if n.id == node_id {
                            NodeRole::Leader
                        } else {
                            NodeRole::Follower
                        };
                    }
                }
            }

            _ => {}
        }
    }

    pub fn schedule(&mut self) {}
}
