// Says so when agentvm stops answering: what is on screen is then the last thing it said, and
// may be out of date. It goes away by itself once the server answers again.
import { useTasks } from "../api/queries";

export function ConnectionBanner() {
  const q = useTasks();
  if (!q.isError) return null;
  return (
    <div className="connection" role="alert">
      <b>agentvm is not answering.</b>
      <span>
        {q.data ? "What you see may be out of date. " : ""}Retrying every second: start the server again (
        <code>bin/agentvm-server</code>) if it stopped.
      </span>
    </div>
  );
}
