CREATE TABLE "pending_membership_bundles" (
	"account_id" bigint PRIMARY KEY,
	"plus_until" timestamp NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "server_upgrades" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"server_uuid" uuid NOT NULL,
	"user_uuid" uuid,
	"kind" varchar(32) NOT NULL,
	"amount" integer DEFAULT 1 NOT NULL,
	"tokens" integer DEFAULT 0 NOT NULL,
	"data" jsonb DEFAULT '{}' NOT NULL,
	"until" timestamp,
	"reminded_until" timestamp,
	"ended" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "neo_usage_windows" ADD COLUMN "bonus_tokens" bigint DEFAULT 0 NOT NULL;
ALTER TABLE "rux_giveaways" ADD COLUMN "prize_kind" varchar(16) DEFAULT 'credits' NOT NULL;
ALTER TABLE "servers" ADD COLUMN "boost_memory" bigint DEFAULT 0 NOT NULL;
ALTER TABLE "servers" ADD COLUMN "boost_cpu" integer DEFAULT 0 NOT NULL;
ALTER TABLE "servers" ADD COLUMN "card_color" varchar(16);
ALTER TABLE "user_memberships" ADD COLUMN "bundle_synced_until" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "bundle_synced" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "bundle_plus_until" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "plus_auto_renew" boolean DEFAULT false NOT NULL;
ALTER TABLE "user_memberships" ADD COLUMN "plusplus_auto_renew" boolean DEFAULT false NOT NULL;
ALTER TABLE "user_memberships" ADD COLUMN "plus_reminded" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "plusplus_reminded" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "plus_renew_failed" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "plusplus_renew_failed" timestamp;
ALTER TABLE "users" ADD COLUMN "panel_access_reminded" timestamp;
ALTER TABLE "users" ADD COLUMN "streak_day" integer DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "streak_claimed" date;
ALTER TABLE "users" ADD COLUMN "referral_code" varchar(16);
ALTER TABLE "users" ADD COLUMN "referred_by" uuid;
ALTER TABLE "users" ADD COLUMN "referral_paid" timestamp;
ALTER TABLE "users" ADD COLUMN "day_pass_until" timestamp;
ALTER TABLE "users" ADD COLUMN "promo_shown" timestamp;
ALTER TABLE "users" ADD COLUMN "badges" varchar(32)[] DEFAULT '{}'::varchar(32)[] NOT NULL;
ALTER TABLE "users" ADD COLUMN "badge" varchar(32);
CREATE INDEX "server_upgrades_server_uuid_kind_idx" ON "server_upgrades" ("server_uuid","kind");
CREATE INDEX "server_upgrades_until_idx" ON "server_upgrades" ("until") WHERE ended IS NULL;
CREATE UNIQUE INDEX "users_referral_code_idx" ON "users" ("referral_code");
CREATE INDEX "users_referred_by_idx" ON "users" ("referred_by");
CREATE INDEX "users_panel_access_until_idx" ON "users" ("panel_access_until");
ALTER TABLE "server_upgrades" ADD CONSTRAINT "server_upgrades_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_upgrades" ADD CONSTRAINT "server_upgrades_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "users" ADD CONSTRAINT "users_referred_by_users_uuid_fkey" FOREIGN KEY ("referred_by") REFERENCES "users"("uuid") ON DELETE SET NULL;