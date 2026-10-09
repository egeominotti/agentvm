// The fonts restty draws with, in order: JetBrains Mono (regular, bold) for text, then symbol
// fonts for what it lacks. restty has no system fallback: a character in none of these comes out
// as a box. TTF, not WOFF2: restty's text shaper misreads WOFF2 glyphs.
export const RESTTY_FONTS = [
  { url: "/fonts/JetBrainsMonoNerdFontMono-Regular.ttf", weight: 400 },
  { url: "/fonts/JetBrainsMonoNerdFontMono-Bold.ttf", weight: 700 },
  // Noto's symbol fonts, cut to the symbol blocks (~240 KB together): dingbats and spinners,
  // technical marks (⏺ ⎿ ⏵), then the arrows and math signs neither of the others has.
  { url: "/fonts/NotoSansSymbols2-Symbols.ttf" },
  { url: "/fonts/NotoSansSymbols-Symbols.ttf" },
  { url: "/fonts/NotoSansMath-Symbols.ttf" },
];

/** What Claude Code and the VMs' shell draw beyond letters: spinners, bullets, marks, arrows. */
export const DRAWN_SYMBOLS = "·✢✳✶✻✽⏺●⎿⏵⏸✔✓✗✘↯⧉❯›»…⚠☐☒◯◉○◐◑◒◓⬢⬡⏳⌛⇣⇡↑↓←→⎇✎✚✖✦★☆♦◆◇▶►▸▹▪▫■□▢◼◻⏎⌘⌥⇧⌃⎋⌫⏹⏴⏷⏶";
