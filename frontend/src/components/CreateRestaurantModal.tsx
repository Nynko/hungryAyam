import { createSignal, Show } from "solid-js";
import {
  createRestaurant,
  restaurantsError,
  clearRestaurantsError,
} from "@/stores/restaurantStore";
import { isAuthenticated } from "@/stores/authStore";
import AuthPanel from "@/components/AuthPanel";
import type { CreateRestaurant } from "@bindings/CreateRestaurant";

interface CreateRestaurantModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function CreateRestaurantModal(props: CreateRestaurantModalProps) {
  const [name, setName] = createSignal("");
  const [imageUrl, setImageUrl] = createSignal("");
  const [submitting, setSubmitting] = createSignal(false);
  const [validationError, setValidationError] = createSignal<string | null>(null);

  const resetForm = () => {
    setName("");
    setImageUrl("");
    setValidationError(null);
  };

  const handleClose = () => {
    if (submitting()) return;
    resetForm();
    clearRestaurantsError();
    props.onClose();
  };

  const handleSubmit = async (e: SubmitEvent) => {
    e.preventDefault();

    const trimmedName = name().trim();
    if (!trimmedName) {
      setValidationError("Restaurant name is required.");
      return;
    }

    const request: CreateRestaurant = {
      name: trimmedName,
      image_url: imageUrl().trim() || null,
    };

    setSubmitting(true);
    setValidationError(null);

    const result = await createRestaurant(request);

    setSubmitting(false);

    if (result) {
      resetForm();
      props.onClose();
    }
  };

  const displayError = () => validationError() || restaurantsError();

  return (
    <div class="modal" classList={{ "is-active": props.isOpen }}>
      <div class="modal-background" onClick={handleClose} />
      <div class="modal-card">
        <header class="modal-card-head">
          <p class="modal-card-title">➕ New Restaurant</p>
          <button
            class="delete"
            aria-label="close"
            onClick={handleClose}
            disabled={submitting()}
          />
        </header>

        <Show
          when={isAuthenticated()}
          fallback={
            <section class="modal-card-body">
              <div class="notification is-warning is-light mb-4">
                <p>You need to be authenticated to create a restaurant.</p>
              </div>
              <AuthPanel />
            </section>
          }
        >
          <form onSubmit={handleSubmit}>
            <section class="modal-card-body">
              <Show when={displayError()}>
                <div class="notification is-danger is-light">
                  <button
                    class="delete"
                    type="button"
                    onClick={() => {
                      setValidationError(null);
                      clearRestaurantsError();
                    }}
                  />
                  {displayError()}
                </div>
              </Show>

              {/* Name field */}
              <div class="field">
                <label class="label">Name</label>
                <div class="control">
                  <input
                    class="input"
                    type="text"
                    placeholder="e.g. Ayam Goreng Palace"
                    value={name()}
                    onInput={(e) => setName(e.currentTarget.value)}
                    required
                    autofocus
                    disabled={submitting()}
                  />
                </div>
              </div>

              {/* Image URL field */}
              <div class="field">
                <label class="label">Image URL</label>
                <div class="control">
                  <input
                    class="input"
                    type="url"
                    placeholder="https://example.com/logo.png (optional)"
                    value={imageUrl()}
                    onInput={(e) => setImageUrl(e.currentTarget.value)}
                    disabled={submitting()}
                  />
                </div>
                <p class="help">
                  Optional — a logo or photo for the restaurant card.
                </p>
              </div>

              {/* Image preview */}
              <Show when={imageUrl().trim()}>
                <div class="field">
                  <label class="label">Preview</label>
                  <figure
                    class="image is-3by2"
                    style={{
                      "max-width": "240px",
                      "background-color": "var(--bulma-scheme-main-bis)",
                      overflow: "hidden",
                      "border-radius": "4px",
                    }}
                  >
                    <img
                      src={imageUrl().trim()}
                      alt="Preview"
                      style={{
                        "object-fit": "cover",
                        width: "100%",
                        height: "100%",
                      }}
                      onError={(e) => {
                        (e.currentTarget as HTMLImageElement).style.display =
                          "none";
                      }}
                    />
                  </figure>
                </div>
              </Show>
            </section>

            <footer class="modal-card-foot">
              <div class="buttons">
                <button
                  class="button is-primary"
                  type="submit"
                  classList={{ "is-loading": submitting() }}
                  disabled={submitting() || !name().trim()}
                >
                  Create
                </button>
                <button
                  class="button"
                  type="button"
                  onClick={handleClose}
                  disabled={submitting()}
                >
                  Cancel
                </button>
              </div>
            </footer>
          </form>
        </Show>
      </div>
    </div>
  );
}