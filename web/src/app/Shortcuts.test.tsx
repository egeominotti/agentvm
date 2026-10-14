import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { TOGGLE_PANEL_EVENT } from "./keys";
import { Shortcuts } from "./Shortcuts";

afterEach(() => {
  cleanup();
  location.hash = "";
});

it("leaves terminal control keys to the VM", () => {
  let toggles = 0;
  const count = () => toggles++;
  window.addEventListener(TOGGLE_PANEL_EVENT, count);
  try {
    const view = render(
      <>
        <Shortcuts />
        <div className="xterm">
          <textarea aria-label="Terminal input" />
        </div>
      </>,
    );
    fireEvent.keyDown(view.getByRole("textbox"), { key: "j", ctrlKey: true });
    expect(toggles).toBe(0);
    fireEvent.keyDown(document.body, { key: "j", metaKey: true });
    expect(toggles).toBe(1);
  } finally {
    window.removeEventListener(TOGGLE_PANEL_EVENT, count);
  }
});

it("forgets a navigation leader when a key is typed into a field", () => {
  const view = render(
    <>
      <Shortcuts />
      <input aria-label="Search" />
    </>,
  );
  location.hash = "#/wall";
  fireEvent.keyDown(document.body, { key: "g" });
  fireEvent.keyDown(view.getByRole("textbox"), { key: "x" });
  fireEvent.keyDown(document.body, { key: "s" });
  expect(location.hash).toBe("#/wall");
});
