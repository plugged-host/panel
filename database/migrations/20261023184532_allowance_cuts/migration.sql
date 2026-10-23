CREATE TABLE "user_allowance_cuts" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"reason" varchar(64) NOT NULL,
	"cpu" bigint DEFAULT 0 NOT NULL,
	"memory" bigint DEFAULT 0 NOT NULL,
	"disk" bigint DEFAULT 0 NOT NULL,
	"result" jsonb,
	"created" timestamp DEFAULT now() NOT NULL,
	"settled" timestamp
);

CREATE INDEX "user_allowance_cuts_settled_user_uuid_idx" ON "user_allowance_cuts" ("settled","user_uuid");
ALTER TABLE "user_allowance_cuts" ADD CONSTRAINT "user_allowance_cuts_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;