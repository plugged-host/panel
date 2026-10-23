CREATE TABLE "deleted_servers" (
	"uuid" uuid PRIMARY KEY,
	"owner_uuid" uuid NOT NULL,
	"name" varchar(1020) NOT NULL,
	"suspended" boolean DEFAULT false NOT NULL,
	"backups" uuid[] NOT NULL,
	"deleted" timestamp DEFAULT now() NOT NULL,
	"hold_until" timestamp NOT NULL,
	"trimmed" timestamp,
	"purged" timestamp
);

CREATE INDEX "deleted_servers_owner_uuid_idx" ON "deleted_servers" ("owner_uuid");
CREATE INDEX "deleted_servers_purged_hold_until_idx" ON "deleted_servers" ("purged","hold_until");
ALTER TABLE "deleted_servers" ADD CONSTRAINT "deleted_servers_owner_uuid_users_uuid_fkey" FOREIGN KEY ("owner_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;