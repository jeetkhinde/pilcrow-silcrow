import { useState } from "react";

export default function DemoCounter() {
  const [count, setCount] = useState(0);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
      <button
        type="button"
        aria-label="Decrease count"
        onClick={() => setCount((c) => c - 1)}
        style={{ padding: "0.25rem 0.75rem", fontSize: "1.1rem", cursor: "pointer" }}
      >
        −
      </button>
      <span style={{ fontWeight: 700, fontSize: "1.25rem", minWidth: "2ch", textAlign: "center" }}>
        {count}
      </span>
      <button
        type="button"
        aria-label="Increase count"
        onClick={() => setCount((c) => c + 1)}
        style={{ padding: "0.25rem 0.75rem", fontSize: "1.1rem", cursor: "pointer" }}
      >
        +
      </button>
    </div>
  );
}
