export interface WebConfig {
    host: string;
    port: number;
    backendUrl: string;
    requestBodyLimitBytes: number;
}
export interface BackendConfig {
    host: string;
    port: number;
}
export interface LiveConfig {
    promoteAfterHits: number;
    patchDebounceSeconds: number;
    purgeAfterSeconds: number;
}
export interface FsrConfig {
    watcher: 'embedded' | 'external';
    pollIntervalMs: number;
    promoteAfterHits: number;
    patchDebounceSecs: number;
    purgeAfterSeconds: number;
    maxSseConnections: number;
    connectionTtlSecs: number;
    keepaliveSecs: number;
    redisUrl?: string;
    artifactTtlSecs: number;
    idleEvictSecs: number;
    idleThresholdSecs: number;
    revalidateSeconds?: number;
}
export interface ReactRuntimeConfig {
    ssr: boolean;
    nodeBin: string;
    concurrency: number;
}
export interface ClientRuntimeConfig {
    react: ReactRuntimeConfig;
    inlineRuntime: boolean;
}
export interface ImageConfig {
    enabled: boolean;
    cacheDir: string;
    domains: string[];
    maxWidth: number;
    maxHeight: number;
    quality: number;
    formats: string[];
    concurrency: number;
    staticDir: string;
}
export interface I18nConfig {
    defaultLocale: string;
    locales: string[];
    localesDir: string;
}
export type SwStrategy = 'network-first' | 'cache-first' | 'stale-while-revalidate';
export interface ServiceWorkerConfig {
    enabled: boolean;
    strategy: SwStrategy;
    precache: string[];
    exclude: string[];
    offlineFallback?: string;
}
export type CacheProvider = 'memory' | 'filesystem' | 'sqlite' | 'redis';
export interface CacheConfig {
    provider: CacheProvider;
    url?: string;
    path?: string;
    dir?: string;
}
export interface PilcrowConfig {
    web: WebConfig;
    backend: BackendConfig;
    cache: CacheConfig;
    serviceWorker: ServiceWorkerConfig;
    i18n: I18nConfig;
    images: ImageConfig;
    client: ClientRuntimeConfig;
    live: LiveConfig;
    fsr: FsrConfig;
}
export declare const DEFAULT_CONFIG: PilcrowConfig;
export declare function defineConfig(config: Partial<PilcrowConfig>): PilcrowConfig;
export declare function loadConfigFromEnv(baseConfig: PilcrowConfig): PilcrowConfig;
//# sourceMappingURL=config.d.ts.map