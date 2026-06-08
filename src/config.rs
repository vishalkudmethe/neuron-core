//! Configuration Manager for token budgeting and profiles (Neuron.toml).

use std::path::Path;

#[derive(Debug, Clone)]
pub struct NeuronConfig {
    pub profile: String,
    pub token_cap: usize,
    pub max_granularity: bool,
    pub include_evolution_ledger: bool,
}

impl Default for NeuronConfig {
    fn default() -> Self {
        Self {
            profile: "antigravity".to_string(),
            token_cap: 150000,
            max_granularity: true,
            include_evolution_ledger: true,
        }
    }
}

impl NeuronConfig {
    /// Load project-specific settings from `Neuron.toml` at the project root.
    /// Uses manual parsing to avoid external TOML dependency, keeping build times zero-cost.
    pub async fn load(project_root: &Path) -> Self {
        let config_path = project_root.join("Neuron.toml");
        if !config_path.exists() {
            return Self::default();
        }

        let contents = match tokio::fs::read_to_string(&config_path).await {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        let mut config = Self::default();
        for line in contents.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim().trim_matches('"').trim_matches('\'').trim();
                match key {
                    "profile" => {
                        config.profile = val.to_string();
                        // Adjust default limits based on selected profile preset
                        match val {
                            "antigravity" => {
                                config.token_cap = 250000;
                                config.max_granularity = true;
                                config.include_evolution_ledger = true;
                            }
                            "claude" => {
                                config.token_cap = 100000;
                                config.max_granularity = true;
                                config.include_evolution_ledger = false;
                            }
                            "openai" => {
                                config.token_cap = 30000;
                                config.max_granularity = false;
                                config.include_evolution_ledger = false;
                            }
                            _ => {}
                        }
                    }
                    "token_cap" => {
                        if let Ok(cap) = val.parse::<usize>() {
                            config.token_cap = cap;
                        }
                    }
                    "max_granularity" => {
                        if let Ok(b) = val.parse::<bool>() {
                            config.max_granularity = b;
                        }
                    }
                    "include_evolution_ledger" => {
                        if let Ok(b) = val.parse::<bool>() {
                            config.include_evolution_ledger = b;
                        }
                    }
                    _ => {}
                }
            }
        }
        config
    }
}
