import { usePilcrowNamedAction } from "pilcrow/react";

type Row = {
  id: number;
  name: string;
  status: string;
};

type RefreshState = {
  ok: boolean;
  message?: string;
};

export default function FragmentGrid({ rows = [] }: { rows?: Row[] }) {
  const [state, refresh] = usePilcrowNamedAction<RefreshState>("refresh");

  return (
    <div>
      <table>
        <thead>
          <tr>
            <th>Component</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              <td>{row.name}</td>
              <td>{row.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <form action={refresh}>
        <button type="submit">Refresh fragment action</button>
      </form>
      {state.message ? <p role="status">{state.message}</p> : null}
    </div>
  );
}
