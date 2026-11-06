CREATE TABLE "server_custom_domains" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"server_uuid" uuid NOT NULL,
	"domain" varchar(253) NOT NULL,
	"target" varchar(255) NOT NULL,
	"verification_token" varchar(64) NOT NULL,
	"verified_at" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "suggestion_votes" (
	"suggestion_uuid" uuid,
	"user_uuid" uuid,
	"created" timestamp DEFAULT now() NOT NULL,
	CONSTRAINT "suggestion_votes_pk" PRIMARY KEY("suggestion_uuid","user_uuid")
);

CREATE TABLE "suggestions" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"title" varchar(480) NOT NULL,
	"body" text NOT NULL,
	"server_type" varchar(16) NOT NULL,
	"status" varchar(16) DEFAULT 'under_review' NOT NULL,
	"changelog" text,
	"merged_into" uuid,
	"created" timestamp DEFAULT now() NOT NULL,
	"updated" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "ticket_messages" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"ticket_uuid" uuid NOT NULL,
	"author_uuid" uuid,
	"author_type" varchar(8) NOT NULL,
	"body" text NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "tickets" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"server_uuid" uuid,
	"subject" varchar(800) NOT NULL,
	"category" varchar(16) NOT NULL,
	"status" varchar(16) DEFAULT 'open' NOT NULL,
	"priority" varchar(16) DEFAULT 'normal' NOT NULL,
	"assigned_to" uuid,
	"created" timestamp DEFAULT now() NOT NULL,
	"updated" timestamp DEFAULT now() NOT NULL,
	"closed" timestamp
);

ALTER TABLE "user_noprofit_accounts" ADD COLUMN "ultra" boolean DEFAULT false NOT NULL;
ALTER TABLE "user_noprofit_accounts" ALTER COLUMN "email" DROP NOT NULL;
ALTER TABLE "users" ALTER COLUMN "email" DROP NOT NULL;
CREATE UNIQUE INDEX "server_custom_domains_domain_idx" ON "server_custom_domains" ("domain");
CREATE INDEX "server_custom_domains_server_uuid_idx" ON "server_custom_domains" ("server_uuid");
CREATE INDEX "suggestion_votes_user_uuid_idx" ON "suggestion_votes" ("user_uuid");
CREATE INDEX "suggestions_status_created_idx" ON "suggestions" ("status","created");
CREATE INDEX "suggestions_user_uuid_created_idx" ON "suggestions" ("user_uuid","created");
CREATE INDEX "suggestions_merged_into_idx" ON "suggestions" ("merged_into");
CREATE INDEX "ticket_messages_ticket_uuid_created_idx" ON "ticket_messages" ("ticket_uuid","created");
CREATE INDEX "tickets_user_uuid_updated_idx" ON "tickets" ("user_uuid","updated");
CREATE INDEX "tickets_status_updated_idx" ON "tickets" ("status","updated");
CREATE INDEX "tickets_assigned_to_idx" ON "tickets" ("assigned_to");
ALTER TABLE "server_custom_domains" ADD CONSTRAINT "server_custom_domains_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestion_votes" ADD CONSTRAINT "suggestion_votes_suggestion_uuid_suggestions_uuid_fkey" FOREIGN KEY ("suggestion_uuid") REFERENCES "suggestions"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestion_votes" ADD CONSTRAINT "suggestion_votes_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestions" ADD CONSTRAINT "suggestions_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestions" ADD CONSTRAINT "suggestions_merged_into_suggestions_uuid_fkey" FOREIGN KEY ("merged_into") REFERENCES "suggestions"("uuid") ON DELETE SET NULL;
ALTER TABLE "ticket_messages" ADD CONSTRAINT "ticket_messages_ticket_uuid_tickets_uuid_fkey" FOREIGN KEY ("ticket_uuid") REFERENCES "tickets"("uuid") ON DELETE CASCADE;
ALTER TABLE "ticket_messages" ADD CONSTRAINT "ticket_messages_author_uuid_users_uuid_fkey" FOREIGN KEY ("author_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "tickets" ADD CONSTRAINT "tickets_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "tickets" ADD CONSTRAINT "tickets_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE SET NULL;
ALTER TABLE "tickets" ADD CONSTRAINT "tickets_assigned_to_users_uuid_fkey" FOREIGN KEY ("assigned_to") REFERENCES "users"("uuid") ON DELETE SET NULL;
-- plugged.host: no product stores an email address any more. The columns stay for Laravel
-- compatibility; the panel never reads or writes them.
UPDATE "users" SET "email" = NULL;
UPDATE "user_noprofit_accounts" SET "email" = NULL;
