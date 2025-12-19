# Zera NPC Agent Example

This example demonstrates how to use Thymos to build a Zera-style NPC agent with hybrid memory, concept extraction, relevance evaluation, and lifecycle management.

## Features Demonstrated

- **Hybrid Memory**: Private thoughts vs shared world observations
- **Game-Specific Relevance**: Zera-style relevance evaluation based on game state
- **RPG Concepts**: Character, location, and item concept extraction
- **Personality System**: Trait-based NPC personalities
- **Multi-Agent Coordination**: Multiple NPCs sharing world state

## Running the Example

### Prerequisites

1. Start a Locai server on `localhost:3000`:
   ```bash
   locai-server
   ```

2. Run the example:
   ```bash
   cargo run --example zera_npc
   ```

## Architecture

### Components

- **`npc.rs`**: `ZeraNPC` struct wrapping Thymos `Agent` with game-specific methods
- **`relevance.rs`**: `ZeraRelevanceEvaluator` implementing game-specific relevance logic
- **`concepts.rs`**: RPG concept extraction configuration
- **`personality.rs`**: Personality trait system
- **`game_context.rs`**: Shared game state management

### Memory Flow

1. **World Observations** → Stored in **shared memory** (visible to all NPCs)
2. **Internal Thoughts** → Stored in **private memory** (NPC-specific)
3. **Memory Search** → Can search private, shared, or both scopes

### Relevance Evaluation

The `ZeraRelevanceEvaluator` calculates relevance based on:
- Party membership
- Distance (zones away)
- Recent interactions
- Active quests
- Recent mentions

## Example Output

```
Zera NPC Agent Example
======================

Creating Elder Rowan NPC...
✓ Elder Rowan created

1. World Observation (Shared Memory)
------------------------------------
✓ Stored observation in shared memory

2. Internal Thought (Private Memory)
-------------------------------------
✓ Stored thought in private memory

3. Multi-Agent Shared Memory
-----------------------------
Blacksmith found 1 shared memories about Oakshire
  - The party entered the village of Oakshire

4. Private Memory Isolation
-----------------------------
Blacksmith searched shared memory for 'trustworthy':
  ✓ No results (private thought not visible)

5. Relevance Evaluation
------------------------
Elder Rowan relevance: 1.00 (Active)

6. Memory Scope Demonstration
------------------------------
Elder Rowan private memories: 1
Elder Rowan shared memories: 1

✓ Zera NPC example complete!
```

## Adapting for Other Games

To adapt this example for other games or domains:

1. **Update `relevance.rs`**: Modify relevance calculation logic for your domain
2. **Update `concepts.rs`**: Configure concept types relevant to your domain
3. **Update `game_context.rs`**: Adjust game state structure for your needs
4. **Update `personality.rs`**: Add domain-specific personality traits

## See Also

- [Hybrid Memory Mode Example](../hybrid_mode.rs)
- [Multi-Agent Coordination Example](../multi_agent/main.rs)
- [Thymos Documentation](../../../README.md)


