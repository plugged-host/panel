CREATE TABLE "discord_boost_payouts" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"discord_id" varchar(32) NOT NULL,
	"user_uuid" uuid,
	"level" smallint NOT NULL,
	"credits" bigint NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "discord_invites" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"inviter_discord_id" varchar(32) NOT NULL,
	"joined_discord_id" varchar(32) NOT NULL,
	"invite_code" varchar(32),
	"account_created" timestamp NOT NULL,
	"joined" timestamp NOT NULL,
	"eligible" boolean NOT NULL,
	"matures" timestamp NOT NULL,
	"rewarded" timestamp,
	"credits" bigint DEFAULT 0 NOT NULL,
	"departed" timestamp,
	"leave_count" integer DEFAULT 0 NOT NULL,
	"rejoin_count" integer DEFAULT 0 NOT NULL,
	"revoked" timestamp,
	"revoked_credits" bigint DEFAULT 0 NOT NULL,
	"last_seen" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "discord_members" (
	"discord_id" varchar(32) PRIMARY KEY,
	"user_uuid" uuid,
	"username" varchar(64),
	"avatar" varchar(64),
	"in_guild" boolean DEFAULT false NOT NULL,
	"premium_since" timestamp,
	"boosts" smallint DEFAULT 0 NOT NULL,
	"invite_debt" bigint DEFAULT 0 NOT NULL,
	"checked" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "discord_boost_payouts_discord_id_level_idx" ON "discord_boost_payouts" ("discord_id","level");
CREATE UNIQUE INDEX "discord_boost_payouts_user_uuid_level_idx" ON "discord_boost_payouts" ("user_uuid","level");
CREATE UNIQUE INDEX "discord_invites_joined_discord_id_idx" ON "discord_invites" ("joined_discord_id");
CREATE INDEX "discord_invites_inviter_discord_id_idx" ON "discord_invites" ("inviter_discord_id");
CREATE INDEX "discord_invites_rewarded_matures_idx" ON "discord_invites" ("rewarded","matures");
CREATE INDEX "discord_invites_last_seen_idx" ON "discord_invites" ("last_seen");
CREATE UNIQUE INDEX "discord_members_user_uuid_idx" ON "discord_members" ("user_uuid");
CREATE INDEX "discord_members_checked_idx" ON "discord_members" ("checked");
ALTER TABLE "discord_boost_payouts" ADD CONSTRAINT "discord_boost_payouts_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "discord_members" ADD CONSTRAINT "discord_members_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;