ALTER TABLE "servers" ADD COLUMN "suspension_kind" varchar(16);
ALTER TABLE "servers" ADD COLUMN "suspension_reason" text;
ALTER TABLE "servers" ADD COLUMN "suspended_at" timestamp;
ALTER TABLE "servers" ADD COLUMN "suspended_by" uuid;
ALTER TABLE "servers" ADD COLUMN "suspension_delete_at" timestamp;
CREATE INDEX "servers_suspension_delete_at_idx" ON "servers" ("suspension_delete_at");
ALTER TABLE "servers" ADD CONSTRAINT "servers_suspended_by_users_uuid_fkey" FOREIGN KEY ("suspended_by") REFERENCES "users"("uuid") ON DELETE SET NULL;