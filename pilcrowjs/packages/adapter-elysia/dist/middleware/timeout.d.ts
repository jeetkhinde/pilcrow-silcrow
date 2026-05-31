import { Elysia } from 'elysia';
export declare const timeout: (timeoutMs?: number) => (app: Elysia) => Elysia<"", {
    decorator: {};
    store: {};
    derive: {};
    resolve: {};
}, {
    typebox: {};
    error: {};
}, {
    schema: {};
    standaloneSchema: {};
    macro: {};
    macroFn: {};
    parser: {};
    response: {};
}, {}, {
    derive: {};
    resolve: {};
    schema: {};
    standaloneSchema: {};
    response: {};
}, {
    derive: {
        readonly abortController: AbortController;
        readonly timeoutId: NodeJS.Timeout;
    };
    resolve: {};
    schema: {};
    standaloneSchema: {};
    response: {
        200: "Request Timeout";
    };
}>;
export declare function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T>;
//# sourceMappingURL=timeout.d.ts.map