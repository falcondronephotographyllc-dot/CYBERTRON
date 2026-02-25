use serde::{Serialize, Deserialize};
use sysinfo::System;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub total_cpu_cores: usize,
    pub average_cpu_usage: f32,
    pub load_average_one: f64,
}

impl HardwareSnapshot {

    pub fn collect() -> Self {

        let mut system = System::new_all();
        system.refresh_all();

        let total_memory_mb = system.total_memory() / 1024;
        let used_memory_mb = system.used_memory() / 1024;

        let cpus = system.cpus();
        let total_cpu_cores = cpus.len();

        let average_cpu_usage =
            if total_cpu_cores > 0 {
                cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
                    / total_cpu_cores as f32
            } else {
                0.0
            };

        let load_average_one = System::load_average().one;

        Self {
            total_memory_mb,
            used_memory_mb,
            total_cpu_cores,
            average_cpu_usage,
            load_average_one,
        }
    }

    pub fn available_compute_capacity(&self) -> f32 {
        100.0 - self.average_cpu_usage
    }

    pub fn memory_pressure(&self) -> f32 {
        if self.total_memory_mb == 0 {
            0.0
        } else {
            self.used_memory_mb as f32 / self.total_memory_mb as f32
        }
    }
}
