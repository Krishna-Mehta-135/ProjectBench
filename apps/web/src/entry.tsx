import React from "react";
import { createRoot } from "react-dom/client";
import PublicSite from "./PublicSite";

if (import.meta.env.DEV && window.location.pathname.startsWith("/app")) {
  await import("./main");
} else {
  createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <PublicSite />
    </React.StrictMode>,
  );
}
