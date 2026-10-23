CREATE TABLE "server_start_queue" (
	"server_uuid" uuid PRIMARY KEY,
	"user_uuid" uuid,
	"node_uuid" uuid NOT NULL,
	"action" varchar(16) NOT NULL,
	"status" varchar(16) NOT NULL,
	"priority" smallint DEFAULT 0 NOT NULL,
	"queued" timestamp DEFAULT now() NOT NULL,
	"last_seen" timestamp,
	"released" timestamp,
	"reason" varchar(16)
);

CREATE INDEX "server_start_queue_node_uuid_status_idx" ON "server_start_queue" ("node_uuid","status");
ALTER TABLE "server_start_queue" ADD CONSTRAINT "server_start_queue_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_start_queue" ADD CONSTRAINT "server_start_queue_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_start_queue" ADD CONSTRAINT "server_start_queue_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;