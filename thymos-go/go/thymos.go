// Package thymos provides Go bindings for the Thymos agent framework.
//
// Thymos is a domain-agnostic agent framework providing:
//   - Semantic memory (via embedded Locai)
//   - Memory lifecycle management with forgetting curves
//   - Concept extraction and entity tracking
//   - Multi-agent coordination via pub/sub
//
// # Basic Usage
//
//	agent, err := thymos.NewAgent("my_agent")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	defer agent.Close()
//
//	// Store memories
//	id, _ := agent.Remember("Alice met Bob in Paris")
//
//	// Search memories
//	results, _ := agent.SearchMemories("Alice", 10)
//
// # Memory Management
//
// All Agent instances should be closed when no longer needed. While Go finalizers
// are registered as a safety net, explicit Close() calls are recommended for
// deterministic resource cleanup.
//
// # Thread Safety
//
// All methods are thread-safe and can be called from multiple goroutines.
package thymos

/*
#cgo LDFLAGS: -L${SRCDIR}/../../target/debug -L${SRCDIR}/../../target/release -lthymos_go -ldl -lm -lpthread -Wl,-rpath,${SRCDIR}/../../target/debug:${SRCDIR}/../../target/release
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Error handling
extern const char* thymos_get_last_error(void);
extern void thymos_clear_error(void);

// String utilities
extern void thymos_free_string(char* s);

// Configuration
extern void* thymos_memory_config_new(void);
extern void* thymos_memory_config_with_data_dir(const char* data_dir);
extern void thymos_free_memory_config(void* handle);
extern void* thymos_config_new(void);
extern void* thymos_config_load(void);
extern void* thymos_config_load_from_file(const char* path);
extern void thymos_free_config(void* handle);

// Agent lifecycle
extern void* thymos_agent_new(const char* agent_id);
extern void* thymos_agent_new_with_memory_config(const char* agent_id, const void* config);
extern void* thymos_agent_new_with_config(const char* agent_id, const void* config);
extern void thymos_free_agent(void* handle);

// Agent properties
extern char* thymos_agent_id(const void* handle);
extern char* thymos_agent_description(const void* handle);
extern char* thymos_agent_status(const void* handle);
extern int thymos_agent_set_status(const void* handle, const char* status);
extern void* thymos_agent_state(const void* handle);
extern void thymos_free_agent_state(void* state);
extern int thymos_agent_is_hybrid(const void* handle);

// Memory operations
extern char* thymos_agent_remember(const void* handle, const char* content);
extern char* thymos_agent_remember_fact(const void* handle, const char* content);
extern char* thymos_agent_remember_conversation(const void* handle, const char* content);
extern char* thymos_agent_remember_private(const void* handle, const char* content);
extern char* thymos_agent_remember_shared(const void* handle, const char* content);

// Memory search
extern void* thymos_agent_search_memories(const void* handle, const char* query, size_t limit);
extern void* thymos_agent_search_private(const void* handle, const char* query, size_t limit);
extern void* thymos_agent_search_shared(const void* handle, const char* query, size_t limit);
extern void* thymos_agent_get_memory(const void* handle, const char* memory_id);
extern void thymos_free_memory(void* m);
extern void thymos_free_search_results(void* results);

// Utilities
extern char* thymos_version(void);

// Structures
typedef struct {
    char* id;
    char* content;
    char* properties_json;
    char* created_at;
    char* last_accessed;
} ThymosMemory;

typedef struct {
    ThymosMemory* memories;
    size_t count;
    size_t capacity;
} ThymosSearchResults;

typedef struct {
    char* status;
    char* started_at;
    char* last_active;
    char* properties_json;
} ThymosAgentState;
*/
import "C"
import (
	"encoding/json"
	"errors"
	"fmt"
	"runtime"
	"sync"
	"unsafe"
)

// Error represents a Thymos error
type Error struct {
	Message string
}

func (e *Error) Error() string {
	return e.Message
}

// ErrNilHandle is returned when an operation is attempted on a closed agent
var ErrNilHandle = errors.New("thymos: agent handle is nil (agent may be closed)")

// ErrNotHybridMode is returned when a hybrid-only operation is called on a non-hybrid agent
var ErrNotHybridMode = errors.New("thymos: operation only available in hybrid mode")

// getLastError retrieves the last error from the Rust side
func getLastError() error {
	errPtr := C.thymos_get_last_error()
	if errPtr == nil {
		return nil
	}
	errMsg := C.GoString(errPtr)
	if errMsg == "" {
		return nil
	}
	return &Error{Message: errMsg}
}

// clearError clears the last error
func clearError() {
	C.thymos_clear_error()
}

// Version returns the Thymos library version
func Version() string {
	cVersion := C.thymos_version()
	if cVersion == nil {
		return "unknown"
	}
	defer C.thymos_free_string(cVersion)
	return C.GoString(cVersion)
}

// ============================================================================
// Configuration
// ============================================================================

// MemoryConfig holds memory system configuration
type MemoryConfig struct {
	handle unsafe.Pointer
	mu     sync.Mutex
}

// NewMemoryConfig creates a new default memory configuration
func NewMemoryConfig() *MemoryConfig {
	handle := C.thymos_memory_config_new()
	if handle == nil {
		return nil
	}

	config := &MemoryConfig{handle: handle}
	runtime.SetFinalizer(config, (*MemoryConfig).Close)
	return config
}

// NewMemoryConfigWithDataDir creates a memory configuration with a custom data directory
func NewMemoryConfigWithDataDir(dataDir string) (*MemoryConfig, error) {
	cDataDir := C.CString(dataDir)
	defer C.free(unsafe.Pointer(cDataDir))

	handle := C.thymos_memory_config_with_data_dir(cDataDir)
	if handle == nil {
		return nil, getLastError()
	}

	config := &MemoryConfig{handle: handle}
	runtime.SetFinalizer(config, (*MemoryConfig).Close)
	return config, nil
}

// Close releases the memory configuration resources
func (c *MemoryConfig) Close() {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.handle != nil {
		C.thymos_free_memory_config(c.handle)
		c.handle = nil
	}
}

// Config holds full Thymos configuration
type Config struct {
	handle unsafe.Pointer
	mu     sync.Mutex
}

// NewConfig creates a new default Thymos configuration
func NewConfig() *Config {
	handle := C.thymos_config_new()
	if handle == nil {
		return nil
	}

	config := &Config{handle: handle}
	runtime.SetFinalizer(config, (*Config).Close)
	return config
}

// LoadConfig loads configuration from file and environment
//
// Searches for thymos.toml, thymos.yaml, or thymos.json in standard locations.
// Environment variables with THYMOS_ prefix override file settings.
func LoadConfig() (*Config, error) {
	handle := C.thymos_config_load()
	if handle == nil {
		return nil, getLastError()
	}

	config := &Config{handle: handle}
	runtime.SetFinalizer(config, (*Config).Close)
	return config, nil
}

// LoadConfigFromFile loads configuration from a specific file
func LoadConfigFromFile(path string) (*Config, error) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	handle := C.thymos_config_load_from_file(cPath)
	if handle == nil {
		return nil, getLastError()
	}

	config := &Config{handle: handle}
	runtime.SetFinalizer(config, (*Config).Close)
	return config, nil
}

// Close releases the configuration resources
func (c *Config) Close() {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.handle != nil {
		C.thymos_free_config(c.handle)
		c.handle = nil
	}
}

// ============================================================================
// Agent
// ============================================================================

// Agent represents a Thymos agent with memory and lifecycle management
type Agent struct {
	handle unsafe.Pointer
	mu     sync.RWMutex
}

// NewAgent creates a new agent with the given ID using default configuration
func NewAgent(agentID string) (*Agent, error) {
	cAgentID := C.CString(agentID)
	defer C.free(unsafe.Pointer(cAgentID))

	handle := C.thymos_agent_new(cAgentID)
	if handle == nil {
		return nil, getLastError()
	}

	agent := &Agent{handle: handle}
	runtime.SetFinalizer(agent, (*Agent).Close)
	return agent, nil
}

// NewAgentWithMemoryConfig creates a new agent with custom memory configuration
func NewAgentWithMemoryConfig(agentID string, config *MemoryConfig) (*Agent, error) {
	if config == nil || config.handle == nil {
		return nil, errors.New("thymos: memory config is nil")
	}

	cAgentID := C.CString(agentID)
	defer C.free(unsafe.Pointer(cAgentID))

	handle := C.thymos_agent_new_with_memory_config(cAgentID, config.handle)
	if handle == nil {
		return nil, getLastError()
	}

	agent := &Agent{handle: handle}
	runtime.SetFinalizer(agent, (*Agent).Close)
	return agent, nil
}

// NewAgentWithConfig creates a new agent with full Thymos configuration
func NewAgentWithConfig(agentID string, config *Config) (*Agent, error) {
	if config == nil || config.handle == nil {
		return nil, errors.New("thymos: config is nil")
	}

	cAgentID := C.CString(agentID)
	defer C.free(unsafe.Pointer(cAgentID))

	handle := C.thymos_agent_new_with_config(cAgentID, config.handle)
	if handle == nil {
		return nil, getLastError()
	}

	agent := &Agent{handle: handle}
	runtime.SetFinalizer(agent, (*Agent).Close)
	return agent, nil
}

// Close releases the agent resources
//
// After Close is called, all methods will return ErrNilHandle.
// Close is idempotent and safe to call multiple times.
func (a *Agent) Close() {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.handle != nil {
		C.thymos_free_agent(a.handle)
		a.handle = nil
	}
}

// IsClosed returns true if the agent has been closed
func (a *Agent) IsClosed() bool {
	a.mu.RLock()
	defer a.mu.RUnlock()
	return a.handle == nil
}

// ID returns the agent's unique identifier
func (a *Agent) ID() (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cID := C.thymos_agent_id(a.handle)
	if cID == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// Description returns the agent's description
func (a *Agent) Description() (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cDesc := C.thymos_agent_description(a.handle)
	if cDesc == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cDesc)

	return C.GoString(cDesc), nil
}

// IsHybrid returns true if the agent is using hybrid memory mode
func (a *Agent) IsHybrid() (bool, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return false, ErrNilHandle
	}

	result := C.thymos_agent_is_hybrid(a.handle)
	if result < 0 {
		return false, getLastError()
	}
	return result == 1, nil
}

// ============================================================================
// Status and State
// ============================================================================

// Status represents the agent's operational status
type Status string

const (
	StatusActive    Status = "Active"
	StatusListening Status = "Listening"
	StatusDormant   Status = "Dormant"
	StatusArchived  Status = "Archived"
)

// Status returns the current agent status
func (a *Agent) Status() (Status, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cStatus := C.thymos_agent_status(a.handle)
	if cStatus == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cStatus)

	return Status(C.GoString(cStatus)), nil
}

// SetStatus sets the agent status
//
// Valid statuses: StatusActive, StatusListening, StatusDormant, StatusArchived
func (a *Agent) SetStatus(status Status) error {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return ErrNilHandle
	}

	cStatus := C.CString(string(status))
	defer C.free(unsafe.Pointer(cStatus))

	result := C.thymos_agent_set_status(a.handle, cStatus)
	if result != 0 {
		return getLastError()
	}
	return nil
}

// State holds the full agent state
type State struct {
	Status     Status
	StartedAt  *string
	LastActive string
	Properties map[string]interface{}
}

// State returns the full agent state
func (a *Agent) State() (*State, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return nil, ErrNilHandle
	}

	cState := C.thymos_agent_state(a.handle)
	if cState == nil {
		return nil, getLastError()
	}
	defer C.thymos_free_agent_state(cState)

	state := (*C.ThymosAgentState)(cState)

	result := &State{
		Status:     Status(C.GoString(state.status)),
		LastActive: C.GoString(state.last_active),
		Properties: make(map[string]interface{}),
	}

	if state.started_at != nil {
		startedAt := C.GoString(state.started_at)
		result.StartedAt = &startedAt
	}

	if state.properties_json != nil {
		propsJSON := C.GoString(state.properties_json)
		if err := json.Unmarshal([]byte(propsJSON), &result.Properties); err != nil {
			result.Properties = make(map[string]interface{})
		}
	}

	return result, nil
}

// ============================================================================
// Memory
// ============================================================================

// Memory represents a stored memory
type Memory struct {
	ID           string
	Content      string
	Properties   map[string]interface{}
	CreatedAt    string
	LastAccessed *string
}

func convertCMemory(cMem *C.ThymosMemory) *Memory {
	mem := &Memory{
		ID:         C.GoString(cMem.id),
		Content:    C.GoString(cMem.content),
		CreatedAt:  C.GoString(cMem.created_at),
		Properties: make(map[string]interface{}),
	}

	if cMem.last_accessed != nil {
		lastAccessed := C.GoString(cMem.last_accessed)
		mem.LastAccessed = &lastAccessed
	}

	if cMem.properties_json != nil {
		propsJSON := C.GoString(cMem.properties_json)
		if err := json.Unmarshal([]byte(propsJSON), &mem.Properties); err != nil {
			mem.Properties = make(map[string]interface{})
		}
	}

	return mem
}

// Remember stores a memory and returns its ID
func (a *Agent) Remember(content string) (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))

	cID := C.thymos_agent_remember(a.handle, cContent)
	if cID == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// RememberFact stores a fact memory (durable, context-independent knowledge)
//
// Facts are intended for knowledge like "Paris is the capital of France".
func (a *Agent) RememberFact(content string) (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))

	cID := C.thymos_agent_remember_fact(a.handle, cContent)
	if cID == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// RememberConversation stores a conversation memory (dialogue context)
//
// Conversation memories are intended for dialogue history and ephemeral context.
func (a *Agent) RememberConversation(content string) (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))

	cID := C.thymos_agent_remember_conversation(a.handle, cContent)
	if cID == nil {
		return "", getLastError()
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// RememberPrivate stores a memory in the private backend (hybrid mode only)
//
// Returns ErrNotHybridMode if the agent is not in hybrid mode.
func (a *Agent) RememberPrivate(content string) (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))

	cID := C.thymos_agent_remember_private(a.handle, cContent)
	if cID == nil {
		err := getLastError()
		if err != nil && err.Error() == "remember_private only available in hybrid mode" {
			return "", ErrNotHybridMode
		}
		return "", err
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// RememberShared stores a memory in the shared backend (hybrid mode only)
//
// Returns ErrNotHybridMode if the agent is not in hybrid mode.
func (a *Agent) RememberShared(content string) (string, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return "", ErrNilHandle
	}

	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))

	cID := C.thymos_agent_remember_shared(a.handle, cContent)
	if cID == nil {
		err := getLastError()
		if err != nil && err.Error() == "remember_shared only available in hybrid mode" {
			return "", ErrNotHybridMode
		}
		return "", err
	}
	defer C.thymos_free_string(cID)

	return C.GoString(cID), nil
}

// ============================================================================
// Memory Search
// ============================================================================

// SearchMemories searches for memories matching the query
//
// Set limit to 0 for no limit.
func (a *Agent) SearchMemories(query string, limit int) ([]*Memory, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return nil, ErrNilHandle
	}

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	cLimit := C.size_t(limit)
	if limit < 0 {
		cLimit = 0
	}

	resultsPtr := C.thymos_agent_search_memories(a.handle, cQuery, cLimit)
	if resultsPtr == nil {
		err := getLastError()
		if err == nil {
			return []*Memory{}, nil
		}
		return nil, err
	}
	defer C.thymos_free_search_results(resultsPtr)

	results := (*C.ThymosSearchResults)(resultsPtr)
	if results.count == 0 {
		return []*Memory{}, nil
	}

	memories := make([]*Memory, 0, results.count)
	memArray := (*[1 << 28]C.ThymosMemory)(unsafe.Pointer(results.memories))[:results.count:results.count]

	for i := range memArray {
		memories = append(memories, convertCMemory(&memArray[i]))
	}

	return memories, nil
}

// SearchPrivate searches private memories (hybrid mode only)
//
// Returns ErrNotHybridMode if the agent is not in hybrid mode.
func (a *Agent) SearchPrivate(query string, limit int) ([]*Memory, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return nil, ErrNilHandle
	}

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	cLimit := C.size_t(limit)
	if limit < 0 {
		cLimit = 0
	}

	resultsPtr := C.thymos_agent_search_private(a.handle, cQuery, cLimit)
	if resultsPtr == nil {
		err := getLastError()
		if err != nil && err.Error() == "search_private only available in hybrid mode" {
			return nil, ErrNotHybridMode
		}
		if err == nil {
			return []*Memory{}, nil
		}
		return nil, err
	}
	defer C.thymos_free_search_results(resultsPtr)

	results := (*C.ThymosSearchResults)(resultsPtr)
	if results.count == 0 {
		return []*Memory{}, nil
	}

	memories := make([]*Memory, 0, results.count)
	memArray := (*[1 << 28]C.ThymosMemory)(unsafe.Pointer(results.memories))[:results.count:results.count]

	for i := range memArray {
		memories = append(memories, convertCMemory(&memArray[i]))
	}

	return memories, nil
}

// SearchShared searches shared memories (hybrid mode only)
//
// Returns ErrNotHybridMode if the agent is not in hybrid mode.
func (a *Agent) SearchShared(query string, limit int) ([]*Memory, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return nil, ErrNilHandle
	}

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	cLimit := C.size_t(limit)
	if limit < 0 {
		cLimit = 0
	}

	resultsPtr := C.thymos_agent_search_shared(a.handle, cQuery, cLimit)
	if resultsPtr == nil {
		err := getLastError()
		if err != nil && err.Error() == "search_shared only available in hybrid mode" {
			return nil, ErrNotHybridMode
		}
		if err == nil {
			return []*Memory{}, nil
		}
		return nil, err
	}
	defer C.thymos_free_search_results(resultsPtr)

	results := (*C.ThymosSearchResults)(resultsPtr)
	if results.count == 0 {
		return []*Memory{}, nil
	}

	memories := make([]*Memory, 0, results.count)
	memArray := (*[1 << 28]C.ThymosMemory)(unsafe.Pointer(results.memories))[:results.count:results.count]

	for i := range memArray {
		memories = append(memories, convertCMemory(&memArray[i]))
	}

	return memories, nil
}

// GetMemory retrieves a memory by its ID
//
// Returns nil, nil if the memory is not found.
func (a *Agent) GetMemory(memoryID string) (*Memory, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	if a.handle == nil {
		return nil, ErrNilHandle
	}

	cMemoryID := C.CString(memoryID)
	defer C.free(unsafe.Pointer(cMemoryID))

	memPtr := C.thymos_agent_get_memory(a.handle, cMemoryID)
	if memPtr == nil {
		err := getLastError()
		if err == nil {
			return nil, nil // Memory not found
		}
		return nil, err
	}
	defer C.thymos_free_memory(memPtr)

	return convertCMemory((*C.ThymosMemory)(memPtr)), nil
}

// String returns a string representation of the memory
func (m *Memory) String() string {
	return fmt.Sprintf("Memory{ID: %s, Content: %q}", m.ID, m.Content)
}
