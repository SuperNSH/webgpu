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
