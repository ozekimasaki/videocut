import { Component, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

class ShellBoundary extends Component<{ children: ReactNode }, { message: string | null }> {
  state = { message: null as string | null };

  static getDerivedStateFromError(error: Error): { message: string } {
    return { message: error.message };
  }

  render() {
    if (this.state.message) {
      return (
        <pre className="crash" data-testid="crash">
          {this.state.message}
        </pre>
      );
    }
    return this.props.children;
  }
}

const root = document.getElementById("root");
if (!root) throw new Error("root が見つかりません");
createRoot(root).render(
  <ShellBoundary>
    <App />
  </ShellBoundary>,
);
