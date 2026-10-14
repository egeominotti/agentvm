import { expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const script = resolve(import.meta.dir, "../package-dmg.sh");

test("DMG packaging rejects missing arguments", () => {
  const result = Bun.spawnSync(["bash", script]);
  expect(result.exitCode).toBe(2);
});

test("the real DMG preserves the signed app and installs through Applications", () => {
  const source = resolve(process.env.AGENTVM_DMG_TEST_APP ?? "macos/dist/agentvm Native.app");
  const temporary = mkdtempSync(join(tmpdir(), "agentvm-dmg-test-"));
  const providedImage = process.env.AGENTVM_DMG_TEST_IMAGE;
  const image = providedImage ? resolve(providedImage) : join(temporary, "agentvm.dmg");
  const mount = join(temporary, "mounted");
  let attached = false;
  try {
    const originalSignature = signature(source);
    if (!providedImage) run(["bash", script, source, image]);
    run(["hdiutil", "verify", "-quiet", image]);
    mkdirSync(mount);
    run(["hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen", "-quiet", "-mountpoint", mount, image]);
    attached = true;
    const app = join(mount, "agentvm Native.app");
    expect(readlinkSync(join(mount, "Applications"))).toBe("/Applications");
    expect(() => writeFileSync(join(mount, "readonly-check"), "read only")).toThrow();
    run(["codesign", "--verify", "--deep", "--strict", app]);
    expect(signature(app)).toBe(originalSignature);
    run([join(app, "Contents/MacOS/AgentVMNative"), "--check-resources"], {
      AGENTVM_PORT: "0", AGENTVM_HOME: join(temporary, "unused-profile"),
    });
    run(["codesign", "--verify", "--deep", "--strict", source]);
    expect(signature(source)).toBe(originalSignature);
  } finally {
    if (attached) {
      const detached = Bun.spawnSync(["hdiutil", "detach", "-quiet", mount]);
      if (detached.exitCode !== 0) run(["hdiutil", "detach", "-force", "-quiet", mount]);
    }
    rmSync(temporary, { recursive: true, force: true });
  }
}, 180_000);

function run(command: string[], environment: Record<string, string> = {}): string {
  const result = Bun.spawnSync(command, { env: { ...process.env, ...environment } });
  if (result.exitCode !== 0) {
    throw new Error(`${command[0]} failed (${result.exitCode}): ${result.stderr.toString()}${result.stdout.toString()}`);
  }
  return result.stdout.toString();
}

function signature(app: string): string {
  const result = Bun.spawnSync(["codesign", "-d", "--verbose=4", app]);
  expect(result.exitCode).toBe(0);
  const hash = result.stderr.toString().match(/^CDHash=(\w+)$/m)?.[1];
  expect(hash).toBeDefined();
  return hash!;
}
