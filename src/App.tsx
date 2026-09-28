import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Flyout } from "./views/Flyout";
import { Overlay } from "./views/Overlay";
import { SettingsWindow } from "./views/Settings";

/** One bundle, three windows: pick the view from the window label. */
export function App() {
  const label = getCurrentWebviewWindow().label;
  if (label === "overlay") return <Overlay />;
  if (label === "flyout") return <Flyout />;
  return <SettingsWindow />;
}
