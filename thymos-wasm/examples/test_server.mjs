#!/usr/bin/env node
/**
 * Test script for Thymos WASM component with Locai server
 * 
 * Prerequisites:
 *   1. Locai server running on http://localhost:3000
 *   2. Run: npx jco transpile ... (done by run_test.sh)
 */

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load the generated module
const { instantiate } = await import('./thymos-js/thymos_wasm.js');

// WASI shims for Node.js with proper HTTP support
function createWasiShims() {
    // Resource tracking for jco compatibility
    const resourceRegistry = new FinalizationRegistry(() => {});
    
    class Pollable {
        constructor(ready = true) {
            this._ready = ready;
            resourceRegistry.register(this, 'pollable');
        }
        block() {}
        ready() { return this._ready; }
        subscribe() { return this; }
        [Symbol.dispose]() {}
    }

    class IoError {
        constructor(message) { 
            this.message = message;
            resourceRegistry.register(this, 'error');
        }
        toDebugString() { return this.message; }
        [Symbol.dispose]() {}
    }

    class InputStream {
        constructor(data = new Uint8Array()) {
            this.data = data instanceof Uint8Array ? data : new Uint8Array(data);
            this.pos = 0;
            resourceRegistry.register(this, 'input-stream');
        }
        read(len) {
            const n = Math.min(Number(len), this.data.length - this.pos);
            const chunk = this.data.slice(this.pos, this.pos + n);
            this.pos += n;
            return chunk;
        }
        blockingRead(len) { return this.read(len); }
        subscribe() { return new Pollable(true); }
        [Symbol.dispose]() {}
    }

    class OutputStream {
        constructor() { 
            this.chunks = [];
            resourceRegistry.register(this, 'output-stream');
        }
        checkWrite() { return BigInt(1048576); }
        write(data) { this.chunks.push(new Uint8Array(data)); }
        blockingWriteAndFlush(data) { this.chunks.push(new Uint8Array(data)); }
        blockingFlush() {}
        subscribe() { return new Pollable(true); }
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
        [Symbol.dispose]() {}
    }

    class Fields {
        constructor() { 
            this._map = new Map();
            resourceRegistry.register(this, 'fields');
        }
        static fromList(entries) {
            const f = new Fields();
            for (const [k, v] of entries) {
                f.set(k, [v]);
            }
            return f;
        }
        get(key) { 
            return this._map.get(key.toLowerCase()) || []; 
        }
        set(key, values) { 
            this._map.set(key.toLowerCase(), values); 
        }
        delete(key) { 
            this._map.delete(key.toLowerCase()); 
        }
        append(key, value) {
            const existing = this._map.get(key.toLowerCase()) || [];
            existing.push(value);
            this._map.set(key.toLowerCase(), existing);
        }
        entries() {
            const result = [];
            for (const [k, vs] of this._map) {
                for (const v of vs) {
                    result.push([k, v]);
                }
            }
            return result;
        }
        clone() {
            const f = new Fields();
            f._map = new Map(this._map);
            return f;
        }
        [Symbol.dispose]() {}
    }

    class OutgoingRequest {
        constructor(headers) {
            this._headers = headers;
            this._method = 'GET';
            this._scheme = 'http';
            this._authority = '';
            this._pathWithQuery = '/';
            this._body = null;
            resourceRegistry.register(this, 'outgoing-request');
        }
        method() { return { tag: this._method.toLowerCase() }; }
        setMethod(method) { 
            if (method && method.tag) {
                this._method = method.tag.toUpperCase();
            }
        }
        scheme() { return { tag: this._scheme }; }
        setScheme(scheme) { 
            if (scheme && scheme.tag) {
                this._scheme = scheme.tag;
            }
        }
        authority() { return this._authority; }
        setAuthority(auth) { this._authority = auth || ''; }
        pathWithQuery() { return this._pathWithQuery; }
        setPathWithQuery(path) { this._pathWithQuery = path || '/'; }
        headers() { return this._headers; }
        body() {
            if (!this._body) {
                this._body = new OutgoingBody();
            }
            return this._body;
        }
        [Symbol.dispose]() {}
    }

    class OutgoingBody {
        constructor() {
            this._stream = new OutputStream();
            resourceRegistry.register(this, 'outgoing-body');
        }
        write() { return this._stream; }
        static finish(body, trailers) {
            // Nothing needed
        }
        [Symbol.dispose]() {}
    }

    class IncomingResponse {
        constructor(status, headers, bodyData) {
            this._status = status;
            this._headers = headers;
            this._bodyData = bodyData;
            this._consumed = false;
            resourceRegistry.register(this, 'incoming-response');
        }
        status() { return this._status; }
        headers() { return this._headers; }
        consume() {
            if (this._consumed) {
                throw { tag: 'already-consumed' };
            }
            this._consumed = true;
            return new IncomingBody(this._bodyData);
        }
        [Symbol.dispose]() {}
    }

    class IncomingBody {
        constructor(data) {
            this._stream = new InputStream(data);
            resourceRegistry.register(this, 'incoming-body');
        }
        stream() { return this._stream; }
        static finish(body) { return undefined; }
        [Symbol.dispose]() {}
    }

    class FutureIncomingResponse {
        constructor(responsePromise) {
            this._promise = responsePromise;
            this._result = undefined;
            this._done = false;
            resourceRegistry.register(this, 'future-incoming-response');
            
            // Start the request
            this._promise.then(
                (response) => {
                    this._result = { tag: 'ok', val: response };
                    this._done = true;
                },
                (error) => {
                    this._result = { tag: 'err', val: { tag: 'internal-error', val: String(error) } };
                    this._done = true;
                }
            );
        }
        subscribe() { return new Pollable(this._done); }
        get() {
            if (!this._done) return undefined;
            return this._result;
        }
        [Symbol.dispose]() {}
    }

    class RequestOptions {
        constructor() {
            this._connectTimeout = null;
            this._firstByteTimeout = null;
            this._betweenBytesTimeout = null;
            resourceRegistry.register(this, 'request-options');
        }
        connectTimeout() { return this._connectTimeout; }
        setConnectTimeout(t) { this._connectTimeout = t; }
        firstByteTimeout() { return this._firstByteTimeout; }
        setFirstByteTimeout(t) { this._firstByteTimeout = t; }
        betweenBytesTimeout() { return this._betweenBytesTimeout; }
        setBetweenBytesTimeout(t) { this._betweenBytesTimeout = t; }
        [Symbol.dispose]() {}
    }

    class Descriptor {
        constructor() {
            resourceRegistry.register(this, 'descriptor');
        }
        openAt() { throw { tag: 'no-entry' }; }
        stat() {
            return {
                type: 'directory',
                linkCount: BigInt(1),
                size: BigInt(0),
                dataAccessTimestamp: null,
                dataModificationTimestamp: null,
                statusChangeTimestamp: null,
            };
        }
        writeViaStream() { return new OutputStream(); }
        readViaStream() { return new InputStream(); }
        appendViaStream() { return new OutputStream(); }
        getType() { return 'directory'; }
        [Symbol.dispose]() {}
    }

    // HTTP handler using Node.js fetch
    function handle(request, options) {
        const scheme = request._scheme || 'http';
        const authority = request._authority || 'localhost';
        const pathWithQuery = request._pathWithQuery || '/';
        const url = `${scheme}://${authority}${pathWithQuery}`;
        
        const method = request._method || 'GET';
        
        // Build headers
        const headers = {};
        if (request._headers && request._headers._map) {
            for (const [k, vs] of request._headers._map) {
                if (vs.length > 0) {
                    headers[k] = new TextDecoder().decode(vs[0]);
                }
            }
        }
        
        // Get body if present
        let body = undefined;
        if (request._body && request._body._stream && request._body._stream.chunks.length > 0) {
            body = request._body._stream.getContents();
        }
        
        // Make the request
        const responsePromise = fetch(url, {
            method,
            headers,
            body,
        }).then(async (resp) => {
            const respHeaders = new Fields();
            for (const [k, v] of resp.headers) {
                respHeaders.set(k, [new TextEncoder().encode(v)]);
            }
            const bodyData = new Uint8Array(await resp.arrayBuffer());
            return new IncomingResponse(resp.status, respHeaders, bodyData);
        });
        
        return new FutureIncomingResponse(responsePromise);
    }

    return {
        'wasi:cli/environment': {
            getEnvironment: () => [],
        },
        'wasi:cli/exit': {
            exit: (status) => process.exit(status?.tag === 'ok' ? 0 : 1),
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
            handle,
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
            poll: (pollables) => pollables.map((_, i) => i),
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
    console.log("=== Thymos WASM Component Test (Server Mode) ===\n");

    // Instantiate
    console.log("Instantiating WASM component...");
    const imports = createWasiShims();
    const { agent, memory, storage } = await instantiate(getCoreModule, imports);
    console.log("✓ Component instantiated\n");

    // Test 1: Check initial state
    console.log("1. Initial state check...");
    console.log(`   Connected to server: ${storage.isConnected()}`);
    console.log(`   Memory count: ${memory.count()}`);

    // Test 2: Connect to Locai server
    console.log("\n2. Connecting to Locai server...");
    try {
        storage.connect("http://localhost:3000", undefined);
        console.log(`   ✓ Connected! isConnected: ${storage.isConnected()}`);
    } catch (e) {
        console.error(`   ✗ Failed to connect: ${e}`);
        console.log("\n   Make sure Locai server is running on http://localhost:3000");
        console.log("   Falling back to in-memory mode for remaining tests...\n");
        
        // Continue with in-memory mode
        runInMemoryTests(agent, memory, storage);
        return;
    }

    // Test 3: Create an agent
    console.log("\n3. Creating agent...");
    try {
        agent.create("wasm-server-test-agent");
        console.log(`   ✓ Agent created: ${agent.id()}`);
        console.log(`   ✓ Status: ${JSON.stringify(agent.status())}`);
    } catch (e) {
        console.error(`   ✗ Failed: ${e}`);
    }

    // Test 4: Store memories via server
    console.log("\n4. Storing memories (via Locai server)...");
    const testMemories = [
        { content: "WASM test: Alice met Bob at the coffee shop in Paris", type: null },
        { content: "WASM test: Paris is the capital of France", type: "fact" },
        { content: "WASM test: The meeting discussed the project timeline", type: "conversation" },
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
            console.log(`   ✓ Stored: "${mem.content.substring(0, 45)}..." (${id})`);
        } catch (e) {
            console.error(`   ✗ Failed to store: ${e}`);
        }
    }

    // Test 5: Search via server (semantic search)
    console.log("\n5. Searching memories (semantic search via Locai)...");
    const queries = ["coffee meeting", "French capital", "project planning"];

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

    // Test 6: Disconnect
    console.log("\n6. Disconnecting...");
    try {
        storage.disconnect();
        console.log(`   ✓ Disconnected. isConnected: ${storage.isConnected()}`);
    } catch (e) {
        console.error(`   ✗ Failed: ${e}`);
    }

    console.log("\n=== Test Complete ===");
    console.log("\nThe WASM component successfully connected to Locai server!");
}

function runInMemoryTests(agent, memory, storage) {
    console.log("--- Running In-Memory Tests ---\n");
    
    agent.create("fallback-agent");
    console.log(`Created agent: ${agent.id()}`);
    
    memory.remember("Test memory 1");
    memory.remember("Test memory 2");
    console.log(`Stored 2 memories. Count: ${memory.count()}`);
    
    const results = memory.search("memory", { limit: 5 });
    console.log(`Search results: ${results.length}`);
    
    console.log("\n=== In-Memory Test Complete ===");
}

main().catch(console.error);

