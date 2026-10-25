CREATE TABLE "user_noprofit_credit_changes" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"account_id" bigint NOT NULL,
	"amount" bigint NOT NULL,
	"type" varchar(64) NOT NULL,
	"description" varchar(255) NOT NULL,
	"data" jsonb NOT NULL,
	"applied" bigint,
	"balance_after" bigint,
	"attempts" integer DEFAULT 0 NOT NULL,
	"last_error" text,
	"sent" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "user_noprofit_accounts" ADD COLUMN "user_suspended" boolean DEFAULT false NOT NULL;
CREATE INDEX "user_noprofit_credit_changes_user_uuid_created_idx" ON "user_noprofit_credit_changes" ("user_uuid","created");
CREATE INDEX "user_noprofit_credit_changes_sent_created_idx" ON "user_noprofit_credit_changes" ("sent","created");
ALTER TABLE "user_noprofit_credit_changes" ADD CONSTRAINT "user_noprofit_credit_changes_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;