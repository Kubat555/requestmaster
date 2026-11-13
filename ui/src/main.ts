import { mount } from "svelte";
import App from "./App.svelte";
import { tauriApi } from "./lib/api";
import "./styles/tokens.css";
import "./styles/base.css";

// Components take the commands from the context, so tests can pass an in-memory `Api` instead.
mount(App, { target: document.getElementById("app")!, context: new Map([["api", tauriApi]]) });
