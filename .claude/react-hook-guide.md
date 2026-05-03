# React/Silcrow Hook Guide

Answer React usage questions from this file. Do not scan the repo.

---

## `useSilcrowAtom(scope, fallback)`

Live/shared state. Re-renders whenever the atom changes.

```tsx
type Cart = {count: number; total: string};

function CartBadge() {
  const cart = useSilcrowAtom<Cart>("route:/cart", {count: 0, total: "$0.00"});
  return <span>{cart.count} items — {cart.total}</span>;
}
```

---

## `useSilcrowRoute(path, fallback)`

Read route-backed data. Sugar over `useSilcrowAtom("route:${path}", fallback)`.

```tsx
type ProductData = {items: {id: number; name: string}[]};

function ProductList() {
  const products = useSilcrowRoute<ProductData>("/products", {items: []});
  return products.items.map((item) => <p key={item.id}>{item.name}</p>);
}
```

---

## `useSilcrowPrefetch(path)`

Returns a memoized Promise for use with React 19 `use()` and `<Suspense>`.
Use this when you need to split the Suspense boundary from the subscriber.

```tsx
function ProductRows({promise}: {promise: Promise<ProductData>}) {
  const data = use(promise);
  return <pre>{JSON.stringify(data)}</pre>;
}

function ProductList() {
  const promise = useSilcrowPrefetch<ProductData>("/products");
  return (
    <Suspense fallback={<p>Loading...</p>}>
      <ProductRows promise={promise} />
    </Suspense>
  );
}
```

---

## `useSilcrowResource(path, fallback)`

Prefetch + suspend + subscribe in a single call. Requires `<Suspense>` above the caller.
Use this when you want both initial async loading and live updates with minimal boilerplate.

```tsx
function ProductList() {
  const products = useSilcrowResource<ProductData>("/products", {items: []});
  return products.items.map((item) => <p key={item.id}>{item.name}</p>);
}

// Wrap caller in Suspense:
<Suspense fallback={<p>Loading...</p>}>
  <ProductList />
</Suspense>
```

---

## `useSilcrowAction(url, initialState?, options?)`

React 19 `useActionState` backed by Silcrow transport. Returns `[state, action, pending]`.
Use when you want the tuple directly or are wiring a non-form interaction.

```tsx
type State = {ok: boolean; message?: string};

function AddToCart() {
  const [state, action, pending] = useSilcrowAction<State>("/cart/add/1");
  return (
    <form action={action}>
      <button disabled={pending}>Add</button>
      {state.message && <p>{state.message}</p>}
    </form>
  );
}
```

---

## `useSilcrowForm(url, initialState?, options?)`

Object wrapper over `useSilcrowAction`. Returns `{state, action, pending, ok, message, errors}`.
Use when the tuple is noisy in JSX or you want structured error access.

```tsx
type CreateState = {ok: boolean; message?: string; errors?: Record<string, string>};

function AddToCart() {
  const form = useSilcrowForm<CreateState>("/cart/add/1");
  return (
    <form action={form.action}>
      <button disabled={form.pending}>Add</button>
      {form.message && <p role="status">{form.message}</p>}
      {form.errors?.quantity && <p role="alert">{form.errors.quantity}</p>}
    </form>
  );
}
```

---

## `usePilcrowNamedAction(name, initialState?, options?)`

Resolves a Pilcrow named action relative to the current page context.
Use inside a React island when the action name is a Pilcrow page/fragment action, not a full URL.

```tsx
type CreateState = {ok: boolean; message?: string};

function SubmitForm() {
  const [state, action, pending] = usePilcrowNamedAction<CreateState>("add");
  return (
    <form action={action}>
      <button disabled={pending}>Submit</button>
    </form>
  );
}
```

---

## `silcrowSubmitHandler(url, options?)`

Async submit callback for React Hook Form or other form libraries.
Use when the form library owns validation, dirty state, arrays, and focus.

```tsx
const onSubmit = form.handleSubmit(async (values) => {
  const result = await silcrowSubmitHandler<CreateState, CartValues>("/cart/add/1")(values);
  if (!result.ok) form.setError("quantity", {message: "Invalid quantity"});
});
```

---

## `publishSilcrowAtom(scope, data)`

Manually patch a Silcrow atom. Sends patch data — not a React state updater.

```tsx
publishSilcrowAtom("route:/cart", {count: 3});
```

---

## Choosing between hooks

| I want to… | Use |
|---|---|
| Show live-updating data | `useSilcrowAtom` / `useSilcrowRoute` |
| Load async data + keep it live | `useSilcrowResource` (needs Suspense) |
| Just get a Promise for `use()` | `useSilcrowPrefetch` |
| Simple form submission | `useSilcrowForm` |
| Raw tuple (`[state, action, pending]`) | `useSilcrowAction` |
| Named page action | `usePilcrowNamedAction` |
| React Hook Form integration | `silcrowSubmitHandler` |
| Push a state update manually | `publishSilcrowAtom` |
