import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ConfirmButton } from "./ConfirmButton";

afterEach(cleanup);

it("requires another confirmation when the destructive action changes", () => {
  const deleted: string[] = [];
  const view = render(
    <ConfirmButton confirm="Delete one snapshot?" onConfirm={() => deleted.push("one")}>
      Delete
    </ConfirmButton>,
  );
  fireEvent.click(view.getByRole("button", { name: "Delete" }));
  view.rerender(
    <ConfirmButton confirm="Delete two snapshots?" onConfirm={() => deleted.push("two")}>
      Delete
    </ConfirmButton>,
  );
  fireEvent.click(view.getByRole("button"));
  expect(deleted).toEqual([]);
  fireEvent.click(view.getByRole("button", { name: "Delete two snapshots?" }));
  expect(deleted).toEqual(["two"]);
});

it("asks again when different snapshots have the same confirmation text", () => {
  const deleted: string[] = [];
  const view = render(
    <ConfirmButton confirmationKey="first" confirm="Delete one snapshot?" onConfirm={() => deleted.push("first")}>
      Delete
    </ConfirmButton>,
  );
  fireEvent.click(view.getByRole("button", { name: "Delete" }));
  view.rerender(
    <ConfirmButton confirmationKey="second" confirm="Delete one snapshot?" onConfirm={() => deleted.push("second")}>
      Delete
    </ConfirmButton>,
  );
  fireEvent.click(view.getByRole("button"));
  expect(deleted).toEqual([]);
  fireEvent.click(view.getByRole("button", { name: "Delete one snapshot?" }));
  expect(deleted).toEqual(["second"]);
});

it("does not revive a confirmation after changing the selection back", () => {
  let deleted = 0;
  const button = (selection: string) => (
    <ConfirmButton confirmationKey={selection} confirm="Delete one snapshot?" onConfirm={() => deleted++}>
      Delete
    </ConfirmButton>
  );
  const view = render(button("first"));
  fireEvent.click(view.getByRole("button", { name: "Delete" }));
  view.rerender(button("second"));
  view.rerender(button("first"));
  fireEvent.click(view.getByRole("button"));
  expect(deleted).toBe(0);
});
