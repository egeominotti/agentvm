// A menu of actions under a button (Radix: keyboard, focus and screen readers handled). A risky
// item asks first: its first selection shows the question, the second one acts.
import * as Dropdown from "@radix-ui/react-dropdown-menu";
import { type ReactNode, useState } from "react";

export function Menu({ trigger, children }: { trigger: ReactNode; children: ReactNode }) {
  return (
    <Dropdown.Root>
      <Dropdown.Trigger asChild>{trigger}</Dropdown.Trigger>
      <Dropdown.Portal>
        <Dropdown.Content className="menu" align="end" sideOffset={6}>
          {children}
        </Dropdown.Content>
      </Dropdown.Portal>
    </Dropdown.Root>
  );
}

type ItemProps = { children: ReactNode; onSelect: () => void; danger?: boolean; confirm?: string; disabled?: boolean };

export function MenuItem({ children, onSelect, danger, confirm, disabled }: ItemProps) {
  const [armed, setArmed] = useState(false);
  return (
    <Dropdown.Item
      className={`menu-item${danger ? " danger" : ""}`}
      disabled={disabled}
      onSelect={(e) => {
        if (confirm && !armed) {
          e.preventDefault(); // keep the menu open for the second selection
          setArmed(true);
          return;
        }
        onSelect();
      }}
    >
      {armed && confirm ? confirm : children}
    </Dropdown.Item>
  );
}

export const MenuSeparator = () => <Dropdown.Separator className="menu-sep" />;
export const MenuLabel = ({ children }: { children: ReactNode }) => (
  <Dropdown.Label className="menu-label">{children}</Dropdown.Label>
);
