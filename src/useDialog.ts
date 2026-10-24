import { useEffect, useRef } from "react";

/** One temporary dialog at a time; keep keyboard interaction inside it. */
export function useDialog(onClose: () => void) {
  const ref = useRef<HTMLElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    // StrictMode replays effects in development; retain the original launcher.
    if (!opener.current)
      opener.current = document.activeElement as HTMLElement | null;
    const previous = opener.current;
    const workspace = document.querySelector<HTMLElement>(".main-shell");
    const overflow = document.body.style.overflow;
    const wasInert = workspace?.inert || false;
    if (workspace) workspace.inert = true;
    document.body.style.overflow = "hidden";
    const controls = () =>
      Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]',
        ),
      ).filter((el) => el.getClientRects().length && !el.closest("[inert]"));
    const first = () => (controls()[0] || dialog).focus();
    first();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        close.current();
      } else if (event.key === "Tab") {
        const items = controls();
        const index = items.indexOf(document.activeElement as HTMLElement);
        if (!items.length) {
          event.preventDefault();
          dialog.focus();
        } else if (event.shiftKey && index <= 0) {
          event.preventDefault();
          items.at(-1)!.focus();
        } else if (
          !event.shiftKey &&
          (index < 0 || index === items.length - 1)
        ) {
          event.preventDefault();
          items[0].focus();
        }
      }
    };
    const focus = (event: FocusEvent) => {
      if (!dialog.contains(event.target as Node)) first();
    };
    document.addEventListener("keydown", keydown);
    document.addEventListener("focusin", focus);
    return () => {
      document.removeEventListener("keydown", keydown);
      document.removeEventListener("focusin", focus);
      document.body.style.overflow = overflow;
      if (workspace) workspace.inert = wasInert;
      queueMicrotask(() => {
        if (document.querySelector('[role="dialog"]')) return;
        if (previous?.isConnected) previous.focus();
        else
          document
            .querySelector<HTMLElement>('.bottom-nav [aria-current="page"]')
            ?.focus();
      });
    };
  }, []);
  return ref;
}
