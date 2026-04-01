import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Layout } from "./components/Layout";
import { Dashboard } from "./components/Dashboard";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles/index.css";
import { ToastProvider } from "./components/ui/useToast";
import { Toaster } from "./components/ui/Toaster";

function App() {
  useEffect(() => {
    // Apply the saved theme as a data-theme attribute on the document root
    invoke<string>("get_theme")
      .then((theme) => {
        document.documentElement.setAttribute("data-theme", theme ?? "system");
      })
      .catch(() => {
        document.documentElement.setAttribute("data-theme", "system");
      });
  }, []);

  return (
    <ErrorBoundary>
      <ToastProvider>
        <Layout>
          <Dashboard />
        </Layout>
        <Toaster />
      </ToastProvider>
    </ErrorBoundary>
  );
}

export default App;
