import { describe, expect, it } from "vitest";
import { matching, sectionOf } from "./sections";

describe("settings sections", () => {
  it("open the page an address names, old addresses included", () => {
    expect(sectionOf("tailscale")).toBe("tailscale");
    expect(sectionOf("agent")).toBe("general");
    expect(sectionOf("browser")).toBe("preferences");
    expect(sectionOf("s3")).toBe("backups");
    expect(sectionOf(undefined)).toBe("general");
    expect(sectionOf("nope")).toBe("general");
  });

  it("are found by what people type", () => {
    expect([...matching("ssh")]).toEqual(["tailscale"]);
    expect(matching("dark").has("preferences")).toBe(true);
    expect(matching("r2 bucket").has("backups")).toBe(true);
    expect(matching("memory").has("resources")).toBe(true);
    expect(matching("").size).toBe(10);
    expect(matching("nothing like this").size).toBe(0);
  });
});
