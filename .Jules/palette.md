# 2024-05-17 - Improved Save Button Feedback

**Learning:** Preventing duplicate form submissions and adding immediate visual feedback via a "Saving..." state combined with disabling the button greatly improves user confidence when performing asynchronous operations (like PUT requests) on vanilla JS forms.
**Action:** When adding vanilla JS fetch handlers to forms, always consider adding a disabled state with a descriptive message ("Saving...", "Loading...") to the submit button while the network request is pending.
**Learning:** Found that category filter links on the Products page used a generic div without proper navigation semantics, missing aria-current for screen readers, and lacking focus-visible and active styles.
**Action:** Applied role='navigation', aria-label, aria-current='page', and proper CSS states (.active, :focus-visible) to anchor tags mimicking pill buttons, ensuring keyboard and screen reader accessibility.

## 2026-05-20 - Global Layout Accessibility (Skip Links & Landmarks)
**Learning:** Found that the global application layout lacked a proper `<main>` landmark and a "Skip to content" link. In applications using client-side routing (like Silcrow), this is crucial because screen reader focus isn't naturally reset to the top of the new document on navigation.
**Action:** Always include a visually hidden "Skip to main content" link at the very top of `<body>` that becomes visible on focus, and ensure the primary content area is wrapped in a `<main>` tag with an `id` that the skip link targets. Also ensure main navigation elements have clear `:focus-visible` indicators.

## 2024-05-23 - Accessible Async Form Submission Feedback
**Learning:** When making async calls (like fetch PUT requests), visual status indicators (e.g. "Saved!") are often missed by screen readers if they are not explicitly marked. Furthermore, providing only "happy path" feedback leaves users in the dark when an error occurs, making the application feel unreliable and confusing for both sighted and non-sighted users.
**Action:** Always include `role="status"` and `aria-live="polite"` on status message containers. Ensure that `try/catch` blocks for fetch requests provide explicit error messages in the UI (e.g., changing color to red and updating the text) rather than silently failing or only logging to the console.

## 2026-05-26 - Accessible Labels for Inline Forms
**Learning:** Found that inline forms (like newsletter signups) often omit explicit labels in favor of placeholders, creating an accessibility barrier for screen readers. Placeholders alone are insufficient for identifying the input's purpose to assistive technologies.
**Action:** When working on inline forms, always provide an accessible label by adding an `aria-label` attribute or a visually-hidden `<label>` element to ensure full accessibility while preserving the design layout.

## 2024-06-19 - Global Layout Accessibility (Skip Links & Focus States)
**Learning:** Found that the global layout missed a "Skip to main content" link and lacked proper visual focus indicators on sidebar interactive elements (due to Tailwind's preflight resetting outlines), hampering keyboard navigation.
**Action:** Always include a visually hidden "Skip to main content" link targeting the primary content area, and explicitly apply `:focus-visible` styles (`focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2`) to all interactive elements like links and buttons when using Tailwind CSS.
