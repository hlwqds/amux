# Session Chains

Chains run multiple agent sessions in sequence, passing output from one step into the next.

## When to Use

- **Review loops** — one agent writes code, another reviews it.
- **Multi-agent pipelines** — e.g., Claude implements, Codex tests.
- **Iterative refinement** — feed output back with a different prompt.

## Configuration

Add chains to `config.json` under the `"chains"` array:

```json
{
  "chains": [
    {
      "name": "implement-review",
      "steps": [
        { "agent": "Claude", "prompt": "Implement the feature described in the issue" },
        { "agent": "Codex",  "prompt": "Review the following code for bugs:\n{prev_output}" }
      ]
    }
  ]
}
```

### ChainStep

| Field | Type | Description |
|-------|------|-------------|
| `agent` | `"Claude" \| "Codex" \| "Omp"` | Agent for this step. |
| `prompt` | `string` | Prompt template. Supports `{prev_output}` substitution. |

### SessionChain

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Chain name (shown in the TUI picker). |
| `steps` | `Array<ChainStep>` | Ordered list of agent steps. |

## Template Variables

| Variable | Description |
|----------|-------------|
| `{prev_output}` | Output captured from the previous step. Empty string for the first step. |

Only `{prev_output}` is currently supported. The variable is replaced with the plain-text output extracted from the previous step's session.

## Examples

### Claude writes → Codex reviews

```json
{
  "name": "code-review",
  "steps": [
    { "agent": "Claude", "prompt": "Fix all TODO comments in the codebase" },
    { "agent": "Codex",  "prompt": "Review these changes for correctness and style:\n{prev_output}" }
  ]
}
```

### Three-step: implement → test → review

```json
{
  "name": "full-pipeline",
  "steps": [
    { "agent": "Claude", "prompt": "Implement user authentication" },
    { "agent": "Claude", "prompt": "Write tests for the following implementation:\n{prev_output}" },
    { "agent": "Codex",  "prompt": "Security review this auth implementation:\n{prev_output}" }
  ]
}
```

### GSD planning → Claude execution

```json
{
  "name": "gsd-execute",
  "steps": [
    { "agent": "Omp",    "prompt": "Plan the database migration for adding user preferences" },
    { "agent": "Claude", "prompt": "Execute this plan step by step:\n{prev_output}" }
  ]
}
```
