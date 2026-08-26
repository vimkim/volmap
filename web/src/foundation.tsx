import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./foundation.css";

export function Foundation() {
  return <p role="status">React viewer foundation ready.</p>;
}

const host = typeof document === "undefined" ? null : document.getElementById("volmap-react-root");
if (host !== null) {
  createRoot(host).render(
    <StrictMode>
      <Foundation />
    </StrictMode>,
  );
}
