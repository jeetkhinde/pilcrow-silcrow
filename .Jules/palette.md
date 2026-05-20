# 2024-05-17 - Improved Save Button Feedback

**Learning:** Preventing duplicate form submissions and adding immediate visual feedback via a "Saving..." state combined with disabling the button greatly improves user confidence when performing asynchronous operations (like PUT requests) on vanilla JS forms.
**Action:** When adding vanilla JS fetch handlers to forms, always consider adding a disabled state with a descriptive message ("Saving...", "Loading...") to the submit button while the network request is pending.
**Learning:** Found that category filter links on the Products page used a generic div without proper navigation semantics, missing aria-current for screen readers, and lacking focus-visible and active styles.
**Action:** Applied role='navigation', aria-label, aria-current='page', and proper CSS states (.active, :focus-visible) to anchor tags mimicking pill buttons, ensuring keyboard and screen reader accessibility.

## 2026-05-20 - Global Layout Accessibility (Skip Links & Landmarks)
**Learning:** Found that the global application layout lacked a proper `<main>` landmark and a "Skip to content" link. In applications using client-side routing (like Silcrow), this is crucial because screen reader focus isn't naturally reset to the top of the new document on navigation.
**Action:** Always include a visually hidden "Skip to main content" link at the very top of `<body>` that becomes visible on focus, and ensure the primary content area is wrapped in a `<main>` tag with an `id` that the skip link targets. Also ensure main navigation elements have clear `:focus-visible` indicators.
