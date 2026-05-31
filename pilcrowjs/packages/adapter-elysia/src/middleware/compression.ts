import { Elysia } from 'elysia';
import { gzipSync, deflateSync } from 'node:zlib';

export const compression = () => (app: Elysia) => {
  return app.mapResponse(async ({ response, request, set }): Promise<Response | undefined> => {
    if (response === undefined || response === null) {
      return undefined;
    }

    const acceptEncoding = request.headers.get('accept-encoding') || '';
    let body: any;
    let headers: Record<string, string> = {};
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
      } catch {
        return response;
      }
    } else if (typeof response === 'string') {
      body = Buffer.from(response);
      headers['content-type'] = 'text/html; charset=utf-8';
    } else if (typeof response === 'object') {
      body = Buffer.from(JSON.stringify(response));
      headers['content-type'] = 'application/json; charset=utf-8';
    } else {
      return undefined;
    }

    // Skip compression for small responses (< 1024 bytes)
    if (body.byteLength < 1024) {
      const mergedHeaders: Record<string, string> = { ...headers };
      if (set.headers) {
        Object.entries(set.headers).forEach(([k, v]) => {
          if (v !== undefined && v !== null) {
            mergedHeaders[k.toLowerCase()] = String(v);
          }
        });
      }
      return new Response(body, {
        status,
        headers: mergedHeaders as any,
      });
    }

    if (acceptEncoding.includes('gzip')) {
      const compressed = gzipSync(body);
      const mergedHeaders: Record<string, string> = {
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
        headers: mergedHeaders as any,
      });
    } else if (acceptEncoding.includes('deflate')) {
      const compressed = deflateSync(body);
      const mergedHeaders: Record<string, string> = {
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
        headers: mergedHeaders as any,
      });
    }

    const mergedHeaders: Record<string, string> = { ...headers };
    if (set.headers) {
      Object.entries(set.headers).forEach(([k, v]) => {
        if (v !== undefined && v !== null) {
          mergedHeaders[k.toLowerCase()] = String(v);
        }
      });
    }
    return new Response(body, {
      status,
      headers: mergedHeaders as any,
    });
  });
};
