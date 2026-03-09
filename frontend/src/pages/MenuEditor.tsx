import { createSignal, createResource, Show, For, onMount, onCleanup } from "solid-js";
import { A, useParams, useNavigate } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";
import type { Menu } from "@bindings/Menu";
import type { MenuSection } from "@bindings/MenuSection";
import type { ApiResponse } from "@bindings/ApiResponse";
import MenuEditorToolbar from "@/components/menu-editor/MenuEditorToolbar";
import SectionEditor from "@/components/menu-editor/SectionEditor";
import CreateModeSelector from "@/components/menu-editor/CreateModeSelector";
import OfferEditor from "@/components/menu-editor/OfferEditor";
import {
  editorState,
  editorLoading,
  editorError,
  dirty,
  initNewMenu,
  fetchAndLoadMenu,
  addSection,
  moveSectionToIndex,
} from "@/stores/menuEditorStore";
import { isAuthenticated } from "@/stores/authStore";
import { setupSortableMonitor } from "@/lib/dnd";

// ── Data fetchers ─────────────────────────────────────────────────

async function fetchRestaurant(id: string): Promise<Restaurant> {
  const res = await fetch(`/api/restaurants/${id}`);
  if (!res.ok) {
    if (res.status === 404) throw new Error("Restaurant not found");
    throw new Error(`Failed to load restaurant (${res.status})`);
  }
  const json: ApiResponse<Restaurant> = await res.json();
  if (!json.success || json.data == null) {
    throw new Error(json.error ?? "Unexpected response");
  }
  return json.data;
}

// ── Component ─────────────────────────────────────────────────────

export default function MenuEditor() {
  const params = useParams<{ id: string; menuId?: string }>();
  const navigate = useNavigate();

  const isEditMode = () => !!params.menuId;
  const [modeSelected, setModeSelected] = createSignal(false);
  const [showAddSection, setShowAddSection] = createSignal(false);
  const [newSectionName, setNewSectionName] = createSignal("");

  // Fetch restaurant info
  const [restaurant] = createResource(() => params.id, fetchRestaurant);

  // ── Initialize store on mount ───────────────────────────────────
  onMount(async () => {
    if (isEditMode()) {
      // Edit mode: load existing menu
      await fetchAndLoadMenu(params.menuId!);
      setModeSelected(true); // skip mode selector
    }
    // Create mode: store init happens when user picks manual mode
  });

  // ── Warn on navigation with unsaved changes ─────────────────────
  const handleBeforeUnload = (e: BeforeUnloadEvent) => {
    if (dirty()) {
      e.preventDefault();
    }
  };

  onMount(() => window.addEventListener("beforeunload", handleBeforeUnload));
  onCleanup(() => window.removeEventListener("beforeunload", handleBeforeUnload));

  // ── Drag-and-drop monitor for top-level sections ────────────────
  onMount(() => {
    const cleanup = setupSortableMonitor({
      type: "section",
      // Only handle top-level sections (parentId === null)
      canMonitor: (src) => src.parentId === null,
      onReorder: (sourceId, _sourceIndex, destinationIndex) => {
        moveSectionToIndex(sourceId, destinationIndex);
      },
    });
    onCleanup(cleanup);
  });

  // ── Handlers ────────────────────────────────────────────────────

  const handleSelectManual = () => {
    initNewMenu(params.id);
    setModeSelected(true);
  };

  const handleSaved = () => {
    navigate(`/restaurants/${params.id}`, { replace: true });
  };

  const handleCancel = () => {
    navigate(`/restaurants/${params.id}`);
  };

  const handleAddSection = () => {
    const name = newSectionName().trim();
    if (!name) return;
    addSection(null, name);
    setNewSectionName("");
    setShowAddSection(false);
  };

  // ── Sorted sections ─────────────────────────────────────────────
  const sortedSections = () =>
    [...editorState.draft.sections].sort((a, b) => a.position - b.position);

  return (
    <section class="section">
      <div class="container">
        {/* ── Breadcrumb ───────────────────────────────────── */}
        <nav class="breadcrumb is-small mb-4" aria-label="breadcrumbs">
          <ul>
            <li>
              <A href="/restaurants">Restaurants</A>
            </li>
            <li>
              <A href={`/restaurants/${params.id}`}>
                {restaurant()?.name ?? "Restaurant"}
              </A>
            </li>
            <li class="is-active">
              <a href="#" aria-current="page">
                {isEditMode() ? "Edit Menu" : "New Menu"}
              </a>
            </li>
          </ul>
        </nav>

        {/* ── Auth guard ───────────────────────────────────── */}
        <Show when={!isAuthenticated()}>
          <div class="notification is-warning is-light">
            <p>
              <strong>Authentication required.</strong> You need to be logged in
              to {isEditMode() ? "edit" : "create"} a menu.
            </p>
            <A href="/login" class="button is-small is-warning is-outlined mt-2">
              Log in
            </A>
          </div>
        </Show>

        {/* ── Loading state ────────────────────────────────── */}
        <Show when={editorLoading()}>
          <div class="has-text-centered py-6">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey mt-2">Loading menu…</p>
          </div>
        </Show>

        {/* ── Editor error (fetch level) ───────────────────── */}
        <Show when={editorError() && !modeSelected()}>
          <div class="notification is-danger is-light">
            <p>
              <strong>Error:</strong> {editorError()}
            </p>
            <A
              href={`/restaurants/${params.id}`}
              class="button is-small is-danger is-outlined mt-3"
            >
              ← Back to restaurant
            </A>
          </div>
        </Show>

        {/* ── Mode selector (create only) ──────────────────── */}
        <Show when={!isEditMode() && !modeSelected() && !editorLoading() && isAuthenticated()}>
          <div class="mb-5">
            <div class="has-text-centered mb-4">
              <h1 class="title is-3">
                Create a Menu
                <Show when={restaurant()}>
                  <span class="has-text-grey has-text-weight-normal">
                    {" "}
                    for {restaurant()!.name}
                  </span>
                </Show>
              </h1>
            </div>
            <CreateModeSelector onSelectManual={handleSelectManual} />
          </div>
        </Show>

        {/* ── Main editor ──────────────────────────────────── */}
        <Show when={modeSelected() && isAuthenticated() && !editorLoading()}>
          {/* Page title */}
          <div class="mb-4">
            <h1 class="title is-3">
              {isEditMode() ? "Edit Menu" : "Create Menu"}
              <Show when={restaurant()}>
                <span class="has-text-grey has-text-weight-normal">
                  {" "}
                  — {restaurant()!.name}
                </span>
              </Show>
            </h1>
          </div>

          {/* Toolbar (name, description, toggles, save/cancel) */}
          <MenuEditorToolbar onSaved={handleSaved} onCancel={handleCancel} />

          {/* ── Offer Editor (integrated into menu) ─────────── */}
          <OfferEditor
            restaurantId={params.id}
            menuId={editorState.draft.id}
            menuSections={editorState.draft.sections as unknown as MenuSection[]}
          />

          {/* ── Sections ────────────────────────────────────── */}
          <div class="mb-4">
            <div class="is-flex is-justify-content-space-between is-align-items-center mb-3">
              <h2 class="title is-5 mb-0">Sections</h2>
              <span class="has-text-grey is-size-7">
                {editorState.draft.sections.length} section
                {editorState.draft.sections.length !== 1 ? "s" : ""}
              </span>
            </div>

            {/* Sections list */}
            <Show when={sortedSections().length > 0}>
              <For each={sortedSections()}>
                {(section, index) => (
                  <SectionEditor
                    section={section}
                    depth={0}
                    siblingCount={sortedSections().length}
                    sortedIndex={index()}
                    draggable={true}
                    onMoveUp={() => moveSectionToIndex(section.id, index() - 1)}
                    onMoveDown={() => moveSectionToIndex(section.id, index() + 1)}
                  />
                )}
              </For>
            </Show>

            {/* Empty state */}
            <Show when={sortedSections().length === 0}>
              <div class="notification is-light has-text-centered">
                <p class="is-size-4 mb-2">📋</p>
                <p class="has-text-grey">
                  No sections yet. Add your first section to start building the
                  menu.
                </p>
              </div>
            </Show>

            {/* Add section */}
            <Show
              when={showAddSection()}
              fallback={
                <button
                  class="button is-primary is-outlined mt-3"
                  onClick={() => setShowAddSection(true)}
                >
                  <span class="icon is-small">
                    <span>📁</span>
                  </span>
                  <span>Add section</span>
                </button>
              }
            >
              <div
                class="box p-4 mt-3"
                style={{ "background-color": "var(--bulma-success-light)", "max-width": "500px" }}
              >
                <p class="has-text-weight-semibold is-size-6 mb-2">
                  New section
                </p>
                <div class="field has-addons">
                  <div class="control is-expanded">
                    <input
                      class="input"
                      type="text"
                      placeholder="Section name (e.g. Appetizers, Mains, Drinks)"
                      value={newSectionName()}
                      onInput={(e) => setNewSectionName(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleAddSection();
                        if (e.key === "Escape") {
                          setShowAddSection(false);
                          setNewSectionName("");
                        }
                      }}
                      ref={(el) => setTimeout(() => el.focus(), 0)}
                    />
                  </div>
                  <div class="control">
                    <button
                      class="button is-primary"
                      disabled={!newSectionName().trim()}
                      onClick={handleAddSection}
                    >
                      Add
                    </button>
                  </div>
                  <div class="control">
                    <button
                      class="button is-light"
                      onClick={() => {
                        setShowAddSection(false);
                        setNewSectionName("");
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </section>
  );
}