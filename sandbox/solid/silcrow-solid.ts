import { createStore, reconcile } from "solid-js/store";
import { onMount, onCleanup } from "solid-js";

declare const Silcrow: {
  subscribe: (scope: string, fn: (data: unknown) => void) => () => void;
  snapshot: (scope: string) => unknown;
  submit: (
    url: string,
    body?: unknown,
    options?: Record<string, unknown>
  ) => Promise<{
    ok: boolean;
    status: number;
    data: unknown;
    html: string | null;
    headers: Headers;
  }>;
  go: (path: string) => void;
  prefetch: (path: string) => Promise<unknown>;
};

/**
 * Binds a SolidJS store to a Silcrow atom scope.
 *
 * The store is seeded from `Silcrow.snapshot(scope)` immediately so SSR-hydrated
 * data is available synchronously on first render. A subscription is registered on
 * mount and torn down on cleanup — SSE/WS patches flow in automatically.
 *
 * Components using this store never fetch data themselves; Silcrow owns the network.
 */
export function createSilcrowStore<T extends object>(scope: string): T {
  const [store, setStore] = createStore<T>(
    (Silcrow.snapshot(scope) ?? {}) as T
  );

  onMount(() => {
    // Catch any patch that arrived between snapshot() and subscribe().
    const current = Silcrow.snapshot(scope);
    if (current) setStore(reconcile(current as T));

    const unsub = Silcrow.subscribe(scope, (data) => {
      setStore(reconcile(data as T));
    });
    onCleanup(unsub);
  });

  return store;
}
