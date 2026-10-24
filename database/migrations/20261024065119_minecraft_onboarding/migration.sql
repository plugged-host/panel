CREATE TABLE "server_onboardings" (
	"server_uuid" uuid PRIMARY KEY,
	"answers" jsonb,
	"completed" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "server_onboardings" ADD CONSTRAINT "server_onboardings_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;