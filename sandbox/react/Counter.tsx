/**
 * Demonstrates the three React 19 + Silcrow patterns that replace hooks:
 *
 *  1. useSilcrowAtom  — replaces useState + useEffect for external/live data
 *  2. use() + Suspense — replaces useState + useEffect for async route data
 *  3. useSilcrowAction — tiny React 19 wrapper over Silcrow.submit
 *  4. React Hook Form + Zod — complex form UX, Silcrow transport
 *
 * No fetch() calls. No useEffect. No manual cleanup.
 * Silcrow owns the network; React owns the view.
 */
import { Suspense, use } from "react";
import { useFormStatus } from "react-dom";
import { useForm } from "react-hook-form";
import {
  useSilcrowAtom,
  useSilcrowPrefetch,
  useSilcrowAction,
  silcrowSubmitHandler,
} from "pilcrow/react";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

// ── Types ────────────────────────────────────────────────────────────────────

type CartAtom = { count: number; total: string };
type CreateState = { ok: boolean; message?: string; errors?: Record<string, string> };

// ── Pattern 1: useSilcrowAtom ────────────────────────────────────────────────
// Replaces:  useState(null) + useEffect(() => fetch + EventSource + cleanup)
// Powered by: useSyncExternalStore + Silcrow.subscribe + Silcrow.snapshot
// Live updates (SSE/WS) flow in automatically — zero component code for that.

function CartBadge() {
  const cart = useSilcrowAtom<CartAtom>("route:/cart", { count: 0, total: "$0.00" });
  return (
    <span>
      Cart: {cart.count} items — {cart.total}
    </span>
  );
}

// ── Pattern 2: use() + Suspense ──────────────────────────────────────────────
// Replaces:  useState(null) + useState(true) + useEffect(() => fetch)
// Powered by: Silcrow.prefetch (memoised Promise) + React 19 use()
// Silcrow returns the SAME Promise instance until mutation → no suspend-loop.

type Product = { id: number; name: string; price: string };
type ProductData = { items: Product[] };

function ProductRows({ promise }: { promise: Promise<ProductData> }) {
  const data = use(promise); // suspends here; no useState, no useEffect
  const live = useSilcrowAtom<ProductData>("route:/products", data);
  // `live` picks up SSE patches after initial load; falls back to `data` until first patch
  return (
    <ul>
      {live.items.map((item) => (
        <li key={item.id}>
          {item.name} — {item.price}
        </li>
      ))}
    </ul>
  );
}

function ProductList() {
  const promise = useSilcrowPrefetch<ProductData>("/products");
  return (
    <Suspense fallback={<p>Loading products…</p>}>
      <ProductRows promise={promise} />
    </Suspense>
  );
}

// ── Pattern 3a: tiny React 19 action wrapper ─────────────────────────────────
// Replaces:  useState(null) + useState(false) + manual fetch + error handling
// Powered by: Silcrow.submit (returns {ok, status, data}) + useActionState
// The server controls the response shape; React just renders it.

function SubmitButton({ label }: { label: string }) {
  const { pending } = useFormStatus();
  return (
    <button type="submit" disabled={pending}>
      {pending ? "Saving..." : label}
    </button>
  );
}

function DirectAddToCartForm({ productId }: { productId: number }) {
  const [state, formAction] = useSilcrowAction<CreateState>(`/cart/add/${productId}`);
  return (
    <form action={formAction}>
      <input type="hidden" name="product_id" value={productId} />
      <SubmitButton label="Add to cart" />
      {state.message && <p role="alert">{state.message}</p>}
      {state.errors?.quantity && <p role="alert">{state.errors.quantity}</p>}
    </form>
  );
}

// ── Pattern 3b: React Hook Form + Zod for complex forms ──────────────────────
// RHF owns client form UX; Silcrow still owns the network and route invalidation.

const cartFormSchema = z.object({
  product_id: z.coerce.number().int().positive(),
  quantity: z.coerce.number().int().min(1, "Quantity must be at least 1"),
});

type CartFormValues = z.infer<typeof cartFormSchema>;

function HookFormAddToCartForm({ productId }: { productId: number }) {
  const form = useForm<CartFormValues>({
    resolver: zodResolver(cartFormSchema),
    defaultValues: {
      product_id: productId,
      quantity: 1,
    },
  });

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await silcrowSubmitHandler<CreateState, CartFormValues>(
      `/cart/add/${productId}`,
    )(values);

    if (!result.ok && result.data?.errors) {
      for (const [name, message] of Object.entries(result.data.errors)) {
        form.setError(name as keyof CartFormValues, { message: String(message) });
      }
      return;
    }

    form.reset({ product_id: productId, quantity: 1 });
  });

  return (
    <form onSubmit={onSubmit}>
      <input type="hidden" {...form.register("product_id", { valueAsNumber: true })} />
      <label>
        Quantity
        <input type="number" min="1" {...form.register("quantity", { valueAsNumber: true })} />
      </label>
      <button type="submit" disabled={form.formState.isSubmitting}>
        {form.formState.isSubmitting ? "Saving..." : "Add with RHF"}
      </button>
      {form.formState.errors.quantity?.message && (
        <p role="alert">{form.formState.errors.quantity.message}</p>
      )}
    </form>
  );
}

// ── Props passed from the Pilcrow template ───────────────────────────────────
// <react src="/react/Counter.tsx" strategy="visible" initial-count="3" />
// Extra attributes become string props via data-prop-* → camelCase.

type Props = {
  initialCount?: string;
};

export default function Counter({ initialCount = "0" }: Props) {
  return (
    <div>
      <p>Initial count from Pilcrow SSR: {initialCount}</p>
      <CartBadge />
      <ProductList />
      <DirectAddToCartForm productId={1} />
      <HookFormAddToCartForm productId={1} />
    </div>
  );
}
