/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Display name shown in the navbar, footer, and page titles. */
  readonly VITE_APP_TITLE: string;
  /** Optional URL or path for the app logo/icon. */
  readonly VITE_APP_IMAGE_URL: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}