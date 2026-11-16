import { describe, expect, it } from "vitest";
import { formatTime, remaining } from "./time";

describe("formatTime", () => {
  it("drops the hours under an hour", () => {
    expect(formatTime(67.9)).toBe("1:07");
    expect(formatTime(0)).toBe("0:00");
  });
  it("shows hours from one hour", () => {
    expect(formatTime(3723)).toBe("1:02:03");
  });
  it("never goes negative or NaN", () => {
    expect(formatTime(-5)).toBe("0:00");
    expect(formatTime(Number.NaN)).toBe("0:00");
  });
});

describe("remaining", () => {
  it("counts down with a true minus sign", () => {
    expect(remaining(1872, 3180)).toBe("−21:48");
  });
  it("is zero at the end and past it", () => {
    expect(remaining(3180, 3180)).toBe("−0:00");
    expect(remaining(4000, 3180)).toBe("−0:00");
  });
});
