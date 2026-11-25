# NSH (macOS): `nsh test --target macos` now and then loses a step: "the host did not take the step in 30 s"

Draft, not filed. NSH nightly `2964346` (2026-11-24), GitHub's `macos-latest` (macOS 26.7.2, arm64), in this repository's
gallery: 4 of 2,464 tests (44 tests, 14 times on 4 runners; run 45535377025 of the branch `ci/macos-stall`, since
deleted), each a different test, plus one in the On screen run 45519539574. Never on an Apple M3 Max (macOS 27.2).

The step is sent about a second after the host starts and gets no answer. The host's threads, sampled 14 s and 24 s into
it ([`stacks.txt`](stacks.txt)): all idle. The main thread waits for an event, `nsh-test-agent` waits in `recv` for a
`TestStep` (it took none, or its answer went), `nsh-engine` waits for a message, `nsh-dev-client` polls its socket. The
process is at 44 to 48 MB with no app frame and no GPU frame on any thread: the app's page with the canvas is not open
yet. So the step, or its `StepDone`, is lost between the dev server and the agent.
