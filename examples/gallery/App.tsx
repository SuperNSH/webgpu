// The gallery: react-native-webgpu's example app (apps/example/src) on NSH.
// A Home list routes to each demo, as the reference's Home.tsx does.

import { type JSX, mount, Scroll, Text, View } from "nsh";
import { createNavStack, NavPage, NavStack } from "nsh/base";
import "nsh-webgpu";
import { byScreen, type Entry, examples } from "./src/examples";
import { Probe, ProbeContext, snapshotMode } from "./src/harness/probe";
import { ListRow, Notice, PageHeader, Paragraph, SettingRow, StatusLine } from "./src/ui";

type Route = { name: "home" } | { name: "screen"; screen: string };

const nav = createNavStack<Route>([{ name: "home" }]);

function EntryList(props: { entries: Entry[]; header?: JSX.Element }) {
  return (
    <Scroll class="flex-1 bg-background" testID="list">
      {props.header}
      <Each of={props.entries} key={(entry) => entry.screen}>
        {(entry) => (
          <ListRow
            testID={`open-${entry().screen}`}
            title={entry().title}
            detail={entry().kind === "skip" ? "Not ported: see why" : undefined}
            onPress={() => nav.push({ name: "screen", screen: entry().screen })}
          />
        )}
      </Each>
      <View class="h-8" />
    </Scroll>
  );
}

function Home() {
  return (
    <NavPage title="WebGPU Gallery" header={<PageHeader title="WebGPU Gallery" />}>
      <EntryList
        entries={examples}
        header={
          <SettingRow
            testID="snapshot-mode"
            label="Snapshot mode"
            detail="Demos with a reference snapshot render it (1024², the test's time) and report the pixels that differ"
            checked={snapshotMode()}
            onChange={(checked) => snapshotMode.set(checked)}
          />
        }
      />
    </NavPage>
  );
}

function Demo(props: { entry: Extract<Entry, { kind: "demo" }> }) {
  const entry = props.entry;
  const probe = new Probe(entry.screen, entry.probe);
  const Component = entry.component;
  return (
    <View class="flex-1 bg-background">
      <ProbeContext.Provider value={probe}>
        <View class="flex-1">
          <Component />
        </View>
      </ProbeContext.Provider>
      <If when={!entry.diagnostic}>
        <StatusLine testID={`probe-${entry.screen}`} text={probe.status()} />
      </If>
    </View>
  );
}

function Skipped(props: { entry: Extract<Entry, { kind: "skip" }> }) {
  const entry = props.entry;
  return (
    <Scroll class="flex-1 bg-background">
      <Notice title="Not ported">
        <Paragraph testID={`skip-${entry.screen}`}>{entry.reason}</Paragraph>
        <Paragraph>
          What nsh-webgpu does not do, and why, is listed in docs/parity.md; every demo's verdict is in
          docs/inventory.md, table 4.
        </Paragraph>
      </Notice>
    </Scroll>
  );
}

function Body(props: { entry: Entry }) {
  const entry = props.entry;
  if (entry.kind === "demo") return <Demo entry={entry} />;
  if (entry.kind === "list") return <EntryList entries={entry.entries} />;
  return <Skipped entry={entry} />;
}

// The route's component is the NavPage, not one that returns a component
// that is: NSH 0.7.1 never lays such a page out on the web with `nsh run`
// (longbridge/nsh#3270, merged after it).
function Screen(props: { screen: string }) {
  const entry = byScreen.get(props.screen);
  return (
    <NavPage title={entry?.title ?? props.screen} header={<PageHeader title={entry?.title ?? props.screen} onBack={() => nav.pop()} />}>
      {entry ? <Body entry={entry} /> : <Text>Unknown screen {props.screen}</Text>}
    </NavPage>
  );
}

function App() {
  return (
    <NavStack state={nav} class="flex-1 bg-background">
      {(route) => (route.name === "home" ? <Home /> : <Screen screen={route.screen} />)}
    </NavStack>
  );
}

mount(App);
