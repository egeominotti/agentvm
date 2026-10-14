// The shared helper's deployment target depends on its build host.
// Declare the actual bundled requirement so Launch Services can reject unsupported systems.
import { join } from "node:path";
import { readdirSync } from "node:fs";

const app = process.argv[2];
if (!app) throw new Error("usage: bun macos/package-metadata.ts <application>");
const plist = join(app, "Contents/Info.plist");
const versions = [run([
  "/usr/bin/plutil", "-extract", "LSMinimumSystemVersion", "raw", "-o", "-", plist,
]).trim()];

for (const relative of [
  "MacOS/AgentVMNative", ...readdirSync(join(app, "Contents/Resources/runtime/bin"))
    .map(name => `Resources/runtime/bin/${name}`),
]) {
  const commands = run(["/usr/bin/otool", "-l", join(app, "Contents", relative)]);
  const minimum = commands.match(/\bminos\s+([\d.]+)/)?.[1] ??
    commands.match(/cmd LC_VERSION_MIN_MACOSX[\s\S]*?\bversion\s+([\d.]+)/)?.[1];
  if (!minimum) throw new Error(`missing macOS deployment target: ${relative}`);
  versions.push(minimum);
}

versions.sort((left, right) => {
  const a = left.split(".").map(Number), b = right.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference) return difference;
  }
  return 0;
});
const minimum = versions.at(-1)!;
run(["/usr/libexec/PlistBuddy", "-c", `Set :LSMinimumSystemVersion ${minimum}`, plist]);
console.log(`bundled runtime requires macOS ${minimum} or newer`);

function run(command: string[]): string {
  const result = Bun.spawnSync(command);
  if (result.exitCode !== 0) throw new Error(`${command[0]}: ${result.stderr.toString()}`);
  return result.stdout.toString();
}
