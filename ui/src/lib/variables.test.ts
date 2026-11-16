import { describe, expect, it } from "vitest";
import { strings } from "./strings";
import { normalizeContent, type RequestContent, type ScopeVariable } from "./types";
import { applyCompletion, completionAt, describeVariable, findTokens, requestTokenNames, suggest } from "./variables";

function variable(name: string, more: Partial<ScopeVariable> = {}): ScopeVariable {
  return { name, scope: "environment", value: "v", secret: false, ...more };
}

function draft(content: Partial<RequestContent>) {
  return normalizeContent("request", { method: "GET", url: "", ...content });
}

describe("findTokens", () => {
  it("finds the tokens left to right with UTF-16 offsets", () => {
    expect(findTokens("{{a}}/{{b}}")).toEqual([
      { from: 0, to: 5, name: "a" },
      { from: 6, to: 11, name: "b" },
    ]);
  });

  it("takes the innermost token of nested ones", () => {
    expect(findTokens("{{a{{b}}}}")).toEqual([{ from: 3, to: 8, name: "b" }]);
  });

  it("takes an empty name as a token", () => {
    expect(findTokens("{{}}")).toEqual([{ from: 0, to: 4, name: "" }]);
  });

  it("does not take a token that is not closed", () => {
    expect(findTokens("{{a}")).toEqual([]);
    expect(findTokens("{{a")).toEqual([]);
    expect(findTokens("")).toEqual([]);
    expect(findTokens("{")).toEqual([]);
  });

  it("counts Cyrillic text in UTF-16 units, not in bytes", () => {
    expect(findTokens("я{{имя}}")).toEqual([{ from: 1, to: 8, name: "имя" }]);
  });

  it("skips one brace of three", () => {
    expect(findTokens("{{{a}}}")).toEqual([{ from: 1, to: 6, name: "a" }]);
  });

  it("keeps the spaces of a name", () => {
    expect(findTokens("{{ a b }}").map((token) => token.name)).toEqual([" a b "]);
  });
});

describe("completionAt", () => {
  it("finds the beginning of a name typed after {{", () => {
    expect(completionAt("a{{to", 5)).toEqual({ from: 3, prefix: "to" });
  });

  it("finds nothing after a closed token", () => {
    expect(completionAt("{{a}} b", 7)).toBeNull();
  });

  it("takes the part of the name before the caret", () => {
    expect(completionAt("{{token}}", 4)).toEqual({ from: 2, prefix: "to" });
    expect(completionAt("{{", 2)).toEqual({ from: 2, prefix: "" });
  });

  it("finds nothing without {{ before the caret", () => {
    expect(completionAt("{to", 3)).toBeNull();
    expect(completionAt("to", 2)).toBeNull();
    expect(completionAt("", 0)).toBeNull();
    expect(completionAt("{{a", 1)).toBeNull();
  });
});

describe("applyCompletion", () => {
  it("puts the name and the closing braces", () => {
    expect(applyCompletion("x={{to", 6, 4, "token")).toEqual({ text: "x={{token}}", caret: 11 });
  });

  it("does not double the closing braces that are already there", () => {
    expect(applyCompletion("{{to}}", 4, 2, "token")).toEqual({ text: "{{token}}", caret: 9 });
  });

  it("replaces the whole name of a closed token and leaves the text after it", () => {
    expect(applyCompletion("{{toxx}}/y", 4, 2, "token")).toEqual({ text: "{{token}}/y", caret: 9 });
  });

  it("leaves the text after the caret of a token that is not closed", () => {
    expect(applyCompletion("{{h/users", 3, 2, "host")).toEqual({ text: "{{host}}/users", caret: 8 });
  });
});

describe("suggest", () => {
  const variables = [variable("xhost"), variable("Host"), variable("token"), variable("hostname")];

  it("puts the names that start with the prefix before the ones that contain it; the case does not matter", () => {
    expect(suggest(variables, "ho").map((found) => found.name)).toEqual(["Host", "hostname", "xhost"]);
    expect(suggest(variables, "HOST").map((found) => found.name)).toEqual(["Host", "hostname", "xhost"]);
  });

  it("gives every variable for an empty prefix and none for a prefix nothing has", () => {
    expect(suggest(variables, "")).toEqual(variables);
    expect(suggest(variables, "zzz")).toEqual([]);
  });

  it("gives at most 50", () => {
    const many = Array.from({ length: 80 }, (_, index) => variable(`v${index}`));
    expect(suggest(many, "v")).toHaveLength(50);
    expect(suggest(many, "")).toHaveLength(50);
  });
});

describe("requestTokenNames", () => {
  it("takes the names in the order of appearance and without repeats", () => {
    const names = requestTokenNames(
      draft({
        url: "{{host}}/{{path}}",
        query: [{ key: "{{qk}}", value: "{{host}}" }],
        headers: [{ key: "{{hk}}", value: "{{hv}}" }],
      }),
    );
    expect(names).toEqual(["host", "path", "qk", "hk", "hv"]);
  });

  it("does not take a header that is switched off", () => {
    const names = requestTokenNames(
      draft({
        headers: [
          { key: "A", value: "{{off}}", enabled: false },
          { key: "B", value: "{{on}}" },
        ],
        query: [{ key: "{{qoff}}", value: "1", enabled: false }],
      }),
    );
    expect(names).toEqual(["on"]);
  });

  it("takes the value of a path variable but not its key", () => {
    const names = requestTokenNames(
      draft({
        path_variables: [
          { key: "{{key}}", value: "{{value}}" },
          { key: "x", value: "{{off}}", enabled: false },
        ],
      }),
    );
    expect(names).toEqual(["value"]);
  });

  it("takes the text of a raw body and the enabled fields of an url-encoded one", () => {
    expect(requestTokenNames(draft({ body: { mode: "raw", data: { text: '{"a": {{id}}}', language: "json" } } }))).toEqual(["id"]);
    expect(
      requestTokenNames(
        draft({
          body: {
            mode: "url_encoded",
            data: [
              { key: "{{k}}", value: "{{v}}" },
              { key: "{{koff}}", value: "{{voff}}", enabled: false },
            ],
          },
        }),
      ),
    ).toEqual(["k", "v"]);
  });

  it("takes the key and the text of a form-data part but not the path of a file", () => {
    const names = requestTokenNames(
      draft({
        body: {
          mode: "form_data",
          data: [
            { key: "{{name}}", value: { text: "{{text}}" } },
            { key: "file", value: { files: [{ path: "C:/{{path}}.png" }] }, content_type: "{{type}}" },
            { key: "{{off}}", value: { text: "{{offtext}}" }, enabled: false },
          ],
        },
      }),
    );
    expect(names).toEqual(["name", "text"]);
  });

  it("takes the query and the variables of a GraphQL body and nothing of a binary one", () => {
    expect(requestTokenNames(draft({ body: { mode: "graphql", data: { query: "{{q}}", variables: "{{v}}" } } }))).toEqual(["q", "v"]);
    expect(requestTokenNames(draft({ body: { mode: "binary", data: { file: { path: "{{p}}" } } } }))).toEqual([]);
  });

  it("takes the fields of the request's own auth", () => {
    expect(requestTokenNames(draft({ auth: { type: "bearer", token: "{{token}}" } }))).toEqual(["token"]);
    expect(requestTokenNames(draft({ auth: { type: "basic", username: "{{u}}", password: "{{p}}" } }))).toEqual(["u", "p"]);
    expect(
      requestTokenNames(draft({ auth: { type: "oauth2_token", token: "{{t}}", header_prefix: "{{pre}}", placement: "header" } })),
    ).toEqual(["t", "pre"]);
    expect(requestTokenNames(draft({ auth: { type: "api_key", key: "{{k}}", value: "{{v}}", placement: "query" } }))).toEqual(["k", "v"]);
    expect(requestTokenNames(draft({ auth: { type: "no_auth" } }))).toEqual([]);
    expect(requestTokenNames(draft({ auth: null }))).toEqual([]);
  });

  it("does not take the method", () => {
    expect(requestTokenNames(draft({ method: "{{m}}", url: "u" }))).toEqual([]);
  });
});

describe("describeVariable", () => {
  it("says the scope in words and the value", () => {
    expect(describeVariable("a", variable("a", { scope: "global", value: "1" }))).toEqual({ scope: "Глобальная", value: "1" });
    expect(describeVariable("a", variable("a", { scope: "collection", value: "2" }))).toEqual({ scope: "Коллекция", value: "2" });
    expect(describeVariable("a", variable("a", { scope: "environment", value: "3" }))).toEqual({ scope: "Окружение", value: "3" });
  });

  it("masks a secret value", () => {
    expect(describeVariable("a", variable("a", { value: null, secret: true }))).toEqual({ scope: "Окружение", value: "••••••" });
    // Whatever came with a secret variable is not shown.
    expect(describeVariable("a", variable("a", { value: "leak", secret: true })).value).toBe("••••••");
  });

  it("explains a dynamic value, an empty one and a variable that is not found", () => {
    expect(describeVariable("$guid", variable("$guid", { scope: "dynamic", value: null }))).toEqual({
      scope: "Динамическая",
      value: "значение создаётся при отправке",
    });
    expect(describeVariable("a", variable("a", { value: "" })).value).toBe("пустое значение");
    expect(describeVariable("nope", null)).toEqual({ scope: "Переменная не найдена", value: "" });
    expect(strings.variables.unresolved(2)).toBe("Не найдено переменных: 2");
  });
});
