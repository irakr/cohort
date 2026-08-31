import { Rail } from "./app/Rail";
import { NavProvider, useNav } from "./app/router";
import { OpenAssists } from "./screens/OpenAssists";
import { NewAssist } from "./screens/NewAssist";
import { AssistDetail } from "./screens/AssistDetail";
import { CloseAssist } from "./screens/CloseAssist";
import { MyRecord } from "./screens/MyRecord";

function Screens() {
  const { screen, currentUserId } = useNav();
  // Remount screens when the acting user changes so everything refetches.
  switch (screen.name) {
    case "assists":
      return <OpenAssists key={currentUserId} />;
    case "new":
      return <NewAssist key={currentUserId} />;
    case "assist":
      return <AssistDetail key={`${currentUserId}:${screen.ref}`} assistRef={screen.ref} />;
    case "close":
      return <CloseAssist key={`${currentUserId}:${screen.ref}`} assistRef={screen.ref} />;
    case "record":
      return <MyRecord key={currentUserId} />;
  }
}

export default function App() {
  return (
    <NavProvider>
      <div style={{ display: "grid", gridTemplateColumns: "64px 1fr", minHeight: "100vh" }}>
        <Rail />
        <main style={{ minWidth: 0 }}>
          <Screens />
        </main>
      </div>
    </NavProvider>
  );
}
