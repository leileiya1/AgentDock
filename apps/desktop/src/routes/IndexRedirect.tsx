import { Navigate } from "react-router-dom";
import { useProjects } from "@/hooks/useProjects";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";

/** "/" → onboarding when no project, else the first project (03 §3). */
export function IndexRedirect() {
  const projects = useProjects();

  if (projects.isLoading) {
    return (
      <div style={{ padding: 24, maxWidth: 480 }}>
        <SkeletonRows rows={3} />
      </div>
    );
  }
  if (projects.isError) {
    return (
      <div style={{ padding: 48, display: "grid", placeItems: "center" }}>
        <ErrorState error={projects.error} onRetry={() => projects.refetch()} />
      </div>
    );
  }
  if (!projects.data || projects.data.length === 0) {
    return <Navigate to="/onboarding" replace />;
  }
  return <Navigate to={`/p/${projects.data[0].id}`} replace />;
}
