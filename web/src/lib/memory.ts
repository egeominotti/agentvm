// The memory a VM can be given, as the server allows it.

const USUAL = [1024, 2048, 4096, 6144, 8192, 12288, 16384];
/** Kept for macOS; the server never gives VMs less than 1 GB to pick from. */
const RESERVED_MB = 8192;

/** The usual sizes that fit this Mac (at least 1 GB), plus the value in use if it is another. */
export function memoryChoices(hostRamMb: number, current: number): number[] {
  const top = Math.max(hostRamMb - RESERVED_MB, 1024);
  const fit = USUAL.filter((m) => m <= top);
  return [...new Set([...fit, current])].toSorted((a, b) => a - b);
}
