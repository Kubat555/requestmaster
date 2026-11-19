// Entry of the lodash bundle (`built/lodash.js`): lodash 4.18.1 (Postman has lodash 3 under the global `_`).
// Loaded into the context of a script only when the script first reads `_` or calls `require('lodash')` —
// the primitive `load_bundle("lodash")` runs this file in the middle of the script.
//
// No primitive of the host ever comes here: the bundle leaves lodash under a global name, and the prelude
// takes the name away at once (`loadLodash` in `index.js`).
import lodash from "lodash";

globalThis.__rqm_lodash = lodash;
