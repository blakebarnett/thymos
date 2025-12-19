/** @module Interface thymos:agent/types@0.1.0 **/
export type AgentId = string;
/**
 * # Variants
 * 
 * ## `"active"`
 * 
 * ## `"listening"`
 * 
 * ## `"dormant"`
 * 
 * ## `"archived"`
 */
export type AgentStatus = 'active' | 'listening' | 'dormant' | 'archived';
export interface AgentState {
  status: AgentStatus,
  startedAt?: string,
  lastActive: string,
  propertiesJson: string,
}
export type ThymosError = ThymosErrorConfiguration | ThymosErrorMemory | ThymosErrorAgent | ThymosErrorInvalidInput | ThymosErrorNotSupported;
export interface ThymosErrorConfiguration {
  tag: 'configuration',
  val: string,
}
export interface ThymosErrorMemory {
  tag: 'memory',
  val: string,
}
export interface ThymosErrorAgent {
  tag: 'agent',
  val: string,
}
export interface ThymosErrorInvalidInput {
  tag: 'invalid-input',
  val: string,
}
export interface ThymosErrorNotSupported {
  tag: 'not-supported',
  val: string,
}
export type MemoryId = string;
export interface Memory {
  id: MemoryId,
  content: string,
  propertiesJson: string,
  createdAt: string,
  lastAccessed?: string,
}
/**
 * # Variants
 * 
 * ## `"episodic"`
 * 
 * ## `"fact"`
 * 
 * ## `"conversation"`
 */
export type MemoryType = 'episodic' | 'fact' | 'conversation';
export interface RememberOptions {
  memoryType?: MemoryType,
  tagsJson?: string,
  priority?: number,
}
export interface SearchOptions {
  limit?: number,
}
