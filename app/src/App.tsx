import { Rail } from "./app/Rail";
import { Notifications } from "./app/notifications";
import { NavProvider, useNav } from "./app/router";
import { OpenAssists } from "./screens/OpenAssists";
import { NewAssist } from "./screens/NewAssist";
import { AssistDetail } from "./screens/AssistDetail";
import { CloseAssist } from "./screens/CloseAssist";
import { MyRecord } from "./screens/MyRecord";
import { Setup } from "./screens/Setup";

function Screens() {
  const { screen } = useNav();
  switch (screen.name) {
    case "assists":
      return <OpenAssists />;
    case "new":
      return <NewAssist />;
    case "assist":
      return <AssistDetail key={screen.ref} assistRef={screen.ref} />;
    case "close":
      return <CloseAssist key={screen.ref} assistRef={screen.ref} />;
    case "record":
      return <MyRecord />;
  }
}

function Shell() {
  const { currentUserId } = useNav();
  if (!currentUserId) {
    return <Setup />;
  }
  // Remount everything when the identity changes (sign-out and back in).
  return (
    <div
      key={currentUserId}
      style={{ display: "grid", gridTemplateColumns: "64px 1fr", minHeight: "100vh" }}
    >
      <Rail />
      <main style={{ minWidth: 0 }}>
        <Screens />
      </main>
      <Notifications />
    </div>
  );
}

export default function App() {
  return (
    <NavProvider>
      <Shell />
    </NavProvider>
  );
}
