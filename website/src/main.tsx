import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import "./App.css";

// `BASE_URL` is the GH Pages prefix injected by Vite (`/spotifai/` in
// production, `/` in dev). Stripping the trailing slash gives the
// router its `basename` without breaking root-path matches.
const basename = (import.meta.env.BASE_URL || "/").replace(/\/$/, "") || "/";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter basename={basename}>
      <App />
    </BrowserRouter>
  </StrictMode>,
);
