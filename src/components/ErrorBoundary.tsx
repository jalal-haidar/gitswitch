import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            padding: "2rem",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            minHeight: "200px",
            gap: "1rem",
            color: "var(--text-color, #ccc)",
          }}
        >
          <h2 style={{ margin: 0 }}>Something went wrong</h2>
          <p
            style={{
              margin: 0,
              opacity: 0.7,
              fontSize: "0.875rem",
              maxWidth: "400px",
              textAlign: "center",
            }}
          >
            {this.state.error?.message || "An unexpected error occurred."}
          </p>
          <button
            onClick={this.handleReset}
            className="btn btn-secondary"
            style={{ marginTop: "0.5rem" }}
          >
            Try Again
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
