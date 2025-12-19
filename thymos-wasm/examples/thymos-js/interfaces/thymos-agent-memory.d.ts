/** @module Interface thymos:agent/memory@0.1.0 **/
export function remember(content: string): MemoryId;
export function rememberTyped(content: string, memoryType: MemoryType): MemoryId;
export function rememberWithOptions(content: string, options: RememberOptions): MemoryId;
export function search(query: string, options: SearchOptions | undefined): Array<Memory>;
export function get(id: MemoryId): Memory | undefined;
export { _delete as delete };
function _delete(id: MemoryId): boolean;
export function count(): bigint;
export type MemoryId = import('./thymos-agent-types.js').MemoryId;
export type Memory = import('./thymos-agent-types.js').Memory;
export type MemoryType = import('./thymos-agent-types.js').MemoryType;
export type RememberOptions = import('./thymos-agent-types.js').RememberOptions;
export type SearchOptions = import('./thymos-agent-types.js').SearchOptions;
export type ThymosError = import('./thymos-agent-types.js').ThymosError;
