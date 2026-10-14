// A button for what cannot be undone: the first click says what is about to happen, the second
// does it. Left alone for a few seconds, it goes back to asking.
import { type ReactNode, useEffect, useState } from "react";
import { Button } from "./Button";

type Props = {
  children: ReactNode;
  confirm: string;
  confirmationKey?: string;
  onConfirm: () => void;
  disabled?: boolean;
  size?: "md" | "sm";
};

const ARMED_MS = 4000;

export function ConfirmButton({
  children,
  confirm,
  confirmationKey = confirm,
  onConfirm,
  disabled,
  size = "sm",
}: Props) {
  const [confirmed, setConfirmed] = useState<{ text: string; key: string } | null>(null);
  const armed = confirmed?.text === confirm && confirmed.key === confirmationKey;
  useEffect(() => setConfirmed(null), [confirm, confirmationKey]);
  useEffect(() => {
    if (!confirmed) return;
    const timer = setTimeout(() => setConfirmed(null), ARMED_MS);
    return () => clearTimeout(timer);
  }, [confirmed]);
  return (
    <Button
      size={size}
      variant="danger"
      className={armed ? "armed" : ""}
      disabled={disabled}
      onClick={() => {
        if (!armed) return setConfirmed({ text: confirm, key: confirmationKey });
        setConfirmed(null);
        onConfirm();
      }}
    >
      {armed ? confirm : children}
    </Button>
  );
}
