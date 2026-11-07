CREATE TABLE "suggestion_attachments" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"suggestion_uuid" uuid NOT NULL,
	"message_uuid" uuid,
	"uploader_uuid" uuid,
	"name" varchar(1020) NOT NULL,
	"mime" varchar(128) NOT NULL,
	"size" bigint NOT NULL,
	"path" text NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "suggestion_messages" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"suggestion_uuid" uuid NOT NULL,
	"author_uuid" uuid,
	"author_type" varchar(8) NOT NULL,
	"body" text NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE INDEX "suggestion_attachments_suggestion_uuid_created_idx" ON "suggestion_attachments" ("suggestion_uuid","created");
CREATE INDEX "suggestion_messages_suggestion_uuid_created_idx" ON "suggestion_messages" ("suggestion_uuid","created");
ALTER TABLE "suggestion_attachments" ADD CONSTRAINT "suggestion_attachments_suggestion_uuid_suggestions_uuid_fkey" FOREIGN KEY ("suggestion_uuid") REFERENCES "suggestions"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestion_attachments" ADD CONSTRAINT "suggestion_attachments_2VzKMzWCWqlh_fkey" FOREIGN KEY ("message_uuid") REFERENCES "suggestion_messages"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestion_attachments" ADD CONSTRAINT "suggestion_attachments_uploader_uuid_users_uuid_fkey" FOREIGN KEY ("uploader_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "suggestion_messages" ADD CONSTRAINT "suggestion_messages_suggestion_uuid_suggestions_uuid_fkey" FOREIGN KEY ("suggestion_uuid") REFERENCES "suggestions"("uuid") ON DELETE CASCADE;
ALTER TABLE "suggestion_messages" ADD CONSTRAINT "suggestion_messages_author_uuid_users_uuid_fkey" FOREIGN KEY ("author_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;