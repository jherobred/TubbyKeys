import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { App } from "./App";
import "./styles.css";

// The visualizer window is transparent: drop the page background before first paint.
if (getCurrentWebviewWindow().label === "overlay") {
  document.documentElement.classList.add("overlay-root");
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
