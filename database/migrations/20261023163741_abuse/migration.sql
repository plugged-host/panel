CREATE TABLE "abuse_findings" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"fingerprint" varchar(160) NOT NULL,
	"server_uuid" uuid,
	"node_uuid" uuid,
	"rule" varchar(64) NOT NULL,
	"category" varchar(32) NOT NULL,
	"severity" varchar(16) NOT NULL,
	"source" varchar(16) NOT NULL,
	"status" varchar(16) DEFAULT 'open' NOT NULL,
	"summary" varchar(2000) NOT NULL,
	"evidence" jsonb DEFAULT '{}' NOT NULL,
	"hits" integer DEFAULT 0 NOT NULL,
	"first_seen" timestamp DEFAULT now() NOT NULL,
	"last_seen" timestamp DEFAULT now() NOT NULL,
	"action" varchar(32),
	"actioned" timestamp,
	"resolved_by" uuid,
	"resolved" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "abuse_findings_fingerprint_idx" ON "abuse_findings" ("fingerprint");
CREATE INDEX "abuse_findings_status_last_seen_idx" ON "abuse_findings" ("status","last_seen");
CREATE INDEX "abuse_findings_server_uuid_idx" ON "abuse_findings" ("server_uuid");
ALTER TABLE "abuse_findings" ADD CONSTRAINT "abuse_findings_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "abuse_findings" ADD CONSTRAINT "abuse_findings_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;
ALTER TABLE "abuse_findings" ADD CONSTRAINT "abuse_findings_resolved_by_users_uuid_fkey" FOREIGN KEY ("resolved_by") REFERENCES "users"("uuid") ON DELETE SET NULL;