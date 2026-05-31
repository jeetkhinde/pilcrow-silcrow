export const defaultHubConfig = {
    maxConnections: 1000,
    connectionTtlSecs: 3600,
    keepaliveSecs: 30,
};
// Global active connections counter
let activeConnectionsCount = 0;
export function getActiveConnectionsCount() {
    return activeConnectionsCount;
}
export async function* fsrHubStream(options) {
    const { route, slots, watcher, config = defaultHubConfig } = options;
    // Check connection counter
    if (activeConnectionsCount >= config.maxConnections) {
        throw new Error('SERVICE_UNAVAILABLE: FSR connection limit reached');
    }
    activeConnectionsCount++;
    const emitter = watcher.getEmitter();
    const queue = [];
    let resolveNext = null;
    let lagged = false;
    const onPatch = (patch) => {
        if (queue.length >= 100) {
            lagged = true;
            queue.shift(); // drop oldest to make room
        }
        queue.push(patch);
        if (resolveNext) {
            resolveNext();
            resolveNext = null;
        }
    };
    emitter.on('patch', onPatch);
    let keepaliveTimer = null;
    let triggerKeepalive = false;
    const resetKeepalive = () => {
        if (keepaliveTimer)
            clearInterval(keepaliveTimer);
        keepaliveTimer = setInterval(() => {
            triggerKeepalive = true;
            if (resolveNext) {
                resolveNext();
                resolveNext = null;
            }
        }, config.keepaliveSecs * 1000);
    };
    resetKeepalive();
    let ttlExpired = false;
    const ttlTimer = setTimeout(() => {
        ttlExpired = true;
        if (resolveNext) {
            resolveNext();
            resolveNext = null;
        }
    }, config.connectionTtlSecs * 1000);
    try {
        while (!ttlExpired) {
            if (queue.length === 0 && !triggerKeepalive && !lagged && !ttlExpired) {
                await new Promise((resolve) => {
                    resolveNext = resolve;
                });
            }
            if (ttlExpired)
                break;
            if (lagged) {
                lagged = false;
                yield { event: 'fsr-resync', data: 'lagged' };
                continue;
            }
            if (triggerKeepalive) {
                triggerKeepalive = false;
                yield { data: '' }; // keepalive comment/heartbeat
                continue;
            }
            while (queue.length > 0) {
                const patch = queue.shift();
                if (patch.route !== route)
                    continue;
                if (slots.length > 0 && !slots.includes(patch.slot))
                    continue;
                resetKeepalive(); // reset heartbeat on message
                yield {
                    event: 'fsr',
                    data: JSON.stringify({ [patch.slot]: patch.value })
                };
            }
        }
    }
    finally {
        activeConnectionsCount--;
        emitter.off('patch', onPatch);
        if (keepaliveTimer)
            clearInterval(keepaliveTimer);
        clearTimeout(ttlTimer);
    }
}
export async function fsrSnapshotHandler(route, slots, store) {
    const matchingSlots = await store.fetchSlotsForSnapshot(route, slots);
    const result = {};
    for (const slot of matchingSlots) {
        try {
            const val = await store.reExecuteQuery(slot);
            if (val !== null) {
                result[slot.slot] = val;
            }
        }
        catch (e) {
            console.warn(`FSR snapshot: query error for slot ${slot.slot}:`, e.message);
        }
    }
    return result;
}
//# sourceMappingURL=hub.js.map