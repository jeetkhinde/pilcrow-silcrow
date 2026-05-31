import { createContext, use, useActionState, useCallback, useContext, useMemo, useRef, useState, useSyncExternalStore, createElement, } from "react";
import { submitSilcrow, } from "./submit.js";
export const PilcrowReactContext = createContext({});
export function PilcrowReactProvider({ value, children, }) {
    return createElement(PilcrowReactContext.Provider, { value }, children);
}
function appendActionName(base, name) {
    if (/^https?:\/\//.test(name) || name.startsWith("/") || name.startsWith("?/")) {
        return name;
    }
    const cleanBase = base || (typeof window !== "undefined" ? window.location.pathname : "/");
    const separator = cleanBase.includes("?") ? "&" : "?";
    return `${cleanBase}${separator}/${encodeURIComponent(name)}`;
}
export function resolvePilcrowAction(name, base) {
    return appendActionName(base ?? "", name);
}
/**
 * Subscribe a React component to a Silcrow atom scope.
 */
export function useSilcrowAtom(scope, fallback) {
    return useSyncExternalStore((notify) => window.Silcrow?.subscribe?.(scope, notify) ?? (() => { }), () => window.Silcrow?.snapshot?.(scope) ?? fallback, () => window.Silcrow?.snapshot?.(scope) ?? fallback);
}
/**
 * Patch a Silcrow atom scope.
 */
export function publishSilcrowAtom(scope, data) {
    window.Silcrow?.publish?.(scope, data);
}
/**
 * Prefetch a route and return Silcrow's memoized promise for React `use()`.
 */
export function useSilcrowPrefetch(path) {
    return useMemo(() => window.Silcrow?.prefetch?.(path) ??
        Promise.reject(new Error("Silcrow is not loaded")), [path]);
}
/**
 * Read a route atom by path.
 */
export function useSilcrowRoute(path, fallback) {
    return useSilcrowAtom(`route:${path}`, fallback);
}
/**
 * Create a React 19 form action backed by Silcrow transport.
 */
export function useSilcrowAction(url, initialState = { ok: true }, options) {
    const submitOptions = options
        ? {
            method: options.method,
            scope: options.scope,
            headers: options.headers,
            optimistic: options.optimistic,
        }
        : undefined;
    return useActionState(submitSilcrow(url, submitOptions), initialState, options?.permalink);
}
/**
 * React 19 action wrapper that resolves a Pilcrow page/fragment named action.
 */
export function usePilcrowNamedAction(name, initialState = { ok: true }, options) {
    const context = useContext(PilcrowReactContext);
    const url = resolvePilcrowAction(name, options?.base ?? context.actionBase);
    return useSilcrowAction(url, initialState, options);
}
/**
 * Prefetch a route, suspend until ready, then subscribe to live updates.
 */
export function useSilcrowResource(path, fallback) {
    const initial = use(useSilcrowPrefetch(path));
    return useSilcrowRoute(path, initial ?? fallback);
}
/**
 * Object-style wrapper over `useSilcrowAction` for simple native forms.
 */
export function useSilcrowForm(url, initialState = { ok: true }, options) {
    const [state, action, pending] = useSilcrowAction(url, initialState, options);
    return useMemo(() => ({
        state,
        action,
        pending,
        ok: state.ok,
        message: state.message,
        errors: state.errors,
    }), [state, action, pending]);
}
/**
 * A simple mutation hook that wraps `Silcrow.submit` with optional optimistic updates.
 */
export function useSilcrowMutation(options) {
    const [pending, setPending] = useState(false);
    const [error, setError] = useState(null);
    const [data, setData] = useState(null);
    const optionsRef = useRef(options);
    optionsRef.current = options;
    const pendingCountRef = useRef(0);
    const reset = useCallback(() => {
        setPending(false);
        setError(null);
        setData(null);
    }, []);
    const mutate = useCallback(async (body) => {
        const { url, method, headers, optimistic, onSuccess, onError } = optionsRef.current;
        if (!window.Silcrow?.submit) {
            const err = new Error("Silcrow is not loaded");
            setError(err);
            onError?.(err);
            throw err;
        }
        pendingCountRef.current += 1;
        if (pendingCountRef.current === 1)
            setPending(true);
        setError(null);
        try {
            const result = await window.Silcrow.submit(url, body ?? null, {
                method: method ?? "POST",
                headers,
                optimistic,
            });
            if (result.ok) {
                setData(result.data);
                onSuccess?.(result);
            }
            else {
                const err = new Error("Request failed with status " + result.status);
                setError(err);
                onError?.(err);
            }
            return result;
        }
        catch (err) {
            setError(err);
            onError?.(err);
            throw err;
        }
        finally {
            pendingCountRef.current -= 1;
            if (pendingCountRef.current === 0)
                setPending(false);
        }
    }, []);
    return { mutate, pending, error, data, reset };
}
//# sourceMappingURL=hooks.js.map