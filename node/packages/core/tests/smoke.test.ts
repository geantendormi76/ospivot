import assert from "node:assert/strict";
import test from "node:test";
import { WORKSPACE_NODE_CORE } from "../src/index.ts";

test("node core package exports marker constant", () => {
  assert.equal(WORKSPACE_NODE_CORE, "workspace_node_core");
});
