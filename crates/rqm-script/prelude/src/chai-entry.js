// Entry of the chai bundle (`built/chai.js`): chai 4.4.1 with the assertions of a response. Loaded into the
// context of a script only when the script first reads `pm.expect` or `pm.response.to` — the primitive
// `load_bundle("chai")` runs this file in the middle of the script.
//
// No primitive of the host ever comes here: the bundle hands the prelude one function through a global name,
// and the prelude takes the name away at once (`loadChai` in `index.js`).
import chai from "chai";
import { createAssertions } from "./assertions.js";

const apply = Reflect.apply;

/**
 * `isResponse(value)` tells an object made by the response factory of the prelude. `quietly(run)` runs
 * `run()` with the stubs of the names for later silent (`withoutStubs` of the prelude).
 */
globalThis.__rqm_chai = function (isResponse, quietly) {
  // chai builds the text of a failed assertion by printing its subject, and the printer reads every own
  // property, the ones that are not enumerable too. A stub read there would throw its own text in place
  // of the `AssertionError`, so the stubs keep silent while an assertion is checked.
  const assert = chai.Assertion.prototype.assert;
  chai.Assertion.prototype.assert = function () {
    const self = this;
    const given = arguments;
    return quietly(function () {
      return apply(assert, self, given);
    });
  };
  chai.use(createAssertions(isResponse));
  return chai;
};
