import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const extensionRoot = new URL("../", import.meta.url);

const grammar = JSON.parse(
  await readFile(new URL("syntaxes/synapse.tmLanguage.json", extensionRoot))
);
const languageConfiguration = JSON.parse(
  await readFile(new URL("language-configuration.json", extensionRoot))
);

function rule(name) {
  const match = grammar.repository.declarations.patterns.find(
    (candidate) => candidate.name === name
  );

  assert.ok(match, `missing grammar rule ${name}`);
  return new RegExp(match.match);
}

test("uses the current Synapse line-comment syntax", () => {
  assert.equal(languageConfiguration.comments.lineComment, "//");

  const [documentation, ordinary] = grammar.repository.comments.patterns;
  assert.match("/// Documented declaration.", new RegExp(documentation.match));
  assert.match("// Ordinary comment.", new RegExp(ordinary.match));
  assert.doesNotMatch("# Legacy comment.", new RegExp(ordinary.match));
});

test("recognizes command-group declarations", () => {
  const match = rule("meta.declaration.command-group.synapse").exec(
    "commands CameraCommands {"
  );

  assert.ok(match);
  assert.equal(match[1], "commands");
  assert.equal(match[2], "CameraCommands");

  const keyword = new RegExp(
    grammar.repository.keywords.patterns[0].match
  );
  assert.match("commands", keyword);
});

test("gives declaration rules precedence over generic keywords", () => {
  const includes = grammar.patterns.map((pattern) => pattern.include);

  assert.ok(
    includes.indexOf("#declarations") < includes.indexOf("#keywords"),
    "declarations must run before keywords to capture declaration names"
  );
});

test("recognizes represented and unrepresented enum names", () => {
  const enumRule = rule("meta.declaration.enum.synapse");
  const represented = enumRule.exec("enum u8 CameraMode {");
  const unrepresented = enumRule.exec("enum CameraMode {");

  assert.ok(represented);
  assert.equal(represented[1], "enum");
  assert.equal(represented[2], "u8");
  assert.equal(represented[3], "CameraMode");

  assert.ok(unrepresented);
  assert.equal(unrepresented[1], "enum");
  assert.equal(unrepresented[2], undefined);
  assert.equal(unrepresented[3], "CameraMode");
});

test("keeps generic attribute highlighting for @cc", () => {
  const attribute = grammar.repository.attributes.patterns[0];
  const match = new RegExp(attribute.begin).exec("@cc(4)");

  assert.ok(match);
  assert.equal(match[1], "@");
  assert.equal(match[2], "cc");
  assert.equal(match[3], "(");
});
