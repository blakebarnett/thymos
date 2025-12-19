package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	thymos "github.com/blakebarnett/thymos-go"
)

func main() {
	fmt.Printf("Thymos Go Bindings Example\n")
	fmt.Printf("Version: %s\n\n", thymos.Version())

	// Create a temporary directory for this example
	tempDir, err := os.MkdirTemp("", "thymos-go-example-*")
	if err != nil {
		log.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tempDir)

	// Example 1: Using custom memory configuration
	fmt.Println("=== Example 1: Agent with Custom Memory Config ===")
	memConfig, err := thymos.NewMemoryConfigWithDataDir(filepath.Join(tempDir, "agent1"))
	if err != nil {
		log.Fatalf("Failed to create memory config: %v", err)
	}
	defer memConfig.Close()

	agent1, err := thymos.NewAgentWithMemoryConfig("research_agent", memConfig)
	if err != nil {
		log.Fatalf("Failed to create agent: %v", err)
	}
	defer agent1.Close()

	// Get agent info
	id, _ := agent1.ID()
	desc, _ := agent1.Description()
	fmt.Printf("Agent ID: %s\n", id)
	fmt.Printf("Description: %s\n", desc)

	// Store various types of memories
	memID, err := agent1.Remember("Alice met Bob in Paris last summer")
	if err != nil {
		log.Fatalf("Failed to remember: %v", err)
	}
	fmt.Printf("Stored memory: %s\n", memID)

	// Store a fact
	factID, err := agent1.RememberFact("Paris is the capital of France")
	if err != nil {
		log.Fatalf("Failed to remember fact: %v", err)
	}
	fmt.Printf("Stored fact: %s\n", factID)

	// Store a conversation
	convID, err := agent1.RememberConversation("User asked about travel destinations in Europe")
	if err != nil {
		log.Fatalf("Failed to remember conversation: %v", err)
	}
	fmt.Printf("Stored conversation: %s\n", convID)

	// Search memories
	fmt.Println("\n=== Searching Memories ===")
	results, err := agent1.SearchMemories("Paris", 10)
	if err != nil {
		log.Fatalf("Failed to search: %v", err)
	}
	fmt.Printf("Found %d memories for 'Paris':\n", len(results))
	for i, mem := range results {
		fmt.Printf("  %d. %s\n", i+1, mem.Content)
	}

	// Get specific memory
	fmt.Println("\n=== Get Memory by ID ===")
	mem, err := agent1.GetMemory(memID)
	if err != nil {
		log.Fatalf("Failed to get memory: %v", err)
	}
	if mem != nil {
		fmt.Printf("Retrieved: %s\n", mem)
		fmt.Printf("  Created at: %s\n", mem.CreatedAt)
		if mem.LastAccessed != nil {
			fmt.Printf("  Last accessed: %s\n", *mem.LastAccessed)
		}
	}

	// Example 2: Agent state management
	fmt.Println("\n=== Example 2: Agent State Management ===")
	status, err := agent1.Status()
	if err != nil {
		log.Fatalf("Failed to get status: %v", err)
	}
	fmt.Printf("Current status: %s\n", status)

	// Get full state
	state, err := agent1.State()
	if err != nil {
		log.Fatalf("Failed to get state: %v", err)
	}
	fmt.Printf("Full state:\n")
	fmt.Printf("  Status: %s\n", state.Status)
	fmt.Printf("  Last active: %s\n", state.LastActive)
	if state.StartedAt != nil {
		fmt.Printf("  Started at: %s\n", *state.StartedAt)
	}

	// Change status
	if err := agent1.SetStatus(thymos.StatusListening); err != nil {
		log.Fatalf("Failed to set status: %v", err)
	}
	newStatus, _ := agent1.Status()
	fmt.Printf("Updated status: %s\n", newStatus)

	// Check if hybrid mode
	isHybrid, _ := agent1.IsHybrid()
	fmt.Printf("Is hybrid mode: %v\n", isHybrid)

	// Example 3: Default agent (simplest usage)
	fmt.Println("\n=== Example 3: Simple Default Agent ===")
	agent2, err := thymos.NewAgent("simple_agent")
	if err != nil {
		log.Fatalf("Failed to create simple agent: %v", err)
	}
	defer agent2.Close()

	_, err = agent2.Remember("This is a simple test memory")
	if err != nil {
		log.Fatalf("Failed to remember: %v", err)
	}
	fmt.Println("Simple agent created and stored a memory successfully!")

	// Example 4: Hybrid mode operations (will fail gracefully in non-hybrid mode)
	fmt.Println("\n=== Example 4: Hybrid Mode Operations ===")
	_, err = agent1.RememberPrivate("Private thought")
	if err != nil {
		if err == thymos.ErrNotHybridMode {
			fmt.Println("Agent is not in hybrid mode - private/shared operations not available")
		} else {
			fmt.Printf("RememberPrivate error: %v\n", err)
		}
	} else {
		fmt.Println("Stored private memory (hybrid mode)")
	}

	fmt.Println("\n=== All Examples Complete ===")
}
