// Light or dark: the system's choice unless one was picked here, remembered in this browser.
// index.html applies it before the first paint (no dark flash in light mode); this keeps it in
// sync when the choice or the system's appearance changes.
import { useEffect } from "react";

export type ThemePref = "system" | "light" | "dark";
export type Theme = "light" | "dark";

/** Also read by the script in index.html: keep the two in step. */
const KEY = "agentvm.theme";
const LIGHT_QUERY = "(prefers-color-scheme: light)";

export function parsePref(value: string | null): ThemePref {
  return value === "light" || value === "dark" ? value : "system";
}

export function resolveTheme(pref: ThemePref, systemDark: boolean): Theme {
  return pref === "system" ? (systemDark ? "dark" : "light") : pref;
}

export function storedPref(): ThemePref {
  try {
    return parsePref(localStorage.getItem(KEY));
  } catch {
    return "system";
  }
}

/** The choice made in this page, for a browser that refuses to store it. */
let chosen: ThemePref | null = null;

function apply() {
  const systemDark = !window.matchMedia?.(LIGHT_QUERY).matches;
  document.documentElement.dataset.theme = resolveTheme(chosen ?? storedPref(), systemDark);
}

/** Remembers the choice and applies it at once. */
export function setThemePref(pref: ThemePref) {
  chosen = pref;
  try {
    if (pref === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, pref);
  } catch {
    // Not remembered: it still applies (from `chosen`) until the page is reloaded.
  }
  apply();
}

/** Mounted once: follows the system's appearance, and a choice made in another tab. */
export function useThemeSync() {
  useEffect(() => {
    apply();
    const media = window.matchMedia?.(LIGHT_QUERY);
    media?.addEventListener("change", apply);
    const elsewhere = () => {
      chosen = null;
      apply();
    };
    window.addEventListener("storage", elsewhere);
    return () => {
      media?.removeEventListener("change", apply);
      window.removeEventListener("storage", elsewhere);
    };
  }, []);
}
