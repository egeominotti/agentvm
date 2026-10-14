import { expect, it } from "vitest";
import { loginPhase } from "./account-login";
it("ends pending account polling at the server's finite expiration", () => {
  expect(loginPhase("pending", 2000, 1999)).toBe("pending");
  expect(loginPhase("pending", 2000, 2000)).toBe("expired");
  expect(loginPhase("pending", 2000, 3000)).toBe("expired");
});
it("preserves terminal login results instead of polling or overwriting them at expiration", () => {
  expect(loginPhase("connected", 2000, 3000)).toBe("connected");
  expect(loginPhase("failed", 2000, 1000)).toBe("failed");
});
