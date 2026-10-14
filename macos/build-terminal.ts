// Bundle the existing terminal engine offline without changing the dashboard's build.
const result = await Bun.build({
  entrypoints: ["terminal.ts", "preview.ts"].map((file) => new URL(file, import.meta.url).pathname),
  outdir: new URL("Sources/AgentVMNative/Resources/dist/", import.meta.url).pathname,
  target: "browser", format: "esm", minify: true,
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
