import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";
import { installTauriDevShim } from "@/lib/tauriDevShim";
import { queryClient } from "@/lib/queryClient";
import { router } from "@/App";
import { TooltipProvider } from "@/components/ui/tooltip";
// Tailwind is the single styling system; theme.css maps design tokens (02 §2).
import "@/styles/theme.css";

// Browser preview outside the Tauri shell: feed sample data instead of crashing
// on the missing native `invoke` (DEV + no Tauri host only; inert in the app).
installTauriDevShim();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <TooltipProvider delayDuration={300} skipDelayDuration={200}>
        <RouterProvider router={router} />
      </TooltipProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
