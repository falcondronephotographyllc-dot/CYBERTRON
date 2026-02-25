use std::fs::{OpenOptions, File};
use std::io::{Write, BufRead, BufReader};
use std::path::PathBuf;

use unicron_core::{ClusterState, GovernanceEvent};

pub struct Wal {
    path: PathBuf,
}

impl Wal {

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append_event(
        &mut self,
        state: &mut ClusterState,
        event: GovernanceEvent,
    ) {

        // Apply to in-memory state first
        state.apply(event.clone());

        // Try writing to disk, but never panic
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            if let Ok(serialized) = serde_json::to_string(&event) {
                let _ = writeln!(file, "{}", serialized);
            }
        }
    }

    pub fn replay(&mut self) -> ClusterState {

        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return ClusterState::new(),
        };

        let reader = BufReader::new(file);
        let mut state = ClusterState::new();

        for line in reader.lines() {

            let line = match line {
                Ok(l) => l,
                Err(_) => continue, // skip malformed read
            };

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<GovernanceEvent>(&line) {
                Ok(event) => state.apply(event),
                Err(_) => {
                    // Skip malformed or partial lines safely
                    continue;
                }
            }
        }

        state
    }
}
