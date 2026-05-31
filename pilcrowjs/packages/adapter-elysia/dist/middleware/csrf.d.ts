import { Elysia } from 'elysia';
export declare const csrf: () => (app: Elysia) => Elysia<"", {
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
    derive: {};
    resolve: {};
    schema: {};
    standaloneSchema: {};
    response: {
        200: "CSRF: missing Host header" | "CSRF: missing Origin and Referer on state-changing request" | "CSRF: cross-origin form submission blocked";
    };
}>;
//# sourceMappingURL=csrf.d.ts.map