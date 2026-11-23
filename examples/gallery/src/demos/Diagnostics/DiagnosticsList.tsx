// apps/example/src/Diagnostics/DiagnosticsList.tsx: the sub-list of
// diagnostic screens, each reproducing a bug or stressing an invariant of
// the native implementation. The ones that test React Native machinery
// (worklet runtimes, JS reloads, Android view props) say why they are not
// ported.

import type { JSX } from "nsh";
import type { Entry } from "../../examples";
import { AsyncStarvation } from "./AsyncStarvation";
import { BackgroundDetach } from "./BackgroundDetach";
import { ContextEdgeCases } from "./ContextEdgeCases";
import { DeviceDestroyBeforeDetach } from "./DeviceDestroyBeforeDetach";
import { DeviceLostHang } from "./DeviceLostHang";
import { RenderAfterUnmount } from "./RenderAfterUnmount";
import { SurfaceChurn } from "./SurfaceChurn";
import { ViewFormatsUseAfterFree } from "./ViewFormatsUseAfterFree";

const diagnostic = (screen: string, title: string, component: () => JSX.Element): Entry => ({
  kind: "demo",
  screen,
  title,
  component,
  diagnostic: true,
});

export const diagnostics: Entry[] = [
  diagnostic("AsyncStarvation", "⚠️ Async Runner Starvation", AsyncStarvation),
  diagnostic("DeviceLostHang", "⚠️ Device Lost Hang", DeviceLostHang),
  {
    kind: "skip",
    screen: "WorkletRequestAdapter",
    title: "⚠️ Worklet requestAdapter",
    reason:
      "Calls requestAdapter() on a react-native-worklets UI runtime, a crash specific to React Native's worklet " +
      "runtimes. NSH has one JS runtime, so there is nothing to reproduce.",
  },
  {
    kind: "skip",
    screen: "ReloadLifecycle",
    title: "⚠️ Runtime Reload Lifecycle (#461)",
    reason:
      "Reloads the JS runtime with DevSettings.reload() while canvases render. NSH apps have no runtime reload " +
      "API to call from the app (hot updates are the dev host's).",
  },
  diagnostic("ContextEdgeCases", "⚠️ Context Edge Cases", ContextEdgeCases),
  diagnostic("ViewFormatsUseAfterFree", "⚠️ viewFormats Use-After-Free", ViewFormatsUseAfterFree),
  diagnostic("RenderAfterUnmount", "⚠️ Render After Unmount", RenderAfterUnmount),
  diagnostic("BackgroundDetach", "⚠️ Background Detach", BackgroundDetach),
  diagnostic("SurfaceChurn", "⚠️ Surface Churn", SurfaceChurn),
  {
    kind: "skip",
    screen: "TransparencyMode",
    title: "⚠️ Android View Props",
    reason:
      "Switches Android's backing views (TextureView, SurfaceView, HardwareBufferView) and their opacity: Android " +
      "props only, ignored on Apple platforms. Android's canvas belongs to the surfaces-other area and is not " +
      "exercised by this gallery.",
  },
  diagnostic("DeviceDestroyBeforeDetach", "⚠️ Device Destroy Before Detach", DeviceDestroyBeforeDetach),
];
