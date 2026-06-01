import type { NodePgDatabase } from 'drizzle-orm/node-postgres';
export type HitStatus = 'Tombstoned' | 'JustPromoted' | 'Normal';
export interface StaleSlot {
    route: string;
    slot: string;
    query: string | null;
    queryParams: any;
    dependsOn: string[];
    promoted: boolean;
    debounceSecs: number | null;
    htmlPath: string | null;
    jsonPath: string | null;
    columnName: string | null;
}
export interface EvictedRoute {
    route: string;
    htmlPath: string | null;
    jsonPath: string | null;
}
export interface InspectRow {
    route: string;
    slot: string;
    dependsOn: string[];
    stale: boolean;
    version: number;
    hitCount: number;
    promoted: boolean;
    htmlPath: string | null;
    jsonPath: string | null;
    lastHit: string | null;
}
export declare class FsrStore {
    private db;
    private globalDebounceSecs;
    private redis;
    private pool;
    constructor(db: NodePgDatabase<any>, globalDebounceSecs?: number, redis?: any);
    withPool(pool: any): this;
    withGlobalDebounce(secs: number): this;
    withRedis(redis: any): this;
    ensureRouteRow(route: string, promoteAfter?: number): Promise<void>;
    upsertSlot(route: string, slot: string, querySql: string | null, queryParams: any, dependsOn: string[], debounceSecs?: number, columnName?: string): Promise<void>;
    incrementHit(route: string): Promise<HitStatus>;
    tombstone(route: string): Promise<void>;
    isTombstoned(route: string): Promise<boolean>;
    invalidateDepKey(depKey: string): Promise<string[]>;
    invalidateRoute(route: string): Promise<void>;
    fetchStaleSlots(): Promise<StaleSlot[]>;
    getPromotedPaths(route: string): Promise<{
        htmlPath: string | null;
        jsonPath: string | null;
    } | null>;
    setBakedPaths(route: string, htmlPath: string, jsonPath: string | null): Promise<void>;
    evictIdleRoutes(thresholdSecs: number): Promise<EvictedRoute[]>;
    markFresh(route: string, slot: string): Promise<void>;
    fetchSlotsForSnapshot(route: string, slots: string[]): Promise<StaleSlot[]>;
    fetchAllForInspect(): Promise<InspectRow[]>;
    reExecuteQuery(slot: StaleSlot): Promise<any>;
}
//# sourceMappingURL=store.d.ts.map