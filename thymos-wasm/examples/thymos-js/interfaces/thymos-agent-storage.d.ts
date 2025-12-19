/** @module Interface thymos:agent/storage@0.1.0 **/
export function connect(serverUrl: string, apiKey: string | undefined): void;
export function disconnect(): void;
export function isConnected(): boolean;
export function save(path: string): bigint;
export function load(path: string): bigint;
export function exists(path: string): boolean;
export function clear(): void;
export type ThymosError = import('./thymos-agent-types.js').ThymosError;
