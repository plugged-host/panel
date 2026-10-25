CREATE TABLE "partner_applications" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"discord_id" varchar(32) NOT NULL,
	"guild_id" varchar(32) NOT NULL,
	"guild_name" varchar(400) NOT NULL,
	"invite" varchar(64),
	"game" varchar(16) NOT NULL,
	"description" text NOT NULL,
	"server_uuids" uuid[] DEFAULT '{}'::uuid[] NOT NULL,
	"facts" jsonb NOT NULL,
	"problems" text[] DEFAULT '{}'::text[] NOT NULL,
	"checked" timestamp NOT NULL,
	"assessment" jsonb,
	"assessed" timestamp,
	"status" varchar(16) DEFAULT 'pending' NOT NULL,
	"monthly_credits" bigint DEFAULT 0 NOT NULL,
	"reason" varchar(2000),
	"decided_by" uuid,
	"decided" timestamp,
	"approved" timestamp,
	"next_payment" timestamp,
	"failing_since" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "partner_payments" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"application_uuid" uuid NOT NULL,
	"period" varchar(7) NOT NULL,
	"credits" bigint NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "partner_applications_user_uuid_pending_idx" ON "partner_applications" ("user_uuid") WHERE status = 'pending';
CREATE UNIQUE INDEX "partner_applications_guild_id_open_idx" ON "partner_applications" ("guild_id") WHERE status IN ('pending', 'approved', 'paused', 'suspended');
CREATE INDEX "partner_applications_status_created_idx" ON "partner_applications" ("status","created");
CREATE INDEX "partner_applications_status_next_payment_idx" ON "partner_applications" ("status","next_payment");
CREATE INDEX "partner_applications_user_uuid_created_idx" ON "partner_applications" ("user_uuid","created");
CREATE UNIQUE INDEX "partner_payments_application_uuid_period_idx" ON "partner_payments" ("application_uuid","period");
ALTER TABLE "partner_applications" ADD CONSTRAINT "partner_applications_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "partner_applications" ADD CONSTRAINT "partner_applications_decided_by_users_uuid_fkey" FOREIGN KEY ("decided_by") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "partner_payments" ADD CONSTRAINT "partner_payments_OPM7f4F1uSue_fkey" FOREIGN KEY ("application_uuid") REFERENCES "partner_applications"("uuid") ON DELETE CASCADE;