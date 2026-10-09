// The terminals' font (JetBrains Mono Nerd Font, regular and bold), loaded early: xterm.js measures
// its cells once, and the GPU's glyph atlas is drawn from that measure.

export const monoFont = (): Promise<unknown> =>
  document.fonts.check('13px "JetBrains Mono NF"') && document.fonts.check('bold 13px "JetBrains Mono NF"')
    ? Promise.resolve()
    : Promise.all([
        document.fonts.load('13px "JetBrains Mono NF"'),
        document.fonts.load('bold 13px "JetBrains Mono NF"'),
      ]);
