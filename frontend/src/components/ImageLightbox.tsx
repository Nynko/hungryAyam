import { Show, Portal } from "solid-js/web";

interface ImageLightboxProps {
  src: string;
  alt: string;
  onClose: () => void;
}

export default function ImageLightbox(props: ImageLightboxProps) {
  return (
    <Portal>
      <div
        style={{
          position: "fixed",
          inset: "0",
          "z-index": "9999",
          display: "flex",
          "align-items": "center",
          "justify-content": "center",
          background: "rgba(0,0,0,0.75)",
          cursor: "zoom-out",
        }}
        onClick={props.onClose}
        onKeyDown={(e) => e.key === "Escape" && props.onClose()}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-label={`Image: ${props.alt}`}
      >
        <img
          src={props.src}
          alt={props.alt}
          style={{
            "max-width": "90vw",
            "max-height": "90vh",
            "object-fit": "contain",
            "border-radius": "8px",
            "box-shadow": "0 8px 32px rgba(0,0,0,0.5)",
            cursor: "default",
          }}
          onClick={(e) => e.stopPropagation()}
        />
      </div>
    </Portal>
  );
}
