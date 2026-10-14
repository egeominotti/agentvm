import type { CustomCli } from "../../api/generated/CustomCli";
import { Panel, Row } from "./kit";

const EMPTY: CustomCli = { name: "Custom CLI", install: null, interactive: "", headless: null, resume: null };
export function CustomCommands({
  value,
  onChange,
}: {
  value: CustomCli | null | undefined;
  onChange: (v: CustomCli) => void;
}) {
  const v = value ?? EMPTY;
  return (
    <Panel
      title="Reusable custom CLI"
      description="Commands run inside the disposable VM. Install commands can use a shell; launch commands take an executable and arguments. Use {prompt}, {model} and {endpoint}; no secrets belong in commands. Select Custom CLI above or in New VM."
    >
      {(
        [
          ["name", "Name"],
          ["install", "Install command"],
          ["interactive", "Interactive command"],
          ["headless", "Automatic command"],
          ["resume", "Resume command"],
        ] as const
      ).map(([field, label]) => (
        <Row key={field} label={label} htmlFor={`custom-${field}`}>
          <input
            id={`custom-${field}`}
            className="text-input mono"
            spellCheck={false}
            value={v[field] ?? ""}
            onChange={(e) =>
              onChange({ ...v, [field]: e.target.value || (field === "name" || field === "interactive" ? "" : null) })
            }
          />
        </Row>
      ))}
    </Panel>
  );
}
