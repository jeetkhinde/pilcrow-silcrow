import {
  createContext,
  createElement,
  useActionState,
  useContext,
  useMemo, use,
  useSyncExternalStore,
  type ReactNode,
} from "react";

/**
 * Full response shape returned by `window.Silcrow.submit`.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string};
 * const result: SilcrowSubmitResult<CreateState> =
 *   await window.Silcrow!.submit("/cart/add/1", {quantity: 1});
 * if (result.ok) console.log(result.data.message);
 */
export type SilcrowSubmitResult<T = unknown> = {
  ok: boolean;
  status: number;
  data: T;
  html: string | null;
  headers: Headers;
};

/**
 * Network options forwarded to `Silcrow.submit`.
 *
 * @example
 * submitSilcrow<CreateState>("/cart/add/1", {
 *   method: "POST",
 *   scope: "cart:add",
 *   headers: {"x-source": "react-island"},
 * });
 */
export type SilcrowSubmitOptions = {
  method?: string;
  scope?: string;
  headers?: Record<string, string>;
};

/**
 * Options for the tiny React 19 action wrapper.
 *
 * `permalink` is passed to React's `useActionState`; the other fields are
 * passed to Silcrow's submit transport.
 *
 * @example
 * useSilcrowAction<CreateState>(
 *   "/cart/add/1",
 *   {ok: true},
 *   {scope: "cart:add", permalink: "/cart"},
 * );
 */
export type SilcrowActionOptions = SilcrowSubmitOptions & {
  permalink?: string;
};

export type PilcrowReactContextValue = {
  actionBase?: string;
};

const PilcrowReactContext = createContext<PilcrowReactContextValue>( {} );

export function PilcrowReactProvider( {
  value,
  children,
}: {
  value: PilcrowReactContextValue;
  children: ReactNode;
} ) {
  return createElement( PilcrowReactContext.Provider, {value}, children );
}

function appendActionName( base: string, name: string ): string {
  if ( /^https?:\/\//.test( name ) || name.startsWith( "/" ) || name.startsWith( "?/" ) ) {
    return name;
  }
  const cleanBase = base || ( typeof window !== "undefined" ? window.location.pathname : "/" );
  const separator = cleanBase.includes( "?" ) ? "&" : "?";
  return `${cleanBase}${separator}/${encodeURIComponent( name )}`;
}

export function resolvePilcrowAction( name: string, base?: string ): string {
  return appendActionName( base ?? "", name );
}

/**
 * Browser global installed by `silcrow.js`.
 *
 * Use this directly when integrating with another React library, such as
 * React Hook Form. For simple native forms, prefer `submitSilcrow` or
 * `useSilcrowAction`.
 *
 * @example
 * const data = await window.Silcrow!.prefetch<ProductData>("/products");
 * const unsubscribe = window.Silcrow!.subscribe("route:/cart", () => {});
 * window.Silcrow!.publish("route:/cart", {count: 2});
 */
declare global {
  interface Window {
    Silcrow?: {
      subscribe?: ( scope: string, fn: () => void ) => () => void;
      snapshot?: <T = unknown>( scope: string ) => T | undefined;
      publish?: ( scope: string, data: unknown ) => void;
      prefetch?: <T = unknown>( path: string ) => Promise<T>;
      submit?: <T = unknown>(
        url: string,
        body?: BodyInit | object | null,
        options?: SilcrowSubmitOptions,
      ) => Promise<SilcrowSubmitResult<T>>;
    };
  }
}

/**
 * Subscribe a React component to a Silcrow atom scope.
 *
 * @example
 * type Cart = {count: number; total: string};
 * const cart = useSilcrowAtom<Cart>("route:/cart", {count: 0, total: "$0.00"});
 * return <span>{cart.count}</span>;
 */
export function useSilcrowAtom<T>( scope: string, fallback: T ): T {
  return useSyncExternalStore<T>(
    ( notify ) => window.Silcrow?.subscribe?.( scope, notify ) ?? ( () => {} ),
    () => window.Silcrow?.snapshot?.<T>( scope ) ?? fallback,
    () => window.Silcrow?.snapshot?.<T>( scope ) ?? fallback,
  );
}

/**
 * Patch a Silcrow atom scope.
 *
 * This accepts patch data, not an updater function.
 *
 * @example
 * publishSilcrowAtom("route:/cart", {count: 3});
 */
export function publishSilcrowAtom<T>( scope: string, data: T ): void {
  window.Silcrow?.publish?.( scope, data );
}

/**
 * Prefetch a route and return Silcrow's memoized promise for React `use()`.
 *
 * @example
 * function Products() {
 *   const promise = useSilcrowPrefetch<ProductData>("/products");
 *   return <Suspense fallback={<p>Loading...</p>}><Rows promise={promise} /></Suspense>;
 * }
 */
export function useSilcrowPrefetch<T>( path: string ): Promise<T> {
  return useMemo(
    () =>
      window.Silcrow?.prefetch?.<T>( path ) ??
      Promise.reject( new Error( "Silcrow is not loaded" ) ),
    [ path ],
  );
}

/**
 * Read a route atom by path.
 *
 * @example
 * const products = useSilcrowRoute<ProductData>("/products", {items: []});
 */
export function useSilcrowRoute<T>( path: string, fallback: T ): T {
  return useSilcrowAtom<T>( `route:${path}`, fallback );
}

/**
 * Create a React 19 form action backed by Silcrow transport.
 *
 * Use this with React's `useActionState` when you want the raw primitive.
 *
 * @example
 * const [state, action, pending] = useActionState<CreateState, FormData>(
 *   submitSilcrow<CreateState>("/cart/add/1"),
 *   {ok: true},
 * );
 * return <form action={action}><button disabled={pending}>Add</button></form>;
 */
export function submitSilcrow<T>(
  url: string,
  options?: SilcrowSubmitOptions,
) {
  return async function action( _prev: T, formData: FormData ): Promise<T> {
    if ( !window.Silcrow?.submit ) {
      throw new Error( "Silcrow is not loaded" );
    }
    const result = await window.Silcrow.submit<T>( url, formData, {
      method: options?.method ?? "POST",
      scope: options?.scope,
      headers: options?.headers,
    } );
    return result.data ?? ( {ok: result.ok, status: result.status} as T );
  };
}

/**
 * Create an async submit callback for React Hook Form or other form libraries.
 *
 * Silcrow/Pilcrow stays responsible for transport; the form library owns
 * validation, dirty/touched state, focus, arrays, and nested fields.
 *
 * @example
 * const onSubmit = form.handleSubmit(async (values) => {
 *   const result = await silcrowSubmitHandler<CreateState, CartValues>(
 *     "/cart/add/1",
 *   )(values);
 *   if (!result.ok) form.setError("quantity", {message: "Invalid quantity"});
 * });
 */
export function silcrowSubmitHandler<Result = unknown, Values = object>(
  url: string,
  options?: SilcrowSubmitOptions,
) {
  return async function submit( values: Values ): Promise<SilcrowSubmitResult<Result>> {
    if ( !window.Silcrow?.submit ) {
      throw new Error( "Silcrow is not loaded" );
    }
    return window.Silcrow.submit<Result>( url, values as object, {
      method: options?.method ?? "POST",
      scope: options?.scope,
      headers: options?.headers,
    } );
  };
}

/**
 * Tiny React 19 convenience wrapper over `useActionState(submitSilcrow(...))`.
 *
 * This is intentionally not a form framework. For complex client-side form UX,
 * use React Hook Form and `silcrowSubmitHandler`.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string; errors?: Record<string, string>};
 * const [state, action, pending] =
 *   useSilcrowAction<CreateState>("/cart/add/1");
 * return <form action={action}><button disabled={pending}>Add</button></form>;
 */
export function useSilcrowAction<State>(
  url: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions,
) {
  const submitOptions = options
    ? {method: options.method, scope: options.scope, headers: options.headers}
    : undefined;
  return useActionState<State, FormData>(
    submitSilcrow<State>( url, submitOptions ),
    initialState,
    options?.permalink,
  );
}

/**
 * React 19 action wrapper that resolves a Pilcrow page/fragment named action.
 *
 * @example
 * const [state, action, pending] = usePilcrowNamedAction<CreateState>("add");
 */
export function usePilcrowNamedAction<State>(
  name: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions & {base?: string;},
) {
  const context = useContext( PilcrowReactContext );
  const url = resolvePilcrowAction( name, options?.base ?? context.actionBase );
  return useSilcrowAction<State>( url, initialState, options );
}


export function useSilcrowResource<T>( path: string, fallback: T ): T {
  const initial = use( useSilcrowPrefetch<T>( path ) );
  return useSilcrowRoute<T>( path, initial ?? fallback );
}


/**
 * Object-style wrapper for simple native forms backed by Silcrow transport.
 *
 * Use this when a tuple from `useSilcrowAction` is correct but too noisy in JSX.
 * This is still just React 19 `useActionState` + `Silcrow.submit`; it is not a
 * replacement for React Hook Form or Zod in complex client-side form UX.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string; errors?: Record<string, string>};
 * const form = useSilcrowForm<CreateState>("/cart/add/1");
 * return (
 *   <form action={form.action}>
 *     <button disabled={form.pending}>Add</button>
 *     {form.message ? <p role="status">{form.message}</p> : null}
 *     {form.errors?.quantity ? <p role="alert">{form.errors.quantity}</p> : null}
 *   </form>
 * );
 */
export type SilcrowFormState = {
  ok: boolean;
  message?: string;
  errors?: Record<string, string>;
};

export type SilcrowFormResult<State extends SilcrowFormState> = {
  state: State;
  action: ( formData: FormData ) => void;
  pending: boolean;

  ok: State[ "ok" ];
  message: State[ "message" ];
  errors: State[ "errors" ];
};

export function useSilcrowForm<
  State extends SilcrowFormState = SilcrowFormState,
>(
  url: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions,
): SilcrowFormResult<State> {
  const [ state, action, pending ] = useSilcrowAction<State>(
    url,
    initialState,
    options,
  );

  return useMemo(
    () => ( {
      state,
      action,
      pending,
      ok: state.ok,
      message: state.message,
      errors: state.errors,
    } ),
    [ state, action, pending ],
  );
}