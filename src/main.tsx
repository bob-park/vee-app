import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PrefsProvider } from "./prefs.tsx";
import { Panel } from "./panel/Panel.tsx";
import { Settings } from "./settings/Settings.tsx";
import { Toast } from "./toast/Toast.tsx";
import "./theme.css";

const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const View = label === "panel" ? Panel : label === "toast" ? Toast : Settings;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PrefsProvider>
      <View />
    </PrefsProvider>
  </StrictMode>,
);
