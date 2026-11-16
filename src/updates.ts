import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { ref, shallowRef, watch } from "vue";
import { api } from "./api";

const CHECK_EVERY = 6 * 60 * 60 * 1000;
const RELEASES = "https://github.com/wlaith/cardamoi/releases";

/** A newer version, already downloaded and waiting for a restart. Shallow: Update holds private fields a proxy breaks. */
export const readyUpdate = shallowRef<Update | null>(null);
/** How the last check went, for the Updates section in Preferences. */
export const checkState = ref<"idle" | "checking" | "upToDate" | "failed" | "dev">("idle");
export const checkError = ref<string | null>(null);
export const installing = ref(false);
export const installError = ref<string | null>(null);
/** Hidden until next launch. */
export const updateDismissed = ref(false);
export const currentVersion = ref<string | null>(null);
getVersion().then((version) => (currentVersion.value = version), () => {});

watch(readyUpdate, (update) => {
  if (update) api.updateReady(update.version).catch((error) => console.warn("Couldn't enable Restart to Update:", error));
});

export async function checkForUpdate() {
  if (checkState.value === "checking" || readyUpdate.value) return;
  checkState.value = "checking";
  checkError.value = null;
  try {
    if (import.meta.env.DEV) {
      if (!import.meta.env.VITE_PREVIEW_UPDATE) return void (checkState.value = "dev");
      await new Promise((done) => setTimeout(done, 1000));
      readyUpdate.value = previewUpdate();
    } else {
      const update = await check();
      if (update) {
        await update.download();
        readyUpdate.value = update;
      }
    }
    checkState.value = readyUpdate.value ? "idle" : "upToDate";
  } catch (error) {
    console.warn("Update check failed:", error);
    checkError.value = String(error);
    checkState.value = "failed";
  }
}

export function startUpdateChecks(): () => void {
  if (import.meta.env.DEV && !import.meta.env.VITE_PREVIEW_UPDATE) return () => {};
  checkForUpdate();
  const timer = window.setInterval(checkForUpdate, CHECK_EVERY);
  return () => window.clearInterval(timer);
}

function previewUpdate(): Update {
  return {
    version: "9.9.9",
    install: () => new Promise((_, reject) => setTimeout(() => reject("this is only a preview"), 2000)),
  } as unknown as Update;
}

export function openRelease(version: string) {
  openUrl(`${RELEASES}/tag/v${version}`).catch(() => {});
}

/** The GitHub page listing every release. */
export function openAllReleases() {
  openUrl(RELEASES).catch(() => {});
}

export async function installUpdate() {
  const update = readyUpdate.value;
  if (!update || installing.value) return;
  updateDismissed.value = false;
  installing.value = true;
  installError.value = null;
  try {
    await update.install();
    await relaunch();
  } catch (error) {
    installError.value = String(error);
    installing.value = false;
  }
}
