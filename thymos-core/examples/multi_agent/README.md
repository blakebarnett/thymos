# Multi-Agent Coordination Example

This example demonstrates multiple agents coordinating through shared memory and events. It shows how Thymos enables multi-agent scenarios with both private and shared state.

## Scenario: Research Team

Multiple research agents collaborating on a shared knowledge base:

- **Researcher Agent**: Gathers information, stores in shared memory
- **Analyzer Agent**: Reads shared memories, generates insights
- **Coordinator Agent**: Monitors relevance, adjusts agent priorities

## Running the Example

### Prerequisites

1. Start a Locai server on `localhost:3000`:
   ```bash
   locai-server
   ```

2. Run the example:
   ```bash
   cargo run --example multi_agent
   ```

## Architecture

### Agent Types

#### Researcher Agent
- Conducts research on a specific topic
- Stores findings in **shared memory** (visible to all)
- Stores confidence notes in **private memory** (agent-specific)

#### Analyzer Agent
- Reads from **shared memory** to find research findings
- Generates insights from the findings
- Stores insights back in **shared memory**

#### Coordinator Agent
- Monitors all agents
- Evaluates relevance for each agent
- Can adjust priorities based on relevance scores

### Memory Flow

```
Researcher 1 ──┐
               ├──> Shared Memory ──> Analyzer ──> Shared Memory
Researcher 2 ──┘                              │
                                              └──> Coordinator
```

## Example Output

```
Multi-Agent Research Team Example
==================================

Creating agents...
✓ All agents created

Phase 1: Research
------------------
✓ Researcher 1 completed research
✓ Researcher 2 completed research

Phase 2: Analysis
------------------
✓ Analyzer generated 1 insights
  - Analysis of 2 findings reveals interesting patterns (confidence: 0.90)

Phase 3: Coordination
----------------------
Coordinating 3 agents...
  Agent researcher_1 relevance: 0.60 (Listening)
  Agent researcher_2 relevance: 0.60 (Listening)
  Agent analyzer relevance: 0.60 (Listening)

Memory Demonstration
--------------------
Shared Memory (visible to all):
  - Finding about quantum computing: Research finding about quantum computing...
  - Finding about machine learning: Research finding about machine learning...
  - Insight: Analysis of 2 findings reveals interesting patterns

Private Memory (researcher_1 only):
  - Research confidence for quantum computing: 0.85

✓ Multi-agent coordination example complete!
```

## Docker Setup

See `docker-compose.multi-agent.yml` for a Docker Compose setup that runs multiple agents with a shared Locai server.

## Adapting for Other Scenarios

To adapt this example for other multi-agent scenarios:

1. **Update agent types**: Modify `researcher.rs`, `analyzer.rs`, and `coordinator.rs` for your domain
2. **Update coordination logic**: Change how agents interact and coordinate
3. **Update relevance evaluation**: Modify `coordinator.rs` to use domain-specific relevance logic

## See Also

- [Hybrid Memory Mode Example](../hybrid_mode.rs)
- [Zera NPC Example](../zera_npc/main.rs)
- [Thymos Documentation](../../../README.md)


