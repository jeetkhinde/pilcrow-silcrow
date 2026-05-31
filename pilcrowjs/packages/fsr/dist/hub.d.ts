import { FsrStore } from './store.js';
import { FsrWatcher } from './watcher.js';
import { SSEEvent } from '@pilcrowjs/core';
export interface FsrHubConfig {
    maxConnections: number;
    connectionTtlSecs: number;
    keepaliveSecs: number;
}
export declare const defaultHubConfig: FsrHubConfig;
export declare function getActiveConnectionsCount(): number;
export interface FsrHubStreamOptions {
    route: string;
    slots: string[];
    watcher: FsrWatcher;
    config?: FsrHubConfig;
}
export declare function fsrHubStream(options: FsrHubStreamOptions): AsyncGenerator<SSEEvent, void, unknown>;
export declare function fsrSnapshotHandler(route: string, slots: string[], store: FsrStore): Promise<Record<string, any>>;
//# sourceMappingURL=hub.d.ts.map