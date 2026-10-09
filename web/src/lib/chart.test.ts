import { expect, it } from "vitest";
import { drawable, linePath, niceMax } from "./chart";

it("rounds the top of the axis to a number that reads well", () => {
  expect(niceMax(0)).toBe(1);
  expect(niceMax(7)).toBe(10);
  expect(niceMax(180)).toBe(200);
  expect(niceMax(2300)).toBe(2500);
});

it("draws gaps where a value is missing", () => {
  const x = (i: number) => i * 10;
  const y = (v: number) => 100 - v;
  expect(linePath([1, 2, null, 4, 5], x, y)).toBe("M0.0,99.0L10.0,98.0M30.0,96.0L40.0,95.0");
});

it("a single point is still drawn, as a dot-sized segment", () => {
  expect(
    linePath(
      [49],
      () => 150,
      () => 20,
    ),
  ).toBe("M150.0,20.0h0.1");
});

it("draws a line only from two moments with a value: one alone is no line", () => {
  expect(drawable([], [[]])).toBe(false);
  expect(drawable([1], [[0]])).toBe(false);
  expect(drawable([1, 2], [[null, null]])).toBe(false);
  expect(
    drawable(
      [1, 2],
      [
        [null, 3],
        [4, null],
      ],
    ),
  ).toBe(false);
  expect(drawable([1, 2], [[1, 3]])).toBe(true);
  expect(drawable([1, 2, 3], [[null, 3, 5]])).toBe(true);
});
