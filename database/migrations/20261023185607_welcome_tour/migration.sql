CREATE TABLE "user_welcome_tours" (
	"user_uuid" uuid PRIMARY KEY,
	"version" integer NOT NULL,
	"step" integer DEFAULT 1 NOT NULL,
	"server_step" integer DEFAULT 1 NOT NULL,
	"completed" timestamp,
	"skipped" timestamp,
	"server_done" timestamp,
	"updated" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "user_welcome_tours" ADD CONSTRAINT "user_welcome_tours_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;