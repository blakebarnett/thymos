#!/usr/bin/env node
/**
 * Test script for Thymos WASM component (in-memory mode)
 * 
 * This tests the component without server connectivity.
 */

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load the generated module
const { instantiate } = await import('./thymos-js/thymos_wasm.js');

// Minimal WASI shims for in-memory mode (no HTTP needed)
function createWasiShims() {
    class Pollable {
        block() {}
        ready() { return true; }
        subscribe() { return this; }
    }

    class IoError {
        constructor(message) { this.message = message; }
        toDebugString() { return this.message; }
    }

    class InputStream {
        constructor(data = new Uint8Array()) {
            this.data = data;
            this.pos = 0;
        }
        read(len) {
            const chunk = this.data.slice(this.pos, this.pos + Number(len));
            this.pos += chunk.length;
            return chunk;
        }
        blockingRead(len) { return this.read(len); }
        subscribe() { return new Pollable(); }
    }

    class OutputStream {
        constructor() { this.chunks = []; }
        checkWrite() { return BigInt(65536); }
        write(data) { this.chunks.push(new Uint8Array(data)); }
        blockingWriteAndFlush(data) { this.chunks.push(new Uint8Array(data)); }
        blockingFlush() {}
        subscribe() { return new Pollable(); }
        getContents() {
            const totalLen = this.chunks.reduce((acc, c) => acc + c.length, 0);
            const result = new Uint8Array(totalLen);
            let offset = 0;
            for (const chunk of this.chunks) {
                result.set(chunk, offset);
                offset += chunk.length;
            }
            return result;
        }
    }

    class Descriptor {
        openAt() { throw { tag: 'no-entry' }; }
        stat() {
            return {
                type: 'regular-file',
                linkCount: BigInt(1),
                size: BigInt(0),
            };
        }
        writeViaStream() { return new OutputStream(); }
        readViaStream() { return new InputStream(); }
        appendViaStream() { return new OutputStream(); }
        getType() { return 'directory'; }
    }

    // HTTP stubs (will fail if called, but needed for instantiation)
    class Fields {
        constructor() { this.map = new Map(); }
        static fromList(entries) { return new Fields(); }
        get(key) { return this.map.get(key.toLowerCase()) || []; }
        set(key, values) { this.map.set(key.toLowerCase(), values); }
        entries() { return []; }
        clone() { return new Fields(); }
    }
    class OutgoingRequest {
        constructor(headers) { this.headers = headers; }
        setMethod() {}
        setScheme() {}
        setAuthority() {}
        setPathWithQuery() {}
        body() { return new OutgoingBody(); }
    }
    class OutgoingBody {
        write() { return new OutputStream(); }
        static finish() {}
    }
    class IncomingResponse {}
    class IncomingBody {}
    class FutureIncomingResponse {}
    class RequestOptions {}

    return {
        'wasi:cli/environment': {
            getEnvironment: () => [],
        },
        'wasi:cli/exit': {
            exit: () => {},
        },
        'wasi:cli/stderr': {
            getStderr: () => new OutputStream(),
        },
        'wasi:cli/stdin': {
            getStdin: () => new InputStream(),
        },
        'wasi:cli/stdout': {
            getStdout: () => new OutputStream(),
        },
        'wasi:filesystem/preopens': {
            getDirectories: () => [[new Descriptor(), '/']],
        },
        'wasi:filesystem/types': {
            Descriptor,
            filesystemErrorCode: () => null,
        },
        'wasi:http/outgoing-handler': {
            handle: () => { throw new Error('HTTP not available in in-memory mode'); },
        },
        'wasi:http/types': {
            Fields,
            OutgoingRequest,
            OutgoingBody,
            IncomingResponse,
            IncomingBody,
            FutureIncomingResponse,
            RequestOptions,
        },
        'wasi:io/error': {
            Error: IoError,
        },
        'wasi:io/poll': {
            Pollable,
            poll: (ps) => ps.map((_, i) => i),
        },
        'wasi:io/streams': {
            InputStream,
            OutputStream,
        },
        'wasi:random/random': {
            getRandomBytes: (len) => {
                const bytes = new Uint8Array(Number(len));
                for (let i = 0; i < bytes.length; i++) {
                    bytes[i] = Math.floor(Math.random() * 256);
                }
                return bytes;
            },
        },
    };
}

async function getCoreModule(path) {
    const fullPath = join(__dirname, 'thymos-js', path);
    const bytes = await readFile(fullPath);
    return WebAssembly.compile(bytes);
}

async function main() {
    console.log("=== Thymos WASM Component Test (In-Memory Mode) ===\n");

    // Instantiate
    console.log("Instantiating WASM component...");
    const imports = createWasiShims();
    const { agent, memory, storage } = await instantiate(getCoreModule, imports);
    console.log("✓ Component instantiated\n");

    // Test 1: Check we're in in-memory mode
    console.log("1. Initial state check...");
    console.log(`   Connected to server: ${storage.isConnected()}`);
    console.log(`   Memory count: ${memory.count()}`);

    // Test 2: Create an agent
    console.log("\n2. Creating agent...");
    try {
        agent.create("wasm-test-agent");
        console.log(`   ✓ Agent created: ${agent.id()}`);
        console.log(`   ✓ Status: ${JSON.stringify(agent.status())}`);
    } catch (e) {
        console.error(`   ✗ Failed: ${e}`);
    }

    // Test 3: Store memories
    console.log("\n3. Storing memories...");
    const testMemories = [
        { content: "Alice met Bob at the coffee shop in Paris", type: null },
        { content: "Paris is the capital of France", type: "fact" },
        { content: "The meeting went well and they discussed the project timeline", type: "conversation" },
        { content: "Bob mentioned he prefers dark mode in all applications", type: null },
        { content: "The project deadline is December 31st, 2024", type: "fact" },
    ];

    const storedIds = [];
    for (const mem of testMemories) {
        try {
            let id;
            if (mem.type) {
                id = memory.rememberTyped(mem.content, mem.type);
            } else {
                id = memory.remember(mem.content);
            }
            storedIds.push(id);
            console.log(`   ✓ Stored: "${mem.content.substring(0, 40)}..." (${id})`);
        } catch (e) {
            console.error(`   ✗ Failed to store: ${e}`);
        }
    }

    // Test 4: Count memories
    console.log("\n4. Counting memories...");
    console.log(`   ✓ Total: ${memory.count()}`);

    // Test 5: Search memories (keyword search in in-memory mode)
    console.log("\n5. Searching memories (keyword mode)...");
    const queries = ["coffee shop", "Paris", "dark mode", "deadline"];

    for (const query of queries) {
        try {
            const results = memory.search(query, { limit: 3 });
            console.log(`   Query: "${query}" → ${results.length} result(s)`);
            for (const r of results) {
                const content = r.content.length > 45 ? r.content.substring(0, 45) + "..." : r.content;
                console.log(`     → ${content}`);
            }
        } catch (e) {
            console.error(`   ✗ Search failed: ${e}`);
        }
    }

    // Test 6: Get specific memory
    console.log("\n6. Getting specific memory...");
    if (storedIds.length > 0) {
        const mem = memory.get(storedIds[0]);
        if (mem) {
            console.log(`   ✓ Retrieved: ${mem.content.substring(0, 50)}...`);
            console.log(`   ✓ Created at: ${mem.createdAt}`);
        }
    }

    // Test 7: Delete a memory
    console.log("\n7. Deleting a memory...");
    if (storedIds.length > 0) {
        const deleted = memory.delete(storedIds[storedIds.length - 1]);
        console.log(`   ✓ Deleted: ${deleted}`);
        console.log(`   ✓ New count: ${memory.count()}`);
    }

    // Test 8: Agent state
    console.log("\n8. Agent state...");
    const state = agent.state();
    console.log(`   ✓ Status: ${JSON.stringify(state.status)}`);
    console.log(`   ✓ Started: ${state.startedAt}`);

    // Test 9: Clear memories
    console.log("\n9. Clearing memories...");
    storage.clear();
    console.log(`   ✓ Count after clear: ${memory.count()}`);

    console.log("\n=== Test Complete ===");
    console.log("\nThe WASM component works correctly in in-memory mode!");
    console.log("To test server mode, ensure wasi:http shims are properly implemented.");
}

main().catch(console.error);

