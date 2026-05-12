type Props = {
  name?: string;
  message?: string;
};

export default function Greeting({ name = "World", message = "Hello" }: Props) {
  return (
    <p>
      {message}, <strong>{name}</strong>! (rendered by React SSR)
    </p>
  );
}
