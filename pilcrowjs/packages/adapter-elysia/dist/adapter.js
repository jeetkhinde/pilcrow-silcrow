import { Elysia } from 'elysia';
import { wrapRequest, ElysiaResponseImpl, handleElysiaResponse } from './context.js';
import { bodyLimit, csrf, timeout, compression, layoutIntercept } from './middleware/index.js';
import { createRequire } from 'module';
const requireFn = typeof require !== 'undefined' ? require : createRequire(import.meta.url);
export class ElysiaAdapter {
    app;
    constructor(options) {
        if (options?.elysia) {
            this.app = options.elysia;
        }
        else {
            // @ts-ignore
            if (typeof Bun === 'undefined') {
                try {
                    const { node } = requireFn('@elysiajs/node');
                    this.app = new Elysia({ adapter: node() });
                }
                catch (e) {
                    console.warn('Failed to load @elysiajs/node adapter, falling back to default:', e);
                    this.app = new Elysia();
                }
            }
            else {
                this.app = new Elysia();
            }
        }
    }
    registerPage(pattern, layouts, handler) {
        this.app.get(pattern, async (ctx) => {
            const req = wrapRequest(ctx);
            const res = new ElysiaResponseImpl(ctx);
            await handler(req, res);
            return handleElysiaResponse(res, ctx);
        });
    }
    registerAction(pattern, handler) {
        this.app.post(pattern, async (ctx) => {
            const req = wrapRequest(ctx);
            const res = new ElysiaResponseImpl(ctx);
            await handler(req, res);
            return handleElysiaResponse(res, ctx);
        });
    }
    registerSSE(pattern, handler) {
        this.app.get(pattern, async (ctx) => {
            const req = wrapRequest(ctx);
            const res = new ElysiaResponseImpl(ctx);
            await handler(req, res);
            return handleElysiaResponse(res, ctx);
        });
    }
    registerAsset(urlPath, filePath) {
        this.app.get(urlPath, () => {
            // @ts-ignore
            if (typeof Bun !== 'undefined') {
                // @ts-ignore
                return Bun.file(filePath);
            }
            // Node fallback for fs
            try {
                const fs = require('fs');
                return new Response(fs.readFileSync(filePath));
            }
            catch (e) {
                return new Response('File not found', { status: 404 });
            }
        });
    }
    applyMiddleware(config) {
        if (config.bodyLimitBytes !== undefined) {
            this.app.use(bodyLimit(config.bodyLimitBytes));
        }
        else {
            this.app.use(bodyLimit());
        }
        if (config.csrf !== false) {
            this.app.use(csrf());
        }
        if (config.timeoutMs !== undefined) {
            this.app.use(timeout(config.timeoutMs));
        }
        else {
            this.app.use(timeout());
        }
        if (config.compression !== false) {
            this.app.use(compression());
        }
        this.app.use(layoutIntercept());
    }
    async listen(port, callback) {
        this.app.listen(port, () => {
            const hostname = this.app.server?.hostname || 'localhost';
            const serverPort = this.app.server?.port || port;
            callback?.(`http://${hostname}:${serverPort}`);
        });
    }
}
//# sourceMappingURL=adapter.js.map