// The gallery's styled controls: a small set on nsh/base and the default
// theme's tokens (no raw colors here; the demos' own backgrounds are demo
// content, as the reference's). Targets: iOS and Android phones (system
// header, touch rows) and the desktops (the page header below, with Back).

import { type JSX, Text, View } from "nsh";
import { Button, Switch } from "nsh/base";

/** The header a page shows on the desktops (phones show the system's). */
export function PageHeader(props: { title: string; onBack?: () => void }) {
  return (
    <View class="h-12 flex-row items-center gap-3 border-b border-border bg-background px-4">
      <If when={props.onBack}>
        <Button
          testID="back"
          accessibilityLabel="Back"
          onPress={() => props.onBack?.()}
          class="rounded-(--radius) px-2 py-1 hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring"
        >
          <Text class="text-primary">Back</Text>
        </Button>
      </If>
      <Text class="text-base font-semibold text-foreground line-clamp-1">
        {props.title}
      </Text>
    </View>
  );
}

/** One row of a list that opens something. */
export function ListRow(props: { title: string; detail?: string; testID: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      onPress={() => props.onPress()}
      class="min-h-14 justify-center gap-0.5 border-b border-border bg-background px-4 py-3 hover:bg-muted active:bg-muted focus:bg-muted"
    >
      <Text class="text-base text-foreground">{props.title}</Text>
      <If when={props.detail}>{(detail) => <Text class="text-xs text-muted-foreground">{detail()}</Text>}</If>
    </Button>
  );
}

/** A labelled on/off setting row. */
export function SettingRow(props: {
  label: string;
  detail: string;
  checked: boolean;
  testID: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <View class="flex-row items-center gap-3 border-b border-border bg-background px-4 py-3">
      <View class="flex-1 gap-0.5">
        <Text class="text-base text-foreground">{props.label}</Text>
        <Text class="text-xs text-muted-foreground">{props.detail}</Text>
      </View>
      <Switch
        testID={props.testID}
        accessibilityLabel={props.label}
        checked={props.checked}
        onChange={({ checked }) => props.onChange(checked)}
        hitSlop={8}
        trackClass="h-6 w-10 justify-center rounded-full bg-muted px-0.5 checked:bg-primary transition-colors"
        thumbClass="h-5 w-5 rounded-full bg-background shadow-sm checked:translate-x-4 transition-transform"
      />
    </View>
  );
}

/** The probe's one line under a demo (muted, selectable by the tests). */
export function StatusLine(props: { testID: string; text: string }) {
  return (
    <View class="border-t border-border bg-background px-4 py-2">
      <Text testID={props.testID} class="text-xs text-muted-foreground line-clamp-2">
        {props.text}
      </Text>
    </View>
  );
}

/** A small text button for a demo's own controls. */
export function ActionButton(props: { label: string; testID?: string; onPress: () => void; children?: JSX.Element }) {
  return (
    <Button
      testID={props.testID}
      onPress={() => props.onPress()}
      class="items-center rounded-(--radius) bg-primary px-4 py-2 hover:bg-primary/90 active:bg-primary/80 focus:ring-2 focus:ring-ring"
    >
      <Text class="text-primary-foreground">{props.label}</Text>
    </Button>
  );
}

/** A page of text: why a demo is not here, or what a screen checks. */
export function Notice(props: { title: string; children?: JSX.Element }) {
  return (
    <View class="gap-3 p-4">
      <Text class="text-lg font-semibold text-foreground">{props.title}</Text>
      {props.children}
    </View>
  );
}

/** A paragraph of a notice. */
export function Paragraph(props: { testID?: string; children?: JSX.Element }) {
  return (
    <Text testID={props.testID} class="text-sm text-muted-foreground">
      {props.children}
    </Text>
  );
}
