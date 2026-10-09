// Where a machine's details (the inspector) go. In a wide window they sit beside the terminal,
// shown or hidden as last chosen. In a narrow one there is no room beside it: they come as a
// drawer over the terminal only when asked for, never by themselves, so narrowing the window
// never hides Claude's screen.
import { useEffect, useState } from "react";

/** The width under which the details become a drawer (as in responsive.css). */
const NARROW = "(max-width: 1280px)";

export function panelShown({ narrow, docked, drawer }: { narrow: boolean; docked: boolean; drawer: boolean }): boolean {
  return narrow ? drawer : docked;
}

/** Whether the window is too narrow for the details beside the terminal; follows resizes. */
export function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => window.matchMedia?.(NARROW).matches ?? false);
  useEffect(() => {
    const media = window.matchMedia?.(NARROW);
    if (!media) return;
    const follow = () => setNarrow(media.matches);
    media.addEventListener("change", follow);
    return () => media.removeEventListener("change", follow);
  }, []);
  return narrow;
}
