export const DEFAULT_CONFIG = {
    web: {
        host: '127.0.0.1',
        port: 3000,
        backendUrl: 'http://127.0.0.1:4000',
        requestBodyLimitBytes: 2 * 1024 * 1024, // 2 MiB
    },
    backend: {
        host: '127.0.0.1',
        port: 4000,
    },
    cache: {
        provider: 'memory',
    },
    serviceWorker: {
        enabled: false,
        strategy: 'network-first',
        precache: [],
        exclude: [],
    },
    i18n: {
        defaultLocale: 'en',
        locales: [],
        localesDir: 'locales',
    },
    images: {
        enabled: false,
        cacheDir: '.pilcrow-image-cache',
        domains: [],
        maxWidth: 3840,
        maxHeight: 2160,
        quality: 75,
        formats: ['webp', 'jpeg'],
        concurrency: 4,
        staticDir: 'public',
    },
    client: {
        react: {
            ssr: false,
            nodeBin: 'node',
            concurrency: 4,
        },
        inlineRuntime: false,
    },
    live: {
        promoteAfterHits: 100,
        patchDebounceSeconds: 30,
        purgeAfterSeconds: 2_592_000, // 30 days
    },
    fsr: {
        watcher: 'embedded',
        pollIntervalMs: 500,
        promoteAfterHits: 100,
        patchDebounceSecs: 0,
        purgeAfterSeconds: 2_592_000,
        maxSseConnections: 1000,
        connectionTtlSecs: 3600,
        keepaliveSecs: 30,
        artifactTtlSecs: 86_400, // 24h
        idleEvictSecs: 1_800, // 30m
        idleThresholdSecs: 86_400, // 24h
    },
};
export function defineConfig(config) {
    const merged = { ...DEFAULT_CONFIG };
    if (config.web)
        merged.web = { ...DEFAULT_CONFIG.web, ...config.web };
    if (config.backend)
        merged.backend = { ...DEFAULT_CONFIG.backend, ...config.backend };
    if (config.cache)
        merged.cache = { ...DEFAULT_CONFIG.cache, ...config.cache };
    if (config.serviceWorker)
        merged.serviceWorker = { ...DEFAULT_CONFIG.serviceWorker, ...config.serviceWorker };
    if (config.i18n)
        merged.i18n = { ...DEFAULT_CONFIG.i18n, ...config.i18n };
    if (config.images)
        merged.images = { ...DEFAULT_CONFIG.images, ...config.images };
    if (config.client) {
        merged.client = {
            ...DEFAULT_CONFIG.client,
            ...config.client,
            react: { ...DEFAULT_CONFIG.client.react, ...config.client.react },
        };
    }
    if (config.live)
        merged.live = { ...DEFAULT_CONFIG.live, ...config.live };
    if (config.fsr)
        merged.fsr = { ...DEFAULT_CONFIG.fsr, ...config.fsr };
    if (config.port !== undefined)
        merged.port = config.port;
    if (config.pagesDir !== undefined)
        merged.pagesDir = config.pagesDir;
    if (config.apiDir !== undefined)
        merged.apiDir = config.apiDir;
    return merged;
}
export function loadConfigFromEnv(baseConfig) {
    const config = { ...baseConfig };
    if (process.env.PILCROW_WEB_HOST) {
        config.web.host = process.env.PILCROW_WEB_HOST;
    }
    if (process.env.PILCROW_WEB_PORT) {
        const port = parseInt(process.env.PILCROW_WEB_PORT, 10);
        if (!isNaN(port))
            config.web.port = port;
    }
    if (process.env.PILCROW_BACKEND_URL) {
        config.web.backendUrl = process.env.PILCROW_BACKEND_URL;
    }
    if (process.env.PILCROW_BACKEND_HOST) {
        config.backend.host = process.env.PILCROW_BACKEND_HOST;
    }
    if (process.env.PILCROW_BACKEND_PORT) {
        const port = parseInt(process.env.PILCROW_BACKEND_PORT, 10);
        if (!isNaN(port))
            config.backend.port = port;
    }
    return config;
}
//# sourceMappingURL=config.js.map