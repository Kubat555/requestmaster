import { describe, expect, it } from "vitest";
import { formatBytes, formatMs, statusClass } from "./format";

describe("formatBytes", () => {
  it("shows bytes, kilobytes and megabytes with a decimal comma", () => {
    expect(formatBytes(0)).toBe("0 Б");
    expect(formatBytes(512)).toBe("512 Б");
    expect(formatBytes(1536)).toBe("1,5 КБ");
    expect(formatBytes(21_757_952)).toBe("20,8 МБ");
  });
});

describe("formatMs", () => {
  it("shows milliseconds below a second and seconds above", () => {
    expect(formatMs(0)).toBe("0 мс");
    expect(formatMs(123.4)).toBe("123 мс");
    expect(formatMs(1240)).toBe("1,24 с");
  });
});

describe("statusClass", () => {
  it("groups status codes by their first digit", () => {
    expect(statusClass(204)).toBe("ok");
    expect(statusClass(302)).toBe("redirect");
    expect(statusClass(404)).toBe("client");
    expect(statusClass(503)).toBe("server");
  });
});
