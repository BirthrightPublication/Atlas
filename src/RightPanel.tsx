import { ReactNode, useEffect, useRef } from "react";
import { X } from "lucide-react";

export type Utility = "notifications" | "updates" | "assistant";
const titles = {
  notifications: "Notifications",
  updates: "Updates",
  assistant: "Atlas Assistant",
};

export function RightPanel({
  active,
  onClose,
  children,
}: {
  active: Utility | null;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const wasOpen = useRef(false);
  useEffect(() => {
    if (active) {
      if (!wasOpen.current)
        opener.current = document.activeElement as HTMLElement;
      ref.current?.focus();
    } else if (
      wasOpen.current &&
      !document.querySelector('[aria-modal="true"]')
    ) {
      opener.current?.focus();
    }
    wasOpen.current = !!active;
  }, [active]);
  return (
    <aside
      className="utility-panel"
      hidden={!active}
      ref={ref}
      tabIndex={-1}
      aria-label={active ? titles[active] : "Utility panel"}
      id="utility-panel"
    >
      <header className="utility-heading">
        <h2>{active ? titles[active] : ""}</h2>
        <button
          className="icon-button"
          title="Close panel (Escape)"
          aria-label="Close panel"
          onClick={onClose}
        >
          <X size={19} />
        </button>
      </header>
      <div className="utility-body">{children}</div>
    </aside>
  );
}
