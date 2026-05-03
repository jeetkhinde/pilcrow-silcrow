/**
 * Demonstrates every pilcrow/react hook:
 *
 *  1. useSilcrowAtom      — raw atom subscription (live/shared state)
 *  2. useSilcrowRoute     — shorthand for route-backed atoms
 *  3. useSilcrowPrefetch  — memoised Promise for use() + Suspense
 *  4. useSilcrowResource  — prefetch + suspend + subscribe in one call
 *  5. useSilcrowAction    — raw [state, action, pending] tuple
 *  6. useSilcrowForm      — object wrapper over useSilcrowAction
 *  7. usePilcrowNamedAction — resolves a named page/fragment action
 *  8. silcrowSubmitHandler  — async callback for React Hook Form
 *  9. publishSilcrowAtom  — manually push a state patch
 *
 * No fetch(). No useEffect(). No manual cleanup.
 * Silcrow owns the network; React owns the view.
 */
import { Suspense, use, useState } from "react";
import { useFormStatus } from "react-dom";
import { useForm } from "react-hook-form";
import {
  useSilcrowAtom,
  useSilcrowRoute,
  useSilcrowPrefetch,
  useSilcrowResource,
  useSilcrowAction,
  useSilcrowForm,
  usePilcrowNamedAction,
  silcrowSubmitHandler,
  publishSilcrowAtom,
} from "pilcrow/react";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

// ── Types ────────────────────────────────────────────────────────────────────

type CartAtom = { count: number; total: string };
type CreateState = { ok: boolean; message?: string; errors?: Record<string, string> };
type Product = { id: number; name: string; price: string };
type ProductData = { items: Product[] };
type NotifState = { ok: boolean; message?: string };

// ── Pattern 1: useSilcrowAtom ─────────────────────────────────────────────────
// Use when: subscribing to any named atom (route or custom scope).
// Re-renders whenever the atom changes; SSE/WS patches flow in automatically.

function CartBadge() {
  const cart = useSilcrowAtom<CartAtom>("route:/cart", { count: 0, total: "$0.00" });
  return (
    <span>
      Cart: {cart.count} items — {cart.total}
    </span>
  );
}

// ── Pattern 2: useSilcrowRoute ────────────────────────────────────────────────
// Use when: you want route-backed data without spelling out the "route:" prefix.
// Sugar over useSilcrowAtom("route:/path", fallback) — identical runtime behaviour.

function UserSummary() {
  type User = { name: string; plan: string };
  const user = useSilcrowRoute<User>("/me", { name: "—", plan: "free" });
  return (
    <p>
      {user.name} · {user.plan}
    </p>
  );
}

// ── Pattern 3: useSilcrowPrefetch + use() ─────────────────────────────────────
// Use when: you need the Suspense boundary in a parent while the child subscribes.
// useSilcrowPrefetch returns the SAME Promise until mutation — no suspend-loop.

function ProductRows({ promise }: { promise: Promise<ProductData> }) {
  const data = use(promise);
  const live = useSilcrowAtom<ProductData>("route:/products", data);
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

function ProductListPrefetch() {
  const promise = useSilcrowPrefetch<ProductData>("/products");
  return (
    <Suspense fallback={<p>Loading products…</p>}>
      <ProductRows promise={promise} />
    </Suspense>
  );
}

// ── Pattern 4: useSilcrowResource ─────────────────────────────────────────────
// Use when: you want prefetch + suspend + live subscription in one call.
// The component itself suspends — wrap the caller in <Suspense>.

function ReviewList() {
  type Review = { id: number; author: string; body: string };
  type ReviewData = { items: Review[] };
  const reviews = useSilcrowResource<ReviewData>("/reviews", { items: [] });
  return (
    <ul>
      {reviews.items.map((r) => (
        <li key={r.id}>
          <strong>{r.author}</strong>: {r.body}
        </li>
      ))}
    </ul>
  );
}

function ReviewSection() {
  return (
    <Suspense fallback={<p>Loading reviews…</p>}>
      <ReviewList />
    </Suspense>
  );
}

// ── Pattern 5: useSilcrowAction — raw tuple ───────────────────────────────────
// Use when: you need the pending flag directly (no <form> wrapper, button groups, etc).
// Returns [state, action, pending].

function SubmitButton({ label }: { label: string }) {
  const { pending } = useFormStatus();
  return (
    <button type="submit" disabled={pending}>
      {pending ? "Saving…" : label}
    </button>
  );
}

function DirectAddToCartForm({ productId }: { productId: number }) {
  const [state, formAction, pending] = useSilcrowAction<CreateState>(`/cart/add/${productId}`);
  return (
    <form action={formAction}>
      <input type="hidden" name="product_id" value={productId} />
      <button type="submit" disabled={pending}>
        {pending ? "Adding…" : "Add to cart"}
      </button>
      {state.message && <p role="alert">{state.message}</p>}
      {state.errors?.quantity && <p role="alert">{state.errors.quantity}</p>}
    </form>
  );
}

// ── Pattern 6: useSilcrowForm — object wrapper ────────────────────────────────
// Use when: the tuple is noisy in JSX, or you want named fields (form.errors, form.ok).
// Returns { state, action, pending, ok, message, errors }.

function WishlistForm({ productId }: { productId: number }) {
  const form = useSilcrowForm<CreateState>(`/wishlist/add/${productId}`);
  return (
    <form action={form.action}>
      <input type="hidden" name="product_id" value={productId} />
      <SubmitButton label="Save to wishlist" />
      {form.message && <p role="status">{form.message}</p>}
      {form.errors?.product_id && <p role="alert">{form.errors.product_id}</p>}
    </form>
  );
}

// ── Pattern 7: usePilcrowNamedAction ──────────────────────────────────────────
// Use when: the action name is a Pilcrow page/fragment action, not a full URL.
// Resolves relative to the current page context — no path needed.

function NotifyMeForm() {
  const [state, action, pending] = usePilcrowNamedAction<NotifState>("notify");
  return (
    <form action={action}>
      <input type="email" name="email" placeholder="you@example.com" required />
      <button type="submit" disabled={pending}>
        {pending ? "Subscribing…" : "Notify me"}
      </button>
      {state.message && <p role="status">{state.message}</p>}
    </form>
  );
}

// ── Pattern 8: silcrowSubmitHandler + React Hook Form ─────────────────────────
// Use when: the form library owns validation, dirty tracking, arrays, and focus.
// Silcrow still owns the network call and route invalidation.

const cartFormSchema = z.object({
  product_id: z.coerce.number().int().positive(),
  quantity: z.coerce.number().int().min(1, "Quantity must be at least 1"),
});

type CartFormValues = z.infer<typeof cartFormSchema>;

function HookFormAddToCartForm({ productId }: { productId: number }) {
  const form = useForm<CartFormValues>({
    resolver: zodResolver(cartFormSchema),
    defaultValues: { product_id: productId, quantity: 1 },
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
        {form.formState.isSubmitting ? "Saving…" : "Add with RHF"}
      </button>
      {form.formState.errors.quantity?.message && (
        <p role="alert">{form.formState.errors.quantity.message}</p>
      )}
    </form>
  );
}

// ── Pattern 9: publishSilcrowAtom ─────────────────────────────────────────────
// Use when: you need to push a patch to a Silcrow atom from React (e.g. optimistic UI).
// Does NOT trigger a network request — patches the local atom store directly.

function CartClearButton() {
  const [cleared, setCleared] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        publishSilcrowAtom("route:/cart", { count: 0, total: "$0.00" });
        setCleared(true);
      }}
      disabled={cleared}
    >
      {cleared ? "Cart cleared (local)" : "Clear cart (optimistic)"}
    </button>
  );
}

// ── Props passed from the Pilcrow template ────────────────────────────────────
// <react src="/react/Counter.tsx" strategy="visible" initial-count="3" />
// Extra attributes become string props via data-prop-* → camelCase.

type Props = {
  initialCount?: string;
};

export default function Counter({ initialCount = "0" }: Props) {
  return (
    <div>
      <p>Initial count from Pilcrow SSR: {initialCount}</p>

      <h3>1 · useSilcrowAtom</h3>
      <CartBadge />

      <h3>2 · useSilcrowRoute</h3>
      <UserSummary />

      <h3>3 · useSilcrowPrefetch + use()</h3>
      <ProductListPrefetch />

      <h3>4 · useSilcrowResource</h3>
      <ReviewSection />

      <h3>5 · useSilcrowAction (tuple)</h3>
      <DirectAddToCartForm productId={1} />

      <h3>6 · useSilcrowForm (object)</h3>
      <WishlistForm productId={1} />

      <h3>7 · usePilcrowNamedAction</h3>
      <NotifyMeForm />

      <h3>8 · silcrowSubmitHandler + React Hook Form</h3>
      <HookFormAddToCartForm productId={1} />

      <h3>9 · publishSilcrowAtom</h3>
      <CartClearButton />
    </div>
  );
}
