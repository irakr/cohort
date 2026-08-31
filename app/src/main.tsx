import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initLogging } from "./log";
import "./styles/tokens.css";
import "./styles/components.css";

void initLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
