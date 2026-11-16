import { describe, expect, it } from "vitest";
import { dayLabel, formatBytes, formatClock, formatMs, methodClass, methodLabel, statusClass } from "./format";

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

describe("methodClass", () => {
  it("gives the five coloured methods a class of their own, whatever the case", () => {
    expect(methodClass("GET")).toBe("method-get");
    expect(methodClass("post")).toBe("method-post");
    expect(methodClass("Put")).toBe("method-put");
    expect(methodClass("PATCH")).toBe("method-patch");
    expect(methodClass("DELETE")).toBe("method-delete");
  });

  it("gives any other method the common class", () => {
    expect(methodClass("PROPFIND")).toBe("method-other");
    expect(methodClass("")).toBe("method-other");
  });
});

describe("methodLabel", () => {
  it("cuts a method longer than six characters", () => {
    expect(methodLabel("GET")).toBe("GET");
    expect(methodLabel("DELETE")).toBe("DELETE");
    expect(methodLabel("PROPFIND")).toBe("PROPFI");
  });
});

// The inputs are built from the local time, so that the tests do not depend on the time zone of the machine.
describe("formatClock", () => {
  it("shows the local time of a moment as hours, minutes and seconds", () => {
    expect(formatClock(new Date(2026, 10, 13, 9, 5, 7).toISOString())).toBe("09:05:07");
    expect(formatClock(new Date(2026, 10, 13, 23, 59, 59).toISOString())).toBe("23:59:59");
  });
});

describe("dayLabel", () => {
  const now = new Date(2026, 10, 13, 12, 0);

  it("calls the day of `now` today, whatever the time", () => {
    expect(dayLabel(new Date(2026, 10, 13, 0, 0, 1).toISOString(), now)).toBe("Сегодня");
    expect(dayLabel(new Date(2026, 10, 13, 23, 59, 59).toISOString(), now)).toBe("Сегодня");
  });

  it("calls the day before yesterday, up to its last second", () => {
    expect(dayLabel(new Date(2026, 10, 12, 23, 59, 59).toISOString(), now)).toBe("Вчера");
    expect(dayLabel(new Date(2026, 10, 12, 0, 0, 0).toISOString(), now)).toBe("Вчера");
  });

  it("shows any other day as a date", () => {
    expect(dayLabel(new Date(2026, 10, 1, 12, 0).toISOString(), now)).toBe("01.11.2026");
    expect(dayLabel(new Date(2026, 10, 11, 23, 59, 59).toISOString(), now)).toBe("11.11.2026");
    expect(dayLabel(new Date(2025, 11, 31, 8, 0).toISOString(), now)).toBe("31.12.2025");
  });
});
