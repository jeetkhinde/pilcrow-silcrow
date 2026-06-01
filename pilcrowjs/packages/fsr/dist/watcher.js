import { EventEmitter } from 'node:events';
import fs from 'node:fs/promises';
import { injectFsrSlots } from './baking.js';
export class FsrWatcher {
    store;
    redis;
    config;
    active = false;
    abortController = new AbortController();
    emitter = new EventEmitter();
    constructor(store, redis, config) {
        this.store = store;
        this.redis = redis;
        this.config = config;
    }
    getEmitter() {
        return this.emitter;
    }
    async start() {
        if (this.active)
            return;
        this.active = true;
        this.abortController = new AbortController();
        const signal = this.abortController.signal;
        // 1. Scheduled invalidations
        for (const scheduled of this.config.scheduledInvalidations) {
            this.spawnSupervisedInvalidation(scheduled, signal);
        }
        // 2. Idle eviction
        if (this.config.idleEvictSecs > 0) {
            this.spawnSupervisedIdleEviction(signal);
        }
        // 3. Watcher main loop
        if (this.redis) {
            this.spawnSupervisedRedisWatcher(signal);
        }
        else {
            this.spawnSupervisedPollingWatcher(signal);
        }
    }
    async stop() {
        this.active = false;
        this.abortController.abort();
    }
    notifyChange(depKey) {
        // Invalidate dep keys in DB
        this.store.invalidateDepKey(depKey).catch(err => {
            console.error(`Failed to invalidate dep key ${depKey}:`, err);
        });
    }
    spawnSupervisedInvalidation(scheduled, signal) {
        const run = async () => {
            while (!signal.aborted) {
                await new Promise(resolve => setTimeout(resolve, scheduled.intervalMs));
                if (signal.aborted)
                    break;
                try {
                    await this.store.invalidateDepKey(scheduled.depKey);
                }
                catch (err) {
                    console.error(`FSR: scheduled invalidation failed for ${scheduled.depKey}:`, err.message);
                }
            }
        };
        run();
    }
    spawnSupervisedIdleEviction(signal) {
        const run = async () => {
            while (!signal.aborted) {
                await new Promise(resolve => setTimeout(resolve, this.config.idleEvictSecs * 1000));
                if (signal.aborted)
                    break;
                try {
                    const evicted = await this.store.evictIdleRoutes(this.config.idleThresholdSecs);
                    for (const r of evicted) {
                        console.log(`FSR: idle eviction for route ${r.route}`);
                        if (this.redis) {
                            await this.redis.deleteRouteKeys(r.route).catch(() => { });
                        }
                        if (r.htmlPath) {
                            await fs.unlink(r.htmlPath).catch(() => { });
                        }
                        if (r.jsonPath) {
                            await fs.unlink(r.jsonPath).catch(() => { });
                        }
                    }
                }
                catch (err) {
                    console.error('FSR: idle eviction loop failed:', err.message);
                }
            }
        };
        run();
    }
    spawnSupervisedPollingWatcher(signal) {
        const run = async () => {
            while (!signal.aborted) {
                try {
                    await this.watcherTick();
                }
                catch (err) {
                    console.error('FSR watcher tick failed:', err.message);
                }
                await new Promise(resolve => setTimeout(resolve, this.config.pollIntervalMs));
            }
        };
        run();
    }
    spawnSupervisedRedisWatcher(signal) {
        const run = async () => {
            let subClient = null;
            while (!signal.aborted) {
                try {
                    if (!this.redis)
                        break;
                    subClient = this.redis.getClient().duplicate();
                    await subClient.subscribe('pilcrow:invalidate');
                    console.log('FSR watcher: subscribed to pilcrow:invalidate');
                    const handleMessage = async (channel, _message) => {
                        if (channel === 'pilcrow:invalidate') {
                            try {
                                await this.watcherTickRedis();
                            }
                            catch (err) {
                                console.error('FSR watcher: tick failed after invalidation event:', err.message);
                            }
                        }
                    };
                    subClient.on('message', handleMessage);
                    // Spawn a reconciliation tick timer (runs every 60 seconds as a fallback check)
                    const reconciliationInterval = setInterval(async () => {
                        try {
                            await this.watcherTickRedis();
                        }
                        catch (err) {
                            console.error('FSR watcher: reconciliation tick failed:', err.message);
                        }
                    }, 60000);
                    signal.addEventListener('abort', () => {
                        clearInterval(reconciliationInterval);
                        subClient?.quit().catch(() => { });
                    });
                    // Wait until aborted or connection closes
                    await new Promise((_, reject) => {
                        subClient.on('end', () => reject(new Error('Redis connection closed')));
                        signal.addEventListener('abort', () => reject(new Error('Aborted')));
                    });
                }
                catch (err) {
                    if (signal.aborted)
                        break;
                    console.warn('FSR watcher: Redis connection dropped or failed. Switching to poll fallback...', err.message);
                    // Polling fallback while disconnected
                    try {
                        await this.watcherTickRedis();
                    }
                    catch (e) {
                        console.error('FSR watcher: fallback tick failed:', e.message);
                    }
                    // Wait fallback interval before attempting reconnection
                    await new Promise(resolve => setTimeout(resolve, Math.max(100, this.config.pollIntervalMs)));
                }
                finally {
                    if (subClient) {
                        subClient.quit().catch(() => { });
                    }
                }
            }
        };
        run();
    }
    async watcherTick() {
        const stale = await this.store.fetchStaleSlots();
        if (stale.length === 0)
            return;
        // Phase 1: run DB queries
        const results = [];
        for (const slotRow of stale) {
            try {
                const value = await this.store.reExecuteQuery(slotRow);
                results.push({ slotRow, value });
            }
            catch (err) {
                console.warn(`FSR watcher: failed to re-execute query for ${slotRow.route}/${slotRow.slot}:`, err.message);
                results.push({ slotRow, value: null, err });
            }
        }
        // Phase 2a: build layout slot batches for file baking
        const htmlPatches = new Map();
        const jsonPatches = new Map();
        for (const { slotRow, value, err } of results) {
            if (err)
                continue;
            if (slotRow.promoted) {
                if (slotRow.htmlPath) {
                    if (!htmlPatches.has(slotRow.htmlPath))
                        htmlPatches.set(slotRow.htmlPath, []);
                    htmlPatches.get(slotRow.htmlPath).push([slotRow.slot, value]);
                }
                if (slotRow.jsonPath) {
                    if (!jsonPatches.has(slotRow.jsonPath))
                        jsonPatches.set(slotRow.jsonPath, []);
                    jsonPatches.get(slotRow.jsonPath).push([slotRow.slot, value]);
                }
            }
        }
        // Phase 2b: write to files
        for (const [htmlPath, patches] of htmlPatches.entries()) {
            await this.patchHtmlFileBatchReturning(htmlPath, patches);
        }
        for (const [jsonPath, patches] of jsonPatches.entries()) {
            await this.patchJsonFileBatch(jsonPath, patches);
        }
        // Phase 2c: broadcast SSE and mark fresh
        for (const { slotRow, value, err } of results) {
            if (err)
                continue;
            this.emitter.emit('patch', {
                route: slotRow.route,
                slot: slotRow.slot,
                value
            });
            try {
                await this.store.markFresh(slotRow.route, slotRow.slot);
            }
            catch (err) {
                console.warn(`FSR watcher: failed to mark slot fresh for ${slotRow.route}/${slotRow.slot}:`, err.message);
            }
        }
    }
    async watcherTickRedis() {
        const stale = await this.store.fetchStaleSlots();
        if (stale.length === 0)
            return;
        // Phase 1: run DB queries
        const results = [];
        for (const slotRow of stale) {
            try {
                const value = await this.store.reExecuteQuery(slotRow);
                results.push({ slotRow, value });
            }
            catch (err) {
                console.warn(`FSR watcher (Redis): failed to re-execute query for ${slotRow.route}/${slotRow.slot}:`, err.message);
                results.push({ slotRow, value: null, err });
            }
        }
        // Phase 2a: build batches
        const htmlPatches = new Map();
        const jsonPatches = new Map();
        const redisJsonPatches = new Map();
        for (const { slotRow, value, err } of results) {
            if (err)
                continue;
            if (slotRow.promoted) {
                if (slotRow.htmlPath) {
                    if (!htmlPatches.has(slotRow.htmlPath))
                        htmlPatches.set(slotRow.htmlPath, []);
                    htmlPatches.get(slotRow.htmlPath).push([slotRow.slot, value]);
                }
                if (slotRow.jsonPath) {
                    if (!jsonPatches.has(slotRow.jsonPath))
                        jsonPatches.set(slotRow.jsonPath, []);
                    jsonPatches.get(slotRow.jsonPath).push([slotRow.slot, value]);
                    if (!redisJsonPatches.has(slotRow.route))
                        redisJsonPatches.set(slotRow.route, []);
                    redisJsonPatches.get(slotRow.route).push([slotRow.slot, value]);
                }
            }
        }
        // Phase 2b: patch files
        const htmlPatched = new Map();
        for (const [htmlPath, patches] of htmlPatches.entries()) {
            const patched = await this.patchHtmlFileBatchReturning(htmlPath, patches);
            htmlPatched.set(htmlPath, patched);
        }
        for (const [jsonPath, patches] of jsonPatches.entries()) {
            await this.patchJsonFileBatch(jsonPath, patches);
        }
        // Phase 2c: Redis JSON read/merge/write
        if (this.redis) {
            for (const [route, patches] of redisJsonPatches.entries()) {
                try {
                    const existing = await this.redis.getJson(route) || {};
                    for (const [slot, val] of patches) {
                        existing[slot] = val;
                    }
                    await this.redis.setJson(route, existing);
                }
                catch (e) {
                    console.warn(`FSR watcher: Redis setJson failed for ${route}:`, e.message);
                }
            }
        }
        // Phase 2d: update Redis HASH, Redis HTML, publish, SSE, mark fresh
        for (const { slotRow, value, err } of results) {
            if (err)
                continue;
            if (this.redis) {
                let valStr = '';
                if (value === null || value === undefined)
                    valStr = '';
                else if (typeof value === 'string')
                    valStr = value;
                else if (typeof value === 'object')
                    valStr = JSON.stringify(value);
                else
                    valStr = String(value);
                try {
                    await this.redis.patchSlot(slotRow.route, slotRow.slot, valStr);
                }
                catch (e) {
                    console.warn(`FSR watcher: Redis patchSlot failed for ${slotRow.route}/${slotRow.slot}:`, e.message);
                }
                if (slotRow.promoted && slotRow.htmlPath) {
                    const patchedHtml = htmlPatched.get(slotRow.htmlPath);
                    if (patchedHtml) {
                        try {
                            await this.redis.setHtml(slotRow.route, patchedHtml);
                        }
                        catch (e) {
                            console.warn(`FSR watcher: Redis setHtml failed for ${slotRow.route}:`, e.message);
                        }
                    }
                }
                try {
                    await this.redis.publishPatch({
                        route: slotRow.route,
                        slot: slotRow.slot,
                        value
                    });
                }
                catch (e) {
                    console.warn(`FSR watcher: Redis publishPatch failed for ${slotRow.route}/${slotRow.slot}:`, e.message);
                }
            }
            this.emitter.emit('patch', {
                route: slotRow.route,
                slot: slotRow.slot,
                value
            });
            try {
                await this.store.markFresh(slotRow.route, slotRow.slot);
            }
            catch (e) {
                console.warn(`FSR watcher: failed to mark slot fresh for ${slotRow.route}/${slotRow.slot}:`, e.message);
            }
        }
    }
    async patchHtmlFileBatchReturning(htmlPath, patches) {
        try {
            const html = await fs.readFile(htmlPath, 'utf8');
            const patched = injectFsrSlots(html, patches);
            await fs.writeFile(htmlPath, patched, 'utf8');
            return patched;
        }
        catch (err) {
            console.warn(`FSR watcher: failed to patch HTML file at ${htmlPath}:`, err.message);
            return null;
        }
    }
    async patchJsonFileBatch(jsonPath, patches) {
        try {
            let content = '{}';
            try {
                content = await fs.readFile(jsonPath, 'utf8');
            }
            catch {
                // ignore missing file, use empty JSON
            }
            let obj = {};
            try {
                obj = JSON.parse(content);
            }
            catch {
                obj = {};
            }
            for (const [slot, value] of patches) {
                obj[slot] = value;
            }
            await fs.writeFile(jsonPath, JSON.stringify(obj), 'utf8');
        }
        catch (err) {
            console.warn(`FSR watcher: failed to patch JSON file at ${jsonPath}:`, err.message);
        }
    }
}
//# sourceMappingURL=watcher.js.map