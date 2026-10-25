CREATE TABLE "server_chat_messages" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"server_uuid" uuid NOT NULL,
	"user_uuid" uuid,
	"message" text NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "users" ADD COLUMN "panel_access_until" timestamp;
CREATE INDEX "server_chat_messages_server_uuid_created_idx" ON "server_chat_messages" ("server_uuid","created");
CREATE INDEX "server_chat_messages_user_uuid_idx" ON "server_chat_messages" ("user_uuid");
ALTER TABLE "server_chat_messages" ADD CONSTRAINT "server_chat_messages_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_chat_messages" ADD CONSTRAINT "server_chat_messages_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;