import { Mascot } from "./Art";

/**
 * Windows puts new tray icons behind the ^ button and gives apps no way to
 * move themselves out. This shows the one drag that does it.
 */
export function TrayTip() {
  return (
    <div className="tray-tip">
      <div className="tray-demo" aria-hidden="true">
        <div className="tray-demo-menu">
          <i />
          <i />
          <i />
        </div>
        <div className="tray-demo-bar">
          <span className="tray-demo-chevron">^</span>
          <i />
          <i />
          <span className="tray-demo-clock">9:41</span>
        </div>
        <div className="tray-demo-icon">
          <Mascot size={18} />
        </div>
        <div className="tray-demo-cursor" />
      </div>
      <p>
        Windows hides new tray icons behind <kbd>^</kbd>. Drag the TubbyKeys icon onto the taskbar once and it stays
        one click away.
      </p>
    </div>
  );
}
