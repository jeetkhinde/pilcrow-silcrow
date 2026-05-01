import { useState } from "react";

type Props = {
  initialCount?: string;
};

export default function Counter({ initialCount = "0" }: Props) {
  const [count, setCount] = useState(Number(initialCount) || 0);

  return (
    <button type="button" onClick={() => setCount(count + 1)}>
      Count: {count}
    </button>
  );
}
