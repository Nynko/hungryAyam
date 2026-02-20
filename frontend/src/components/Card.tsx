import type { JSX, ParentComponent } from "solid-js";
import { splitProps } from "solid-js";

interface CardProps {
  class?: string;
  style?: JSX.CSSProperties;
}

interface ClickableCardProps extends CardProps {
  onClick: () => void;
  disabled?: boolean;
}

/**
 * A simple Bulma `.card` wrapper — non-interactive display container.
 */
export const Card: ParentComponent<CardProps> = (props) => {
  const [local, rest] = splitProps(props, ["class", "style", "children"]);

  return (
    <div
      class={`card ${local.class ?? ""}`}
      style={local.style}
    >
      {local.children}
    </div>
  );
};

/**
 * A clickable Bulma `.card` with a CSS-driven hover shadow transition.
 * Uses the `.card-clickable` class defined in bulma.scss.
 */
export const ClickableCard: ParentComponent<ClickableCardProps> = (props) => {
  const [local, rest] = splitProps(props, [
    "class",
    "style",
    "children",
    "onClick",
    "disabled",
  ]);

  return (
    <div
      class={`card ${local.disabled ? "" : "card-clickable"} ${local.class ?? ""}`}
      style={{
        ...(local.style ?? {}),
        ...(local.disabled ? { opacity: "0.55", cursor: "not-allowed" } : {}),
      }}
      onClick={() => {
        if (!local.disabled) local.onClick();
      }}
    >
      {local.children}
    </div>
  );
};