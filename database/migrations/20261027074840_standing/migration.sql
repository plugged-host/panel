CREATE TABLE "user_strikes" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"reference" varchar(64) NOT NULL,
	"reason" text,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "user_strikes_user_uuid_reference_idx" ON "user_strikes" ("user_uuid","reference");
ALTER TABLE "user_strikes" ADD CONSTRAINT "user_strikes_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;