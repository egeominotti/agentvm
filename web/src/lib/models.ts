// Claude models to choose from: [id, name, description].
export const MODELS: [string, string, string][] = [
  ["default", "Account default", "Whatever Claude Code picks"],
  ["opus", "Opus", "Most capable"],
  ["sonnet", "Sonnet", "Fast, great for most work"],
  ["haiku", "Haiku", "Fastest and lightest"],
  ["fable", "Fable", "Fable family"],
  ["opusplan", "Opus plans, Sonnet builds", "Opus in plan mode, Sonnet otherwise"],
  ["opus[1m]", "Opus, 1M context", "Long context window"],
  ["sonnet[1m]", "Sonnet, 1M context", "Long context window"],
];

export const isKnownModel = (id: string) => MODELS.some((m) => m[0] === id);
export const modelLabel = (id: string) => MODELS.find((m) => m[0] === id)?.[1] ?? id;
