use serde::{Serialize, Deserialize};
use hardware_monitor::HardwareSnapshot;
use unicron_core::{Capability, GovernanceEvent};
use std::collections::{BTreeSet, BTreeMap};

pub type ProjectId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeProfile {
    pub estimated_cpu_cost: f32,
    pub parallelizable: bool,
    pub max_effective_units: usize,
}

impl ComputeProfile {
    pub fn marginal_multiplier(unit_index: usize) -> f32 {
        1.0 / (1.0 + unit_index as f32)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalImpactEstimate {
    pub expected_return_score: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UrgencyLevel {
    pub urgency_score: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSpec {
    pub project_id: ProjectId,
    pub required_capabilities: BTreeSet<Capability>,
    pub compute_profile: ComputeProfile,
    pub capital_impact: CapitalImpactEstimate,
    pub urgency: UrgencyLevel,
    pub manual_weight: f32,
}

#[derive(Clone, Debug)]
pub struct PriorityWeights {
    pub capital_weight: f32,
    pub urgency_weight: f32,
    pub compute_penalty_weight: f32,
    pub manual_weight: f32,
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            capital_weight: 1.0,
            urgency_weight: 1.0,
            compute_penalty_weight: 1.0,
            manual_weight: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PriorityController {
    pub weights: PriorityWeights,
}

impl PriorityController {
    pub fn score(
        &self,
        task: &TaskSpec,
        unit_index: usize,
        available_capacity_percent: f32,
    ) -> f32 {

        let marginal_multiplier =
            ComputeProfile::marginal_multiplier(unit_index);

        let capital_component =
            self.weights.capital_weight
                * task.capital_impact.expected_return_score
                * marginal_multiplier;

        let urgency_component =
            self.weights.urgency_weight
                * task.urgency.urgency_score;

        let compute_penalty =
            self.weights.compute_penalty_weight
                * (task.compute_profile.estimated_cpu_cost
                    / available_capacity_percent.max(1.0));

        let manual_component =
            self.weights.manual_weight
                * task.manual_weight;

        capital_component
            + urgency_component
            - compute_penalty
            + manual_component
    }
}

pub trait ProjectInterface {
    fn id(&self) -> ProjectId;
    fn generate_tasks(&self) -> Vec<TaskSpec>;
}

#[derive(Clone, Debug)]
pub enum PiMode {
    Base6,
    Burst8,
    Emergency10,
}

#[derive(Clone, Debug)]
pub struct ExecutionMode {
    pub trading_active: bool,
    pub pi_mode: PiMode,
    pub optimus_reserve_units: usize,
}

#[derive(Clone, Debug)]
pub struct NodeComputePolicy {
    pub total_units: usize,
}

pub struct ExecutionKernel {
    pub projects: Vec<Box<dyn ProjectInterface>>,
    pub priority_controller: PriorityController,
}

impl ExecutionKernel {

    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            priority_controller: PriorityController {
                weights: PriorityWeights::default(),
            },
        }
    }

    pub fn register_project(&mut self, project: Box<dyn ProjectInterface>) {
        self.projects.push(project);
    }

    fn build_node_policies(
        &self,
        mode: &ExecutionMode,
    ) -> BTreeMap<String, NodeComputePolicy> {

        let mut map = BTreeMap::new();

        // Locked ceilings
        map.insert("MEGATRON".to_string(), NodeComputePolicy { total_units: 20 });
        map.insert("OPTIMUS".to_string(), NodeComputePolicy { total_units: 20 });
        map.insert("STARSCREAM".to_string(), NodeComputePolicy { total_units: 18 });

        let bee_units = match mode.pi_mode {
            PiMode::Base6 => 6,
            PiMode::Burst8 => 8,
            PiMode::Emergency10 => 10,
        };

        map.insert("BEE".to_string(), NodeComputePolicy { total_units: bee_units });

        map
    }

    pub fn execute_cycle(
        &self,
        hw: &HardwareSnapshot,
        mode: &ExecutionMode,
    ) -> Vec<GovernanceEvent> {

        let mut tasks: Vec<TaskSpec> = Vec::new();
        for project in &self.projects {
            tasks.extend(project.generate_tasks());
        }

        let policies = self.build_node_policies(mode);

        let mut node_available: BTreeMap<String, usize> = BTreeMap::new();
        for (node, policy) in policies.iter() {

            let mut units = policy.total_units;

            if node == "OPTIMUS" && mode.trading_active {
                units = units.saturating_sub(mode.optimus_reserve_units);
            }

            node_available.insert(node.clone(), units);
        }

        // allocation[(task_index, node_id)] = units
        let mut allocation: BTreeMap<(usize, String), usize> = BTreeMap::new();

        loop {
            let mut best_score = f32::MIN;
            let mut best_choice: Option<(usize, String)> = None;

            for (task_index, task) in tasks.iter().enumerate() {
                for (node, &available_units) in node_available.iter() {

                    if available_units == 0 {
                        continue;
                    }

                    let current_units =
                        *allocation.get(&(task_index, node.clone()))
                            .unwrap_or(&0);

                    if !task.compute_profile.parallelizable && current_units >= 1 {
                        continue;
                    }

                    if current_units >= task.compute_profile.max_effective_units {
                        continue;
                    }

                    let score = self.priority_controller.score(
                        task,
                        current_units,
                        hw.available_compute_capacity(),
                    );

                    // deterministic tie-break: (score, project_id, node_id)
                    if score > best_score
                        || (score == best_score
                            && (task.project_id.clone(), node.clone())
                                < (
                                    tasks[best_choice
                                        .as_ref()
                                        .map(|(i, _)| *i)
                                        .unwrap_or(task_index)]
                                        .project_id.clone(),
                                    best_choice
                                        .as_ref()
                                        .map(|(_, n)| n.clone())
                                        .unwrap_or(node.clone()),
                                ))
                    {
                        best_score = score;
                        best_choice = Some((task_index, node.clone()));
                    }
                }
            }

            if let Some((task_index, node)) = best_choice {

                if best_score <= 0.0 {
                    break;
                }

                *allocation.entry((task_index, node.clone())).or_insert(0) += 1;
                *node_available.get_mut(&node).unwrap() -= 1;

            } else {
                break;
            }
        }

        let mut events = Vec::new();
        let mut job_counter = 0;

        for ((task_index, node), units) in allocation {
            let task = &tasks[task_index];

            for _ in 0..units {
                events.push(
                    GovernanceEvent::CreateJob {
                        job_id: format!("JOB-{}", job_counter),
                        account_id: "SYSTEM".to_string(),
                        required: task.required_capabilities.clone(),
                        preferred_node: Some(node.clone()),
                    }
                );
                job_counter += 1;
            }
        }

        events
    }
}
