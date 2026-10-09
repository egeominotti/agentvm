import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ConfirmButton } from "./ConfirmButton";

afterEach(() => vi.useRealTimers());

it("acts only on the second click, and says what it is about to do in between", () => {
  const act1 = vi.fn();
  render(
    <ConfirmButton confirm="Delete 3 with their logs?" onConfirm={act1}>
      Clear 3
    </ConfirmButton>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Clear 3" }));
  expect(act1).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Delete 3 with their logs?" }));
  expect(act1).toHaveBeenCalledTimes(1);
});

it("forgets the first click after a few seconds", () => {
  vi.useFakeTimers();
  const act1 = vi.fn();
  render(
    <ConfirmButton confirm="Sure?" onConfirm={act1}>
      Delete
    </ConfirmButton>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  act(() => {
    vi.advanceTimersByTime(5000);
  });
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  expect(act1).not.toHaveBeenCalled();
});
