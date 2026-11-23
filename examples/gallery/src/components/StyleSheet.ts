// react-native's `StyleSheet.create` for the ported demos' style objects:
// NSH's `style` takes the same keys (flex, backgroundColor, width, ...), so
// the objects stay as the reference wrote them, type checked by NSH.

import type { TextStyle, ViewStyle } from "nsh";

export const StyleSheet = {
  create: <T extends Record<string, ViewStyle & TextStyle>>(styles: T): T => styles,
  absoluteFill: { position: "absolute", left: 0, top: 0, right: 0, bottom: 0 } as ViewStyle,
};
