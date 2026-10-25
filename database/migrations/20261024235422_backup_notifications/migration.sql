CREATE TABLE "server_notification_settings" (
	"server_uuid" uuid,
	"user_uuid" uuid,
	"manual_backups" boolean DEFAULT false NOT NULL,
	"scheduled_backups" boolean DEFAULT false NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL,
	CONSTRAINT "server_notification_settings_pk" PRIMARY KEY("server_uuid","user_uuid")
);

CREATE INDEX "server_notification_settings_user_uuid_idx" ON "server_notification_settings" ("user_uuid");
ALTER TABLE "server_notification_settings" ADD CONSTRAINT "server_notification_settings_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_notification_settings" ADD CONSTRAINT "server_notification_settings_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;