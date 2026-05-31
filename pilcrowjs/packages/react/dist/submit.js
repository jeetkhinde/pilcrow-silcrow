/**
 * Create a React 19 form action backed by Silcrow transport.
 *
 * Use this with React's `useActionState` when you want the raw primitive.
 */
export function submitSilcrow(url, options) {
    return async function action(_prev, formData) {
        if (!window.Silcrow?.submit) {
            throw new Error("Silcrow is not loaded");
        }
        const result = await window.Silcrow.submit(url, formData, {
            method: options?.method ?? "POST",
            scope: options?.scope,
            headers: options?.headers,
            optimistic: options?.optimistic,
        });
        return result.data ?? { ok: result.ok, status: result.status };
    };
}
/**
 * Create an async submit callback for React Hook Form or other form libraries.
 *
 * Silcrow/Pilcrow stays responsible for transport; the form library owns
 * validation, dirty/touched state, focus, arrays, and nested fields.
 */
export function silcrowSubmitHandler(url, options) {
    return async function submit(values) {
        if (!window.Silcrow?.submit) {
            throw new Error("Silcrow is not loaded");
        }
        return window.Silcrow.submit(url, values, {
            method: options?.method ?? "POST",
            scope: options?.scope,
            headers: options?.headers,
            optimistic: options?.optimistic,
        });
    };
}
//# sourceMappingURL=submit.js.map