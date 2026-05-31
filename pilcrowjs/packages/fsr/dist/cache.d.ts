import { Redis } from 'ioredis';
export interface InvalidatePayload {
    route: string;
    slots: string[];
    deps: string[];
}
export interface PatchPayload {
    route: string;
    slot: string;
    value: any;
}
export declare class RedisCache {
    private client;
    private artifactTtlSecs;
    constructor(url: string);
    withArtifactTtl(ttlSecs: number): this;
    getClient(): Redis;
    private htmlKey;
    private slotKey;
    private jsonKey;
    getHtml(route: string): Promise<string | null>;
    setHtml(route: string, html: string): Promise<void>;
    patchSlot(route: string, slot: string, value: string): Promise<void>;
    getSlots(route: string): Promise<Record<string, string>>;
    setJson(route: string, json: any): Promise<void>;
    getJson(route: string): Promise<any | null>;
    publishInvalidate(payload: InvalidatePayload): Promise<void>;
    publishPatch(payload: PatchPayload): Promise<void>;
    deleteRouteKeys(route: string): Promise<void>;
    disconnect(): Promise<void>;
}
//# sourceMappingURL=cache.d.ts.map