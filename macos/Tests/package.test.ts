import { expect, test } from "bun:test";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const app = process.env.AGENTVM_NATIVE_OUTPUT ??
  fileURLToPath(new URL("../dist/agentvm Native.app", import.meta.url));

test("the packaged application requires an OS that can run every bundled executable", () => {
  const metadata = Bun.spawnSync([
    "/usr/bin/plutil", "-extract", "LSMinimumSystemVersion", "raw", "-o", "-",
    join(app, "Contents/Info.plist"),
  ]);
  expect(metadata.exitCode).toBe(0);
  const declared = version(metadata.stdout.toString().trim());
  for (const relative of [
    "MacOS/AgentVMNative", "Resources/runtime/bin/agentvm-server", "Resources/runtime/bin/agentvm-vm",
  ]) {
    const loadCommands = Bun.spawnSync(["/usr/bin/otool", "-l", join(app, "Contents", relative)]);
    expect(loadCommands.exitCode).toBe(0);
    const text = loadCommands.stdout.toString();
    const minimum = text.match(/\bminos\s+([\d.]+)/)?.[1] ??
      text.match(/cmd LC_VERSION_MIN_MACOSX[\s\S]*?\bversion\s+([\d.]+)/)?.[1];
    expect(minimum).toBeDefined();
    expect(declared, relative).toBeGreaterThanOrEqual(version(minimum!));
  }
});

function version(value: string): number {
  const [major = 0, minor = 0, patch = 0] = value.split(".").map(Number);
  return major * 65536 + minor * 256 + patch;
}
