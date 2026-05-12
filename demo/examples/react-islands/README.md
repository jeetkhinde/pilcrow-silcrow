# React Islands Example

This example shows the intended shape without making the default Rust sandbox
depend on local npm packages.

```toml
[routing]
ignore_directories = ["react"]

[client.react]
enabled = true
dirs = ["react"]
```

```html
<!-- src/pages/dashboard/index.html -->
<react src="./react/Counter.tsx" strategy="visible" initial-count="{{ count }}" />
```

```tsx
// src/pages/dashboard/react/Counter.tsx
export default function Counter(props: { initialCount: string }) {
  return <button>Count: {props.initialCount}</button>;
}
```

Apps using React islands need local npm dependencies for `vite`, `react`, and
`react-dom`.
