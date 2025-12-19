/** @module Interface thymos:agent/agent@0.1.0 **/
export function create(id: AgentId): void;
export function id(): AgentId;
export function description(): string;
export function status(): AgentStatus;
export function setStatus(status: AgentStatus): void;
export function state(): AgentState;
export type AgentId = import('./thymos-agent-types.js').AgentId;
export type AgentStatus = import('./thymos-agent-types.js').AgentStatus;
export type AgentState = import('./thymos-agent-types.js').AgentState;
export type ThymosError = import('./thymos-agent-types.js').ThymosError;
