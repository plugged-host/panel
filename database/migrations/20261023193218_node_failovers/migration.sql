CREATE TABLE "node_failovers" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"node_uuid" uuid NOT NULL,
	"mode" varchar(16) NOT NULL,
	"status" varchar(16) NOT NULL,
	"started_by" uuid,
	"created" timestamp DEFAULT now() NOT NULL,
	"finished" timestamp
);

CREATE TABLE "server_failovers" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"node_failover_uuid" uuid NOT NULL,
	"server_uuid" uuid NOT NULL,
	"server_name" varchar(1020) NOT NULL,
	"source_node_uuid" uuid NOT NULL,
	"destination_node_uuid" uuid,
	"backup_uuid" uuid,
	"status" varchar(16) NOT NULL,
	"error" text,
	"created" timestamp DEFAULT now() NOT NULL,
	"updated" timestamp DEFAULT now() NOT NULL,
	"finished" timestamp,
	"source_cleaned" timestamp
);

CREATE INDEX "node_failovers_node_uuid_created_idx" ON "node_failovers" ("node_uuid","created");
CREATE INDEX "node_failovers_status_idx" ON "node_failovers" ("status");
CREATE INDEX "server_failovers_node_failover_uuid_idx" ON "server_failovers" ("node_failover_uuid");
CREATE INDEX "server_failovers_status_idx" ON "server_failovers" ("status");
CREATE INDEX "server_failovers_server_uuid_idx" ON "server_failovers" ("server_uuid");
ALTER TABLE "node_failovers" ADD CONSTRAINT "node_failovers_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;
ALTER TABLE "node_failovers" ADD CONSTRAINT "node_failovers_started_by_users_uuid_fkey" FOREIGN KEY ("started_by") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_failovers" ADD CONSTRAINT "server_failovers_node_failover_uuid_node_failovers_uuid_fkey" FOREIGN KEY ("node_failover_uuid") REFERENCES "node_failovers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_failovers" ADD CONSTRAINT "server_failovers_source_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("source_node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_failovers" ADD CONSTRAINT "server_failovers_destination_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("destination_node_uuid") REFERENCES "nodes"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_failovers" ADD CONSTRAINT "server_failovers_backup_uuid_server_backups_uuid_fkey" FOREIGN KEY ("backup_uuid") REFERENCES "server_backups"("uuid") ON DELETE SET NULL;