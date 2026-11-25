# NSH (web): a pushed `NavStack` page from a component within the route's component is never laid out

Draft, not filed. Found with the nightly `2964346` (`nsh 0.6.0`, 2026-11-24), `nsh run --target web`, Chromium.

[`App.tsx`](App.tsx): a `NavStack` whose second route renders `<Screen />`, a component that returns `<Page />`, whose
root is the `NavPage`. Press Open: the page's elements are in the DOM, at 0x0 (its text one letter a line), and stay so;
no error in the console. With `<Page />` in place of `<Screen />` the page is laid out. Each is so every time.

`nsh test --target web` lays the same page out (the gallery's tests found its size), so it seems a pass that only the
test agent's steps bring on. macOS, iOS, Android, Windows and Linux lay it out.
