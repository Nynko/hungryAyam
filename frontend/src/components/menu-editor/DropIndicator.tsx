import type { Edge } from "@atlaskit/pragmatic-drag-and-drop-hitbox/types";
import type { JSX } from "solid-js";

interface DropIndicatorProps {
  /** The closest edge detected by the hitbox algorithm. `null` means hidden. */
  edge: Edge | null;
  /** The gap between sibling items (e.g. "1rem", "8px"). Defaults to "8px". */
  gap?: string;
}

const strokeSize = 2;
const terminalSize = 8;
const offsetToAlignTerminalWithLine = (strokeSize - terminalSize) / 2;

/**
 * A drop indicator line rendered at the top or bottom edge of a sortable item
 * to show where the dragged element will land on drop.
 *
 * Place this inside the sortable item's container (which must have
 * `position: relative`). The indicator uses absolute positioning and
 * accounts for the gap between siblings so the line sits exactly between items.
 *
 * Ported from @atlaskit/pragmatic-drag-and-drop-react-drop-indicator/box,
 * styled with plain CSS (no Tailwind).
 */
export default function DropIndicator(props: DropIndicatorProps) {
  const gap = () => props.gap ?? "8px";

  const isTop = () => props.edge === "top";
  const isBottom = () => props.edge === "bottom";
  const visible = () => isTop() || isBottom();

  const lineStyle = (): JSX.CSSProperties => {
    if (!visible()) return { display: "none" };

    const base: JSX.CSSProperties = {
      position: "absolute",
      "z-index": "10",
      "pointer-events": "none",
      "box-sizing": "border-box",

      /* Horizontal line spanning the width */
      height: `${strokeSize}px`,
      left: `${terminalSize / 2}px`,
      right: "0",

      "background-color": "hsl(204, 86%, 53%)",
    };

    /* Position based on edge — offset into the gap between items */
    if (isTop()) {
      base.top = `calc(-0.5 * (${gap()} + ${strokeSize}px))`;
    } else {
      base.bottom = `calc(-0.5 * (${gap()} + ${strokeSize}px))`;
    }

    return base;
  };

  const terminalStyle = (): JSX.CSSProperties => {
    const base: JSX.CSSProperties = {
      position: "absolute",
      width: `${terminalSize}px`,
      height: `${terminalSize}px`,
      "box-sizing": "border-box",
      "border-radius": "50%",
      border: `${strokeSize}px solid hsl(204, 86%, 53%)`,
      background: "transparent",

      /* Align the circle so its center sits on the line */
      left: `-${terminalSize}px`,
    };

    if (isTop()) {
      base.top = `${offsetToAlignTerminalWithLine}px`;
    } else {
      base.bottom = `${offsetToAlignTerminalWithLine}px`;
    }

    return base;
  };

  return (
    <>
      {visible() && (
        <div style={lineStyle()}>
          <div style={terminalStyle()} />
        </div>
      )}
    </>
  );
}