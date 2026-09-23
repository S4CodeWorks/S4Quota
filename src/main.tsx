import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/source-sans-3";
import "./design-system/index.css";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
