import { Component, type ReactNode } from "react";
import { ErrorState } from "./ErrorState";

interface Props {
  children: ReactNode;
  /** reset key — when it changes, the boundary clears its error (e.g. route id) */
  resetKey?: string;
}

interface State {
  error: unknown;
}

/** Route-level boundary so a render throw never white-screens (02 §7). */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: unknown): State {
    return { error };
  }

  componentDidUpdate(prev: Props) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: "48px 24px", display: "grid", placeItems: "center", height: "100%" }}>
          <ErrorState
            error={this.state.error}
            onRetry={() => this.setState({ error: null })}
          />
        </div>
      );
    }
    return this.props.children;
  }
}
