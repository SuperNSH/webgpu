// getPreferredHighBitDepthCanvasFormat (react-native-webgpu's
// src/formats.ts), area: react-parity. React Native's `Platform.OS` is
// NSH's `platform.os`.

import { platform } from "nsh";

/**
 * The canvas format most likely to reach the display with more than 8 bits
 * per channel, keeping SDR sRGB semantics: `"rgb10a2unorm"` on Android,
 * `"rgba16float"` elsewhere (Apple platforms, the web, the desktops).
 */
export const getPreferredHighBitDepthCanvasFormat = (): GPUTextureFormat => {
  return platform.os === "android" ? "rgb10a2unorm" : "rgba16float";
};
