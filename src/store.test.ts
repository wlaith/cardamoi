// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

const { entries, hideEntries, refresh, showEntries } = await import("./store");

const item = { id: 1, title: "Taxi Driver", status: "done", videos: [] };

describe("refresh", () => {
  beforeEach(() => invoke.mockReset());

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
});
