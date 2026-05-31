import { type ReactNode } from "react";
import { type SilcrowActionOptions, type SilcrowSubmitResult } from "./submit.js";
export type PilcrowReactContextValue = {
    actionBase?: string;
};
export declare const PilcrowReactContext: import("react").Context<PilcrowReactContextValue>;
export declare function PilcrowReactProvider({ value, children, }: {
    value: PilcrowReactContextValue;
    children: ReactNode;
}): import("react").FunctionComponentElement<import("react").ProviderProps<PilcrowReactContextValue>>;
export declare function resolvePilcrowAction(name: string, base?: string): string;
/**
 * Subscribe a React component to a Silcrow atom scope.
 */
export declare function useSilcrowAtom<T>(scope: string, fallback: T): T;
/**
 * Patch a Silcrow atom scope.
 */
export declare function publishSilcrowAtom<T>(scope: string, data: T): void;
/**
 * Prefetch a route and return Silcrow's memoized promise for React `use()`.
 */
export declare function useSilcrowPrefetch<T>(path: string): Promise<T>;
/**
 * Read a route atom by path.
 */
export declare function useSilcrowRoute<T>(path: string, fallback: T): T;
/**
 * Create a React 19 form action backed by Silcrow transport.
 */
export declare function useSilcrowAction<State>(url: string, initialState?: State, options?: SilcrowActionOptions): [State, (payload: FormData) => void, boolean];
/**
 * React 19 action wrapper that resolves a Pilcrow page/fragment named action.
 */
export declare function usePilcrowNamedAction<State>(name: string, initialState?: State, options?: SilcrowActionOptions & {
    base?: string;
}): [State, (payload: FormData) => void, boolean];
/**
 * Prefetch a route, suspend until ready, then subscribe to live updates.
 */
export declare function useSilcrowResource<T>(path: string, fallback: T): T;
/**
 * Shared form state shape expected by `useSilcrowForm`.
 */
export type SilcrowFormState = {
    ok: boolean;
    message?: string;
    errors?: Record<string, string>;
};
export type SilcrowFormResult<State extends SilcrowFormState> = {
    state: State;
    action: (formData: FormData) => void;
    pending: boolean;
    ok: State["ok"];
    message: State["message"];
    errors: State["errors"];
};
/**
 * Object-style wrapper over `useSilcrowAction` for simple native forms.
 */
export declare function useSilcrowForm<State extends SilcrowFormState = SilcrowFormState>(url: string, initialState?: State, options?: SilcrowActionOptions): SilcrowFormResult<State>;
/**
 * Options for `useSilcrowMutation`.
 */
export type SilcrowMutationOptions<Data = unknown> = {
    /** URL to POST to. */
    url: string;
    /** HTTP method (default: `"POST"`). */
    method?: string;
    /** Extra request headers. */
    headers?: Record<string, string>;
    /** Optimistic update to apply immediately before the round-trip. */
    optimistic?: {
        scope: string;
        data: Data;
        mutationId?: string;
    };
    /** Called when the server confirms success. */
    onSuccess?: (result: SilcrowSubmitResult<Data>) => void;
    /** Called when the server returns an error response or network failure. */
    onError?: (error: unknown) => void;
};
/**
 * State returned by `useSilcrowMutation`.
 */
export type SilcrowMutationState<Data = unknown> = {
    mutate: (body?: BodyInit | object | null) => Promise<SilcrowSubmitResult<Data>>;
    pending: boolean;
    error: unknown;
    data: Data | null;
    reset: () => void;
};
/**
 * A simple mutation hook that wraps `Silcrow.submit` with optional optimistic updates.
 */
export declare function useSilcrowMutation<Data = unknown>(options: SilcrowMutationOptions<Data>): SilcrowMutationState<Data>;
//# sourceMappingURL=hooks.d.ts.map