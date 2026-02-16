import { A } from "@solidjs/router";

export default function NotFound() {
  return (
    <section class="section">
      <div class="container">
        <div class="has-text-centered mt-6">
          <p class="title is-1">🐔 404</p>
          <p class="subtitle is-4">This page flew the coop!</p>
          <p class="mb-5">The page you're looking for doesn't exist or has been moved.</p>
          <A href="/" class="button is-primary is-medium">
            Back to Home
          </A>
        </div>
      </div>
    </section>
  );
}