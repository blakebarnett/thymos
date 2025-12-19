#!/usr/bin/env python3
"""
Basic example demonstrating Thymos Python bindings

This example shows how to:
1. Create an agent
2. Store memories
3. Search memories
4. Access agent state

Note: This script sets MALLOC_CONF before importing thymos to work around
jemalloc TLS limitations in Python extensions.
"""

import os
import tempfile

# IMPORTANT: Set MALLOC_CONF before importing thymos to avoid TLS errors
# This is required due to jemalloc's Thread Local Storage limitations in Python extensions
os.environ['MALLOC_CONF'] = 'background_thread:false'

import thymos

def main():
    # Create a temporary directory for memory storage
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create an agent with default configuration
        # Note: In a real application, you'd configure the data directory
        print("Creating agent...")
        agent = thymos.Agent("example_agent")
        
        print(f"Agent ID: {agent.id()}")
        print(f"Agent Status: {agent.status()}")
        
        # Store some memories
        print("\nStoring memories...")
        memory_id1 = agent.remember("Alice met Bob in Paris in 2023")
        print(f"Stored memory: {memory_id1}")
        
        memory_id2 = agent.remember("Bob works at a tech company")
        print(f"Stored memory: {memory_id2}")
        
        memory_id3 = agent.remember("Alice loves traveling and photography")
        print(f"Stored memory: {memory_id3}")
        
        # Search for memories
        print("\nSearching for 'Alice'...")
        results = agent.search_memories("Alice")
        print(f"Found {len(results)} memories:")
        for memory in results:
            print(f"  - {memory.content()}")
            print(f"    ID: {memory.id()}")
            print(f"    Created: {memory.created_at()}")
        
        print("\nSearching for 'Bob'...")
        results = agent.search_memories("Bob")
        print(f"Found {len(results)} memories:")
        for memory in results:
            print(f"  - {memory.content()}")
        
        # Get agent state
        print("\nAgent State:")
        state = agent.state()
        print(f"  Status: {state.status()}")
        print(f"  Started at: {state.started_at()}")
        print(f"  Last active: {state.last_active()}")
        
        # Get a specific memory by ID
        print(f"\nRetrieving memory by ID: {memory_id1}")
        memory = agent.get_memory(memory_id1)
        if memory:
            print(f"  Content: {memory.content()}")
            print(f"  Properties: {memory.properties()}")

if __name__ == "__main__":
    main()

