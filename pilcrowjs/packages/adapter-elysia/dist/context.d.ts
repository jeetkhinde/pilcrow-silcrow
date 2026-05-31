import type { PilcrowRequest, PilcrowResponse, SSEEvent } from '@pilcrowjs/core';
export declare function wrapRequest(ctx: any): PilcrowRequest;
export declare class ElysiaResponseImpl implements PilcrowResponse {
    private ctx;
    status: number;
    headers: Record<string, string>;
    body?: any;
    bodyType?: 'html' | 'json' | 'sse' | 'redirect';
    redirectUrl?: string;
    constructor(ctx: any);
    html(body: string): void;
    json(body: unknown): void;
    redirect(url: string, status?: number): void;
    sse(stream: AsyncIterable<SSEEvent>): void;
}
export declare function sseToReadableStream(stream: AsyncIterable<SSEEvent>): ReadableStream;
export declare function handleElysiaResponse(res: ElysiaResponseImpl, ctx: any): any;
//# sourceMappingURL=context.d.ts.map