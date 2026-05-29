# Demo App Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename `sandbox/` → `demo/`, update its package name, add `/demo/fsr` and `/demo/react-island` showcase pages, update nav, and delete `pilcrow-demos/`.

**Architecture:** The sandbox app becomes the demo app in-place via `git mv`. New showcase pages follow the same file-based routing pattern (sibling `.rs` + `.html` files under `pages/demo/`). The FSR demo uses inline `Live` / `LiveProp` fields for surgical reactive updates.

**Tech Stack:** Rust / Pilcrow file-based routing, `pilcrow_web::LiveProp`, Tokio, React (TSX), Askama templates.

---

## File Map

| Operation | Path |
|---|---|
| Rename dir | `sandbox/` → `demo/` |
| Modify | `demo/Cargo.toml` — package name |
| Modify | `demo/ui/Nav.html` — add Demos nav group |
| Create | `demo/pages/demo/fsr/index.rs` |
| Create | `demo/pages/demo/fsr/index.html` |
| Create | `demo/react/DemoCounter.tsx` |
| Create | `demo/pages/demo/react-island/index.rs` |
| Create | `demo/pages/demo/react-island/index.html` |
| Delete | `pilcrow-demos/` (entire directory) |

---

## Task 1: Rename sandbox → demo

**Files:**
- Rename: `sandbox/` → `demo/`
- Modify: `demo/Cargo.toml`

- [ ] **Step 1: git mv the directory**

Run from the workspace root (`/Users/jagjeet/Development/workspaces/pilcrow-silcrow`):

```bash
git mv sandbox demo
```

Expected: no output, exit 0.

- [ ] **Step 2: Update package name in Cargo.toml**

In `demo/Cargo.toml`, change line 2:

```toml
name = "pilcrow-demo"
```

The full `[package]` block should now read:

```toml
[package]
name = "pilcrow-demo"
version = "0.1.0"
edition = "2021"
```

- [ ] **Step 3: Verify the app builds**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile` with no errors. Ignore warnings about unused items.

- [ ] **Step 4: Commit**

```bash
git add demo/Cargo.toml
git commit -m "chore: rename sandbox → demo, update package name to pilcrow-demo"
```

---

## Task 2: Update Nav with Demos group

**Files:**
- Modify: `demo/ui/Nav.html`

- [ ] **Step 1: Replace the Nav file contents**

Current `demo/ui/Nav.html`:
```html
<nav s-boost>
    <a href="/">Home</a>
    <a href="/about">About</a>
    <a href="/products" s-preload>Products</a>
    <a href="/tickets">Tickets</a>
</nav>
```

Replace with:

```html
<nav s-boost>
    <a href="/">Home</a>
    <a href="/about">About</a>
    <a href="/live">Live</a>
    <a href="/products" s-preload>Products</a>
    <a href="/tickets">Tickets</a>
    <span style="color:#ccc;margin:0 0.25rem">|</span>
    <a href="/demo/fsr">FSR</a>
    <a href="/demo/react-island">React Island</a>
</nav>
```

- [ ] **Step 2: Build to confirm no template errors**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile`.

- [ ] **Step 3: Commit**

```bash
git add demo/ui/Nav.html
git commit -m "feat(demo): add Demos nav group with FSR and React Island links"
```

---

## Task 3: Add FSR demo page handler

**Files:**
- Create: `demo/pages/demo/fsr/index.rs`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p demo/pages/demo/fsr
```

- [ ] **Step 2: Write the handler**

Create `demo/pages/demo/fsr/index.rs`:

```rust
use std::sync::OnceLock;
use tokio::sync::watch;

static TICK: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn tick_tx() -> &'static watch::Sender<u64> {
    TICK.get_or_init(|| {
        let (tx, _rx) = watch::channel(0u64);
        tokio::spawn(async move {
            let tx = TICK.get().unwrap();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                tx.send_modify(|v| *v += 1);
            }
        });
        tx
    })
}

pub struct Props {
    pub server_time: String,
    pub live_count: pilcrow_web::LiveProp<u64>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let rx = tick_tx().subscribe();
    Ok(Props {
        server_time: now_utc(),
        live_count: pilcrow_web::LiveProp::watch(rx),
    })
}

fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "{:02}:{:02}:{:02} UTC",
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
```

- [ ] **Step 3: Build to check the handler compiles**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | grep -E "error|warning\[|Finished"
```

Expected: `Finished` (there may be a warning about the missing HTML template — that is fine at this step).

---

## Task 4: Add FSR demo HTML template

**Files:**
- Create: `demo/pages/demo/fsr/index.html`

- [ ] **Step 1: Write the template**

Create `demo/pages/demo/fsr/index.html`:

```html
---
---
<pilcrow:head>
    <title>FSR Demo — Pilcrow</title>
</pilcrow:head>

<h1>Field-Selective Rendering</h1>
<p>
    One page handler — three field types. FSR composes rendering mode
    per field, not per page. Observe each panel updating (or not) independently.
</p>

<div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:1.25rem;margin-top:1.75rem">

    <div style="border:1px solid #e5e7eb;border-radius:10px;padding:1.25rem">
        <span style="display:inline-block;padding:0.2rem 0.65rem;border-radius:999px;background:#f3f4f6;color:#374151;font-size:0.75rem;font-weight:600;letter-spacing:0.05em;text-transform:uppercase;margin-bottom:0.75rem">Static</span>
        <p style="font-size:0.85rem;color:#6b7280;margin:0 0 0.75rem">Set when the page was server-rendered. Never updates.</p>
        <code style="font-size:1.1rem;font-weight:700;color:#111">{{ server_time }}</code>
    </div>

    <div style="border:1px solid #e5e7eb;border-radius:10px;padding:1.25rem">
        <span style="display:inline-block;padding:0.2rem 0.65rem;border-radius:999px;background:#dbeafe;color:#1e40af;font-size:0.75rem;font-weight:600;letter-spacing:0.05em;text-transform:uppercase;margin-bottom:0.75rem">Live</span>
        <p style="font-size:0.85rem;color:#6b7280;margin:0 0 0.75rem">Patches in place without a full reload. Backed by a <code>LiveProp&lt;u64&gt;</code>.</p>
        <code style="font-size:1.1rem;font-weight:700;color:#111">{{ live_count }}</code>
    </div>

    <div style="border:1px solid #e5e7eb;border-radius:10px;padding:1.25rem">
        <span style="display:inline-block;padding:0.2rem 0.65rem;border-radius:999px;background:#d1fae5;color:#065f46;font-size:0.75rem;font-weight:600;letter-spacing:0.05em;text-transform:uppercase;margin-bottom:0.75rem">Async</span>
        <p style="font-size:0.85rem;color:#6b7280;margin:0 0 0.75rem">Patched after the initial HTML. Backed by a <code>LiveProp&lt;String&gt;</code>.</p>
        <code style="font-size:1.1rem;font-weight:700;color:#111">{{ async_time }}</code>
    </div>

</div>

<p style="margin-top:2rem;font-size:0.85rem;color:#9ca3af">
    The <strong>Static</strong> field is a plain <code>String</code> — set at request time, never touched again.
    The <strong>Live</strong> field is a <code>LiveProp&lt;u64&gt;</code> backed by a Tokio watch channel that ticks every second.
    Reactive fields use <code>LiveProp</code> and are patched after the initial HTML is sent.
</p>
```

- [ ] **Step 2: Build and confirm no errors**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile`.

- [ ] **Step 3: Run and visit the page**

```bash
cargo run --manifest-path demo/Cargo.toml &
sleep 2 && open http://127.0.0.1:3000/demo/fsr
```

Verify:
- All three panels render
- The **Live** counter increments every second without a page reload
- The **Async** value appears ~700 ms after the page loads (briefly blank or absent, then fills in)
- The **Static** value stays fixed

Kill the server: `pkill -f pilcrow-demo`

- [ ] **Step 4: Commit**

```bash
git add demo/pages/demo/fsr/
git commit -m "feat(demo): add /demo/fsr Field-Selective Rendering showcase page"
```

---

## Task 5: Add DemoCounter React component

**Files:**
- Create: `demo/react/DemoCounter.tsx`

This is a minimal, self-contained interactive component with no Silcrow hooks — its sole purpose is to demonstrate that a React island hydrates and runs client-side JavaScript inside otherwise static HTML.

- [ ] **Step 1: Write the component**

Create `demo/react/DemoCounter.tsx`:

```tsx
import { useState } from "react";

export default function DemoCounter() {
  const [count, setCount] = useState(0);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
      <button
        onClick={() => setCount((c) => c - 1)}
        style={{ padding: "0.25rem 0.75rem", fontSize: "1.1rem", cursor: "pointer" }}
      >
        −
      </button>
      <span style={{ fontWeight: 700, fontSize: "1.25rem", minWidth: "2ch", textAlign: "center" }}>
        {count}
      </span>
      <button
        onClick={() => setCount((c) => c + 1)}
        style={{ padding: "0.25rem 0.75rem", fontSize: "1.1rem", cursor: "pointer" }}
      >
        +
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Build to confirm TSX compiles**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile`.

---

## Task 6: Add React island demo page handler

**Files:**
- Create: `demo/pages/demo/react-island/index.rs`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p demo/pages/demo/react-island
```

- [ ] **Step 2: Write the handler**

Create `demo/pages/demo/react-island/index.rs`:

```rust
pub struct Props {
    pub username: String,
    pub member_since: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        username: "pilcrow-user".to_string(),
        member_since: "January 2026".to_string(),
    })
}
```

- [ ] **Step 3: Build**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`.

---

## Task 7: Add React island demo HTML template

**Files:**
- Create: `demo/pages/demo/react-island/index.html`

- [ ] **Step 1: Write the template**

Create `demo/pages/demo/react-island/index.html`:

```html
---
---
<pilcrow:head>
    <title>React Island Demo — Pilcrow</title>
</pilcrow:head>

<h1>React Islands</h1>
<p>
    The surrounding page is plain server-rendered HTML. Each island below is a React
    component hydrated client-side — only the island is interactive, nothing else.
</p>

<section style="border:1px solid #e5e7eb;border-radius:10px;padding:1.5rem;margin-top:1.75rem">
    <div style="display:flex;align-items:center;gap:0.6rem;margin-bottom:0.5rem">
        <span style="display:inline-block;padding:0.2rem 0.65rem;border-radius:999px;background:#f3f4f6;color:#374151;font-size:0.75rem;font-weight:600;letter-spacing:0.05em;text-transform:uppercase">Part 1 — Basic island</span>
        <code style="font-size:0.8rem;color:#6b7280">DemoCounter.tsx · strategy="visible"</code>
    </div>
    <p style="font-size:0.85rem;color:#6b7280;margin:0 0 1rem">
        The server renders an empty shell. React hydrates it client-side and mounts the counter.
        No props from the server — all state lives in React.
    </p>
    <react src="/react/DemoCounter.tsx" strategy="visible" />
</section>

<section style="border:1px solid #e5e7eb;border-radius:10px;padding:1.5rem;margin-top:1.25rem">
    <div style="display:flex;align-items:center;gap:0.6rem;margin-bottom:0.5rem">
        <span style="display:inline-block;padding:0.2rem 0.65rem;border-radius:999px;background:#dbeafe;color:#1e40af;font-size:0.75rem;font-weight:600;letter-spacing:0.05em;text-transform:uppercase">Part 2 — Server props</span>
        <code style="font-size:0.8rem;color:#6b7280">Greeting.tsx · strategy="shell" · props from Rust</code>
    </div>
    <p style="font-size:0.85rem;color:#6b7280;margin:0 0 1rem">
        Props flow from the Rust handler → serialised into the HTML → React reads them at hydration.
        No client-side fetch. The component receives <code>username</code> and <code>member_since</code>
        exactly as returned by <code>load()</code>.
    </p>
    <react src="/react/Greeting.tsx"
           strategy="shell"
           name="{{ username }}"
           message="Welcome back, member since {{ member_since }}" />
</section>

<p style="margin-top:2rem;font-size:0.85rem;color:#9ca3af">
    <strong>strategy="visible"</strong> — hydrates when the element scrolls into view.<br>
    <strong>strategy="shell"</strong> — server-renders the component HTML inside the shell, then hydrates.
</p>
```

- [ ] **Step 2: Build**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile`.

- [ ] **Step 3: Run and verify**

```bash
cargo run --manifest-path demo/Cargo.toml &
sleep 2 && open http://127.0.0.1:3000/demo/react-island
```

Verify:
- Part 1 counter is interactive (click − / + changes the number)
- Part 2 shows "Welcome back, member since January 2026" rendered by Greeting.tsx with the username from Rust
- Surrounding page content is plain HTML (no React outside the islands)
- Nav shows both FSR and React Island links

Kill the server: `pkill -f pilcrow-demo`

- [ ] **Step 4: Commit**

```bash
git add demo/react/DemoCounter.tsx demo/pages/demo/react-island/
git commit -m "feat(demo): add /demo/react-island React island showcase page"
```

---

## Task 8: Delete pilcrow-demos and final verification

**Files:**
- Delete: `pilcrow-demos/` (entire directory)

- [ ] **Step 1: Remove pilcrow-demos from git**

```bash
git rm -r --cached pilcrow-demos/
rm -rf pilcrow-demos/
```

- [ ] **Step 2: Full build of demo app**

```bash
cargo build --manifest-path demo/Cargo.toml 2>&1 | tail -5
```

Expected: `Finished 'dev' profile`.

- [ ] **Step 3: Run and smoke-test all routes**

```bash
cargo run --manifest-path demo/Cargo.toml &
sleep 2
```

Visit each route and confirm it loads without error:
- `http://127.0.0.1:3000/` — home
- `http://127.0.0.1:3000/about` — static page
- `http://127.0.0.1:3000/live` — LiveProp counter
- `http://127.0.0.1:3000/tickets` — live ticket board
- `http://127.0.0.1:3000/products` — product grid
- `http://127.0.0.1:3000/demo/fsr` — FSR showcase (three panels, live ticks, async fills in)
- `http://127.0.0.1:3000/demo/react-island` — island showcase (counter interactive, greeting shows username)

Kill the server: `pkill -f pilcrow-demo`

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: delete pilcrow-demos — superseded by demo app FSR + island pages"
```
