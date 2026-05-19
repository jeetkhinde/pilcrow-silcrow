# 2024-05-17 - Improved Save Button Feedback

**Learning:** Preventing duplicate form submissions and adding immediate visual feedback via a "Saving..." state combined with disabling the button greatly improves user confidence when performing asynchronous operations (like PUT requests) on vanilla JS forms.
**Action:** When adding vanilla JS fetch handlers to forms, always consider adding a disabled state with a descriptive message ("Saving...", "Loading...") to the submit button while the network request is pending.
**Learning:** Found that category filter links on the Products page used a generic div without proper navigation semantics, missing aria-current for screen readers, and lacking focus-visible and active styles.
**Action:** Applied role='navigation', aria-label, aria-current='page', and proper CSS states (.active, :focus-visible) to anchor tags mimicking pill buttons, ensuring keyboard and screen reader accessibility.
