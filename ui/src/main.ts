import { mount } from "svelte";
import App from "./App.svelte";
import { tauriApi } from "./lib/api";
import { session } from "./lib/session.svelte";
import "./styles/tokens.css";
import "./styles/base.css";

// What was remembered is read before the first draw, so the window does not open in another layout.
await session.start(tauriApi);

// Components take the commands from the context, so tests can pass an in-memory `Api` instead.
mount(App, { target: document.getElementById("app")!, context: new Map([["api", tauriApi]]) });
