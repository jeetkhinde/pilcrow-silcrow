export function wrapRequest(ctx) {
    const req = ctx.request;
    const isEnhanced = req.headers.get('silcrow-target') !== null;
    const layoutsPresent = (req.headers.get('x-ps-present') || '')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
    return {
        path: ctx.path || new URL(req.url).pathname,
        method: req.method,
        params: ctx.params || {},
        query: ctx.query || {},
        headers: req.headers,
        formData: () => req.formData(),
        json: () => req.json(),
        isEnhanced,
        layoutsPresent,
        raw: ctx,
    };
}
export class ElysiaResponseImpl {
    ctx;
    status = 200;
    headers = {};
    body;
    bodyType;
    redirectUrl;
    constructor(ctx) {
        this.ctx = ctx;
    }
    html(body) {
        this.body = body;
        this.bodyType = 'html';
        this.ctx.set.headers['content-type'] = 'text/html; charset=utf-8';
    }
    json(body) {
        this.body = body;
        this.bodyType = 'json';
        this.ctx.set.headers['content-type'] = 'application/json';
    }
    redirect(url, status = 303) {
        this.status = status;
        this.redirectUrl = url;
        this.bodyType = 'redirect';
        this.ctx.set.status = status;
        this.ctx.set.headers['location'] = url;
    }
    sse(stream) {
        this.body = stream;
        this.bodyType = 'sse';
    }
}
export function sseToReadableStream(stream) {
    return new ReadableStream({
        async start(controller) {
            const encoder = new TextEncoder();
            try {
                for await (const chunk of stream) {
                    let payload = '';
                    if (chunk.event)
                        payload += `event: ${chunk.event}\n`;
                    if (chunk.id)
                        payload += `id: ${chunk.id}\n`;
                    if (chunk.retry !== undefined)
                        payload += `retry: ${chunk.retry}\n`;
                    const lines = chunk.data.split('\n');
                    for (const line of lines) {
                        payload += `data: ${line}\n`;
                    }
                    payload += '\n';
                    controller.enqueue(encoder.encode(payload));
                }
            }
            catch (err) {
                controller.error(err);
            }
            finally {
                controller.close();
            }
        },
    });
}
export function handleElysiaResponse(res, ctx) {
    if (res.status) {
        ctx.set.status = res.status;
    }
    for (const [key, value] of Object.entries(res.headers)) {
        ctx.set.headers[key] = value;
    }
    if (res.bodyType === 'redirect') {
        return;
    }
    if (res.bodyType === 'sse') {
        ctx.set.headers['content-type'] = 'text/event-stream';
        ctx.set.headers['cache-control'] = 'no-cache';
        ctx.set.headers['connection'] = 'keep-alive';
        return new Response(sseToReadableStream(res.body), {
            status: res.status,
            headers: ctx.set.headers,
        });
    }
    return res.body;
}
//# sourceMappingURL=context.js.map