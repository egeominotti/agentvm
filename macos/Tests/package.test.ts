import { expect, test } from "bun:test";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { cpSync, existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";

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
    "MacOS/AgentVMNative", ...readdirSync(join(app, "Contents/Resources/runtime/bin"))
      .map(name => `Resources/runtime/bin/${name}`),
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

test("a relocated application verifies its terminal resources without starting an engine", () => {
  relocated((copy, folder) => {
    const home = join(folder, "unused-engine-home");
    const result = checkResources(copy, home);
    expect(result.exitCode, result.stderr.toString()).toBe(0);
    expect(result.stdout.toString()).toContain("Native app resources verified.");
    expect(existsSync(home)).toBe(false);
  });
});

test("a relocated application cannot mask missing resources with its original build directory", () => {
  relocated((copy, folder) => {
    rmSync(join(copy, "Contents/Resources/AgentVMNative_AgentVMNative.bundle"), { recursive: true });
    const home = join(folder, "unused-engine-home");
    const result = checkResources(copy, home);
    expect(result.exitCode).toBe(1);
    expect(result.stderr.toString()).toContain("embedded terminal resources are missing");
    expect(existsSync(home)).toBe(false);
  });
});

function checkResources(copy: string, home: string) {
  return Bun.spawnSync([join(copy, "Contents/MacOS/AgentVMNative"), "--check-resources"], {
    // An older executable that ignores the check still cannot connect to the user's engine.
    env: { ...process.env, AGENTVM_HOME: home, AGENTVM_PORT: "0" },
    timeout: 5000,
  });
}

function relocated(check: (copy: string, folder: string) => void) {
  const folder = mkdtempSync(join(tmpdir(), "agentvm-package-relocated-"));
  try {
    const copy = join(folder, "Relocated.app");
    cpSync(app, copy, { recursive: true });
    check(copy, folder);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
}

function version(value: string): number {
  const [major = 0, minor = 0, patch = 0] = value.split(".").map(Number);
  return major * 65536 + minor * 256 + patch;
}
