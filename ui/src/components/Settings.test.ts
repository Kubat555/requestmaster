import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { motion } from "../lib/motion.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import type { MotionLevel, SettingsDto } from "../lib/types";

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

function scriptTimeout(): HTMLInputElement {
  return within(dialog()).getByRole("textbox", { name: strings.settings.scriptTimeout });
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

/** The choice of the motion level and its option by the name. */
function motionChoice(): HTMLElement {
  return within(dialog()).getByRole("tablist", { name: strings.settings.motion });
}

function motionOption(name: string): HTMLElement {
  return within(motionChoice()).getByRole("tab", { name });
}

/** The option of the motion level that is chosen. */
function chosenMotion(): string | null {
  return within(motionChoice()).getByRole("tab", { selected: true }).textContent?.trim() ?? null;
}

async function save(): Promise<void> {
  await fireEvent.click(within(dialog()).getByRole("button", { name: strings.settings.save }));
}

// The tabs, the notifications and the motion level of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
  motion.choose("normal");
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
    await openSettings({
      timeout_ms: 15000,
      verify_tls: false,
      proxy: { mode: "manual", url: "http://10.0.0.1:3128" },
      motion: "normal",
      scripts_enabled: true,
      script_timeout_ms: 10000,
    });

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
    const { saveSettings } = await openSettings({ timeout_ms: 500, verify_tls: true, proxy: { mode: "off" }, motion: "normal", scripts_enabled: true, script_timeout_ms: 10000 });

    await fireEvent.input(timeout(), { target: { value: "0" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "off" }, motion: "normal", scripts_enabled: true, script_timeout_ms: 10000 });

    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    await screen.findByRole("textbox", { name: strings.settings.timeout });
    // The dialog reads the settings every time it opens.
    expect(timeout().value).toBe("0");
    await fireEvent.input(timeout(), { target: { value: "3600000" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenLastCalledWith({ timeout_ms: 3600000, verify_tls: true, proxy: { mode: "off" }, motion: "normal", scripts_enabled: true, script_timeout_ms: 10000 });
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
      motion: "normal",
      scripts_enabled: true,
      script_timeout_ms: 10000,
    });
    expect(screen.getByText("Настройки сохранены")).toBeTruthy();
  });

  it("saves the system proxy without an address, whatever was typed for the manual one", async () => {
    const { saveSettings } = await openSettings({
      timeout_ms: 0,
      verify_tls: true,
      proxy: { mode: "manual", url: "https://proxy.example.test" },
      motion: "normal",
      scripts_enabled: true,
      script_timeout_ms: 10000,
    });

    await fireEvent.click(proxy(strings.settings.proxySystem));
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: "normal", scripts_enabled: true, script_timeout_ms: 10000 });
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

  it("offers the three motion levels, the normal one in a new database, and explains the rich one", async () => {
    await openSettings();

    expect(strings.settings.motion).toBe("Анимации");
    expect(within(motionChoice()).getAllByRole("tab").map((option) => option.textContent?.trim())).toEqual([
      "Выключены",
      "Обычные",
      "Расширенные",
    ]);
    expect(chosenMotion()).toBe("Обычные");
    expect(within(dialog()).getByText("Расширенные — эффекты для мощных компьютеров")).toBeTruthy();
  });

  it.each([
    ["off", "Выключены"],
    ["rich", "Расширенные"],
  ] as const)("opens with the saved motion level %j", async (level, name) => {
    await openSettings({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: level, scripts_enabled: true, script_timeout_ms: 10000 });

    expect(chosenMotion()).toBe(name);
  });

  it("shows a level it does not know, or settings without one, as the normal level", async () => {
    await openSettings({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: "extreme" as MotionLevel, scripts_enabled: true, script_timeout_ms: 10000 });
    expect(chosenMotion()).toBe("Обычные");
  });

  it("saves the chosen motion level and applies it to the window at once", async () => {
    const { saveSettings } = await openSettings();
    expect(document.documentElement.dataset.motion).toBe("normal");

    await fireEvent.click(motionOption(strings.settings.motionLevels.rich));
    // Nothing changes before Save.
    expect(document.documentElement.dataset.motion).toBe("normal");
    expect(motion.level).toBe("normal");
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: "rich", scripts_enabled: true, script_timeout_ms: 10000 });
    expect(motion.level).toBe("rich");
    expect(document.documentElement.dataset.motion).toBe("rich");

    // The dialog opens with what was saved, and switching the motion off works the same way.
    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    await screen.findByRole("textbox", { name: strings.settings.timeout });
    expect(chosenMotion()).toBe("Расширенные");
    await fireEvent.click(motionOption(strings.settings.motionLevels.off));
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenLastCalledWith({ timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: "off", scripts_enabled: true, script_timeout_ms: 10000 });
    expect(document.documentElement.dataset.motion).toBe("off");
  });

  it("leaves the motion level of the window alone when the dialog is cancelled or the save is refused", async () => {
    const { saveSettings } = await openSettings();

    await fireEvent.click(motionOption(strings.settings.motionLevels.rich));
    await fireEvent.click(within(dialog()).getByRole("button", { name: strings.settings.cancel }));
    expect(saveSettings).not.toHaveBeenCalled();
    expect(document.documentElement.dataset.motion).toBe("normal");

    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    await screen.findByRole("textbox", { name: strings.settings.timeout });
    expect(chosenMotion()).toBe("Обычные");
    saveSettings.mockRejectedValueOnce("storage is locked");
    await fireEvent.click(motionOption(strings.settings.motionLevels.rich));
    await save();
    await within(dialog()).findByRole("alert");
    expect(motion.level).toBe("normal");
    expect(document.documentElement.dataset.motion).toBe("normal");
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

  it("script_settings_are_kept_as_read", async () => {
    // Untouched in the dialog: Save must not reset what is stored.
    const { saveSettings } = await openSettings({
      timeout_ms: 0,
      verify_tls: true,
      proxy: { mode: "system" },
      motion: "normal",
      scripts_enabled: false,
      script_timeout_ms: 2000,
    });

    await fireEvent.input(timeout(), { target: { value: "500" } });
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({
      timeout_ms: 500,
      verify_tls: true,
      proxy: { mode: "system" },
      motion: "normal",
      scripts_enabled: false,
      script_timeout_ms: 2000,
    });
  });

  it("scripts_settings_are_saved", async () => {
    const { saveSettings } = await openSettings();
    const enabled = within(dialog()).getByRole<HTMLInputElement>("checkbox", { name: strings.settings.scriptsEnabled });
    expect(enabled.checked).toBe(true);
    expect(scriptTimeout().value).toBe("10000");

    await fireEvent.click(enabled);
    await fireEvent.input(scriptTimeout(), { target: { value: "2000" } });
    await save();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSettings).toHaveBeenCalledWith({
      timeout_ms: 0,
      verify_tls: true,
      proxy: { mode: "system" },
      motion: "normal",
      scripts_enabled: false,
      script_timeout_ms: 2000,
    });
  });

  it("script_timeout_out_of_range_is_not_saved", async () => {
    const { saveSettings } = await openSettings();

    for (const value of ["99", "600001", "1.5", ""]) {
      await fireEvent.input(scriptTimeout(), { target: { value } });
      await save();
      expect(within(dialog()).getByText(strings.settings.scriptTimeoutInvalid)).toBeTruthy();
      expect(scriptTimeout().getAttribute("aria-invalid")).toBe("true");
    }
    expect(saveSettings).not.toHaveBeenCalled();
  });
});
