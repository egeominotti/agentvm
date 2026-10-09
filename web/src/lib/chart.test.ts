import { expect, it } from "vitest";
import { linePath, niceMax } from "./chart";

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
