import { Elysia } from 'elysia';
import type { ServerAdapter, PilcrowRequest, PilcrowResponse, MiddlewareConfig } from '@pilcrowjs/core';
export declare class ElysiaAdapter implements ServerAdapter {
    app: Elysia;
    constructor(options?: {
        elysia?: Elysia;
    });
    registerPage(pattern: string, layouts: string[], handler: (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>): void;
    registerAction(pattern: string, handler: (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>): void;
    registerSSE(pattern: string, handler: (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>): void;
    registerAsset(urlPath: string, filePath: string): void;
    applyMiddleware(config: MiddlewareConfig): void;
    listen(port: number, callback?: (addr: string) => void): Promise<void>;
}
//# sourceMappingURL=adapter.d.ts.map