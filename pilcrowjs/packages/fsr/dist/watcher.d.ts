import { EventEmitter } from 'node:events';
import { FsrStore } from './store.js';
import { RedisCache } from './cache.js';
export interface ScheduledInvalidation {
    depKey: string;
    intervalMs: number;
}
export interface WatcherConfig {
    pollIntervalMs: number;
    promoteAfterHits: number;
    patchDebounceSecs: number;
    purgeAfterSeconds: number;
    scheduledInvalidations: ScheduledInvalidation[];
    idleEvictSecs: number;
    idleThresholdSecs: number;
}
export interface SlotPatch {
    route: string;
    slot: string;
    value: any;
}
export declare class FsrWatcher {
    private store;
    private redis;
    private config;
    private active;
    private abortController;
    private emitter;
    constructor(store: FsrStore, redis: RedisCache | null, config: WatcherConfig);
    getEmitter(): EventEmitter;
    start(): Promise<void>;
    stop(): Promise<void>;
    notifyChange(depKey: string): void;
    private spawnSupervisedInvalidation;
    private spawnSupervisedIdleEviction;
    private spawnSupervisedPollingWatcher;
    private spawnSupervisedRedisWatcher;
    private watcherTick;
    private watcherTickRedis;
    private patchHtmlFileBatch;
    private patchHtmlFileBatchReturning;
    private patchJsonFileBatch;
}
//# sourceMappingURL=watcher.d.ts.map