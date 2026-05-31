import { gzipSync, deflateSync } from 'node:zlib';
export const compression = () => (app) => {
    return app.mapResponse(async ({ response, request, set }) => {
        if (response === undefined || response === null) {
            return undefined;
        }
        const acceptEncoding = request.headers.get('accept-encoding') || '';
        let body;
        let headers = {};
        let status = 200;
        if (response instanceof Response) {
            status = response.status;
            response.headers.forEach((v, k) => {
                headers[k.toLowerCase()] = v;
            });
            if (headers['content-encoding'] || response.status === 204 || response.status === 304) {
                return response;
            }
            try {
                body = Buffer.from(await response.arrayBuffer());
            }
            catch {
                return response;
            }
        }
        else if (typeof response === 'string') {
            body = Buffer.from(response);
            headers['content-type'] = 'text/html; charset=utf-8';
        }
        else if (typeof response === 'object') {
            body = Buffer.from(JSON.stringify(response));
            headers['content-type'] = 'application/json; charset=utf-8';
        }
        else {
            return undefined;
        }
        // Skip compression for small responses (< 1024 bytes)
        if (body.byteLength < 1024) {
            const mergedHeaders = { ...headers };
            if (set.headers) {
                Object.entries(set.headers).forEach(([k, v]) => {
                    if (v !== undefined && v !== null) {
                        mergedHeaders[k.toLowerCase()] = String(v);
                    }
                });
            }
            return new Response(body, {
                status,
                headers: mergedHeaders,
            });
        }
        if (acceptEncoding.includes('gzip')) {
            const compressed = gzipSync(body);
            const mergedHeaders = {
                ...headers,
                'content-encoding': 'gzip',
                'content-length': String(compressed.byteLength),
            };
            if (set.headers) {
                Object.entries(set.headers).forEach(([k, v]) => {
                    if (v !== undefined && v !== null) {
                        mergedHeaders[k.toLowerCase()] = String(v);
                    }
                });
            }
            return new Response(compressed, {
                status,
                headers: mergedHeaders,
            });
        }
        else if (acceptEncoding.includes('deflate')) {
            const compressed = deflateSync(body);
            const mergedHeaders = {
                ...headers,
                'content-encoding': 'deflate',
                'content-length': String(compressed.byteLength),
            };
            if (set.headers) {
                Object.entries(set.headers).forEach(([k, v]) => {
                    if (v !== undefined && v !== null) {
                        mergedHeaders[k.toLowerCase()] = String(v);
                    }
                });
            }
            return new Response(compressed, {
                status,
                headers: mergedHeaders,
            });
        }
        const mergedHeaders = { ...headers };
        if (set.headers) {
            Object.entries(set.headers).forEach(([k, v]) => {
                if (v !== undefined && v !== null) {
                    mergedHeaders[k.toLowerCase()] = String(v);
                }
            });
        }
        return new Response(body, {
            status,
            headers: mergedHeaders,
        });
    });
};
//# sourceMappingURL=compression.js.map