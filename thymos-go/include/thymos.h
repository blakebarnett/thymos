/* Thymos Go FFI bindings */
/* This header is manually maintained for CGO compatibility */

#ifndef THYMOS_FFI_H
#define THYMOS_FFI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Opaque Handle Types
 * These are pointers to opaque Rust structures
 * ============================================================================ */

typedef struct ThymosAgent ThymosAgent;
typedef struct ThymosMemoryConfig ThymosMemoryConfig;
typedef struct ThymosConfigHandle ThymosConfigHandle;

/* ============================================================================
 * Data Structures
 * ============================================================================ */

/* Memory result structure */
typedef struct ThymosMemory {
    char *id;
    char *content;
    char *properties_json;
    char *created_at;
    char *last_accessed;
} ThymosMemory;

/* Search results structure */
typedef struct ThymosSearchResults {
    ThymosMemory *memories;
    size_t count;
    size_t capacity;
} ThymosSearchResults;

/* Agent state structure */
typedef struct ThymosAgentState {
    char *status;
    char *started_at;
    char *last_active;
    char *properties_json;
} ThymosAgentState;

/* ============================================================================
 * Error Handling
 * ============================================================================ */

/* Get the last error message (valid until next FFI call) */
const char *thymos_get_last_error(void);

/* Clear the last error */
void thymos_clear_error(void);

/* ============================================================================
 * Memory Management
 * ============================================================================ */

void thymos_free_string(char *s);
void thymos_free_memory(ThymosMemory *m);
void thymos_free_search_results(ThymosSearchResults *results);
void thymos_free_agent(ThymosAgent *handle);
void thymos_free_memory_config(ThymosMemoryConfig *handle);
void thymos_free_config(ThymosConfigHandle *handle);
void thymos_free_agent_state(ThymosAgentState *state);

/* ============================================================================
 * Configuration
 * ============================================================================ */

/* Create default memory configuration */
ThymosMemoryConfig *thymos_memory_config_new(void);

/* Create memory config with custom data directory (embedded mode) */
ThymosMemoryConfig *thymos_memory_config_with_data_dir(const char *data_dir);

/* Create memory config for server mode (connects to Locai server) */
ThymosMemoryConfig *thymos_memory_config_server(
    const char *server_url,
    const char *api_key  /* can be NULL */
);

/* Create memory config for hybrid mode (private embedded + shared server) */
ThymosMemoryConfig *thymos_memory_config_hybrid(
    const char *private_data_dir,
    const char *shared_url,
    const char *shared_api_key  /* can be NULL */
);

/* Create default Thymos configuration */
ThymosConfigHandle *thymos_config_new(void);

/* Load configuration from file and environment */
ThymosConfigHandle *thymos_config_load(void);

/* Load configuration from specific file */
ThymosConfigHandle *thymos_config_load_from_file(const char *path);

/* ============================================================================
 * Agent Lifecycle
 * ============================================================================ */

/* Create agent with default configuration */
ThymosAgent *thymos_agent_new(const char *agent_id);

/* Create agent with custom memory configuration */
ThymosAgent *thymos_agent_new_with_memory_config(
    const char *agent_id,
    const ThymosMemoryConfig *config
);

/* Create agent with full Thymos configuration */
ThymosAgent *thymos_agent_new_with_config(
    const char *agent_id,
    const ThymosConfigHandle *config
);

/* ============================================================================
 * Agent Properties
 * ============================================================================ */

/* Get agent ID (must free with thymos_free_string) */
char *thymos_agent_id(const ThymosAgent *handle);

/* Get agent description (must free with thymos_free_string) */
char *thymos_agent_description(const ThymosAgent *handle);

/* Get agent status: "Active", "Listening", "Dormant", "Archived" */
char *thymos_agent_status(const ThymosAgent *handle);

/* Set agent status. Returns 0 on success, -1 on error */
int thymos_agent_set_status(const ThymosAgent *handle, const char *status);

/* Get full agent state (must free with thymos_free_agent_state) */
ThymosAgentState *thymos_agent_state(const ThymosAgent *handle);

/* Check if agent is in hybrid mode. Returns 1 if hybrid, 0 otherwise, -1 on error */
int thymos_agent_is_hybrid(const ThymosAgent *handle);

/* ============================================================================
 * Memory Operations
 * ============================================================================ */

/* Store a memory. Returns memory ID (must free with thymos_free_string) */
char *thymos_agent_remember(const ThymosAgent *handle, const char *content);

/* Store a fact memory (durable knowledge) */
char *thymos_agent_remember_fact(const ThymosAgent *handle, const char *content);

/* Store a conversation memory (dialogue context) */
char *thymos_agent_remember_conversation(const ThymosAgent *handle, const char *content);

/* Store memory in private backend (hybrid mode only) */
char *thymos_agent_remember_private(const ThymosAgent *handle, const char *content);

/* Store memory in shared backend (hybrid mode only) */
char *thymos_agent_remember_shared(const ThymosAgent *handle, const char *content);

/* ============================================================================
 * Memory Search
 * ============================================================================ */

/* Search memories. limit=0 for no limit */
ThymosSearchResults *thymos_agent_search_memories(
    const ThymosAgent *handle,
    const char *query,
    size_t limit
);

/* Search private memories (hybrid mode only) */
ThymosSearchResults *thymos_agent_search_private(
    const ThymosAgent *handle,
    const char *query,
    size_t limit
);

/* Search shared memories (hybrid mode only) */
ThymosSearchResults *thymos_agent_search_shared(
    const ThymosAgent *handle,
    const char *query,
    size_t limit
);

/* Get memory by ID. Returns NULL if not found */
ThymosMemory *thymos_agent_get_memory(
    const ThymosAgent *handle,
    const char *memory_id
);

/* ============================================================================
 * Utilities
 * ============================================================================ */

/* Get Thymos library version (must free with thymos_free_string) */
char *thymos_version(void);

#ifdef __cplusplus
}
#endif

#endif /* THYMOS_FFI_H */
