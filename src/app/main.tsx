import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PrefsProvider } from "@/prefs";
import { PanelApp } from "@/app/panel/PanelApp";
import { SettingsApp } from "@/app/settings/SettingsApp";
import { ToastApp } from "@/app/toast/ToastApp";
import { WhatsNewApp } from "@/app/whats-new/WhatsNewApp";
import "./theme.css";

const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const View =
  label === "panel" ? PanelApp : label === "toast" ? ToastApp : label === "whats-new" ? WhatsNewApp : SettingsApp;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PrefsProvider>
      <View />
    </PrefsProvider>
  </StrictMode>,
);
