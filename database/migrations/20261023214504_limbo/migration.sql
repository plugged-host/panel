CREATE TABLE "server_limbo" (
	"server_uuid" uuid PRIMARY KEY,
	"since" timestamp DEFAULT now() NOT NULL,
	"backup_uuid" uuid,
	"parked" timestamp,
	"paid" integer DEFAULT 0 NOT NULL,
	"payer_uuid" uuid,
	"error" text,
	"failed" timestamp,
	"updated" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "servers" ADD COLUMN "last_activity" timestamp DEFAULT now() NOT NULL;
ALTER TABLE "servers" ADD COLUMN "limbo_state" varchar(16);
CREATE INDEX "servers_limbo_state_idx" ON "servers" ("limbo_state");
ALTER TABLE "server_limbo" ADD CONSTRAINT "server_limbo_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_limbo" ADD CONSTRAINT "server_limbo_backup_uuid_server_backups_uuid_fkey" FOREIGN KEY ("backup_uuid") REFERENCES "server_backups"("uuid") ON DELETE SET NULL;
ALTER TABLE "server_limbo" ADD CONSTRAINT "server_limbo_payer_uuid_users_uuid_fkey" FOREIGN KEY ("payer_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;