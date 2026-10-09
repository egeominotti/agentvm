// A button for what cannot be undone: the first click says what is about to happen, the second
// does it. Left alone for a few seconds, it goes back to asking.
import { type ReactNode, useEffect, useState } from "react";
import { Button } from "./Button";

type Props = { children: ReactNode; confirm: string; onConfirm: () => void; disabled?: boolean; size?: "md" | "sm" };

const ARMED_MS = 4000;

export function ConfirmButton({ children, confirm, onConfirm, disabled, size = "sm" }: Props) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), ARMED_MS);
    return () => clearTimeout(timer);
  }, [armed]);
  return (
    <Button
      size={size}
      variant="danger"
      className={armed ? "armed" : ""}
      disabled={disabled}
      onClick={() => {
        if (!armed) return setArmed(true);
        setArmed(false);
        onConfirm();
      }}
    >
      {armed ? confirm : children}
    </Button>
  );
}
