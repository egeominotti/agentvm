// Why a machine failed, what to do, when each step happened, and the evidence.
import { useDiagnostics } from "../../../api/queries";
import { Button } from "../../../components/Button";
import { useToast } from "../../../components/Toast";
import { openFirst, reportText } from "./report";

const time = (at: number) =>
  new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
const step = (s: number) => (s < 60 ? `+${Math.round(s)}s` : `+${Math.floor(s / 60)}m ${Math.round(s % 60)}s`);

export function DiagnosticsTab({ id }: { id: string }) {
  const q = useDiagnostics(id, true);
  const say = useToast();
  if (q.error) return <p className="hint">{q.error.message}</p>;
  const d = q.data;
  if (!d) return <p className="hint">Reading the logs…</p>;
  const first = openFirst(d);
  const copy = () =>
    navigator.clipboard.writeText(reportText(id, d)).then(
      () => say("Diagnostics copied"),
      () => say("Could not copy: select the text instead", "err"),
    );
  return (
    <div className="diag">
      <div className="tab-actions">
        <Button size="sm" variant="ghost" onClick={() => q.refetch()}>
          Refresh
        </Button>
        <Button size="sm" onClick={copy}>
          Copy report
        </Button>
      </div>
      <p className="diag-summary">{d.summary}</p>
      {d.hint ? <p className="diag-hint">{d.hint}</p> : null}
      {d.timeline.length ? (
        <ol className="diag-timeline">
          {d.timeline.map((s, i) => (
            <li key={`${s.state}-${s.at}`} className={`st-${s.state}`}>
              <span className="when">{time(s.at)}</span>
              <b>{s.state.replace("_", " ")}</b>
              {i > 0 ? <span className="took">{step(s.at - (d.timeline[i - 1]?.at ?? s.at))}</span> : null}
            </li>
          ))}
        </ol>
      ) : null}
      {d.logs.map((l) => (
        <details key={l.file} open={l.file === first}>
          <summary>
            {l.name} <span className="file">{l.file}</span>
          </summary>
          <pre>{l.tail || "(empty)"}</pre>
        </details>
      ))}
      {d.server_log.length ? (
        <details>
          <summary>Server log</summary>
          <pre>{d.server_log.join("\n")}</pre>
        </details>
      ) : null}
      {d.logs.length === 0 ? <p className="hint">Its logs were cleaned up in Settings › Storage.</p> : null}
    </div>
  );
}
