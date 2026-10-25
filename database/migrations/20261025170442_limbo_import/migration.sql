ALTER TABLE "server_limbo" ADD COLUMN "home_node_uuid" uuid;
ALTER TABLE "server_limbo" ADD COLUMN "home_ports" integer[] DEFAULT '{}'::integer[] NOT NULL;
ALTER TABLE "server_limbo" ADD CONSTRAINT "server_limbo_home_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("home_node_uuid") REFERENCES "nodes"("uuid") ON DELETE SET NULL;