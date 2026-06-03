use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::Agent;

/// A single step in a session chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainStep {
    pub agent: Agent,
    /// Prompt template. Use `{prev_output}` to inject the previous step's output.
    pub prompt: String,
}

/// A named chain of session steps that run sequentially.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionChain {
    pub name: String,
    pub steps: Vec<ChainStep>,
}

/// Runtime state for an actively executing chain.
#[derive(Clone, Debug)]
pub struct ActiveChain {
    pub chain_name: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub workspace_path: PathBuf,
    /// Extracted output from the previous step, substituted into `{prev_output}`.
    pub prev_output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_step_prompt_substitution() {
        let prompt = "Review:\n{prev_output}".to_string();
        let result = prompt.replace("{prev_output}", "some output text");
        assert_eq!(result, "Review:\nsome output text");
    }

    #[test]
    fn test_chain_step_prompt_no_placeholder() {
        let prompt = "Fix all bugs".to_string();
        let result = prompt.replace("{prev_output}", "ignored");
        assert_eq!(result, "Fix all bugs");
    }

    #[test]
    fn test_session_chain_serialization() {
        let chain = SessionChain {
            name: "review-chain".into(),
            steps: vec![
                ChainStep { agent: Agent::Claude, prompt: "Implement X".into() },
                ChainStep { agent: Agent::Codex, prompt: "Review:\n{prev_output}".into() },
            ],
        };
        let json = serde_json::to_string(&chain).unwrap();
        let parsed: SessionChain = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "review-chain");
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].agent, Agent::Claude);
        assert_eq!(parsed.steps[1].agent, Agent::Codex);
    }

    #[test]
    fn test_config_with_chains_deserialization() {
        let json = r#"{
            "workspaces": [],
            "chains": [
                {
                    "name": "build-review",
                    "steps": [
                        {"agent": "Claude", "prompt": "Build the project"},
                        {"agent": "Codex", "prompt": "Review:\n{prev_output}"}
                    ]
                }
            ]
        }"#;
        let config: crate::types::Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.chains.len(), 1);
        assert_eq!(config.chains[0].name, "build-review");
        assert_eq!(config.chains[0].steps.len(), 2);
    }

    #[test]
    fn test_config_without_chains_default() {
        let json = r#"{"workspaces": []}"#;
        let config: crate::types::Config = serde_json::from_str(json).unwrap();
        assert!(config.chains.is_empty());
    }

    #[test]
    fn test_active_chain_advancement() {
        let mut chain = ActiveChain {
            chain_name: "test".into(),
            current_step: 0,
            total_steps: 3,
            workspace_path: PathBuf::from("/tmp/test"),
            prev_output: None,
        };
        assert!(chain.current_step + 1 < chain.total_steps);
        chain.current_step += 1;
        chain.prev_output = Some("output1".into());
        assert_eq!(chain.current_step, 1);
        assert!(chain.current_step + 1 < chain.total_steps);
        chain.current_step += 1;
        assert_eq!(chain.current_step, 2);
        assert!(chain.current_step + 1 >= chain.total_steps); // last step done
    }
}
