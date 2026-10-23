CREATE TABLE "user_noprofit_accounts" (
	"user_uuid" uuid PRIMARY KEY,
	"account_id" bigint NOT NULL,
	"email" varchar(255) NOT NULL,
	"username" varchar(255) NOT NULL,
	"is_admin" boolean DEFAULT false NOT NULL,
	"avatar" text,
	"credits" bigint DEFAULT 0 NOT NULL,
	"plan" jsonb,
	"discord" jsonb,
	"deleted" timestamp,
	"synced" timestamp DEFAULT now() NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "user_noprofit_accounts_account_id_idx" ON "user_noprofit_accounts" ("account_id");
ALTER TABLE "user_noprofit_accounts" ADD CONSTRAINT "user_noprofit_accounts_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;