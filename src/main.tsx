import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";

createRoot(document.getElementById("root")!).render(<p>{getCurrentWindow().label}</p>);
