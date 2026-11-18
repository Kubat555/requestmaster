// Entry of the chai bundle (`built/chai.js`): chai 4.4.1 with the assertions of a response. Loaded into the
// context of a script only when the script first reads `pm.expect` or `pm.response.to` — the primitive
// `load_bundle("chai")` runs this file in the middle of the script.
//
// No primitive of the host ever comes here: the bundle hands the prelude one function through a global name,
// and the prelude takes the name away at once (`loadChai` in `index.js`).
import chai from "chai";
import { createAssertions } from "./assertions.js";

/** `isResponse(value)` tells an object made by the response factory of the prelude. */
globalThis.__rqm_chai = function (isResponse) {
  chai.use(createAssertions(isResponse));
  return chai;
};
