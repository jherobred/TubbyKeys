import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { App } from "./App";
import "./styles.css";

// The visualizer and tray pop-up windows are transparent: drop the page
// background before the first paint.
if (["overlay", "flyout"].includes(getCurrentWebviewWindow().label)) {
  document.documentElement.classList.add("transparent-root");
}

if (import.meta.env.PROD) {
  // No browser context menu outside text fields in the packaged app.
  document.addEventListener("contextmenu", (e) => {
    if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
