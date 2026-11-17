// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

const { api } = await import("./api");
const { entries, hideEntries, refresh, showEntries, startSync } = await import("./store");

const item = { id: 1, title: "Taxi Driver", status: "done", videos: [] };

describe("refresh", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("keeps the same list when nothing changed", async () => {
    invoke.mockResolvedValue([{ ...item }]);
    await refresh();
    const first = entries.value;
    invoke.mockResolvedValue([{ ...item }]);
    await refresh();
    expect(entries.value).toBe(first);
  });

  it("replaces the list when something changed", async () => {
    invoke.mockResolvedValue([{ ...item }]);
    await refresh();
    const first = entries.value;
    invoke.mockResolvedValue([{ ...item, status: "missing" }]);
    await refresh();
    expect(entries.value).not.toBe(first);
    expect(entries.value[0].status).toBe("missing");
  });

  it("brings back hidden items once they're shown again", async () => {
    invoke.mockResolvedValue([{ ...item }]);
    await refresh();
    hideEntries([1]);
    expect(entries.value).toHaveLength(0);
    showEntries([1]);
    await refresh();
    expect(entries.value).toHaveLength(1);
  });

  it("reads one at a time so an older read never lands last", async () => {
    const pending: ((v: unknown) => void)[] = [];
    invoke.mockImplementation(() => new Promise((r) => pending.push(r)));
    const first = refresh();
    const second = refresh();
    expect(pending).toHaveLength(1);
    pending[0]([{ ...item, status: "downloading" }]);
    await vi.waitFor(() => expect(pending).toHaveLength(2));
    pending[1]([{ ...item, status: "done" }]);
    await Promise.all([first, second]);
    expect(entries.value[0].status).toBe("done");
  });
});

describe("startSync", () => {
  it("speeds up right after an action starts a download", async () => {
    vi.useFakeTimers();
    let library = [{ ...item }];
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "library" ? library : null));
    const stop = startSync();
    await vi.advanceTimersByTimeAsync(0);
    library = [{ ...item, status: "downloading" }];
    await api.resume(1);
    await vi.advanceTimersByTimeAsync(0);
    const reads = () => invoke.mock.calls.filter(([cmd]) => cmd === "library").length;
    const before = reads();
    await vi.advanceTimersByTimeAsync(1000);
    expect(reads()).toBe(before + 1);
    stop();
    vi.useRealTimers();
  });
});
