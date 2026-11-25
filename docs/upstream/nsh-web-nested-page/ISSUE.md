> Filed 2026-11-25 as longbridge/nsh#3268, fixed by the draft PR longbridge/nsh#3270.

## Description

On the web, a pushed `NavStack` page stays at 0x0 when its route renders a component that returns another component,
whose root is the `NavPage`. The page's elements are in the DOM; no error is logged.

## Environment

- NSH: `nsh 0.7.0 (3f8ecd8)`, and the nightly `2964346` (2026-11-24)
- Platform: web, headless Chromium on macOS 27.2
- How it runs: `nsh run --target web`

## Steps to Reproduce

`App.tsx`:

```tsx
import { mount, Text, View } from "nsh";
import { Button, createNavStack, NavPage, NavStack } from "nsh/base";

type Route = { name: "home" } | { name: "page" };
const nav = createNavStack<Route>([{ name: "home" }]);

function Home() {
  return (
    <NavPage title="Home">
      <Button testID="open" onPress={() => nav.push({ name: "page" })}>
        <Text>Open</Text>
      </Button>
    </NavPage>
  );
}

function Page() {
  return (
    <NavPage title="Page">
      <View class="flex-1">
        <Text>The page, laid out</Text>
      </View>
    </NavPage>
  );
}

// <Page /> in place of <Screen /> below is laid out.
function Screen() {
  return <Page />;
}

mount(() => (
  <NavStack state={nav} class="flex-1">
    {(route) => (route.name === "home" ? <Home /> : <Screen />)}
  </NavStack>
));
```

1. Press "Open"

Each time (3 of 3 on each of the two versions). With `<Page />` in place of `<Screen />` the page is laid out
(3 of 3 on 0.7.0).

## Expected

The page fills the window.

## Actual

The page's views are 0x0 and its text is one letter a line.

`nsh test --target web` laid out a page built this way in a larger app, so only `nsh run` was seen to fail.

_Drafted with Claude Code; the reproduction was run on both versions._
