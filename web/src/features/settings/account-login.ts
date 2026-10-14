import type { LoginState } from "../../api/generated/LoginState";
export function loginPhase(state: LoginState, expiresAt: number, now: number): LoginState | "expired" {
  return state === "pending" && now >= expiresAt ? "expired" : state;
}
