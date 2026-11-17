import { mount } from "svelte";
import App from "./App.svelte";
import { tauriApi } from "./lib/api";
import { motion } from "./lib/motion.svelte";
import { session } from "./lib/session.svelte";
import "./styles/tokens.css";
import "./styles/base.css";

// What was remembered is read before the first draw, so the window does not open in another layout. The motion
// level is read at the same time, not after it: the first draw waits for one round of commands, as before, and
// the window does not start at the normal level only to switch a moment later.
await Promise.all([session.start(tauriApi), motion.start(tauriApi)]);

// Components take the commands from the context, so tests can pass an in-memory `Api` instead.
mount(App, { target: document.getElementById("app")!, context: new Map([["api", tauriApi]]) });
