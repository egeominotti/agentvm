import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { RESTTY_FONTS } from "./features/machine/terminal/restty-fonts";
import "./styles/tokens.css";

// The terminal engine and its fonts start loading with the page, not when the first terminal
// opens (restty then finds them in the browser's cache). xterm.js's font loads only if it is used.
for (const { url } of RESTTY_FONTS) fetch(url).catch(() => {});
requestIdleCallback(() => import("./features/machine/terminal/restty-view"), { timeout: 2000 });

const client = new QueryClient({
  defaultOptions: {
    // The server is on this Mac: retry little, and keep showing the last data while refreshing.
    queries: { retry: 1, staleTime: 500, refetchOnWindowFocus: true },
  },
});

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");
createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);
