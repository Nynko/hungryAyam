/**
 * Returns true if the image_url value is an actual image src (http/https URL
 * or a /uploads/ path). Returns false when it is an emoji string.
 */
export function isImageSrc(value: string): boolean {
  return (
    value.startsWith("http://") ||
    value.startsWith("https://") ||
    value.startsWith("/uploads/")
  );
}
