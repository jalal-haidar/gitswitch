import { Layout } from "./components/Layout";
import { Dashboard } from "./components/Dashboard";
import "./styles/index.css";
import { ToastProvider } from "./components/ui/useToast";
import { Toaster } from "./components/ui/Toaster";

function App() {
  return (
    <ToastProvider>
      <Layout>
        <Dashboard />
      </Layout>
      <Toaster />
    </ToastProvider>
  );
}

export default App;
