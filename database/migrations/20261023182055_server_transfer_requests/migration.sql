CREATE TABLE "server_transfer_requests" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"server_uuid" uuid NOT NULL,
	"user_uuid" uuid,
	"origin" varchar(16) NOT NULL,
	"location_uuid" uuid,
	"source_node_uuid" uuid,
	"destination_node_uuid" uuid,
	"tried_node_uuids" uuid[] DEFAULT '{}'::uuid[] NOT NULL,
	"attempts" integer DEFAULT 0 NOT NULL,
	"status" varchar(16) NOT NULL,
	"error" text,
	"restart" boolean DEFAULT false NOT NULL,
	"variables" jsonb DEFAULT '{}' NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL,
	"started" timestamp,
	"finished" timestamp
);

CREATE INDEX "server_transfer_requests_server_uuid_created_idx" ON "server_transfer_requests" ("server_uuid","created");
CREATE INDEX "server_transfer_requests_status_idx" ON "server_transfer_requests" ("status");
ALTER TABLE "server_transfer_requests" ADD CONSTRAINT "server_transfer_requests_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_transfer_requests" ADD CONSTRAINT "server_transfer_requests_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_transfer_requests" ADD CONSTRAINT "server_transfer_requests_location_uuid_locations_uuid_fkey" FOREIGN KEY ("location_uuid") REFERENCES "locations"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_transfer_requests" ADD CONSTRAINT "server_transfer_requests_source_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("source_node_uuid") REFERENCES "nodes"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_transfer_requests" ADD CONSTRAINT "server_transfer_requests_destination_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("destination_node_uuid") REFERENCES "nodes"("uuid") ON DELETE SET NULL;