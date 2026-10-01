/**
 * Stable Node/TypeScript core boundary for Agent harness and shared contracts.
 */
export const WORKSPACE_NODE_CORE = "workspace_node_core";

export interface WorkspaceManifest {
  name: string;
  version: string;
  runtime: "node" | "rust" | "python";
}
