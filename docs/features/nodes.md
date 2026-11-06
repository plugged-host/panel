# Nodes: force nuke

Admin › Nodes › the node's General tab has **Force Nuke** for a node that is offline or not
responding. It is the last resort for a node that cannot come back: it force-deletes every server
on the node and then deletes the node from the panel.

## The guard

The API probes the node (`GET /api/system/overview`, three-second timeout) before doing anything.
If the node answers, the request is refused with `400` and nothing is deleted. A timeout, a refused
connection or any other API error counts as offline. The button is shown only when the panel has
not seen the node recently, but the probe is the authority.

## What it deletes

1. Every server whose `node_uuid` is the node, with `DeleteServerOptions { force: true }`:
   databases, database instances, subdomains, icons, tunnel membership and the row all go, even
   when Wings cannot be reached to delete its copy.
2. The node row itself.

The action is logged as `node:force-nuke` with the node's uuid, name and the number of servers
deleted. It needs the `nodes.force-nuke` permission; the normal Delete still refuses a node with
servers.

## Route

`POST /api/admin/nodes/{node}/force-nuke` → `{ servers_deleted }`; `400` when the node answers.

## Check

- Stop Wings on a test node. After the offline threshold, its General tab shows Force Nuke.
- Press it: the confirmation says what happens; after confirming, the node and its servers are
  gone from the panel, and the activity log has `node:force-nuke`.
- Start Wings first and press it: the API refuses with "node is responding".
- A node with servers cannot be removed with the normal Delete; Force Nuke can.
