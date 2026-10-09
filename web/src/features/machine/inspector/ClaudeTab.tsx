// Claude's work in a machine: what it cost, its tokens and context over time, and the whole
// conversation. It stays after the machine is closed.
import { memo } from "react";
import { useClaudeUsage } from "../../../api/queries";
import { BLUE, LineChart } from "../../../components/LineChart";
import { money, tokens } from "../../../lib/format";
import { Entry } from "./Entry";
import { turns } from "./turns";
import { useConversation } from "./useConversation";

// Memoized: the machine view re-renders every second; the conversation changes on its own clock.
type Props = { id: string; live: boolean; interactive: boolean };

export const ClaudeTab = memo(function ClaudeTab({ id, live, interactive }: Props) {
  const { entries, loaded, error, retry } = useConversation(id, live);
  const samples = useClaudeUsage(id, live).data?.samples ?? [];
  const last = samples.at(-1);
  const per = turns(entries);
  const times = samples.map((s) => s.at);
  const context = samples.some((s) => s.context_pct != null);
  return (
    <div className="claude-tab">
      {last ? (
        <dl className="usage-strip">
          <div>
            <dt>Cost</dt>
            <dd>{money(last.cost_usd)}</dd>
          </div>
          <div>
            <dt>Tokens in</dt>
            <dd>{tokens(last.input_tokens)}</dd>
          </div>
          <div>
            <dt>Tokens out</dt>
            <dd>{tokens(last.output_tokens)}</dd>
          </div>
          {last.context_pct != null ? (
            <div>
              <dt>Context</dt>
              <dd>{Math.round(last.context_pct)}%</dd>
            </div>
          ) : null}
        </dl>
      ) : null}
      <LineChart
        title="Cost at API prices"
        times={times}
        format={money}
        series={[{ name: "Cost", color: BLUE, values: samples.map((s) => s.cost_usd) }]}
      />
      <LineChart
        title="Output tokens per call"
        times={per.map((t) => t.at)}
        format={tokens}
        series={[{ name: "Output", color: BLUE, values: per.map((t) => t.output_tokens) }]}
      />
      <LineChart
        title="Context used"
        times={times}
        max={100}
        format={(v) => `${v.toFixed(0)}%`}
        // Claude Code reports its context only in a terminal's status line.
        empty={interactive ? "No data yet" : "Not reported for automatic tasks"}
        series={context ? [{ name: "Context", color: BLUE, values: samples.map((s) => s.context_pct) }] : []}
      />
      <h3 className="tab-heading">Conversation</h3>
      <div className="conversation">
        {entries.map((e, i) => (
          // Entries only ever get appended: their place is their identity.
          // biome-ignore lint/suspicious/noArrayIndexKey: append-only list
          <Entry key={i} e={e} />
        ))}
        {error ? (
          <p className="msg err">
            Could not read the conversation: {error}{" "}
            <button type="button" className="link" onClick={retry}>
              Try again
            </button>
          </p>
        ) : loaded && !entries.length ? (
          <p className="hint">No conversation yet.</p>
        ) : null}
      </div>
    </div>
  );
});
