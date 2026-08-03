import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";
import { MotionConfig } from "motion/react";
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
    <MotionConfig reducedMotion="user" transition={{ duration: 0.16, ease: [0.2, 0, 0, 1] }}>
      <QueryClientProvider client={queryClient}>
        <TooltipProvider delayDuration={300} skipDelayDuration={200}>
          <RouterProvider router={router} />
        </TooltipProvider>
      </QueryClientProvider>
    </MotionConfig>
  </React.StrictMode>
);
