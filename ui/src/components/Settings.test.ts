import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import type { SettingsDto } from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

/** Renders the window over a fake api with the saved settings and opens the dialog of the settings. */
async function openSettings(settings?: SettingsDto) {
  const api = createFakeApi({ settings });
  const getSettings = vi.spyOn(api, "getSettings");
  const saveSettings = vi.spyOn(api, "saveSettings");
  render(App, { context: new Map([["api", api]]) });
  await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
  // The fields appear when the saved settings are read.
  await screen.findByRole("textbox", { name: strings.settings.timeout });
  return { api, getSettings, saveSettings };
}

function dialog(): HTMLElement {
  return screen.getByRole("dialog", { name: strings.settings.title });
}

function timeout(): HTMLInputElement {
  return within(dialog()).getByRole("textbox", { name: strings.settings.timeout });
}

function verifyTls(): HTMLInputElement {
  return within(dialog()).getByRole("checkbox", { name: strings.settings.verifyTls });
}

function proxy(name: string): HTMLInputElement {
  return within(dialog()).getByRole("radio", { name });
}

function proxyUrl(): HTMLInputElement {
  return within(dialog()).getByRole("textbox", { name: strings.settings.proxyUrl });
}

async function save(): Promise<void> {
  await fireEvent.click(within(dialog()).getByRole("button", { name: strings.settings.save }));
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("SettingsDialog", () => {
  it("has a button with an accessible name in the row of the tabs, after the placement of the response", () => {
    render(App, { context: new Map([["api", createFakeApi()]]) });

    const button = screen.getByRole("button", { name: strings.settings.button });
    expect(button.getAttribute("aria-label")).toBe("Настройки");
    const placement = screen.getByRole("button", { name: strings.layout.button });
    expect(placement.compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens with the defaults of a new database", async () => {
    const { getSettings } = await openSettings();

    expect(getSettings).toHaveBeenCalledTimes(1);
    expect(strings.settings.title).toBe("Настройки");
    expect(timeout().value).toBe("0");
    expect(verifyTls().checked).toBe(true);
    expect(proxy(strings.settings.proxySystem).checked).toBe(true);
    // The address is asked only for a proxy set by hand.
    expect(within(dialog()).queryByRole("textbox", { name: strings.settings.proxyUrl })).toBeNull();
  });

  it("opens with the saved values and explains the fields", async () => {
    await openSettings({ timeout_ms: 15000, verify_tls: false, proxy: { mode: "manual", url: "http://10.0.0.1:3128" } });

    expect(timeout().value).toBe("15000");
    expect(verifyTls().checked).toBe(false);
    expect(proxy(strings.settings.proxyManual).checked).toBe(true);
    expect(proxy(strings.settings.proxySystem).checked).toBe(false);
    expect(proxy(strings.settings.proxyOff).checked).toBe(false);
    expect(proxyUrl().value).toBe("http://10.0.0.1:3128");
    expect(proxyUrl().placeholder).toBe("http://host:port");

    expect(strings.settings.timeout).toBe("Таймаут запроса, мс");
    expect(strings.settings.verifyTls).toBe("Проверять SSL-сертификаты");
    expect(strings.settings.proxySystem).toBe("Системный");
    expect(strings.settings.proxyOff).toBe("Без прокси");
    expect(strings.settings.proxyManual).toBe("Вручную");
    expect(within(dialog()).getByRole("group", { name: "Прокси" })).toBeTruthy();
    expect(within(dialog()).getByText("0 — без ограничения")).toBeTruthy();
    expect(within(dialog()).getByText("Запрос может переопределить это на вкладке Settings")).toBeTruthy();
    expect(within(dialog()).getByText("PAC и WPAD не поддерживаются")).toBeTruthy();
  });

  it.each(["abc", "-1", "3600001", "", "1.5"])("refuses the timeout %j without calling the command", async (text) => {
    const { saveSettings } = await openSettings();

    await fireEvent.input(timeout(), { target: { value: text } });
    await save();

    expect(within(dialog()).getByText("Целое число от 0 до 3 600 000")).toBeTruthy();
    expect(timeout().getAttribute("aria-invalid")).toBe("true");
    expect(saveSettings).not.toHaveBeenCalled();
    expect(screen.queryByText(strings.settings.saved)).toBeNull();
  });

  it("takes the timeout 0 and the largest one", async () => {
    const { saveSettings } = await openSettings({ timeout_ms: 500, verify_tls: true, proxy: { mode: "off" } });

    await fireEvent.input(timeout(), { target: { value: "0" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "off" } });

    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    await screen.findByRole("textbox", { name: strings.settings.timeout });
    // The dialog reads the settings every time it opens.
    expect(timeout().value).toBe("0");
    await fireEvent.input(timeout(), { target: { value: "3600000" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenLastCalledWith({ timeout_ms: 3600000, verify_tls: true, proxy: { mode: "off" } });
  });

  it.each(["host:8080", "", "http://", "ftp://host:21"])(
    "refuses the manual proxy address %j without calling the command",
    async (address) => {
      const { saveSettings } = await openSettings();

      await fireEvent.click(proxy(strings.settings.proxyManual));
      await fireEvent.input(proxyUrl(), { target: { value: address } });
      await save();

      expect(within(dialog()).getByText("Адрес должен начинаться с http:// или https://")).toBeTruthy();
      expect(proxyUrl().getAttribute("aria-invalid")).toBe("true");
      expect(saveSettings).not.toHaveBeenCalled();
    },
  );

  it("saves valid values, closes and tells about it", async () => {
    const { saveSettings } = await openSettings();

    await fireEvent.input(timeout(), { target: { value: "30000" } });
    await fireEvent.click(verifyTls());
    await fireEvent.click(proxy(strings.settings.proxyManual));
    await fireEvent.input(proxyUrl(), { target: { value: "http://127.0.0.1:8888" } });
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(saveSettings).toHaveBeenCalledWith({
      timeout_ms: 30000,
      verify_tls: false,
      proxy: { mode: "manual", url: "http://127.0.0.1:8888" },
    });
    expect(screen.getByText("Настройки сохранены")).toBeTruthy();
  });

  it("saves the system proxy without an address, whatever was typed for the manual one", async () => {
    const { saveSettings } = await openSettings({
      timeout_ms: 0,
      verify_tls: true,
      proxy: { mode: "manual", url: "https://proxy.example.test" },
    });

    await fireEvent.click(proxy(strings.settings.proxySystem));
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" } });
    expect(saveSettings.mock.calls[0][0].proxy).not.toHaveProperty("url");
  });

  it("clears the error of a field when the next save passes", async () => {
    const { saveSettings } = await openSettings();

    await fireEvent.input(timeout(), { target: { value: "abc" } });
    await save();
    expect(within(dialog()).getByText(strings.settings.timeoutInvalid)).toBeTruthy();

    await fireEvent.input(timeout(), { target: { value: "100" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledTimes(1);
  });

  it("shows the refusal of the command in the dialog and stays open", async () => {
    const { saveSettings } = await openSettings();
    saveSettings.mockRejectedValueOnce("invalid proxy address: no host");

    await fireEvent.input(timeout(), { target: { value: "100" } });
    await save();

    expect((await within(dialog()).findByRole("alert")).textContent).toContain("invalid proxy address: no host");
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(timeout().value).toBe("100");
    expect(screen.queryByText(strings.settings.saved)).toBeNull();
  });

  it("closes on Escape and on Cancel without saving", async () => {
    const { saveSettings } = await openSettings();

    await fireEvent.input(timeout(), { target: { value: "777" } });
    await fireEvent(window, createEvent.keyDown(window, { key: "Escape" }));
    expect(screen.queryByRole("dialog")).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    await screen.findByRole("textbox", { name: strings.settings.timeout });
    // What was typed and not saved is gone.
    expect(timeout().value).toBe("0");
    await fireEvent.click(within(dialog()).getByRole("button", { name: strings.settings.cancel }));
    expect(screen.queryByRole("dialog")).toBeNull();

    expect(saveSettings).not.toHaveBeenCalled();
    expect(screen.queryByText(strings.settings.saved)).toBeNull();
  });
});
