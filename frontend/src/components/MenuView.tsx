import { Show, For } from "solid-js";
import type { Menu } from "@bindings/Menu";
import MenuSectionView from "./MenuSectionView";
import { Card } from "@/components/Card";

interface MenuViewProps {
  menu: Menu;
}

export default function MenuView(props: MenuViewProps) {
  const activeSections = () =>
    props.menu.sections
      .filter((s) => s.is_active)
      .sort((a, b) => a.position - b.position);

  return (
    <Card class="mb-5">
      {/* Menu header */}
      <header class="card-header">
        <div class="card-header-title is-flex is-justify-content-space-between is-align-items-center">
          <div>
            <span class="is-size-5 has-text-weight-bold">{props.menu.name}</span>
            <Show when={props.menu.permanent}>
              <span class="tag is-primary is-light ml-2" style={{ "vertical-align": "middle" }}>
                Permanent
              </span>
            </Show>
            <Show when={!props.menu.is_active}>
              <span class="tag is-warning is-light ml-2" style={{ "vertical-align": "middle" }}>
                Inactive
              </span>
            </Show>
          </div>
        </div>
      </header>

      <div class="card-content">
        {/* Menu description */}
        <Show when={props.menu.description}>
          <p class="has-text-grey mb-4">{props.menu.description}</p>
        </Show>

        {/* Sections */}
        <Show
          when={activeSections().length > 0}
          fallback={
            <div class="has-text-centered py-4">
              <p class="has-text-grey-light is-italic">
                This menu has no active sections yet.
              </p>
            </div>
          }
        >
          <For each={activeSections()}>
            {(section) => <MenuSectionView section={section} depth={0} />}
          </For>
        </Show>
      </div>
    </Card>
  );
}