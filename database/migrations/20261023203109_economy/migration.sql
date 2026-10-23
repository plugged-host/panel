CREATE TABLE "user_credit_transactions" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"amount" bigint NOT NULL,
	"balance_after" bigint NOT NULL,
	"type" varchar(64) NOT NULL,
	"description" varchar(255) NOT NULL,
	"data" jsonb NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "user_memberships" (
	"user_uuid" uuid PRIMARY KEY,
	"plus_since" timestamp,
	"plusplus_until" timestamp,
	"plusplus_purchases" integer DEFAULT 0 NOT NULL,
	"plusplus_cut" timestamp,
	"plusplus_settled" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "user_token_claims" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"provider" varchar(32) NOT NULL,
	"nonce" varchar(64) NOT NULL,
	"token_hash" char(64),
	"status" varchar(16) DEFAULT 'pending' NOT NULL,
	"amount" integer DEFAULT 0 NOT NULL,
	"reason" varchar(32),
	"ip" inet,
	"created" timestamp DEFAULT now() NOT NULL,
	"completed" timestamp
);

CREATE TABLE "user_token_transactions" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"amount" bigint NOT NULL,
	"balance_after" bigint NOT NULL,
	"type" varchar(64) NOT NULL,
	"description" varchar(255) NOT NULL,
	"data" jsonb NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "users" ADD COLUMN "credits" bigint DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "extra_cpu" integer DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "extra_memory" integer DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "extra_disk" integer DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "tokens" bigint DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "tokens_earned" bigint DEFAULT 0 NOT NULL;
ALTER TABLE "users" ADD COLUMN "tokens_daily_granted" date;
CREATE INDEX "user_credit_transactions_user_uuid_created_idx" ON "user_credit_transactions" ("user_uuid","created");
CREATE INDEX "user_memberships_plusplus_until_idx" ON "user_memberships" ("plusplus_until");
CREATE UNIQUE INDEX "user_token_claims_nonce_idx" ON "user_token_claims" ("nonce");
CREATE UNIQUE INDEX "user_token_claims_provider_token_hash_idx" ON "user_token_claims" ("provider","token_hash");
CREATE INDEX "user_token_claims_user_uuid_provider_status_idx" ON "user_token_claims" ("user_uuid","provider","status");
CREATE INDEX "user_token_transactions_user_uuid_created_idx" ON "user_token_transactions" ("user_uuid","created");
ALTER TABLE "user_credit_transactions" ADD CONSTRAINT "user_credit_transactions_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "user_memberships" ADD CONSTRAINT "user_memberships_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "user_token_claims" ADD CONSTRAINT "user_token_claims_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "user_token_transactions" ADD CONSTRAINT "user_token_transactions_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;