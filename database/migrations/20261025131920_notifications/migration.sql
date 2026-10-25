CREATE TABLE "user_notifications" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"server_uuid" uuid,
	"kind" varchar(64) NOT NULL,
	"data" jsonb NOT NULL,
	"read" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE INDEX "user_notifications_user_uuid_created_idx" ON "user_notifications" ("user_uuid","created");
CREATE INDEX "user_notifications_user_uuid_unread_idx" ON "user_notifications" ("user_uuid") WHERE read IS NULL;
CREATE INDEX "user_notifications_server_uuid_idx" ON "user_notifications" ("server_uuid");
CREATE INDEX "user_notifications_created_idx" ON "user_notifications" ("created");
ALTER TABLE "user_notifications" ADD CONSTRAINT "user_notifications_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "user_notifications" ADD CONSTRAINT "user_notifications_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE SET NULL;