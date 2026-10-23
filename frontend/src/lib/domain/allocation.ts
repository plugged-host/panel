// plugged.host: new allocations listen on every address and carry the node's name as their alias, so
// players are shown a name that keeps working when the node's address changes
// (docs/features/node-telemetry.md › Allocations).
export const DEFAULT_ALLOCATION_IP = '0.0.0.0';

export function defaultAllocationAlias(node: {
  srvTarget: string | null;
  publicUrl: string | null;
  url: string;
}): string | null {
  try {
    return node.srvTarget ?? new URL(node.publicUrl ?? node.url).hostname;
  } catch {
    return null;
  }
}
