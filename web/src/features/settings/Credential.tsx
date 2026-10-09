// A secret kept in the Keychain (a token, a key): whether it is saved, never its value; a field to
// paste one, and replacing or removing it.
import { useState } from "react";
import { Button } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";

type Props = {
  saved: boolean;
  /** What a value looks like, for the field: "sk-ant-oat01-…". */
  placeholder: string;
  label: string;
  busy?: boolean;
  onSave: (value: string) => void;
  onRemove?: () => void;
};

export function Credential({ saved, placeholder, label, busy, onSave, onRemove }: Props) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");
  if (saved && !editing) {
    return (
      <div className="credential">
        <span className="credential-saved" title="Kept in the macOS Keychain">
          ••••••••••••
        </span>
        <Button size="sm" onClick={() => setEditing(true)}>
          Replace
        </Button>
        {onRemove ? (
          <ConfirmButton confirm="Remove it?" onConfirm={onRemove}>
            Remove
          </ConfirmButton>
        ) : null}
      </div>
    );
  }
  return (
    <form
      className="credential"
      onSubmit={(e) => {
        e.preventDefault();
        if (!value.trim()) return;
        onSave(value.trim());
        setValue("");
        setEditing(false);
      }}
    >
      <input
        className="text-input mono"
        type="password"
        aria-label={label}
        placeholder={placeholder}
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(e) => setValue(e.target.value)}
      />
      {editing ? (
        <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
          Cancel
        </Button>
      ) : null}
      <Button type="submit" size="sm" variant="primary" disabled={busy || !value.trim()}>
        Save
      </Button>
    </form>
  );
}
