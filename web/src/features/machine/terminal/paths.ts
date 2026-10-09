// Paths typed at a shell prompt: quoted only when the shell needs it.

export const shellQuote = (p: string) => (/^[\w@%+=:,./-]+$/.test(p) ? p : `'${p.replace(/'/g, "'\\''")}'`);
