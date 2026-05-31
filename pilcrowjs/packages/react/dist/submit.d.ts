/**
 * Full response shape returned by `window.Silcrow.submit`.
 */
export type SilcrowSubmitResult<T = unknown> = {
    ok: boolean;
    status: number;
    data: T;
    html: string | null;
    headers: Headers;
    mutationId?: string;
};
/**
 * Network options forwarded to `Silcrow.submit`.
 */
export type SilcrowSubmitOptions = {
    method?: string;
    scope?: string;
    headers?: Record<string, string>;
    optimistic?: {
        /** Atom scope to patch optimistically (must match a `s-bind` scope or atom). */
        scope: string;
        /** Data to apply immediately before the round-trip completes. */
        data: unknown;
        /** Stable client id for this mutation; auto-generated when omitted. */
        mutationId?: string;
    };
};
/**
 * Options for the React action wrapper.
 */
export type SilcrowActionOptions = SilcrowSubmitOptions & {
    permalink?: string;
};
/**
 * Browser global installed by `silcrow.js`.
 */
declare global {
    interface Window {
        Silcrow?: {
            subscribe?: (scope: string, fn: () => void) => () => void;
            snapshot?: <T = unknown>(scope: string) => T | undefined;
            publish?: (scope: string, data: unknown) => void;
            prefetch?: <T = unknown>(path: string) => Promise<T>;
            submit?: <T = unknown>(url: string, body?: BodyInit | object | null, options?: SilcrowSubmitOptions) => Promise<SilcrowSubmitResult<T>>;
            publishOptimistic?: (scope: string, data: unknown, mutationId: string) => void;
            confirmOptimistic?: (mutationId: string) => void;
            revertOptimistic?: (mutationId: string) => void;
        };
    }
}
/**
 * Create a React 19 form action backed by Silcrow transport.
 *
 * Use this with React's `useActionState` when you want the raw primitive.
 */
export declare function submitSilcrow<T>(url: string, options?: SilcrowSubmitOptions): (_prev: T, formData: FormData) => Promise<T>;
/**
 * Create an async submit callback for React Hook Form or other form libraries.
 *
 * Silcrow/Pilcrow stays responsible for transport; the form library owns
 * validation, dirty/touched state, focus, arrays, and nested fields.
 */
export declare function silcrowSubmitHandler<Result = unknown, Values = object>(url: string, options?: SilcrowSubmitOptions): (values: Values) => Promise<SilcrowSubmitResult<Result>>;
//# sourceMappingURL=submit.d.ts.map