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

function DemoPage(props: { entry: Extract<Entry, { kind: "demo" }> }) {
  const entry = props.entry;
  const probe = new Probe(entry.screen, entry.probe);
  const Demo = entry.component;
  return (
    <NavPage title={entry.title} header={<PageHeader title={entry.title} onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
        <ProbeContext.Provider value={probe}>
          <View class="flex-1">
            <Demo />
          </View>
        </ProbeContext.Provider>
        <If when={!entry.diagnostic}>
          <StatusLine testID={`probe-${entry.screen}`} text={probe.status()} />
        </If>
      </View>
    </NavPage>
  );
}

function SkipPage(props: { entry: Extract<Entry, { kind: "skip" }> }) {
  const entry = props.entry;
  return (
    <NavPage title={entry.title} header={<PageHeader title={entry.title} onBack={() => nav.pop()} />}>
      <Scroll class="flex-1 bg-background">
        <Notice title="Not ported">
          <Paragraph testID={`skip-${entry.screen}`}>{entry.reason}</Paragraph>
          <Paragraph>
            What nsh-webgpu does not do, and why, is listed in docs/parity.md; every demo's verdict is in
            docs/inventory.md, table 4.
          </Paragraph>
        </Notice>
      </Scroll>
    </NavPage>
  );
}

function ListPage(props: { entry: Extract<Entry, { kind: "list" }> }) {
  const entry = props.entry;
  return (
    <NavPage title={entry.title} header={<PageHeader title={entry.title} onBack={() => nav.pop()} />}>
      <EntryList entries={entry.entries} />
    </NavPage>
  );
}

function Screen(props: { screen: string }) {
  const entry = byScreen.get(props.screen);
  if (!entry) return <Text>Unknown screen {props.screen}</Text>;
  if (entry.kind === "demo") return <DemoPage entry={entry} />;
  if (entry.kind === "list") return <ListPage entry={entry} />;
  return <SkipPage entry={entry} />;
}

function App() {
  return (
    <NavStack state={nav} class="flex-1 bg-background">
      {(route) => (route.name === "home" ? <Home /> : <Screen screen={route.screen} />)}
    </NavStack>
  );
}

mount(App);
