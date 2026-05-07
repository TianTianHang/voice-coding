import { AssistantConsole } from "./components/AssistantConsole";
import { DebugToolsWindow } from "./components/DebugToolsWindow";

function App() {
  if (new URLSearchParams(window.location.search).get("window") === "debug-tools") {
    return <DebugToolsWindow />;
  }

  return <AssistantConsole />;
}

export default App;
