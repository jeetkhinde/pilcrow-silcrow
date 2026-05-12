import { createSignal } from "solid-js";

type Props = {
  initialCount?: string;
};

export default function Counter(props: Props) {
  const [count, setCount] = createSignal(Number(props.initialCount) || 0);
  return (
    <button type="button" onClick={() => setCount((c) => c + 1)}>
      Count: {count()}
    </button>
  );
}
