// Prelude entry: exposes chai and lodash as globals for scripts run in QuickJS.
import chai from "chai";
import _ from "lodash";

globalThis.chai = chai;
globalThis._ = _;
