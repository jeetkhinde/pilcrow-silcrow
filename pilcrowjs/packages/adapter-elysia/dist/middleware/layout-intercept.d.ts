import { Elysia } from 'elysia';
export declare const layoutIntercept: () => (app: Elysia) => Elysia<"", {
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
        readonly isEnhanced: boolean;
        readonly layoutsPresent: string[];
    };
    resolve: {};
    schema: {};
    standaloneSchema: {};
    response: import("elysia").ExtractErrorFromHandle<{
        readonly isEnhanced: boolean;
        readonly layoutsPresent: string[];
    }>;
}>;
//# sourceMappingURL=layout-intercept.d.ts.map