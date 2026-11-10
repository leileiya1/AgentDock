import { createBrowserRouter, Navigate } from "react-router-dom";
import { AppLayout } from "@/routes/AppLayout";
import { IndexRedirect } from "@/routes/IndexRedirect";
import { Onboarding } from "@/routes/Onboarding";
import { TaskList } from "@/routes/TaskList";
import { TaskDetail } from "@/routes/TaskDetail";
import { Settings } from "@/routes/Settings";
import { Town } from "@/routes/Town";

export const router = createBrowserRouter([
  {
    element: <AppLayout />,
    children: [
      { path: "/", element: <IndexRedirect /> },
      { path: "/onboarding", element: <Onboarding /> },
      { path: "/p/:projectId", element: <TaskList /> },
      { path: "/p/:projectId/t/:taskId", element: <TaskDetail /> },
      { path: "/town", element: <Town /> },
      { path: "/settings", element: <Settings /> },
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
]);
